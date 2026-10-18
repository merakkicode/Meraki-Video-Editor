use crate::{
    ring::{ring, Consumer},
    *,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use meraki_core::{AssetId, Time, TrackId};
use meraki_media::{Asset, AssetCatalog};
use meraki_timeline::Timeline;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meter {
    pub id: String,
    pub peak: [f32; 2],
    pub rms: [f32; 2],
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioState {
    pub available: bool,
    pub preparing: bool,
    pub error: Option<String>,
    pub device: String,
    pub underruns: String,
    pub latency_ms: f64,
    pub peak_version: String,
    pub meters: Vec<Meter>,
    pub eq_response: Vec<[f64; 2]>,
}
struct Control {
    timeline: Arc<Timeline>,
    catalog: AssetCatalog,
    target: u64,
}
struct Shared {
    control: Mutex<Control>,
    status: Mutex<AudioState>,
    peaks: Mutex<BTreeMap<AssetId, Peaks>>,
    stop: AtomicBool,
    play: AtomicBool,
    primed: AtomicBool,
    request: AtomicU64,
    producer_epoch: AtomicU64,
    ack: AtomicU64,
    base: AtomicU64,
    cursor: AtomicU64,
    delay: AtomicU64,
    stamp: AtomicU64,
    clock_version: AtomicU64,
    latency: AtomicU64,
    underruns: AtomicU64,
    failed: AtomicBool,
    origin: Instant,
}
#[derive(Clone)]
pub struct AudioEngine {
    shared: Arc<Shared>,
    join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}
struct Callback {
    consumer: Consumer,
    shared: Arc<Shared>,
    epoch: u64,
    cursor: u64,
    last: [f32; 2],
    fade: f32,
    starved: bool,
}
impl Callback {
    /// RT boundary: stack/atomic operations only. No allocation, locks, I/O or event emission.
    fn fill(&mut self, data: &mut [f32], latency: u64) {
        let s = &self.shared;
        let request = s.request.load(Ordering::Acquire);
        if request != self.epoch && s.producer_epoch.load(Ordering::Acquire) == request {
            self.consumer.clear();
            self.epoch = request;
            self.cursor = s.base.load(Ordering::Acquire);
            s.cursor.store(self.cursor, Ordering::Release);
            s.ack.store(request, Ordering::Release);
        }
        let active = request == self.epoch
            && s.play.load(Ordering::Acquire)
            && s.primed.load(Ordering::Acquire);
        let mut underrun = false;
        let mut consumed = 0u64;
        for frame in data.as_chunks_mut::<2>().0 {
            let next = if active { self.consumer.pop() } else { None };
            if let Some(sample) = next {
                self.fade = (self.fade + 1. / 128.).min(1.);
                for c in 0..2 {
                    self.last[c] = sample[c].clamp(-1., 1.) * self.fade;
                    frame[c] = self.last[c];
                }
                self.cursor += 1;
                consumed += 1;
            } else {
                if active {
                    underrun = true;
                }
                self.fade = 0.;
                for (out, last) in frame.iter_mut().zip(&mut self.last) {
                    *last *= 0.94;
                    if last.abs() < 1e-8 {
                        *last = 0.;
                    }
                    *out = *last;
                }
            }
        }
        if underrun && !self.starved {
            s.underruns.fetch_add(1, Ordering::Relaxed);
        }
        self.starved = underrun;
        s.clock_version.fetch_add(1, Ordering::AcqRel);
        s.latency.store(latency, Ordering::Relaxed);
        s.delay.store(latency + consumed, Ordering::Relaxed);
        s.stamp.store(
            s.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        s.cursor.store(self.cursor, Ordering::Release);
        s.clock_version.fetch_add(1, Ordering::Release);
    }
}
impl AudioEngine {
    pub fn spawn(
        timeline: Timeline,
        notify: impl Fn(AudioState) + Send + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            control: Mutex::new(Control {
                timeline: Arc::new(timeline),
                catalog: AssetCatalog::default(),
                target: 0,
            }),
            status: Mutex::new(AudioState {
                available: false,
                preparing: false,
                error: None,
                device: String::new(),
                underruns: "0".into(),
                latency_ms: 0.,
                peak_version: "0".into(),
                meters: Vec::new(),
                eq_response: Vec::new(),
            }),
            peaks: Mutex::new(BTreeMap::new()),
            stop: AtomicBool::new(false),
            play: AtomicBool::new(false),
            primed: AtomicBool::new(false),
            request: AtomicU64::new(1),
            producer_epoch: AtomicU64::new(0),
            ack: AtomicU64::new(0),
            base: AtomicU64::new(0),
            cursor: AtomicU64::new(0),
            delay: AtomicU64::new(0),
            stamp: AtomicU64::new(0),
            clock_version: AtomicU64::new(0),
            latency: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            failed: AtomicBool::new(false),
            origin: Instant::now(),
        });
        let worker = shared.clone();
        let join = thread::Builder::new()
            .name("meraki-audio-worker".into())
            .spawn(move || run(worker, notify))?;
        Ok(Self {
            shared,
            join: Arc::new(Mutex::new(Some(join))),
        })
    }
    pub fn update(&self, timeline: Arc<Timeline>, catalog: &AssetCatalog, reset: bool) {
        let mut c = self.shared.control.lock().unwrap();
        c.timeline = timeline;
        c.catalog = catalog.clone();
        if reset {
            self.shared.play.store(false, Ordering::Release);
            self.shared.primed.store(false, Ordering::Release);
            self.shared.request.fetch_add(1, Ordering::AcqRel);
        }
    }
    pub fn start(&self, at: Time) -> Result<(), String> {
        if !self.state().available {
            return Err(self
                .state()
                .error
                .unwrap_or("Saída de áudio ainda não disponível".into()));
        }
        self.reset(at, true)
    }
    pub fn reset(&self, at: Time, play: bool) -> Result<(), String> {
        let target = sample_index(at)?;
        let mut c = self.shared.control.lock().unwrap();
        c.target = target;
        self.shared.base.store(target, Ordering::Relaxed);
        self.shared.primed.store(false, Ordering::Relaxed);
        self.shared.play.store(play, Ordering::Relaxed);
        self.shared.request.fetch_add(1, Ordering::Release);
        Ok(())
    }
    pub fn is_playing(&self) -> bool {
        self.shared.play.load(Ordering::Acquire) && !self.shared.failed.load(Ordering::Acquire)
    }
    pub fn time(&self) -> Time {
        let s = &self.shared;
        let base = s.base.load(Ordering::Acquire);
        if s.ack.load(Ordering::Acquire) != s.request.load(Ordering::Acquire)
            || !s.primed.load(Ordering::Acquire)
        {
            return Time::new(base.min(i64::MAX as u64) as i64, 48000).unwrap();
        }
        loop {
            let version = s.clock_version.load(Ordering::Acquire);
            if !version.is_multiple_of(2) {
                std::hint::spin_loop();
                continue;
            }
            let cursor = s.cursor.load(Ordering::Acquire);
            let stamp = s.stamp.load(Ordering::Relaxed);
            let delay = s.delay.load(Ordering::Relaxed);
            if s.clock_version.load(Ordering::Acquire) != version {
                continue;
            }
            let now = s.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            let elapsed = (u128::from(now.saturating_sub(stamp)) * 48000 / 1_000_000_000)
                .min(u64::MAX as u128) as u64;
            return Time::new(
                cursor
                    .saturating_sub(delay.saturating_sub(elapsed))
                    .max(base)
                    .min(i64::MAX as u64) as i64,
                48000,
            )
            .unwrap();
        }
    }
    pub fn state(&self) -> AudioState {
        let mut state = self.shared.status.lock().unwrap().clone();
        state.underruns = self.shared.underruns.load(Ordering::Relaxed).to_string();
        state.latency_ms = self.shared.latency.load(Ordering::Relaxed) as f64 / 48.;
        if self.shared.failed.load(Ordering::Acquire) {
            state.available = false;
            state.error = Some(
                "Dispositivo de áudio desconectado. Reabra o aplicativo após reconectar.".into(),
            );
        }
        state
    }
    pub fn peaks(&self) -> BTreeMap<AssetId, Peaks> {
        self.shared.peaks.lock().unwrap().clone()
    }
    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.play.store(false, Ordering::Release);
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}
fn device_stream(
    shared: Arc<Shared>,
    consumer: Consumer,
) -> Result<(cpal::Stream, String), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("Nenhuma saída de áudio padrão")?;
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: 48000,
        buffer_size: cpal::BufferSize::Default,
    };
    let name = format!("{:?} · 48 kHz · estéreo f32", host.id());
    let mut callback = Callback {
        consumer,
        shared: shared.clone(),
        epoch: 0,
        cursor: 0,
        last: [0.; 2],
        fade: 0.,
        starved: false,
    };
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let t = info.timestamp();
                let latency = (t.playback.duration_since(t.callback).as_nanos() * 48000
                    / 1_000_000_000)
                    .min(48000) as u64;
                callback.fill(data, latency);
            },
            move |_| {
                shared.failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|e| format!("Saída 48 kHz estéreo f32 indisponível: {e}"))?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, name))
}
fn run(shared: Arc<Shared>, notify: impl Fn(AudioState)) {
    let (mut producer, consumer) = ring(2048);
    let stream = match device_stream(shared.clone(), consumer) {
        Ok((stream, name)) => {
            let mut s = shared.status.lock().unwrap();
            s.available = true;
            s.device = name;
            Some(stream)
        }
        Err(e) => {
            shared.status.lock().unwrap().error = Some(e);
            None
        }
    };
    let dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            shared.status.lock().unwrap().error = Some(e.to_string());
            return;
        }
    };
    let handle = AudioEngine {
        shared: shared.clone(),
        join: Arc::new(Mutex::new(None)),
    };
    let mut sources: BTreeMap<AssetId, (Asset, Option<PcmSource>)> = BTreeMap::new();
    let mut dsps: BTreeMap<TrackId, ChannelDsp> = BTreeMap::new();
    let mut master = ChannelDsp::default();
    let mut epoch = 0;
    let mut position = 0u64;
    let mut last_event = Instant::now();
    let mut peak_version = 0u64;
    let mut last_files_check = Instant::now();
    let mut meters: BTreeMap<String, Meter> = BTreeMap::new();
    while !shared.stop.load(Ordering::Acquire) {
        let (timeline, catalog, target, request) = {
            let c = shared.control.lock().unwrap();
            (
                c.timeline.clone(),
                c.catalog.clone(),
                c.target,
                shared.request.load(Ordering::Acquire),
            )
        };
        let old_count = sources.len();
        let check_files = last_files_check.elapsed() >= Duration::from_millis(500);
        if check_files {
            last_files_check = Instant::now();
        }
        sources.retain(|id, (asset, _)| {
            catalog.0.get(id) == Some(asset) && (!check_files || asset.path.is_file())
        });
        if sources.len() != old_count {
            shared
                .peaks
                .lock()
                .unwrap()
                .retain(|id, _| sources.contains_key(id));
            peak_version += 1;
            shared.status.lock().unwrap().peak_version = peak_version.to_string();
        }
        meters.retain(|id, _| {
            id == "master" || timeline.tracks.iter().any(|t| t.id.0.to_string() == *id)
        });
        if let Some(asset) = catalog
            .0
            .values()
            .find(|a| !sources.contains_key(&a.id) && a.path.is_file())
        {
            {
                let mut s = shared.status.lock().unwrap();
                s.preparing = true;
            }
            notify(handle.state());
            match prepare(asset, dir.path(), &shared.stop) {
                Ok(pcm) => {
                    shared.status.lock().unwrap().error = None;
                    if let Some(p) = &pcm {
                        shared
                            .peaks
                            .lock()
                            .unwrap()
                            .insert(asset.id, p.peaks.clone());
                    }
                    sources.insert(asset.id, (asset.clone(), pcm));
                }
                Err(e) => {
                    shared.status.lock().unwrap().error = Some(e);
                    sources.insert(asset.id, (asset.clone(), None));
                }
            }
            peak_version += 1;
            {
                let mut s = shared.status.lock().unwrap();
                s.preparing = false;
                s.peak_version = peak_version.to_string();
            }
            notify(handle.state());
            continue;
        }
        if epoch != request {
            shared.producer_epoch.store(request, Ordering::Release);
            if stream.is_none() || shared.ack.load(Ordering::Acquire) == request {
                epoch = request;
                position = target;
                dsps.clear();
                master = ChannelDsp::default();
            } else {
                thread::sleep(Duration::from_millis(1));
                continue;
            }
        }
        if shared.play.load(Ordering::Acquire)
            && stream.is_some()
            && producer.len() <= 1024
            && !shared.failed.load(Ordering::Acquire)
        {
            let mut mixed = [[0f32; 2]; BLOCK];
            let any_solo = timeline
                .tracks
                .iter()
                .any(|t| t.enabled && timeline.mixer.tracks.get(&t.id).is_some_and(|m| m.solo));
            let mut failure = None;
            for track in &timeline.tracks {
                let mut buffer = [[0f32; 2]; BLOCK];
                let channel = timeline
                    .mixer
                    .tracks
                    .get(&track.id)
                    .cloned()
                    .unwrap_or_default();
                for (i, frame) in buffer.iter_mut().enumerate() {
                    let at = Time::new((position + i as u64).min(i64::MAX as u64) as i64, 48000)
                        .unwrap();
                    for clip in track.clips.iter().filter(|c| c.enabled) {
                        if let Ok(source) = clip.source_at(at) {
                            if let Some((_, Some(pcm))) = sources.get_mut(&clip.source.asset) {
                                match pcm.at(source) {
                                    Ok(f) => {
                                        let age = sample_index(
                                            at.checked_sub(clip.timeline_start).unwrap(),
                                        )
                                        .unwrap_or(0);
                                        let tail = sample_index(
                                            clip.end().unwrap().checked_sub(at).unwrap(),
                                        )
                                        .unwrap_or(0);
                                        let fade = (age.min(tail) as f32 / 128.).min(1.);
                                        *frame = f.map(|v| v * fade * clip.volume_at(at));
                                    }
                                    Err(e) => failure = Some(e),
                                }
                            }
                            break;
                        }
                    }
                }
                dsps.entry(track.id).or_default().process(
                    &mut buffer,
                    &channel,
                    track.enabled && (!any_solo || channel.solo),
                );
                let (peak, rms) = levels(&buffer);
                let id = track.id.0.to_string();
                let previous = meters.get(&id);
                let peak = std::array::from_fn(|i| {
                    peak[i].max(previous.map_or(-120., |m| m.peak[i] - 0.2))
                });
                meters.insert(id.clone(), Meter { id, peak, rms });
                mix_stereo(&mut mixed, &buffer);
            }
            master.process(&mut mixed, &timeline.mixer.master, true);
            let (peak, rms) = levels(&mixed);
            let peak = std::array::from_fn(|i| {
                peak[i].max(meters.get("master").map_or(-120., |m| m.peak[i] - 0.2))
            });
            meters.insert(
                "master".into(),
                Meter {
                    id: "master".into(),
                    peak,
                    rms,
                },
            );
            if let Some(e) = failure {
                shared.status.lock().unwrap().error = Some(e);
                shared.play.store(false, Ordering::Release);
            }
            for f in mixed {
                if !producer.push(f) {
                    break;
                }
            }
            position += BLOCK as u64;
            if producer.len() >= 1024 && shared.request.load(Ordering::Acquire) == epoch {
                shared.primed.store(true, Ordering::Release);
            }
        } else {
            thread::sleep(Duration::from_millis(2));
        }
        if last_event.elapsed() >= Duration::from_millis(33) {
            if !shared.play.load(Ordering::Acquire) {
                for m in meters.values_mut() {
                    m.peak = m.peak.map(|v| (v - 3.).max(-120.));
                    m.rms = [-120.; 2];
                }
            }
            {
                let mut s = shared.status.lock().unwrap();
                s.meters = meters.values().cloned().collect();
            }
            notify(handle.state());
            last_event = Instant::now();
        }
    }
    drop(stream);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_clock_rational() {
        assert_eq!(sample_index(Time::new(1001, 30000).unwrap()).unwrap(), 1601);
        assert_eq!(sample_index(Time::new(1, 1).unwrap()).unwrap(), 48000);
    }
}
