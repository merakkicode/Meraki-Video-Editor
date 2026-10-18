use crate::{render, CancelToken, ExportSettingsDto, FramePlan, RenderResult, TimelineRange};
use meraki_media::Tools;
use meraki_project::Project;
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderStatus {
    Ready,
    Rendering,
    Completed,
    Cancelled,
    Error,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderJobDto {
    pub id: String,
    pub version: String,
    pub name: String,
    pub settings: ExportSettingsDto,
    pub timeline_range: TimelineRange,
    pub status: RenderStatus,
    pub current_frame: String,
    pub total_frames: String,
    pub percentage: f64,
    pub render_fps: f64,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct RenderProgressDto {
    pub job_id: String,
    pub current_frame: String,
    pub total_frames: String,
    pub percentage: f64,
    pub render_fps: f64,
    pub eta_seconds: Option<f64>,
    pub job: RenderJobDto,
}
impl From<RenderJobDto> for RenderProgressDto {
    fn from(job: RenderJobDto) -> Self {
        Self {
            job_id: job.id.clone(),
            current_frame: job.current_frame.clone(),
            total_frames: job.total_frames.clone(),
            percentage: job.percentage,
            render_fps: job.render_fps,
            eta_seconds: job.eta_seconds,
            job,
        }
    }
}
struct Entry {
    dto: RenderJobDto,
    project: Arc<Project>,
    cancel: CancelToken,
}
#[derive(Default)]
struct State {
    jobs: Vec<Entry>,
    running: bool,
    next: u64,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    stop: AtomicBool,
    notify: Arc<dyn Fn(RenderProgressDto) + Send + Sync>,
}
#[derive(Clone)]
pub struct RenderQueue {
    shared: Arc<Shared>,
    join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}
fn bump(j: &mut RenderJobDto) {
    j.version = (j.version.parse::<u64>().unwrap_or(0) + 1).to_string();
}
impl RenderQueue {
    pub fn spawn(
        notify: impl Fn(RenderProgressDto) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            notify: Arc::new(notify),
        });
        let worker = shared.clone();
        let join = thread::Builder::new()
            .name("meraki-offline-render".into())
            .spawn(move || run_queue(worker))?;
        Ok(Self {
            shared,
            join: Arc::new(Mutex::new(Some(join))),
        })
    }
    pub fn jobs(&self) -> Vec<RenderJobDto> {
        self.shared
            .state
            .lock()
            .unwrap()
            .jobs
            .iter()
            .map(|e| e.dto.clone())
            .collect()
    }
    pub fn add(
        &self,
        project: Project,
        settings: ExportSettingsDto,
        range: TimelineRange,
    ) -> RenderResult<RenderJobDto> {
        settings.validate()?;
        project.validate()?;
        let total = FramePlan::new(&project.timeline, &range, settings.frame_rate)?.total;
        let mut state = self.shared.state.lock().unwrap();
        if state.jobs.len() >= 32 {
            return Err(
                "Fila limitada a 32 trabalhos por sessão. Reinicie o app para uma nova fila."
                    .into(),
            );
        }
        if state.jobs.iter().any(|e| {
            matches!(e.dto.status, RenderStatus::Ready | RenderStatus::Rendering)
                && e.dto.settings.output.to_string_lossy().to_lowercase()
                    == settings.output.to_string_lossy().to_lowercase()
        }) {
            return Err("Já existe um trabalho pendente para esse destino.".into());
        }
        state.next += 1;
        let dto = RenderJobDto {
            id: state.next.to_string(),
            version: "1".into(),
            name: project.timeline.name.clone(),
            settings,
            timeline_range: range,
            status: RenderStatus::Ready,
            current_frame: "0".into(),
            total_frames: total.to_string(),
            percentage: 0.,
            render_fps: 0.,
            eta_seconds: None,
            error: None,
        };
        state.jobs.push(Entry {
            dto: dto.clone(),
            project: Arc::new(project),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        drop(state);
        (self.shared.notify)(dto.clone().into());
        self.shared.wake.notify_one();
        Ok(dto)
    }
    pub fn start(&self) {
        self.shared.state.lock().unwrap().running = true;
        self.shared.wake.notify_one();
    }
    pub fn cancel(&self, id: &str) -> RenderResult<()> {
        let mut state = self.shared.state.lock().unwrap();
        let e = state
            .jobs
            .iter_mut()
            .find(|e| e.dto.id == id)
            .ok_or("Trabalho não encontrado")?;
        e.cancel.store(true, Ordering::Release);
        if e.dto.status == RenderStatus::Ready {
            e.dto.status = RenderStatus::Cancelled;
            bump(&mut e.dto);
        }
        let dto = e.dto.clone();
        drop(state);
        (self.shared.notify)(dto.into());
        Ok(())
    }
    pub fn shutdown(&self) {
        self.shared.stop.store(true, Ordering::Release);
        {
            let state = self.shared.state.lock().unwrap();
            for e in &state.jobs {
                e.cancel.store(true, Ordering::Release);
            }
        }
        self.shared.wake.notify_all();
        if let Some(join) = self.join.lock().unwrap().take() {
            let _ = join.join();
        }
    }
}
fn run_queue(shared: Arc<Shared>) {
    // Discovery here is cheap; actual probes and child processes run only in this worker.
    let _tools = Tools::discover();
    loop {
        let mut state = shared.state.lock().unwrap();
        while !shared.stop.load(Ordering::Acquire)
            && (!state.running
                || !state
                    .jobs
                    .iter()
                    .any(|e| e.dto.status == RenderStatus::Ready))
        {
            state.running = false;
            state = shared.wake.wait(state).unwrap();
        }
        if shared.stop.load(Ordering::Acquire) {
            break;
        }
        let e = state
            .jobs
            .iter_mut()
            .find(|e| e.dto.status == RenderStatus::Ready)
            .unwrap();
        e.dto.status = RenderStatus::Rendering;
        bump(&mut e.dto);
        let initial = e.dto.clone();
        let project = e.project.clone();
        let cancel = e.cancel.clone();
        drop(state);
        (shared.notify)(initial.clone().into());
        let progress_shared = shared.clone();
        let id = initial.id.clone();
        let callback = Arc::new(
            move |current: i64, total: i64, fps: f64, eta: Option<f64>| {
                let mut state = progress_shared.state.lock().unwrap();
                let e = state.jobs.iter_mut().find(|e| e.dto.id == id).unwrap();
                if e.dto.status != RenderStatus::Rendering {
                    return;
                }
                e.dto.current_frame = current.to_string();
                e.dto.percentage = (current as f64 * 100. / total as f64).min(99.99);
                e.dto.render_fps = fps;
                e.dto.eta_seconds = eta;
                bump(&mut e.dto);
                let dto = e.dto.clone();
                drop(state);
                (progress_shared.notify)(dto.into());
            },
        );
        let result = render(
            &project,
            &initial.settings,
            &initial.timeline_range,
            cancel.clone(),
            callback,
        );
        let mut state = shared.state.lock().unwrap();
        let e = state
            .jobs
            .iter_mut()
            .find(|e| e.dto.id == initial.id)
            .unwrap();
        match result {
            Ok(()) => {
                e.dto.status = RenderStatus::Completed;
                e.dto.current_frame = e.dto.total_frames.clone();
                e.dto.percentage = 100.;
                e.dto.eta_seconds = Some(0.);
            }
            Err(_) if cancel.load(Ordering::Acquire) => {
                e.dto.status = RenderStatus::Cancelled;
                e.dto.eta_seconds = None;
            }
            Err(error) => {
                e.dto.status = RenderStatus::Error;
                e.dto.error = Some(error);
                e.dto.eta_seconds = None;
            }
        }
        bump(&mut e.dto);
        let dto = e.dto.clone();
        drop(state);
        (shared.notify)(dto.into());
    }
}
