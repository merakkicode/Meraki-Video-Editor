use crate::dto::*;
use crate::transport::Transport;
use meraki_core::*;
use meraki_engine::{Edit, Engine};
use meraki_media::{Asset, AssetCatalog};
use meraki_project::Project;
use meraki_timeline::*;
use std::{collections::BTreeMap, thread};
use tokio::sync::{mpsc, oneshot};

pub type Reply = std::result::Result<SnapshotDto, IpcError>;
pub enum Request {
    Get(oneshot::Sender<Reply>),
    Import(Asset, u64, oneshot::Sender<Reply>),
    Insert(AssetId, Option<TrackId>, u64, oneshot::Sender<Reply>),
    Open(Project, u64, oneshot::Sender<Reply>),
    Project(oneshot::Sender<Project>),
    Apply(Result<CommandEnvelope>, oneshot::Sender<Reply>),
    Undo(Result<UndoEnvelope>, oneshot::Sender<Reply>),
    Viewport(
        PreviewViewport,
        oneshot::Sender<std::result::Result<(), IpcError>>,
    ),
}
#[derive(Clone)]
pub struct EngineHandle {
    sender: mpsc::Sender<Request>,
}
impl EngineHandle {
    #[cfg(test)]
    pub fn spawn(notify: impl Fn(&SnapshotDto) + Send + 'static) -> std::io::Result<Self> {
        Self::spawn_for(demo_timeline(), None, notify)
    }
    pub fn spawn_for(
        timeline: Timeline,
        transport: Option<Transport>,
        notify: impl Fn(&SnapshotDto) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, mut receiver) = mpsc::channel::<Request>(32);
        let mut engine = Engine::new(timeline).expect("valid built-in timeline");
        let mut catalog = AssetCatalog::default();
        thread::Builder::new()
            .name("meraki-engine".into())
            .spawn(move || {
                // Future surface adapter consumes these CSS coordinates and converts to physical pixels once.
                let mut _viewport = PreviewViewport::default();
                while let Some(request) = receiver.blocking_recv() {
                    match request {
                        Request::Get(reply) => {
                            let _ = reply.send(Ok(snapshot_dto(&engine, &catalog)));
                        }
                        Request::Apply(envelope, reply) => {
                            let result = (|| {
                                let envelope = envelope?;
                                validate_envelope(envelope.protocol_version, &envelope.request_id)?;
                                let rev = unsigned::<u64>(&envelope.expected_revision)?;
                                let edit = Edit::try_from(envelope.edit)?;
                                engine.apply(rev, edit)
                            })()
                            .map(|_| snapshot_dto(&engine, &catalog))
                            .map_err(|e| IpcError::domain(e, engine.snapshot().revision));
                            if let Ok(snapshot) = &result {
                                if let Some(t) = &transport {
                                    t.update(&engine.snapshot(), &catalog);
                                }
                                notify(snapshot);
                            }
                            let _ = reply.send(result);
                        }
                        Request::Undo(envelope, reply) => {
                            let result = (|| {
                                let envelope = envelope?;
                                validate_envelope(envelope.protocol_version, &envelope.request_id)?;
                                engine.undo(unsigned::<u64>(&envelope.expected_revision)?)
                            })()
                            .map(|_| snapshot_dto(&engine, &catalog))
                            .map_err(|e| IpcError::domain(e, engine.snapshot().revision));
                            if let Ok(snapshot) = &result {
                                if let Some(t) = &transport {
                                    t.update(&engine.snapshot(), &catalog);
                                }
                                notify(snapshot);
                            }
                            let _ = reply.send(result);
                        }
                        Request::Import(mut asset, rev, reply) => {
                            let result = (|| {
                                if rev != engine.snapshot().revision {
                                    return Err(IpcError::domain(
                                        Error::Conflict,
                                        engine.snapshot().revision,
                                    ));
                                }
                                asset.id = catalog
                                    .next_id()
                                    .map_err(|e| external_error(e, engine.snapshot().revision))?;
                                asset
                                    .validate()
                                    .map_err(|e| external_error(e, engine.snapshot().revision))?;
                                insert_asset(&mut engine, &asset, None, rev)
                                    .map_err(|e| IpcError::domain(e, engine.snapshot().revision))?;
                                catalog.0.insert(asset.id, asset);
                                Ok(snapshot_dto(&engine, &catalog))
                            })();
                            if let Ok(snapshot) = &result {
                                if let Some(t) = &transport {
                                    t.update(&engine.snapshot(), &catalog);
                                }
                                notify(snapshot);
                            }
                            let _ = reply.send(result);
                        }
                        Request::Insert(id, track, rev, reply) => {
                            let result = catalog
                                .0
                                .get(&id)
                                .ok_or(Error::NotFound)
                                .and_then(|a| insert_asset(&mut engine, a, track, rev))
                                .map(|_| snapshot_dto(&engine, &catalog))
                                .map_err(|e| IpcError::domain(e, engine.snapshot().revision));
                            if let Ok(snapshot) = &result {
                                if let Some(t) = &transport {
                                    t.update(&engine.snapshot(), &catalog);
                                }
                                notify(snapshot);
                            }
                            let _ = reply.send(result);
                        }
                        Request::Open(project, rev, reply) => {
                            let result = (|| {
                                let next = project
                                    .validate()
                                    .map_err(|e| external_error(e, engine.snapshot().revision))?;
                                engine
                                    .replace(rev, project.timeline)
                                    .map_err(|e| IpcError::domain(e, engine.snapshot().revision))?;
                                catalog = next;
                                Ok(snapshot_dto(&engine, &catalog))
                            })();
                            if let Ok(snapshot) = &result {
                                if let Some(t) = &transport {
                                    t.update(&engine.snapshot(), &catalog);
                                }
                                notify(snapshot);
                            }
                            let _ = reply.send(result);
                        }
                        Request::Project(reply) => {
                            let _ = reply.send(Project::new(
                                (*engine.snapshot().timeline).clone(),
                                &catalog,
                            ));
                        }
                        Request::Viewport(next, reply) => {
                            let result = next
                                .validate()
                                .map(|_| {
                                    _viewport = next;
                                })
                                .map_err(|e| IpcError::domain(e, engine.snapshot().revision));
                            let _ = reply.send(result);
                        }
                    }
                }
                // Actor and resources end when the app drops its final sender.
            })?;
        Ok(Self { sender })
    }
    pub async fn get(&self) -> Reply {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Get(tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn apply(&self, e: Result<CommandEnvelope>) -> Reply {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Apply(e, tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn undo(&self, e: Result<UndoEnvelope>) -> Reply {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Undo(e, tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn viewport(&self, v: PreviewViewport) -> std::result::Result<(), IpcError> {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Viewport(v, tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn import(&self, asset: Asset, rev: u64) -> Reply {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Import(asset, rev, tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn insert(&self, id: AssetId, track: Option<TrackId>, rev: u64) -> Reply {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Insert(id, track, rev, tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn open(&self, p: Project, rev: u64) -> Reply {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Open(p, rev, tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())?
    }
    pub async fn project(&self) -> std::result::Result<Project, IpcError> {
        let (tx, rx) = oneshot::channel();
        self.send(Request::Project(tx)).await?;
        rx.await.map_err(|_| IpcError::unavailable())
    }
    async fn send(&self, request: Request) -> std::result::Result<(), IpcError> {
        self.sender
            .send(request)
            .await
            .map_err(|_| IpcError::unavailable())
    }
}
fn snapshot_dto(engine: &Engine, catalog: &AssetCatalog) -> SnapshotDto {
    let mut dto: SnapshotDto = engine.snapshot().into();
    dto.assets = catalog.0.values().cloned().collect();
    dto
}
pub fn external_error(message: String, revision: u64) -> IpcError {
    IpcError {
        code: ErrorCode::Invalid,
        message,
        revision: revision.to_string(),
    }
}
fn insert_asset(
    engine: &mut Engine,
    asset: &Asset,
    target: Option<TrackId>,
    rev: u64,
) -> Result<()> {
    let current = engine.snapshot();
    let track = current
        .timeline
        .tracks
        .iter()
        .find(|t| {
            t.kind
                == (if asset.audio_only {
                    TrackKind::Audio
                } else {
                    TrackKind::Video
                })
                && !t.locked
                && target.is_none_or(|id| id == t.id)
        })
        .ok_or(Error::Locked)?;
    let start = track
        .clips
        .iter()
        .map(|c| c.end())
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max()
        .unwrap_or(Time::ZERO);
    let id = current
        .timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .map(|c| c.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    let clip = Clip {
        motion: Default::default(),
        id: ClipId(id),
        name: asset.name.clone(),
        transform: Default::default(),
        source: MediaSource {
            asset: asset.id,
            stream_index: asset.stream_index,
            kind: track.kind,
            available_start: Time::ZERO,
            available_duration: asset.duration,
        },
        timeline_start: start,
        duration: asset.duration,
        source_in: Time::ZERO,
        speed: PlaybackRate::new(1, 1)?,
        enabled: true,
        link_group: None,
        effects: None,
    };
    engine.apply(
        rev,
        Edit::Insert {
            track: track.id,
            clip: Box::new(clip),
        },
    )?;
    Ok(())
}
pub fn empty_timeline() -> Timeline {
    Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Novo projeto Meraki".into(),
        frame_rate: FrameRate::new(30, 1).unwrap(),
        audio_sample_rate: 48000,
        tracks: (1..=4)
            .map(|i| Track {
                id: TrackId(i),
                name: format!(
                    "{} {}",
                    if i <= 2 { "Vídeo" } else { "Áudio" },
                    if i <= 2 { i } else { i - 2 }
                ),
                kind: if i <= 2 {
                    TrackKind::Video
                } else {
                    TrackKind::Audio
                },
                locked: false,
                enabled: true,
                solo: false,
                clips: vec![],
            })
            .collect(),
        mixer: Default::default(),
        graphs: BTreeMap::new(),
    }
}
#[cfg(test)]
pub fn demo_timeline() -> Timeline {
    let t = |v| Time::new(v, 1).expect("integer seconds");
    let make_clip = |id, name: &str, kind, start, duration, asset| Clip {
        motion: Default::default(),
        id: ClipId(id),
        name: name.into(),
        transform: Default::default(),
        source: MediaSource {
            asset: AssetId(asset),
            stream_index: 0,
            kind,
            available_start: t(0),
            available_duration: t(600),
        },
        timeline_start: t(start),
        duration: t(duration),
        source_in: t(0),
        speed: PlaybackRate::new(1, 1).expect("unit speed"),
        enabled: true,
        link_group: None,
        effects: None,
    };
    let v = TrackKind::Video;
    let a = TrackKind::Audio;
    Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Entre o céu e o mar".into(),
        frame_rate: FrameRate::new(30, 1).expect("30fps"),
        audio_sample_rate: 48000,
        tracks: vec![
            Track {
                id: TrackId(1),
                name: "Vídeo 1".into(),
                kind: v,
                locked: false,
                enabled: true,
                solo: false,
                clips: vec![
                    make_clip(1, "Costa ao amanhecer", v, 0, 52, 1),
                    make_clip(2, "Entre montanhas", v, 58, 35, 2),
                    make_clip(3, "Última luz", v, 104, 28, 3),
                ],
            },
            Track {
                id: TrackId(2),
                name: "Vídeo 2".into(),
                kind: v,
                locked: false,
                enabled: true,
                solo: false,
                clips: vec![make_clip(4, "O horizonte", v, 34, 58, 4)],
            },
            Track {
                id: TrackId(3),
                name: "Áudio 1".into(),
                kind: a,
                locked: false,
                enabled: true,
                solo: false,
                clips: vec![make_clip(5, "Ocean ambient.wav", a, 0, 154, 5)],
            },
            Track {
                id: TrackId(4),
                name: "Áudio 2".into(),
                kind: a,
                locked: false,
                enabled: true,
                solo: false,
                clips: vec![make_clip(6, "A new beginning.wav", a, 42, 125, 6)],
            },
        ],
        mixer: Default::default(),
        graphs: BTreeMap::new(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn actor_serializes_edits_and_emits_commits() {
        let (events_tx, mut events_rx) = mpsc::unbounded_channel();
        let actor = EngineHandle::spawn(move |s| {
            let _ = events_tx.send(s.revision.clone());
        })
        .unwrap();
        let split = CommandEnvelope {
            protocol_version: 1,
            request_id: "cut-1".into(),
            expected_revision: "0".into(),
            edit: EditDto::Split {
                clip: "1".into(),
                at: TimeDto {
                    numerator: "20".into(),
                    denominator: 1,
                },
                right_id: u128::MAX.to_string(),
            },
        };
        assert_eq!(actor.apply(Ok(split.clone())).await.unwrap().revision, "1");
        assert_eq!(events_rx.recv().await.unwrap(), "1");
        assert_eq!(
            actor.apply(Ok(split)).await.unwrap_err().code,
            ErrorCode::Conflict
        );
        let undo = UndoEnvelope {
            protocol_version: 1,
            request_id: "undo-1".into(),
            expected_revision: "1".into(),
        };
        assert_eq!(actor.undo(Ok(undo)).await.unwrap().revision, "2");
        assert_eq!(
            actor.get().await.unwrap().timeline["tracks"][0]["clips"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[tokio::test]
    async fn invalid_edit_is_atomic() {
        let actor = EngineHandle::spawn(|_| {}).unwrap();
        let edit = CommandEnvelope {
            protocol_version: 1,
            request_id: "move-1".into(),
            expected_revision: "0".into(),
            edit: EditDto::Move {
                clip: "1".into(),
                to: TimeDto {
                    numerator: "20".into(),
                    denominator: 1,
                },
            },
        };
        assert_eq!(
            actor.apply(Ok(edit)).await.unwrap_err().code,
            ErrorCode::Invalid
        );
        assert_eq!(actor.get().await.unwrap().revision, "0");
    }
    #[tokio::test]
    async fn malformed_payload_returns_a_typed_error_without_a_commit() {
        let actor = EngineHandle::spawn(|_| {}).unwrap();
        let malformed = serde_json::json!({"protocolVersion":1,"requestId":"bad","expectedRevision":"0","edit":{"kind":"move","clip":1,"to":{"numerator":"0","denominator":1}}});
        let parsed = serde_json::from_value::<CommandEnvelope>(malformed)
            .map_err(|_| Error::Invalid("Envelope IPC inválido."));
        let error = actor.apply(parsed).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::Invalid);
        assert_eq!(error.revision, "0");
        assert_eq!(actor.get().await.unwrap().revision, "0");
    }
}
