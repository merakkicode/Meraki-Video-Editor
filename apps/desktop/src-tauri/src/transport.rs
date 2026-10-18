use crate::dto::{unsigned, IpcError, TimeDto};
use meraki_compositor::{CompositeRequest, CompositeWorker};
use meraki_core::{Error, Time};
use meraki_engine::Snapshot;
use meraki_media::{AssetCatalog, PresentedFrame};
use meraki_timeline::Timeline;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDto {
    pub compositor: String,
    pub frame: String,
    pub playing: bool,
    pub generation: String,
    pub accepted_sequence: String,
    pub revision: String,
    pub presented_sequence: Option<String>,
    pub presented_time: Option<TimeDto>,
    pub source_pts: Option<TimeDto>,
    pub buffering: bool,
    pub error: Option<String>,
    pub cache_bytes: usize,
    pub decoded: String,
    pub discarded: String,
    pub decode_ms: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransportCommand {
    pub sequence: String,
    pub revision: String,
    pub action: String,
    pub frame: Option<String>,
}
struct Control {
    timeline: Arc<Timeline>,
    catalog: AssetCatalog,
    revision: u64,
    decode_revision: u64,
    playing: bool,
    frame: i64,
    generation: u64,
    accepted: u64,
    changed: bool,
}
struct Shared {
    control: Mutex<Control>,
    worker: CompositeWorker,
    audio: meraki_audio::AudioEngine,
    stop: AtomicBool,
}
#[derive(Clone)]
pub struct Transport {
    shared: Arc<Shared>,
}
pub fn frame_at(t: Time, timeline: &Timeline) -> i64 {
    let rate = timeline.frame_rate;
    ((i128::from(t.numerator()) * i128::from(rate.numerator()))
        / (i128::from(t.denominator()) * i128::from(rate.denominator())))
    .clamp(0, i128::from(i64::MAX)) as i64
}
pub fn end_frame(timeline: &Timeline) -> i64 {
    timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter_map(|c| c.end().ok())
        .map(|t| {
            let r = timeline.frame_rate;
            let n = i128::from(t.numerator()) * i128::from(r.numerator());
            let d = i128::from(t.denominator()) * i128::from(r.denominator());
            ((n + d - 1) / d).clamp(0, i128::from(i64::MAX)) as i64
        })
        .max()
        .unwrap_or(0)
}
impl Transport {
    pub fn spawn(
        timeline: Timeline,
        audio: meraki_audio::AudioEngine,
        notify: impl Fn(TransportDto) + Send + 'static,
    ) -> std::io::Result<Self> {
        let worker = CompositeWorker::spawn()?;
        let shared = Arc::new(Shared {
            control: Mutex::new(Control {
                timeline: Arc::new(timeline),
                catalog: AssetCatalog::default(),
                revision: 0,
                decode_revision: 0,
                playing: false,
                frame: 0,
                generation: 1,
                accepted: 0,
                changed: true,
            }),
            worker,
            audio,
            stop: AtomicBool::new(false),
        });
        let handle = Self {
            shared: shared.clone(),
        };
        let thread_handle = handle.clone();
        thread::Builder::new()
            .name("meraki-transport".into())
            .spawn(move || {
                let mut sequence = 0u64;
                let mut last_frame = -1;
                let mut last_event = Instant::now();
                while !shared.stop.load(Ordering::Acquire) {
                    {
                        let mut c = shared.control.lock().unwrap();
                        if c.playing && !shared.audio.is_playing() {
                            c.playing = false;
                            c.changed = true;
                        }
                        if c.playing {
                            c.frame = frame_at(shared.audio.time(), &c.timeline);
                            let end = end_frame(&c.timeline);
                            if c.frame >= end {
                                c.frame = end.saturating_sub(1).max(0);
                                c.playing = false;
                                let _ = shared.audio.reset(
                                    c.timeline.frame_rate.time_at_frame(c.frame).unwrap(),
                                    false,
                                );
                            }
                        }
                        if c.changed || c.frame != last_frame {
                            sequence = sequence.saturating_add(1);
                            last_frame = c.frame;
                            c.changed = false;
                            if let Ok(time) = c.timeline.frame_rate.time_at_frame(c.frame) {
                                shared.worker.request(CompositeRequest {
                                    generation: c.generation,
                                    sequence,
                                    revision: c.decode_revision,
                                    timeline: c.timeline.clone(),
                                    catalog: c.catalog.clone(),
                                    time,
                                });
                            }
                        }
                    }
                    if last_event.elapsed() >= Duration::from_millis(16) {
                        notify(thread_handle.state());
                        last_event = Instant::now();
                    }
                    thread::sleep(Duration::from_millis(4));
                }
                shared.worker.shutdown();
            })?;
        Ok(handle)
    }
    pub fn refresh(&self) {
        let mut c = self.shared.control.lock().unwrap();
        c.generation = c.generation.saturating_add(1);
        c.changed = true;
    }
    pub fn update(&self, snapshot: &Snapshot, catalog: &AssetCatalog) {
        let mut c = self.shared.control.lock().unwrap();
        let mut previous = (*c.timeline).clone();
        previous.mixer = snapshot.timeline.mixer.clone();
        let only_mix = previous == *snapshot.timeline && c.catalog == *catalog;
        previous.graphs = snapshot.timeline.graphs.clone();
        for old in previous.tracks.iter_mut().flat_map(|t| &mut t.clips) {
            if let Some(new) = snapshot
                .timeline
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|n| n.id == old.id)
            {
                old.transform = new.transform;
                old.effects = new.effects;
                old.motion = new.motion.clone();
            }
        }
        let only_visual_or_mix = previous == *snapshot.timeline && c.catalog == *catalog;
        self.shared
            .audio
            .update(snapshot.timeline.clone(), catalog, !only_visual_or_mix);
        c.timeline = snapshot.timeline.clone();
        c.catalog = catalog.clone();
        c.revision = snapshot.revision;
        if only_mix {
            return;
        }
        c.decode_revision = snapshot.revision;
        if !only_visual_or_mix {
            c.playing = false;
        }
        c.generation = c.generation.saturating_add(1);
        c.frame = c.frame.min(end_frame(&c.timeline).saturating_sub(1)).max(0);
        c.changed = true;
    }
    pub fn command(&self, cmd: TransportCommand) -> Result<TransportDto, IpcError> {
        {
            let mut c = self.shared.control.lock().unwrap();
            let invalid = |e| IpcError::domain(e, c.revision);
            let sequence = unsigned::<u64>(&cmd.sequence).map_err(invalid)?;
            let revision = unsigned::<u64>(&cmd.revision).map_err(invalid)?;
            if revision != c.revision {
                return Err(IpcError::domain(Error::Conflict, c.revision));
            }
            if sequence <= c.accepted {
                drop(c);
                return Ok(self.state());
            }
            let target = match cmd.action.as_str() {
                "seek" => Some(
                    unsigned::<u64>(
                        cmd.frame
                            .as_deref()
                            .ok_or_else(|| invalid(Error::Invalid("Quadro ausente")))?,
                    )
                    .map_err(invalid)?
                    .try_into()
                    .map_err(|_| invalid(Error::Overflow))?,
                ),
                "play" | "pause" => None,
                _ => return Err(invalid(Error::Invalid("Transporte inválido"))),
            };
            c.accepted = sequence;
            c.generation = c.generation.saturating_add(1);
            c.changed = true;
            match cmd.action.as_str() {
                "seek" => {
                    c.frame = target
                        .unwrap_or(0)
                        .min(end_frame(&c.timeline).saturating_sub(1))
                        .max(0);
                    c.playing = false;
                }
                "play" => {
                    let end = end_frame(&c.timeline);
                    if c.frame >= end.saturating_sub(1) {
                        c.frame = 0;
                    }
                    self.shared
                        .audio
                        .start(c.timeline.frame_rate.time_at_frame(c.frame).unwrap())
                        .map_err(|message| crate::actor::external_error(message, c.revision))?;
                    c.playing = end > 0;
                }
                _ => c.playing = false,
            }
            if !c.playing {
                let _ = self
                    .shared
                    .audio
                    .reset(c.timeline.frame_rate.time_at_frame(c.frame).unwrap(), false);
            }
        }
        Ok(self.state())
    }
    pub fn state(&self) -> TransportDto {
        let c = self.shared.control.lock().unwrap();
        self.shared.worker.with_output(|o| {
            let p = o
                .presented
                .as_ref()
                .filter(|p| p.generation == c.generation);
            TransportDto {
                compositor: o.backend.clone(),
                frame: c.frame.to_string(),
                playing: c.playing,
                generation: c.generation.to_string(),
                accepted_sequence: c.accepted.to_string(),
                revision: c.revision.to_string(),
                presented_sequence: p.map(|p| p.sequence.to_string()),
                presented_time: p.map(|p| p.timeline_time.into()),
                source_pts: p.and_then(|p| p.frame.as_ref().map(|f| f.pts.into())),
                buffering: p.is_none(),
                error: o.error.clone(),
                cache_bytes: o.cache_bytes,
                decoded: o.decoded.to_string(),
                discarded: o.discarded.to_string(),
                decode_ms: p.map_or(0., |p| p.decode_ms),
            }
        })
    }
    pub fn presented(&self, generation: u64) -> Option<PresentedFrame> {
        let c = self.shared.control.lock().unwrap();
        if generation != c.generation {
            return None;
        }
        self.shared.worker.with_output(|o| {
            o.presented
                .as_ref()
                .filter(|p| p.generation == generation)
                .cloned()
        })
    }
    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.worker.shutdown();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_frame_mapping_is_exact() {
        let mut t = crate::actor::empty_timeline();
        t.frame_rate = meraki_core::FrameRate::new(30000, 1001).unwrap();
        assert_eq!(frame_at(Time::new(1001, 30).unwrap(), &t), 1000);
        assert!(meraki_compositor::active_layers(&t, Time::ZERO).is_empty());
    }
}
