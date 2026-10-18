use crate::{
    actor::EngineHandle,
    dto::{ExportPresetDto, ExportSettingsDto, IpcError, RenderJobDto, TimelineRange},
    media_commands::{io_job, IoJobs},
};
use meraki_render::RenderQueue;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
#[tauri::command]
pub async fn get_export_presets(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<ExportPresetDto>, IpcError> {
    Ok(meraki_render::presets(
        engine.project().await?.timeline.frame_rate,
    ))
}
#[tauri::command]
pub fn get_render_jobs(queue: State<'_, RenderQueue>) -> Vec<RenderJobDto> {
    queue.jobs()
}
#[tauri::command]
pub async fn add_render_job(
    engine: State<'_, EngineHandle>,
    queue: State<'_, RenderQueue>,
    settings: ExportSettingsDto,
    timeline_range: TimelineRange,
) -> Result<RenderJobDto, String> {
    let project = engine.project().await.map_err(|e| e.message)?;
    queue.add(project, settings, timeline_range)
}
#[tauri::command]
pub fn start_render_queue(queue: State<'_, RenderQueue>) {
    queue.start();
}
#[tauri::command]
pub fn cancel_render_job(queue: State<'_, RenderQueue>, job_id: String) -> Result<(), String> {
    queue.cancel(&job_id)
}
#[tauri::command]
pub async fn browse_export_folder(
    app: AppHandle,
    jobs: State<'_, IoJobs>,
) -> Result<Option<String>, IpcError> {
    io_job(&jobs, 0, move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|p| {
                p.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await
}
