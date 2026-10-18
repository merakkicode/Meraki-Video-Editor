use crate::{active_layers, compose_at, title_frame};
use meraki_core::{AssetId, ClipId, Time};
use meraki_media::{
    Asset, AssetCatalog, DecodeRequest, DecodeWorker, PixelFormat, PresentedFrame, Tools,
    VideoFrame, WorkerOutput,
};
use meraki_timeline::Timeline;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct CompositeRequest {
    pub generation: u64,
    pub sequence: u64,
    pub revision: u64,
    pub timeline: Arc<Timeline>,
    pub catalog: AssetCatalog,
    pub time: Time,
}
struct Shared {
    request: Mutex<Option<CompositeRequest>>,
    output: Mutex<WorkerOutput>,
    stop: AtomicBool,
}
#[derive(Clone)]
pub struct CompositeWorker {
    shared: Arc<Shared>,
    join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}
impl CompositeWorker {
    pub fn spawn() -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            request: Mutex::new(None),
            output: Mutex::new(WorkerOutput::default()),
            stop: AtomicBool::new(false),
        });
        let copy = shared.clone();
        let join = thread::Builder::new()
            .name("meraki-composition".into())
            .spawn(move || run(copy))?;
        Ok(Self {
            shared,
            join: Arc::new(Mutex::new(Some(join))),
        })
    }
    pub fn request(&self, r: CompositeRequest) {
        let mut latest = self.shared.request.lock().unwrap();
        if latest
            .as_ref()
            .is_some_and(|old| old.sequence >= r.sequence)
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
    }
    pub fn with_output<T>(&self, f: impl FnOnce(&WorkerOutput) -> T) -> T {
        f(&self.shared.output.lock().unwrap())
    }
    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}
struct Slot {
    asset: Asset,
    worker: DecodeWorker,
    requested: u64,
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.worker.shutdown();
    }
}
fn run(shared: Arc<Shared>) {
    let _session = crate::CompositorSession;
    let tools = Tools::discover();
    let mut slots: BTreeMap<ClipId, Slot> = BTreeMap::new();
    let mut processed = 0;
    while !shared.stop.load(Ordering::Acquire) {
        let Some(r) = shared.request.lock().unwrap().clone() else {
            thread::sleep(Duration::from_millis(4));
            continue;
        };
        if processed == r.sequence {
            thread::sleep(Duration::from_millis(4));
            continue;
        }
        let started = Instant::now();
        let result = (|| -> Result<Option<Arc<VideoFrame>>, String> {
            let active = active_layers(&r.timeline, r.time);
            if active.len() > 8 {
                return Err("Prévia limitada a 8 camadas simultâneas".into());
            }
            slots.retain(|id, _| active.iter().any(|(c, _)| c.id == *id));
            let mut frames = vec![];
            let mut bytes = 0;
            let mut decoded = 0;
            for (clip, source) in &active {
                if let Some(title) = &clip.motion.title {
                    frames.push((*clip, title_frame(title)?));
                    continue;
                }
                let asset = r
                    .catalog
                    .0
                    .get(&clip.source.asset)
                    .ok_or("Fonte de vídeo ausente")?;
                if !asset.path.is_file() {
                    let placeholder = meraki_timeline::Title {
                        text: "Mídia Offline".into(),
                        color: [255, 160, 100, 255],
                        ..Default::default()
                    };
                    frames.push((*clip, title_frame(&placeholder)?));
                    slots.remove(&clip.id);
                    continue;
                }
                let selected = meraki_media::preview_asset(asset);
                let asset = &selected;
                if slots.get(&clip.id).is_some_and(|s| s.asset != *asset) {
                    slots.remove(&clip.id);
                }
                if let std::collections::btree_map::Entry::Vacant(e) = slots.entry(clip.id) {
                    e.insert(Slot {
                        asset: asset.clone(),
                        worker: DecodeWorker::spawn(tools.clone(), 12, 32 * 1024 * 1024)
                            .map_err(|e| e.to_string())?,
                        requested: 0,
                    });
                }
                let slot = slots.get_mut(&clip.id).unwrap();
                if slot.requested != r.sequence {
                    slot.worker.request(DecodeRequest {
                        generation: 1,
                        sequence: r.sequence,
                        revision: 0,
                        asset: Some(asset.clone()),
                        source_time: *source,
                        timeline_time: r.time,
                    });
                    slot.requested = r.sequence;
                }
                let (frame, error) = slot.worker.with_output(|o| {
                    bytes += o.cache_bytes;
                    decoded += o.decoded;
                    (
                        o.presented
                            .as_ref()
                            .and_then(|p| p.frame.clone())
                            .filter(|f| f.covers(*source)),
                        o.error.clone(),
                    )
                });
                if let Some(f) = frame {
                    frames.push((*clip, f));
                } else if let Some(e) = error {
                    return Err(e);
                }
            }
            {
                let mut o = shared.output.lock().unwrap();
                o.cache_bytes = bytes;
                o.decoded = decoded;
            }
            if frames.len() != active.len() {
                return Ok(None);
            }
            let size = if std::env::var("MERAKI_PREVIEW").as_deref() == Ok("540") {
                [960, 540]
            } else {
                [1920, 1080]
            };
            let pixels = compose_at(&r.timeline, &frames, size, r.time)?;
            Ok(Some(Arc::new(VideoFrame {
                asset: AssetId(0),
                format: PixelFormat::Rgba8,
                width: size[0],
                height: size[1],
                stride: size[0] * 4,
                pts: r.time,
                duration: r
                    .timeline
                    .frame_rate
                    .time_at_frame(1)
                    .map_err(|e| e.to_string())?,
                pixels: Arc::new(pixels),
            })))
        })();
        match result {
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Ok(Some(frame)) => {
                let latest = shared.request.lock().unwrap();
                let mut out = shared.output.lock().unwrap();
                // Playback may advance while a frame is composed. A new seek or visual revision
                // invalidates the whole result; within one generation publish monotonically.
                if latest
                    .as_ref()
                    .is_some_and(|n| n.generation == r.generation && n.revision == r.revision)
                {
                    out.presented = Some(PresentedFrame {
                        generation: r.generation,
                        sequence: r.sequence,
                        timeline_time: r.time,
                        frame: Some(frame),
                        decode_ms: started.elapsed().as_secs_f64() * 1000.,
                    });
                    out.error = None;
                    out.backend = crate::backend_name();
                } else {
                    out.discarded += 1;
                }
                processed = r.sequence;
            }
            Err(e) => {
                let latest = shared.request.lock().unwrap();
                if latest
                    .as_ref()
                    .is_some_and(|n| n.generation == r.generation && n.revision == r.revision)
                {
                    shared.output.lock().unwrap().error = Some(e);
                }
                processed = r.sequence;
            }
        }
    }
}
