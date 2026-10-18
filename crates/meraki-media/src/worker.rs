use crate::{Asset, DecoderSession, FrameCache, Tools, VideoFrame};
use meraki_core::Time;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct DecodeRequest {
    pub generation: u64,
    pub sequence: u64,
    pub revision: u64,
    pub asset: Option<Asset>,
    pub source_time: Time,
    pub timeline_time: Time,
}
#[derive(Clone)]
pub struct PresentedFrame {
    pub generation: u64,
    pub sequence: u64,
    pub timeline_time: Time,
    pub frame: Option<Arc<VideoFrame>>,
    pub decode_ms: f64,
}
#[derive(Default)]
pub struct WorkerOutput {
    pub backend: String,
    pub presented: Option<PresentedFrame>,
    pub error: Option<String>,
    pub cache_bytes: usize,
    pub decoded: u64,
    pub discarded: u64,
}
struct Shared {
    request: Mutex<Option<DecodeRequest>>,
    wake: Condvar,
    output: Mutex<WorkerOutput>,
    stop: AtomicBool,
}
#[derive(Clone)]
pub struct DecodeWorker {
    shared: Arc<Shared>,
    join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}
impl DecodeWorker {
    pub fn spawn(tools: Tools, capacity: usize, max_bytes: usize) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            request: Mutex::new(None),
            wake: Condvar::new(),
            output: Mutex::new(WorkerOutput::default()),
            stop: AtomicBool::new(false),
        });
        let thread_shared = shared.clone();
        let join = thread::Builder::new()
            .name("meraki-decode".into())
            .spawn(move || run(thread_shared, tools, capacity, max_bytes))?;
        Ok(Self {
            shared,
            join: Arc::new(Mutex::new(Some(join))),
        })
    }
    /// Latest-only mailbox: replacing the request never allocates a queue of seeks.
    pub fn request(&self, r: DecodeRequest) {
        let mut latest = self.shared.request.lock().unwrap();
        if latest
            .as_ref()
            .is_some_and(|old| r.sequence <= old.sequence)
        {
            return;
        }
        if latest
            .as_ref()
            .is_none_or(|old| old.generation != r.generation)
        {
            let mut out = self.shared.output.lock().unwrap();
            out.presented = None;
            out.error = None;
        }
        *latest = Some(r);
        self.shared.wake.notify_one();
    }
    pub fn with_output<T>(&self, f: impl FnOnce(&WorkerOutput) -> T) -> T {
        f(&self.shared.output.lock().unwrap())
    }
    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.wake.notify_all();
        if let Some(join) = self.join.lock().unwrap().take() {
            let _ = join.join();
        }
    }
}
fn publish(shared: &Shared, r: &DecodeRequest, frame: Option<Arc<VideoFrame>>, elapsed: Duration) {
    let latest = shared.request.lock().unwrap();
    let mut output = shared.output.lock().unwrap();
    if latest
        .as_ref()
        .is_some_and(|current| current.sequence == r.sequence && current.generation == r.generation)
    {
        output.presented = Some(PresentedFrame {
            generation: r.generation,
            sequence: r.sequence,
            timeline_time: r.timeline_time,
            frame,
            decode_ms: elapsed.as_secs_f64() * 1000.,
        });
        output.error = None;
    } else {
        output.discarded += 1;
    }
}
fn run(shared: Arc<Shared>, tools: Tools, capacity: usize, max_bytes: usize) {
    let mut cache = FrameCache::new(capacity, max_bytes);
    let mut session: Option<DecoderSession> = None;
    let mut identity = None;
    let mut generation = 0;
    let mut revision = u64::MAX;
    let mut last_pts = None;
    let mut last_target = None;
    let mut published = 0;
    let mut failed = None;
    let mut started = Instant::now();
    let mut started_sequence = 0;
    while !shared.stop.load(Ordering::Acquire) {
        let request = shared.request.lock().unwrap().clone();
        let Some(r) = request else {
            let guard = shared.request.lock().unwrap();
            let _ = shared.wake.wait_timeout(guard, Duration::from_millis(20));
            continue;
        };
        if started_sequence != r.sequence {
            started = Instant::now();
            started_sequence = r.sequence;
        }
        if r.revision != revision {
            cache.clear();
            session = None;
            identity = None;
            last_pts = None;
            revision = r.revision;
        }
        if generation != r.generation {
            session = None;
            last_pts = None;
            generation = r.generation;
            failed = None;
        }
        let Some(asset) = &r.asset else {
            if published != r.sequence {
                publish(&shared, &r, None, started.elapsed());
                published = r.sequence;
            }
            session = None;
            thread::sleep(Duration::from_millis(5));
            continue;
        };
        if identity != Some(asset.id) {
            session = None;
            last_pts = None;
            last_target = None;
            failed = None;
            identity = Some(asset.id);
        }
        if last_target.is_some_and(|at| r.source_time < at) {
            session = None;
            last_pts = None;
            failed = None;
        }
        last_target = Some(r.source_time);
        cache.discard_before(asset.id, r.source_time);
        if published != r.sequence {
            if let Some(f) = cache.get(asset.id, r.source_time) {
                publish(&shared, &r, Some(f), started.elapsed());
                published = r.sequence;
            }
        }
        if failed == Some(r.generation) {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let ahead = r
            .source_time
            .checked_add(Time::new(2, 5).unwrap())
            .unwrap_or(r.source_time);
        // Stop before look-ahead evicts the next playback frames. A fixed 400 ms
        // window can exceed either the frame count or byte budget (especially HD/VFR).
        let frame_bytes = asset.preview_width as usize * asset.preview_height as usize * 4;
        let future_limit = capacity.min(max_bytes / frame_bytes).saturating_sub(2);
        if session.is_some()
            && (last_pts.is_some_and(|pts: Time| pts >= ahead)
                || cache.future_frames(asset.id, r.source_time) >= future_limit)
            && published == r.sequence
        {
            thread::sleep(Duration::from_millis(4));
            continue;
        }
        // Seek outside the buffered window, including backwards after cache eviction.
        if published != r.sequence
            && last_pts.is_some_and(|pts: Time| {
                r.source_time < pts
                    || r.source_time
                        .checked_sub(pts)
                        .is_ok_and(|delta| delta > Time::new(1, 1).unwrap())
            })
        {
            session = None;
            last_pts = None;
        }
        if session.is_none() {
            match DecoderSession::start(&tools, asset, r.source_time) {
                Ok(s) => session = Some(s),
                Err(e) => {
                    shared.output.lock().unwrap().error = Some(e);
                    failed = Some(r.generation);
                    continue;
                }
            }
        }
        match session.as_ref().unwrap().next(Duration::from_millis(5)) {
            Ok(Ok(frame)) => {
                last_pts = Some(frame.pts);
                cache.insert(frame);
                let mut out = shared.output.lock().unwrap();
                out.cache_bytes = cache.bytes();
                out.decoded += 1;
            }
            Ok(Err(e)) => {
                session = None;
                failed = Some(r.generation);
                if published != r.sequence {
                    shared.output.lock().unwrap().error = Some(e);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => {
                session = None;
                failed = Some(r.generation);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obsolete_results_cannot_publish() {
        let shared = Shared {
            request: Mutex::new(None),
            wake: Condvar::new(),
            output: Mutex::new(WorkerOutput::default()),
            stop: AtomicBool::new(false),
        };
        let mut r = DecodeRequest {
            generation: 1,
            sequence: 1,
            revision: 0,
            asset: None,
            source_time: Time::ZERO,
            timeline_time: Time::ZERO,
        };
        *shared.request.lock().unwrap() = Some(r.clone());
        r.sequence = 0;
        publish(&shared, &r, None, Duration::ZERO);
        assert!(shared.output.lock().unwrap().presented.is_none());
        assert_eq!(shared.output.lock().unwrap().discarded, 1);
    }
}
