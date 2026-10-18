use crate::{Asset, MediaResult};
use meraki_core::{AssetId, FrameRate, Time};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Clone)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
impl Tools {
    pub fn discover() -> Self {
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        let mut directories = Vec::new();
        if let Some(dir) = std::env::var_os("MERAKI_FFMPEG_DIR") {
            directories.push(PathBuf::from(dir));
        }
        if let Ok(exe) = std::env::current_exe() {
            for ancestor in exe.ancestors().skip(1) {
                directories.push(ancestor.join("tools/ffmpeg/bin"));
                directories.push(ancestor.join("tools/linux/ffmpeg/bin"));
                directories.push(ancestor.join("lib/meraki/tools/ffmpeg/bin"));
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            for ancestor in cwd.ancestors() {
                directories.push(ancestor.join("tools/ffmpeg/bin"));
                directories.push(ancestor.join("tools/linux/ffmpeg/bin"));
            }
        }
        let base = directories
            .into_iter()
            .find(|p| {
                p.join(format!("ffmpeg{suffix}")).is_file()
                    && p.join(format!("ffprobe{suffix}")).is_file()
            })
            .unwrap_or_default();
        Self {
            ffmpeg: base.join(format!("ffmpeg{suffix}")),
            ffprobe: base.join(format!("ffprobe{suffix}")),
        }
    }
}
pub fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.stdin(Stdio::null());
    cmd
}
/// Blocking API: callers must use their dedicated media-I/O thread, never a Tokio executor.
pub fn probe(tools: &Tools, path: &Path, id: AssetId) -> MediaResult<Asset> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("Arquivo indisponível: {e}"))?;
    if !path.is_file() {
        return Err("Selecione um arquivo local de vídeo.".into());
    }
    let output=command(&tools.ffprobe).args(["-v","error","-protocol_whitelist","file,pipe","-show_entries","stream=codec_type,index,width,height,time_base,start_pts,start_time,duration_ts,duration,avg_frame_rate,r_frame_rate,codec_name,sample_aspect_ratio:stream_side_data=rotation:format=duration","-of","json"]).arg(&path).output().map_err(|e|format!("FFprobe não disponível. Reinstale o Meraki ou configure MERAKI_FFMPEG_DIR no ambiente de desenvolvimento. {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Não foi possível inspecionar o vídeo: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(800)
                .collect::<String>()
        ));
    }
    if output.stdout.len() > 2 * 1024 * 1024 {
        return Err("Metadados excedem 2 MiB.".into());
    }
    probe_json(&output.stdout, path, id)
}
pub fn decimal_time(text: &str) -> MediaResult<Time> {
    let (sign, text) = if let Some(t) = text.strip_prefix('-') {
        (-1i64, t)
    } else {
        (1, text)
    };
    let (whole, part) = text.split_once('.').unwrap_or((text, ""));
    if part.len() > 9
        || whole.is_empty()
        || !whole
            .bytes()
            .chain(part.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err("Tempo decimal inválido.".into());
    }
    let den = 10u32.pow(part.len() as u32);
    let n = whole
        .parse::<i64>()
        .ok()
        .and_then(|n| n.checked_mul(i64::from(den)))
        .and_then(|n| n.checked_add(part.parse::<i64>().unwrap_or(0)))
        .and_then(|n| n.checked_mul(sign))
        .ok_or("Overflow de tempo")?;
    Time::new(n, den).map_err(|e| e.to_string())
}
fn rational(text: &str) -> MediaResult<Time> {
    let (n, d) = text.split_once('/').ok_or("Fração inválida")?;
    Time::new(
        n.parse().map_err(|_| "Numerador inválido")?,
        d.parse().map_err(|_| "Denominador inválido")?,
    )
    .map_err(|e| e.to_string())
}
pub fn probe_json(bytes: &[u8], path: PathBuf, id: AssetId) -> MediaResult<Asset> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let s = json["streams"]
        .as_array()
        .and_then(|v| {
            v.iter()
                .find(|s| s["codec_type"] == "video" || s["width"].is_number())
                .or_else(|| v.iter().find(|s| s["codec_type"] == "audio"))
        })
        .ok_or("Arquivo sem stream de vídeo.")?;
    let audio_only = s["codec_type"] == "audio";
    let integer = |name: &str| -> MediaResult<u32> {
        s[name]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(format!("Campo inválido: {name}"))
    };
    let time_base = rational(s["time_base"].as_str().ok_or("Timebase ausente")?)?;
    if time_base <= Time::ZERO {
        return Err("Timebase inválida".into());
    }
    let fr = rational(s["avg_frame_rate"].as_str().unwrap_or("0/1"))
        .ok()
        .filter(|r| *r > Time::ZERO)
        .or_else(|| {
            rational(s["r_frame_rate"].as_str().unwrap_or("0/1"))
                .ok()
                .filter(|r| *r > Time::ZERO)
        })
        .or_else(|| audio_only.then(|| Time::new(30, 1).unwrap()))
        .ok_or("Taxa de quadros desconhecida")?;
    let frame_rate = FrameRate::new(
        u32::try_from(fr.numerator()).map_err(|_| "Frame rate inválido")?,
        fr.denominator(),
    )
    .map_err(|e| e.to_string())?;
    let start_time = if let Some(p) = s["start_pts"].as_i64() {
        crate::pts_to_time(p, time_base)?
    } else {
        decimal_time(s["start_time"].as_str().unwrap_or("0"))?
    };
    let duration = s["duration_ts"]
        .as_i64()
        .filter(|n| *n > 0)
        .and_then(|n| crate::pts_to_time(n, time_base).ok())
        .or_else(|| s["duration"].as_str().and_then(|v| decimal_time(v).ok()))
        .or_else(|| {
            json["format"]["duration"]
                .as_str()
                .and_then(|v| decimal_time(v).ok())
        })
        .ok_or("Duração finita não encontrada")?;
    let width = if audio_only { 2 } else { integer("width")? };
    let height = if audio_only { 2 } else { integer("height")? };
    let sar = s["sample_aspect_ratio"]
        .as_str()
        .and_then(|v| v.split_once(':'))
        .and_then(|(n, d)| Some((n.parse::<f64>().ok()?, d.parse::<f64>().ok()?)))
        .filter(|(n, d)| *n > 0. && *d > 0.)
        .map_or(1., |(n, d)| n / d);
    let rotation = s["side_data_list"]
        .as_array()
        .and_then(|v| v.iter().find_map(|x| x["rotation"].as_i64()))
        .unwrap_or(0);
    let (dw, dh) = if rotation.rem_euclid(180) == 90 {
        (f64::from(height), f64::from(width) * sar)
    } else {
        (f64::from(width) * sar, f64::from(height))
    };
    let scale = (1920. / dw).min(1080. / dh).min(1.);
    let asset = Asset {
        audio_only,
        id,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        path,
        width,
        height,
        preview_width: (dw * scale).round().clamp(1., 1920.) as u32,
        preview_height: (dh * scale).round().clamp(1., 1080.) as u32,
        frame_rate,
        duration,
        stream_index: integer("index")?,
        time_base,
        start_time,
        codec: s["codec_name"].as_str().unwrap_or("unknown").into(),
    };
    asset.validate()?;
    Ok(asset)
}
/// FFmpeg seeks use microseconds. Round down, then decode forward using exact integer PTS.
pub fn seek_decimal(t: Time) -> String {
    let us = (i128::from(t.numerator()) * 1_000_000).div_euclid(i128::from(t.denominator()));
    let magnitude = us.unsigned_abs();
    format!(
        "{}{}.{:06}",
        if us < 0 { "-" } else { "" },
        magnitude / 1_000_000,
        magnitude % 1_000_000
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probe_maps_nonzero_pts_and_ntsc_exactly() {
        let a=probe_json(br#"{"streams":[{"index":2,"width":1920,"height":1080,"time_base":"1/90000","start_pts":90000,"duration_ts":900900,"avg_frame_rate":"30000/1001","codec_name":"h264"}]}"#,PathBuf::from("test.mp4"),AssetId(8)).unwrap();
        assert_eq!(
            a.source_time(93003).unwrap(),
            Time::new(1001, 30000).unwrap()
        );
        assert_eq!(a.duration, Time::new(1001, 100).unwrap());
        assert_eq!((a.preview_width, a.preview_height), (1920, 1080));
    }
    #[test]
    fn malformed_probe_and_exact_decimal() {
        assert!(probe_json(br#"{"streams":[]}"#, PathBuf::new(), AssetId(1)).is_err());
        assert_eq!(
            decimal_time("1.001").unwrap(),
            Time::new(1001, 1000).unwrap()
        );
        assert!(decimal_time("NaN").is_err());
        assert_eq!(seek_decimal(Time::new(1001, 30000).unwrap()), "0.033366");
        assert_eq!(seek_decimal(Time::new(-1, 2).unwrap()), "-0.500000");
        assert_eq!(seek_decimal(Time::new(-1, 30000).unwrap()), "-0.000034");
    }
}
