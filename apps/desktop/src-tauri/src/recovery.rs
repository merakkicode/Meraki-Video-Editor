use crate::{
    actor::EngineHandle,
    dto::{unsigned, IpcError, SnapshotDto},
    media_commands::{io_job, IoJobs},
};
use meraki_project::Project;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, State};

#[derive(Clone)]
pub struct Recovery {
    pub directory: PathBuf,
    own: PathBuf,
    saved: Arc<Mutex<Option<Project>>>,
    last: Arc<Mutex<Option<Project>>>,
    pending: Arc<AtomicBool>,
    epoch: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}
impl Recovery {
    pub fn spawn(
        directory: PathBuf,
        engine: EngineHandle,
        app: tauri::AppHandle,
    ) -> std::io::Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let own = directory.join(format!(
            "recovery-{}-{}.meraki",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            std::process::id()
        ));
        let pending = !meraki_project::list_journals(&directory)
            .unwrap_or_default()
            .is_empty();
        let this = Self {
            directory,
            own,
            saved: Default::default(),
            last: Default::default(),
            epoch: Default::default(),
            pending: Arc::new(AtomicBool::new(pending)),
            stop: Default::default(),
            join: Default::default(),
        };
        let worker = this.clone();
        let join = thread::Builder::new()
            .name("meraki-autosave".into())
            .spawn(move || {
                let interval = Duration::from_secs(
                    std::env::var("MERAKI_AUTOSAVE_SECONDS")
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(180)
                        .clamp(5, 300),
                );
                let mut next = Instant::now() + interval;
                while !worker.stop.load(Ordering::Acquire) {
                    if Instant::now() >= next {
                        next = Instant::now() + interval;
                        if !worker.pending.load(Ordering::Acquire) {
                            let epoch = worker.epoch.load(Ordering::Acquire);
                            let result = tauri::async_runtime::block_on(engine.project())
                                .map_err(|e| e.message)
                                .and_then(|p| worker.checkpoint_at(&p, epoch));
                            let _ = app.emit_to(
                                "main",
                                "project://autosave",
                                match result {
                                    Ok(written) => {
                                        if written {
                                            "Recuperação automática atualizada".to_string()
                                        } else {
                                            String::new()
                                        }
                                    }
                                    Err(e) => format!("Falha no autosave: {e}"),
                                },
                            );
                        }
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            })?;
        *this.join.lock().unwrap() = Some(join);
        Ok(this)
    }
    fn checkpoint(&self, p: &Project) -> Result<bool, String> {
        self.checkpoint_at(p, self.epoch.load(Ordering::Acquire))
    }
    fn checkpoint_at(&self, p: &Project, epoch: u64) -> Result<bool, String> {
        // Serialize checkpoint/save acknowledgement to prevent a stale journal after manual save.
        let saved = self.saved.lock().unwrap();
        if self.epoch.load(Ordering::Acquire) != epoch {
            return Ok(false);
        }
        if saved.as_ref() == Some(p)
            || self.last.lock().unwrap().as_ref() == Some(p)
            || (saved.is_none()
                && p.assets.is_empty()
                && p.timeline.markers.is_empty()
                && p.timeline.tracks.iter().all(|t| t.clips.is_empty()))
        {
            return Ok(false);
        }
        meraki_project::write_journal(&self.own, p)?;
        *self.last.lock().unwrap() = Some(p.clone());
        Ok(true)
    }
    pub fn saved(&self, p: &Project) {
        let mut saved = self.saved.lock().unwrap();
        self.epoch.fetch_add(1, Ordering::AcqRel);
        *saved = Some(p.clone());
        *self.last.lock().unwrap() = None;
        let _ = std::fs::remove_file(&self.own);
    }
    pub fn preserve_previous(&self, p: &Project) -> Result<(), String> {
        self.checkpoint(p)?;
        let _guard = self.saved.lock().unwrap();
        if self.own.is_file() {
            let name = format!(
                "recovery-{}-switch-{}.meraki",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                std::process::id()
            );
            std::fs::copy(&self.own, self.directory.join(name)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}
#[tauri::command]
pub async fn recovery_list(
    state: State<'_, Recovery>,
    jobs: State<'_, IoJobs>,
) -> Result<Vec<String>, IpcError> {
    let dir = state.directory.clone();
    let own = state.own.clone();
    io_job(&jobs, 0, move || {
        Ok(meraki_project::list_journals(&dir)?
            .into_iter()
            .filter(|n| dir.join(n) != own)
            .collect())
    })
    .await
}
#[tauri::command]
pub async fn recovery_action(
    state: State<'_, Recovery>,
    engine: State<'_, EngineHandle>,
    jobs: State<'_, IoJobs>,
    name: String,
    recover: bool,
    expected_revision: String,
) -> Result<Option<SnapshotDto>, IpcError> {
    let rev = unsigned(&expected_revision).map_err(|e| IpcError::domain(e, 0))?;
    let dir = state.directory.clone();
    let file = name.clone();
    let project = io_job(&jobs, rev, move || {
        if recover {
            meraki_project::recover_journal(&dir, &file).map(Some)
        } else {
            Ok(None)
        }
    })
    .await?;
    let result = if let Some(p) = project {
        let snapshot = engine.open(p.clone(), rev).await?;
        // Preserve the recovered copy in this session before removing its old journal.
        let worker = state.inner().clone();
        io_job(&jobs, rev, move || worker.checkpoint(&p).map(|_| ())).await?;
        Some(snapshot)
    } else {
        None
    };
    let dir = state.directory.clone();
    let pending = io_job(&jobs, rev, move || {
        std::fs::remove_file(meraki_project::journal_path(&dir, &name)?)
            .map_err(|e| e.to_string())?;
        meraki_project::list_journals(&dir)
    })
    .await?;
    state.pending.store(
        !recover && pending.iter().any(|n| state.directory.join(n) != state.own),
        Ordering::Release,
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_save_invalidates_inflight_autosave_and_switch_preserves_unsaved() {
        let dir = tempfile::tempdir().unwrap();
        let recovery = Recovery {
            directory: dir.path().into(),
            own: dir.path().join("recovery-test.meraki"),
            saved: Default::default(),
            last: Default::default(),
            pending: Default::default(),
            epoch: Default::default(),
            stop: Default::default(),
            join: Default::default(),
        };
        let mut p = Project::new(crate::actor::empty_timeline(), &Default::default());
        p.timeline.markers.push(meraki_timeline::Marker {
            id: "test".into(),
            at: meraki_core::Time::ZERO,
            name: "Edição".into(),
            color: "cyan".into(),
        });
        assert!(recovery.checkpoint_at(&p, 0).unwrap());
        assert!(recovery.own.is_file());
        recovery.saved(&p);
        assert!(!recovery.own.exists());
        p.timeline.name = "Outra edição".into();
        assert!(!recovery.checkpoint_at(&p, 0).unwrap());
        assert!(!recovery.own.exists());
        assert!(recovery.checkpoint_at(&p, 1).unwrap());
        recovery.preserve_previous(&p).unwrap();
        assert_eq!(meraki_project::list_journals(dir.path()).unwrap().len(), 2);
    }
}
