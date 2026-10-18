#![forbid(unsafe_code)]
use meraki_render::{ceil_frames, presets, render, timeline_end, TimelineRange};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

fn main() -> Result<(), String> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("Uso: render_project projeto.meraki destino.mp4 (destino novo)".into());
    }
    let project = meraki_project::load(&PathBuf::from(&args[0]))?;
    let rate = project.timeline.frame_rate;
    let mut settings = presets(rate).remove(0).settings;
    settings.output = PathBuf::from(&args[1]);
    if settings.output.is_relative() {
        settings.output = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(&settings.output);
    }
    let range = TimelineRange {
        start_frame: "0".into(),
        end_frame: ceil_frames(timeline_end(&project.timeline), rate)?.to_string(),
    };
    render(
        &project,
        &settings,
        &range,
        Arc::new(AtomicBool::new(false)),
        Arc::new(|frame, total, fps, eta| {
            if frame == total || frame % 60 == 0 {
                println!("{frame}/{total} · {fps:.1} fps · ETA {eta:?}");
            }
        }),
    )?;
    println!("Exportação concluída: {}", settings.output.display());
    Ok(())
}
