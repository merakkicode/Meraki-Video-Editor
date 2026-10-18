use crate::{
    actor::{external_error, EngineHandle},
    dto::{unsigned, IpcError, SnapshotDto},
    media_commands::{io_job, IoJobs},
    transport::Transport,
};
use meraki_media::{probe, Tools};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct ProxyJobs {
    busy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl ProxyJobs {
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}
fn proxy_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join(".meraki/proxies"))
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStatus {
    offline: Vec<String>,
    proxies: Vec<String>,
    enabled: bool,
    busy: bool,
    message: String,
}
#[tauri::command]
pub async fn media_status(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
    proxy: State<'_, ProxyJobs>,
) -> Result<MediaStatus, IpcError> {
    let project = engine.project().await?;
    let dir = proxy_dir(&app).map_err(|e| external_error(e, 0))?;
    let busy = proxy.busy.load(Ordering::Acquire);
    let message = proxy.status.lock().unwrap().clone();
    io_job(&jobs, 0, move || {
        let mut offline = vec![];
        let mut proxies = vec![];
        for a in &project.assets {
            if !a.path.is_file() {
                offline.push(a.id.0.to_string());
            } else if meraki_media::load_proxy(a, &dir).unwrap_or(false) {
                proxies.push(a.id.0.to_string());
            }
        }
        Ok(MediaStatus {
            offline,
            proxies,
            enabled: meraki_media::proxies_enabled(),
            busy,
            message,
        })
    })
    .await
}
#[tauri::command]
pub async fn set_proxies(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
    transport: State<'_, Transport>,
    enabled: bool,
) -> Result<(), IpcError> {
    let project = engine.project().await?;
    let dir = proxy_dir(&app).map_err(|e| external_error(e, 0))?;
    io_job(&jobs, 0, move || {
        if enabled {
            for a in &project.assets {
                let _ = meraki_media::load_proxy(a, &dir);
            }
        }
        meraki_media::enable_proxies(enabled);
        Ok(())
    })
    .await?;
    transport.refresh();
    Ok(())
}
#[tauri::command]
pub async fn generate_proxy(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    proxy: State<'_, ProxyJobs>,
    asset_id: String,
) -> Result<(), IpcError> {
    let id = unsigned::<u128>(&asset_id).map_err(|e| IpcError::domain(e, 0))?;
    let project = engine.project().await?;
    let asset = project
        .assets
        .into_iter()
        .find(|a| a.id.0 == id)
        .ok_or_else(|| external_error("Mídia ausente".into(), 0))?;
    let dir = proxy_dir(&app).map_err(|e| external_error(e, 0))?;
    if proxy
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(external_error(
            "Já existe um proxy em processamento".into(),
            0,
        ));
    }
    if let Some(old) = proxy.join.lock().unwrap().take() {
        let _ = old.join();
    }
    let busy = proxy.busy.clone();
    let stop = proxy.stop.clone();
    let status = proxy.status.clone();
    *status.lock().unwrap() = format!("Gerando proxy: {}", asset.name);
    let spawn = std::thread::Builder::new()
        .name("meraki-proxy".into())
        .spawn(move || {
            let result = meraki_media::generate_proxy(&asset, &dir, &stop);
            let message = match result {
                Ok(()) => format!("Proxy pronto: {}", asset.name),
                Err(e) => format!("Proxy: {e}"),
            };
            *status.lock().unwrap() = message.clone();
            busy.store(false, Ordering::Release);
            app.state::<Transport>().refresh();
            let _ = app.emit_to("main", "media://proxy", message);
        });
    match spawn {
        Ok(j) => {
            *proxy.join.lock().unwrap() = Some(j);
            Ok(())
        }
        Err(e) => {
            proxy.busy.store(false, Ordering::Release);
            Err(external_error(e.to_string(), 0))
        }
    }
}
#[tauri::command]
pub async fn relink_media(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
    asset_id: String,
    expected_revision: String,
) -> Result<Option<SnapshotDto>, IpcError> {
    let rev = unsigned(&expected_revision).map_err(|e| IpcError::domain(e, 0))?;
    let id = unsigned::<u128>(&asset_id).map_err(|e| IpcError::domain(e, rev))?;
    let project = engine.project().await?;
    let Some(old) = project.assets.iter().find(|a| a.id.0 == id).cloned() else {
        return Err(external_error("Mídia não encontrada".into(), rev));
    };
    let updated = io_job(&jobs, rev, move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_title("Reconectar Mídia")
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let asset = probe(
            &Tools::discover(),
            &file.into_path().map_err(|e| e.to_string())?,
            old.id,
        )?;
        let project = project.relink(asset)?;
        Ok(Some(project))
    })
    .await?;
    match updated {
        Some(p) => engine.open(p, rev).await.map(Some),
        None => Ok(None),
    }
}
