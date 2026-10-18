#![forbid(unsafe_code)]
mod recovery;
use recovery::*;
mod workflow;
use workflow::*;
mod actor;
mod audio_commands;
use audio_commands::*;
mod dto;
mod frame_server;
mod media_commands;
mod render_commands;
mod transport;
use actor::{EngineHandle, Reply};
use dto::{IpcError, PreviewViewport};
use media_commands::*;
use meraki_render::RenderQueue;
use render_commands::*;
use tauri::{Emitter, Manager, State};
use transport::Transport;

#[tauri::command]
async fn get_state(engine: State<'_, EngineHandle>) -> Reply {
    engine.get().await
}
#[tauri::command]
async fn apply_edit(engine: State<'_, EngineHandle>, envelope: serde_json::Value) -> Reply {
    engine
        .apply(
            serde_json::from_value(envelope)
                .map_err(|_| meraki_core::Error::Invalid("Envelope IPC inválido.")),
        )
        .await
}
#[tauri::command]
async fn undo(engine: State<'_, EngineHandle>, envelope: serde_json::Value) -> Reply {
    engine
        .undo(
            serde_json::from_value(envelope)
                .map_err(|_| meraki_core::Error::Invalid("Envelope IPC inválido.")),
        )
        .await
}
#[tauri::command]
async fn set_preview_viewport(
    engine: State<'_, EngineHandle>,
    viewport: PreviewViewport,
) -> Result<(), IpcError> {
    engine.viewport(viewport).await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("meraki-frame", |ctx, request, responder| {
            if ctx.webview_label() != "main" {
                responder.respond(
                    tauri::http::Response::builder()
                        .status(403)
                        .body(Vec::<u8>::new())
                        .unwrap(),
                );
                return;
            }
            ctx.app_handle()
                .state::<frame_server::FrameServer>()
                .serve(request, responder);
        })
        .setup(|app| {
            let render_app = app.handle().clone();
            app.manage(RenderQueue::spawn(move |progress| {
                let _ = render_app.emit_to("main", "render://progress", progress);
            })?);
            let handle = app.handle().clone();
            let transport_handle = handle.clone();
            let audio_app = handle.clone();
            let audio = meraki_audio::AudioEngine::spawn(actor::empty_timeline(), move |s| {
                let _ = audio_app.emit_to("main", "audio://state", s);
            })?;
            app.manage(audio.clone());
            let transport = Transport::spawn(actor::empty_timeline(), audio, move |s| {
                let _ = transport_handle.emit_to("main", "transport://state", s);
            })?;
            app.manage(frame_server::FrameServer::spawn(transport.clone())?);
            app.manage(transport.clone());
            app.manage(IoJobs(std::sync::Arc::new(tokio::sync::Semaphore::new(2))));
            let engine = EngineHandle::spawn_for(
                actor::empty_timeline(),
                Some(transport),
                move |snapshot| {
                    let _ = handle.emit_to("main", "engine://snapshot", snapshot);
                },
            )?;
            app.manage(Recovery::spawn(
                app.path().app_data_dir()?.join("recovery"),
                engine.clone(),
                app.handle().clone(),
            )?);
            app.manage(ProxyJobs::default());
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            recovery_list,
            recovery_action,
            media_status,
            generate_proxy,
            set_proxies,
            relink_media,
            get_audio_state,
            get_audio_peaks,
            get_eq_response,
            get_state,
            apply_edit,
            undo,
            set_preview_viewport,
            import_media,
            insert_media,
            save_project,
            open_project,
            transport_command,
            get_transport,
            get_export_presets,
            get_render_jobs,
            add_render_job,
            start_render_queue,
            cancel_render_job,
            browse_export_folder
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Meraki")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<Recovery>().shutdown();
                app.state::<ProxyJobs>().shutdown();
                app.state::<Transport>().shutdown();
                app.state::<meraki_audio::AudioEngine>().shutdown();
                app.state::<RenderQueue>().shutdown();
            }
        });
}
