use crate::{encoding_args, ExportSettingsDto, FramePlan, RenderResult, TimelineRange};
use meraki_core::{ClipId, Time};
use meraki_media::{command, seek_decimal, Asset, DecoderSession, Tools, VideoFrame};
use meraki_project::Project;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub type CancelToken = Arc<AtomicBool>;
pub type ProgressCallback = Arc<dyn Fn(i64, i64, f64, Option<f64>) + Send + Sync>;
/// Per-sample gain expression, with the same cubic easing as native playback.
fn volume_filter(clip: &meraki_timeline::Clip, start: Time) -> RenderResult<String> {
    use meraki_timeline::{seconds, Interpolation, Property};
    let Some(keys) = clip
        .motion
        .keys
        .get(&Property::Volume)
        .filter(|k| !k.is_empty())
    else {
        return Ok("anull".into());
    };
    let offset = start
        .checked_sub(clip.timeline_start)
        .and_then(|t| t.checked_add(clip.motion.offset))
        .map_err(|e| e.to_string())?;
    let x = format!("(t+{:.12})", seconds(offset));
    let mut expr = format!("{:.9}", keys.last().unwrap().value);
    for w in keys.windows(2).rev() {
        let a = &w[0];
        let b = &w[1];
        let u = format!(
            "(({x}-{:.12})/{:.12})",
            seconds(a.at),
            seconds(b.at.checked_sub(a.at).map_err(|e| e.to_string())?)
        );
        let u = match a.interpolation {
            Interpolation::Linear => u,
            Interpolation::Smooth => format!("({u}*{u}*(3-2*{u}))"),
        };
        expr = format!(
            "if(lt({x},{:.12}),({:.9}+({:.9})*{u}),{expr})",
            seconds(b.at),
            a.value,
            b.value - a.value
        );
    }
    expr = format!(
        "if(lt({x},{:.12}),{:.9},{expr})",
        seconds(keys[0].at),
        keys[0].value
    );
    Ok(format!(
        "aeval=exprs='val(0)*({expr})|val(1)*({expr})':channel_layout=stereo"
    ))
}
fn cancelled(c: &CancelToken) -> RenderResult<()> {
    if c.load(Ordering::Acquire) {
        Err("Render cancelado".into())
    } else {
        Ok(())
    }
}

