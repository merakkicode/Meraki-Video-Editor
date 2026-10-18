use crate::{command, seek_decimal, Asset, MediaResult, PixelFormat, Tools, VideoFrame};
use meraki_core::Time;
use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, Stdio},
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// One FFmpeg process plus bounded pipe readers. Dropping kills/reaps the process and joins readers.
/// No blocking read runs in the scheduler, UI thread, or async executor.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub struct DecoderSession {
    child: ChildGuard,
    frames: Option<Receiver<MediaResult<Arc<VideoFrame>>>>,
    readers: Vec<JoinHandle<()>>,
}
impl DecoderSession {
    /// Composition needs source alpha and aspect, without an opaque padded frame.
    pub fn start_composition(tools: &Tools, asset: &Asset, target: Time) -> MediaResult<Self> {
        let mut output = asset.clone();
        let scale = (4096. / asset.width as f64)
            .min(4096. / asset.height as f64)
            .min(1.);
        output.preview_width = (asset.width as f64 * scale).round().max(1.) as u32;
        output.preview_height = (asset.height as f64 * scale).round().max(1.) as u32;
        Self::start(tools, &output, target)
    }
    /// Offline output uses its own full-size frames; never upscale the 720p preview cache.
    pub fn start_export(
        tools: &Tools,
        asset: &Asset,
        target: Time,
        width: u32,
        height: u32,
    ) -> MediaResult<Self> {
        if width < 2
            || height < 2
            || width > 4096
            || height > 4096
            || !width.is_multiple_of(2)
            || !height.is_multiple_of(2)
        {
            return Err("Dimensões de render inválidas".into());
        }
        let mut output = asset.clone();
        output.preview_width = width;
        output.preview_height = height;
        let filter = format!("scale={width}:{height}:flags=lanczos:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black,settb=expr={}/{},showinfo=checksum=0",asset.time_base.numerator(),asset.time_base.denominator());
        Self::with_filter(tools, &output, target, filter)
    }
    pub fn start(tools: &Tools, asset: &Asset, target: Time) -> MediaResult<Self> {
        let filter = format!(
            "scale={}:{}:flags=fast_bilinear,setsar=1,settb=expr={}/{},showinfo=checksum=0",
            asset.preview_width,
            asset.preview_height,
            asset.time_base.numerator(),
            asset.time_base.denominator()
        );
        Self::with_filter(tools, asset, target, filter)
    }
    fn with_filter(
        tools: &Tools,
        asset: &Asset,
        target: Time,
        filter: String,
    ) -> MediaResult<Self> {
        let mut command = command(&tools.ffmpeg);
        // Input seeking lands at the preceding keyframe; keep its frames and absolute PTS.
        // Normalized source time is converted to absolute stream time for seek_timestamp.
        let seek = target
            .checked_add(asset.start_time)
            .map_err(|e| e.to_string())?;
        command
            .args([
                "-hide_banner",
                "-nostdin",
                "-loglevel",
                "info",
                "-threads",
                "2",
                "-filter_threads",
                "1",
                "-protocol_whitelist",
                "file,pipe",
                "-copyts",
                "-seek_timestamp",
                "1",
                "-noaccurate_seek",
                "-ss",
                &seek_decimal(seek),
            ])
            .arg("-i")
            .arg(&asset.path)
            .args([
                "-map",
                &format!("0:{}", asset.stream_index),
                "-an",
                "-sn",
                "-dn",
                "-vf",
                &filter,
                "-fps_mode",
                "passthrough",
                "-pix_fmt",
                "rgba",
                "-threads",
                "1",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = ChildGuard(
            command
                .spawn()
                .map_err(|e| format!("Falha ao iniciar FFmpeg: {e}"))?,
        );
        let stdout = child.0.stdout.take().ok_or("Pipe de vídeo indisponível")?;
        let stderr = child.0.stderr.take().ok_or("Pipe de PTS indisponível")?;
        let (stamp_tx, stamp_rx) = mpsc::sync_channel::<(i64, i64)>(8);
        let log = Arc::new(std::sync::Mutex::new(String::new()));
        let log_writer = log.clone();
        let metadata = thread::Builder::new()
            .name("meraki-pts".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Some(stamp) = parse_stamp(&line) {
                        if stamp_tx.send(stamp).is_err() {
                            break;
                        }
                    } else if !line.contains("showinfo") {
                        let mut text = log_writer.lock().unwrap();
                        if text.len() + line.len() < 4096 {
                            text.push_str(&line);
                            text.push('\n');
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::sync_channel(2);
        let asset = asset.clone();
        let pixels = thread::Builder::new()
            .name("meraki-rgba".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                let len = asset.preview_width as usize * asset.preview_height as usize * 4;
                let mut previous: Option<VideoFrame> = None;
                loop {
                    let mut bytes = vec![0u8; len];
                    if let Err(e) = reader.read_exact(&mut bytes) {
                        if let Some(mut last) = previous.take() {
                            if let Ok(tail) = asset.duration.checked_sub(last.pts) {
                                if tail > Time::ZERO {
                                    last.duration = tail;
                                }
                            }
                            if tx.send(Ok(Arc::new(last))).is_err() {
                                break;
                            }
                        }
                        let detail = log.lock().unwrap().clone();
                        let _ = tx.send(Err(format!("Fim do decoder ({e}). {detail}")));
                        break;
                    }
                    let Ok((pts, duration)) = stamp_rx.recv() else {
                        let _ = tx.send(Err("FFmpeg não forneceu PTS para o frame.".into()));
                        break;
                    };
                    let frame = (|| {
                        let pts = asset.source_time(pts)?;
                        let duration = if duration > 0 {
                            crate::pts_to_time(duration, asset.time_base)?
                        } else {
                            asset
                                .frame_rate
                                .time_at_frame(1)
                                .map_err(|e| e.to_string())?
                        };
                        let f = VideoFrame {
                            asset: asset.id,
                            format: PixelFormat::Rgba8,
                            width: asset.preview_width,
                            height: asset.preview_height,
                            stride: asset.preview_width * 4,
                            pts,
                            duration,
                            pixels: Arc::new(bytes),
                        };
                        f.validate()?;
                        Ok(f)
                    })();
                    match frame {
                        Ok(next) => {
                            if let Some(mut prior) = previous.take() {
                                match next.pts.checked_sub(prior.pts) {
                                    Ok(duration) if duration > Time::ZERO => {
                                        prior.duration = duration;
                                        if tx.send(Ok(Arc::new(prior))).is_err() {
                                            break;
                                        }
                                    }
                                    _ => {
                                        let _ = tx.send(Err("PTS não monotônico no vídeo".into()));
                                        break;
                                    }
                                }
                            }
                            previous = Some(next);
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            child,
            frames: Some(rx),
            readers: vec![pixels, metadata],
        })
    }
    pub fn next(
        &self,
        timeout: Duration,
    ) -> Result<MediaResult<Arc<VideoFrame>>, mpsc::RecvTimeoutError> {
        self.frames.as_ref().unwrap().recv_timeout(timeout)
    }
}
impl Drop for DecoderSession {
    fn drop(&mut self) {
        let _ = self.child.0.kill();
        self.frames.take();
        let _ = self.child.0.wait();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}
pub fn parse_stamp(line: &str) -> Option<(i64, i64)> {
    if !line.contains("showinfo") || !line.contains(" pts:") {
        return None;
    }
    let number = |key: &str| {
        line.split_once(key)?
            .1
            .split_whitespace()
            .next()?
            .parse::<i64>()
            .ok()
    };
    Some((number(" pts:")?, number(" duration:").unwrap_or(0)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pts_parser_preserves_large_timestamps() {
        assert_eq!(parse_stamp("[Parsed_showinfo_3] n: 3 pts:9007199254740993 pts_time:1 duration:3003 duration_time:0.03"),Some((9007199254740993,3003)));
        assert_eq!(parse_stamp("frame=3"), None);
    }
}
