use crate::{
    actor::{external_error, EngineHandle},
    dto::{unsigned, IpcError, SnapshotDto},
    transport::{Transport, TransportCommand, TransportDto},
};
use meraki_core::AssetId;
use meraki_media::{probe, Tools};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
pub struct IoJobs(pub Arc<tokio::sync::Semaphore>);
/// Only the small request/response future uses Tokio. Dialog/probe/disk run on named OS threads.
pub(crate) async fn io_job<T: Send + 'static>(
    jobs: &IoJobs,
    revision: u64,
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, IpcError> {
    let permit = jobs
        .0
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| IpcError::unavailable())?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("meraki-media-io".into())
        .spawn(move || {
            let _permit = permit;
            let _ = tx.send(f());
        })
        .map_err(|e| external_error(e.to_string(), revision))?;
    rx.await
        .map_err(|_| IpcError::unavailable())?
        .map_err(|e| external_error(e, revision))
}
#[tauri::command]
pub async fn import_media(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
    expected_revision: String,
) -> Result<Option<SnapshotDto>, IpcError> {
    let rev = unsigned(&expected_revision).map_err(|e| IpcError::domain(e, 0))?;
    let asset = io_job(&jobs, rev, move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter(
                "Vídeo e áudio",
                &[
                    "mp4", "mov", "mkv", "webm", "avi", "m4v", "mts", "ts", "wav", "mp3", "flac",
                    "m4a", "ogg",
                ],
            )
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|e| e.to_string())?;
        probe(&Tools::discover(), &path, AssetId(0)).map(Some)
    })
    .await?;
    match asset {
        Some(a) => engine.import(a, rev).await.map(Some),
        None => Ok(None),
    }
}
#[tauri::command]
pub async fn insert_media(
    engine: State<'_, EngineHandle>,
    asset_id: String,
    track_id: Option<String>,
    expected_revision: String,
) -> Result<SnapshotDto, IpcError> {
    let rev = unsigned(&expected_revision).map_err(|e| IpcError::domain(e, 0))?;
    let id = unsigned(&asset_id).map_err(|e| IpcError::domain(e, rev))?;
    let track = track_id
        .as_deref()
        .map(unsigned)
        .transpose()
        .map_err(|e| IpcError::domain(e, rev))?
        .map(meraki_core::TrackId);
    engine.insert(AssetId(id), track, rev).await
}
#[tauri::command]
pub async fn save_project(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
) -> Result<Option<String>, IpcError> {
    let project = engine.project().await?;
    let recovery = app.state::<crate::recovery::Recovery>().inner().clone();
    io_job(&jobs, 0, move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Projeto Meraki", &["meraki"])
            .set_file_name("Projeto.meraki")
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let mut path = file.into_path().map_err(|e| e.to_string())?;
        if path.extension().is_none_or(|e| e != "meraki") {
            path.set_extension("meraki");
        }
        meraki_project::save(&path, &project)?;
        recovery.saved(&project);
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}
#[tauri::command]
pub async fn open_project(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
    expected_revision: String,
) -> Result<Option<SnapshotDto>, IpcError> {
    let rev = unsigned(&expected_revision).map_err(|e| IpcError::domain(e, 0))?;
    let recovery = app.state::<crate::recovery::Recovery>().inner().clone();
    let project = io_job(&jobs, rev, move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Projeto Meraki", &["meraki"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|e| e.to_string())?;
        meraki_project::load(&path).map(Some)
    })
    .await?;
    match project {
        Some(p) => {
            let previous = engine.project().await?;
            let preserve = recovery.clone();
            io_job(&jobs, rev, move || preserve.preserve_previous(&previous)).await?;
            let result = engine.open(p.clone(), rev).await?;
            io_job(&jobs, rev, move || {
                recovery.saved(&p);
                Ok(())
            })
            .await?;
            Ok(Some(result))
        }
        None => Ok(None),
    }
}
#[tauri::command]
pub fn transport_command(
    transport: State<'_, Transport>,
    command: TransportCommand,
) -> Result<TransportDto, IpcError> {
    transport.command(command)
}
#[tauri::command]
pub fn get_transport(transport: State<'_, Transport>) -> TransportDto {
    transport.state()
}