/// Bounded metadata query. Polling permits cancellation even if a corrupt file stalls probe.
fn has_audio(tools: &Tools, asset: &Asset, cancel: &CancelToken) -> RenderResult<bool> {
    let mut child = command(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=index",
            "-of",
            "csv=p=0",
        ])
        .arg(&asset.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if cancel.load(Ordering::Acquire) || Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Probe de áudio cancelado ou excedeu 10 s".into());
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("Falha ao inspecionar áudio: {}", asset.name));
    }
    Ok(!out.stdout.is_empty())
}
fn atempo(speed: Time) -> String {
    let mut speed = speed.numerator() as f64 / f64::from(speed.denominator());
    let mut filters = Vec::new();
    while speed > 2. {
        filters.push("atempo=2".to_string());
        speed /= 2.;
    }
    while speed < 0.5 {
        filters.push("atempo=0.5".to_string());
        speed *= 2.;
    }
    filters.push(format!("atempo={speed:.12}"));
    filters.join(",")
}
fn audio_graph(
    project: &Project,
    plan: &FramePlan,
    tools: &Tools,
    cancel: &CancelToken,
) -> RenderResult<(Vec<Asset>, String)> {
    let catalog = project.validate()?;
    let mut inputs = Vec::new();
    let mut indices = BTreeMap::new();
    let mut chains = Vec::new();
    let mut labels = Vec::new();
    let solo = project.timeline.tracks.iter().any(|t| {
        t.enabled
            && project
                .timeline
                .mixer
                .tracks
                .get(&t.id)
                .is_some_and(|m| m.solo)
    });
    for track in project.timeline.tracks.iter().filter(|t| {
        t.enabled
            && (!solo
                || project
                    .timeline
                    .mixer
                    .tracks
                    .get(&t.id)
                    .is_some_and(|m| m.solo))
    }) {
        let channel = project
            .timeline
            .mixer
            .tracks
            .get(&track.id)
            .cloned()
            .unwrap_or_default();
        let dsp = meraki_audio::ffmpeg_channel_filter(&channel);
        for clip in track
            .clips
            .iter()
            .filter(|c| c.enabled && c.motion.title.is_none())
        {
            cancelled(cancel)?;
            let start = clip.timeline_start.max(plan.start);
            let end = clip.end().map_err(|e| e.to_string())?.min(plan.end);
            if start >= end {
                continue;
            }
            let asset = catalog
                .0
                .get(&clip.source.asset)
                .ok_or("Asset de áudio ausente")?;
            if let std::collections::btree_map::Entry::Vacant(e) = indices.entry(asset.id) {
                let index = if has_audio(tools, asset, cancel)? {
                    inputs.push(asset.clone());
                    Some(inputs.len())
                } else {
                    None
                };
                e.insert(index);
            }
            let Some(index) = indices[&asset.id] else {
                continue;
            };
            let source_start = clip
                .source_at(start)
                .map_err(|e| e.to_string())?
                .checked_add(asset.start_time)
                .map_err(|e| e.to_string())?;
            let source_end = source_start
                .checked_add(
                    end.checked_sub(start)
                        .and_then(|d| d.scale(clip.speed.ratio()))
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
            let offset = start.checked_sub(plan.start).map_err(|e| e.to_string())?;
            let samples = i128::from(offset.numerator()) * 48000 / i128::from(offset.denominator());
            let label = format!("a{}", labels.len());
            let automation = volume_filter(clip, start)?;
            chains.push(format!("[{index}:a:0]atrim=start={}:end={},asetpts=PTS-({})/TB,{},aresample=48000:async=1:first_pts=0,aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo,{automation},{dsp},adelay={samples}S:all=1[{label}]",seek_decimal(source_start),seek_decimal(source_end),seek_decimal(source_start),atempo(clip.speed.ratio())));
            labels.push(format!("[{label}]"));
        }
    }
    let duration = plan
        .rate
        .time_at_frame(plan.total)
        .map_err(|e| e.to_string())?;
    if labels.is_empty() {
        chains.push(format!(
            "anullsrc=r=48000:cl=stereo,atrim=duration={}[mix]",
            seek_decimal(duration)
        ));
    } else {
        chains.push(format!(
            "{}amix=inputs={}:normalize=0,apad,atrim=duration={}[mix]",
            labels.join(""),
            labels.len(),
            seek_decimal(duration)
        ));
    }
    if inputs.len() > 32 || chains.len() > 128 {
        return Err("Meraki suporta até 32 fontes e 127 segmentos de áudio por render.".into());
    }
    chains.push(format!(
        "[mix]{}[finalmix]",
        meraki_audio::ffmpeg_channel_filter(&project.timeline.mixer.master)
    ));
    Ok((inputs, chains.join(";")))
}
struct Encoder {
    input: Option<ChildStdin>,
    abort: Arc<AtomicBool>,
    watcher: Option<JoinHandle<RenderResult<ExitStatus>>>,
    readers: Vec<JoinHandle<()>>,
    log: Arc<std::sync::Mutex<String>>,
}
impl Encoder {
    fn spawn(
        mut child: Child,
        cancel: CancelToken,
        total: i64,
        progress: ProgressCallback,
    ) -> RenderResult<Self> {
        let input = child.stdin.take().ok_or("Encoder sem stdin")?;
        let stdout = child.stdout.take().ok_or("Encoder sem progresso")?;
        let stderr = child.stderr.take().ok_or("Encoder sem stderr")?;
        let abort = Arc::new(AtomicBool::new(false));
        let abort_watch = abort.clone();
        // Spawn infallible thread API: on OS thread exhaustion the process terminates,
        // rather than leaving a partially-owned child behind.
        let watcher = thread::spawn(move || loop {
            if cancel.load(Ordering::Acquire) || abort_watch.load(Ordering::Acquire) {
                let _ = child.kill();
                return child.wait().map_err(|e| e.to_string());
            }
            match child.try_wait() {
                Ok(Some(s)) => return Ok(s),
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e.to_string());
                }
            }
        });
        let log = Arc::new(std::sync::Mutex::new(String::new()));
        let logs = log.clone();
        let errors = thread::spawn(move || {
            let mut reader = stderr;
            let mut buf = [0u8; 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut log = logs.lock().unwrap();
                if log.len() < 8192 {
                    log.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
            }
        });
        let status = thread::spawn(move || {
            let started = Instant::now();
            let mut frame = 0;
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(value) = line.strip_prefix("frame=") {
                    frame = value.trim().parse::<i64>().unwrap_or(frame).clamp(0, total);
                }
                if line.starts_with("progress=") {
                    let seconds = started.elapsed().as_secs_f64();
                    let fps = if seconds > 0. {
                        frame as f64 / seconds
                    } else {
                        0.
                    };
                    let eta = (fps > 0.).then(|| (total - frame) as f64 / fps);
                    progress(frame, total, fps, eta);
                }
            }
        });
        Ok(Self {
            input: Some(input),
            abort,
            watcher: Some(watcher),
            readers: vec![errors, status],
            log,
        })
    }
    fn finish(mut self) -> RenderResult<()> {
        self.input.take();
        let status = self
            .watcher
            .take()
            .unwrap()
            .join()
            .map_err(|_| "Thread do encoder falhou")??;
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        if !status.success() {
            return Err(format!(
                "FFmpeg terminou com {status}: {}",
                self.log.lock().unwrap()
            ));
        }
        Ok(())
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.abort.store(true, Ordering::Release);
        self.input.take();
        if let Some(w) = self.watcher.take() {
            let _ = w.join();
        }
        for r in self.readers.drain(..) {
            let _ = r.join();
        }
    }
}

