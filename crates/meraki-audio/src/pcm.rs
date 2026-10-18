use meraki_core::Time;
use meraki_media::{command, seek_decimal, Asset, Tools};
use serde::Serialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peaks {
    pub block_frames: u32,
    pub sample_rate: u32,
    pub frames: String,
    pub values: Vec<[f32; 2]>,
}
pub struct PcmSource {
    _temp: tempfile::NamedTempFile,
    reader: File,
    frames: u64,
    cache_start: u64,
    cache: Vec<[f32; 2]>,
    pub peaks: Peaks,
}
fn wait(
    child: &mut std::process::Child,
    stop: &AtomicBool,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if stop.load(Ordering::Acquire) || Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Preparação de áudio cancelada ou excedeu o prazo".into());
        }
        match child.try_wait() {
            Ok(Some(s)) => {
                return if s.success() {
                    Ok(())
                } else {
                    Err(format!("FFmpeg áudio: {s}"))
                }
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    }
}
pub fn sample_index(t: Time) -> Result<u64, String> {
    let n = i128::from(t.numerator()) * 48000 / i128::from(t.denominator());
    u64::try_from(n).map_err(|_| "Tempo de áudio inválido".into())
}
/// PCM disk cache: bounded RAM, at most 2 GiB per source; no callback disk I/O.
pub fn prepare(
    asset: &Asset,
    directory: &Path,
    stop: &AtomicBool,
) -> Result<Option<PcmSource>, String> {
    let tools = Tools::discover();
    let mut probe = command(&tools.ffprobe)
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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    wait(&mut probe, stop, Duration::from_secs(10))?;
    let out = probe.wait_with_output().map_err(|e| e.to_string())?;
    if out.stdout.is_empty() {
        return Ok(None);
    }
    let frames = sample_index(asset.duration)?;
    if frames > 268_435_456 {
        return Err("Fonte excede o limite Meraki de 2 GiB de PCM por arquivo".into());
    }
    let temp = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    let filter = format!(
        "asetpts=PTS-({})/TB,aresample=48000:async=1:first_pts=0,apad,atrim=end_sample={frames}",
        seek_decimal(asset.start_time)
    );
    let mut child = command(&tools.ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-copyts",
            "-threads",
            "2",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(&asset.path)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-af",
            &filter,
            "-ac",
            "2",
            "-ar",
            "48000",
            "-c:a",
            "pcm_f32le",
            "-f",
            "f32le",
        ])
        .arg(temp.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    wait(&mut child, stop, Duration::from_secs(300))?;
    let mut reader = File::open(temp.path()).map_err(|e| e.to_string())?;
    let frames = reader.metadata().map_err(|e| e.to_string())?.len() / 8;
    let block = frames.div_ceil(10000).max(480) as u32;
    let mut values = Vec::new();
    let mut bytes = vec![0u8; block as usize * 8];
    let mut remaining = frames;
    while remaining > 0 {
        if stop.load(Ordering::Acquire) {
            return Err("Cancelado".into());
        }
        let count = remaining.min(u64::from(block)) as usize;
        reader
            .read_exact(&mut bytes[..count * 8])
            .map_err(|e| e.to_string())?;
        let mut min = 0f32;
        let mut max = 0f32;
        for b in bytes[..count * 8].as_chunks::<4>().0 {
            let v = f32::from_le_bytes(*b);
            if v.is_finite() {
                min = min.min(v);
                max = max.max(v);
            }
        }
        values.push([min.clamp(-1., 1.), max.clamp(-1., 1.)]);
        remaining -= count as u64;
    }
    Ok(Some(PcmSource {
        _temp: temp,
        reader,
        frames,
        cache_start: u64::MAX,
        cache: Vec::new(),
        peaks: Peaks {
            block_frames: block,
            sample_rate: 48000,
            frames: frames.to_string(),
            values,
        },
    }))
}
impl PcmSource {
    fn sample(&mut self, index: u64) -> Result<[f32; 2], String> {
        if index >= self.frames {
            return Ok([0.; 2]);
        }
        if index < self.cache_start || index - self.cache_start >= self.cache.len() as u64 {
            self.cache_start = index / 4096 * 4096;
            let count = (self.frames - self.cache_start).min(4096) as usize;
            let mut bytes = vec![0u8; count * 8];
            self.reader
                .seek(SeekFrom::Start(self.cache_start * 8))
                .and_then(|_| self.reader.read_exact(&mut bytes))
                .map_err(|e| e.to_string())?;
            self.cache = bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|b| {
                    [
                        f32::from_le_bytes(b[..4].try_into().unwrap()),
                        f32::from_le_bytes(b[4..].try_into().unwrap()),
                    ]
                    .map(|v| if v.is_finite() { v } else { 0. })
                })
                .collect();
        }
        Ok(self.cache[(index - self.cache_start) as usize])
    }
    pub fn at(&mut self, time: Time) -> Result<[f32; 2], String> {
        let n = i128::from(time.numerator()) * 48000;
        let d = i128::from(time.denominator());
        if n < 0 {
            return Ok([0.; 2]);
        }
        let index = u64::try_from(n / d).map_err(|_| "Overflow de amostra")?;
        let f = (n % d) as f32 / d as f32;
        let a = self.sample(index)?;
        if f == 0. {
            return Ok(a);
        }
        let b = self.sample(index + 1)?;
        Ok([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f])
    }
}