/// Purely sequential offline loop. Every planned output frame is written once, even on cache hits.
pub fn render(
    project: &Project,
    settings: &ExportSettingsDto,
    range: &TimelineRange,
    cancel: CancelToken,
    progress: ProgressCallback,
) -> RenderResult<()> {
    let _session = meraki_compositor::CompositorSession;
    settings.validate()?;
    let catalog = project.validate()?;
    let plan = FramePlan::new(&project.timeline, range, settings.frame_rate)?;
    cancelled(&cancel)?;
    if settings.output.exists() {
        return Err(
            "O destino já existe. Escolha outro nome; arquivos existentes nunca são sobrescritos."
                .into(),
        );
    }
    let parent = settings.output.parent().ok_or("Pasta de saída ausente")?;
    if !parent.is_dir() {
        return Err("A pasta de saída não existe.".into());
    }
    // Same-volume temporary; publication occurs only after encoder flush and successful exit.
    let temp = tempfile::Builder::new()
        .prefix(".meraki-render-")
        .suffix(".partial")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?;
    let tools = Tools::discover();
    let mut cmd = command(&tools.ffmpeg);
    cmd.args([
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-nostats",
        "-stats_period",
        "0.2",
        "-progress",
        "pipe:1",
        "-y",
        "-copyts",
        "-filter_complex_threads",
        "1",
        "-f",
        "rawvideo",
        "-pixel_format",
        "rgba",
        "-video_size",
    ])
    .arg(format!("{}x{}", settings.width, settings.height))
    .args([
        "-framerate",
        &format!(
            "{}/{}",
            settings.frame_rate.numerator(),
            settings.frame_rate.denominator()
        ),
        "-i",
        "pipe:0",
    ]);
    let mut filter_file = None;
    if settings.audio_codec != "none" {
        let (inputs, graph) = audio_graph(project, &plan, &tools, &cancel)?;
        for asset in inputs {
            cmd.args(["-threads", "2", "-protocol_whitelist", "file,pipe", "-i"])
                .arg(asset.path);
        }
        let mut filter = tempfile::Builder::new()
            .prefix(".meraki-audio-")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        filter
            .write_all(graph.as_bytes())
            .map_err(|e| e.to_string())?;
        cmd.arg("-/filter_complex").arg(filter.path()).args([
            "-map",
            "0:v:0",
            "-map",
            "[finalmix]",
        ]);
        filter_file = Some(filter);
    } else {
        cmd.args(["-map", "0:v:0", "-an"]);
    }
    cmd.args(["-fps_mode", "passthrough"])
        .args(encoding_args(settings)?)
        .arg(temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut encoder = Encoder::spawn(
        cmd.spawn()
            .map_err(|e| format!("Falha ao iniciar encoder: {e}"))?,
        cancel.clone(),
        plan.total,
        progress,
    )?;
    let mut decoders: BTreeMap<ClipId, (DecoderSession, Option<Arc<VideoFrame>>)> = BTreeMap::new();
    for index in 0..plan.total {
        cancelled(&cancel)?;
        let at = plan.time(index)?;
        let layers = meraki_compositor::active_layers(&project.timeline, at);
        if layers.len() > 8 {
            return Err("Render limitado a 8 camadas simultâneas".into());
        }
        decoders.retain(|id, _| layers.iter().any(|(c, _)| c.id == *id));
        let mut frames = Vec::new();
        for (clip, source) in layers {
            if let Some(title) = &clip.motion.title {
                frames.push((clip, meraki_compositor::title_frame(title)?));
                continue;
            }
            if let std::collections::btree_map::Entry::Vacant(e) = decoders.entry(clip.id) {
                let asset = catalog
                    .0
                    .get(&clip.source.asset)
                    .ok_or("Asset ausente do snapshot")?;
                e.insert((
                    DecoderSession::start_composition(&tools, asset, source)?,
                    None,
                ));
            }
            let (decoder, previous) = decoders.get_mut(&clip.id).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            while !previous.as_ref().is_some_and(|f| f.covers(source)) {
                cancelled(&cancel)?;
                if Instant::now() > deadline {
                    return Err("Decoder não entregou o frame em 30 s".into());
                }
                match decoder.next(Duration::from_millis(25)) {
                    Ok(Ok(f)) => {
                        if f.pts > source {
                            return Err("Lacuna de PTS na fonte".into());
                        }
                        *previous = Some(f);
                    }
                    Ok(Err(e)) => return Err(e),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            frames.push((clip, previous.as_ref().unwrap().clone()));
        }
        let pixels = meraki_compositor::compose_at(
            &project.timeline,
            &frames,
            [settings.width, settings.height],
            at,
        )?;
        cancelled(&cancel)?;
        encoder
            .input
            .as_mut()
            .unwrap()
            .write_all(&pixels)
            .map_err(|e| format!("Pipe de exportação encerrado: {e}"))?;
    }
    drop(decoders);
    encoder.finish()?;
    cancelled(&cancel)?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    // Never replace an existing output, including files created while the render was running.
    temp.persist_noclobber(&settings.output)
        .map_err(|e| e.to_string())?;
    drop(filter_file);
    Ok(())
}
pub fn output_parent(path: &Path) -> RenderResult<()> {
    if !path.is_dir() {
        Err("Escolha uma pasta existente".into())
    } else {
        Ok(())
    }
}
