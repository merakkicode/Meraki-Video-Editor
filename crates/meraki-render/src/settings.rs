use crate::RenderResult;
use meraki_core::FrameRate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportSettingsDto {
    pub preset: String,
    pub output: PathBuf,
    pub container: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub codec: String,
    pub rate_mode: String,
    pub crf: u32,
    pub bitrate_mbps: u32,
    pub audio_codec: String,
    pub audio_bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u32,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPresetDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub settings: ExportSettingsDto,
}
pub fn presets(rate: FrameRate) -> Vec<ExportPresetDto> {
    [
        (
            "youtube",
            "YouTube",
            "Qualidade superior · H.264 High · CRF 18",
        ),
        ("social", "TikTok / Reels", "Vertical 9:16 · 1080 × 1920"),
        ("web", "Web Universal", "H.264 · CRF 22 · MP4 faststart"),
        ("master", "Master / ProRes", "ProRes 422 HQ · PCM · MOV"),
        (
            "custom",
            "Personalizado",
            "Controle de resolução, codec e taxa",
        ),
    ]
    .into_iter()
    .map(|(id, name, description)| {
        let master = id == "master";
        ExportPresetDto {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            settings: ExportSettingsDto {
                preset: id.into(),
                output: PathBuf::new(),
                container: if master { "mov" } else { "mp4" }.into(),
                width: if id == "social" { 1080 } else { 1920 },
                height: if id == "social" { 1920 } else { 1080 },
                frame_rate: rate,
                codec: if master { "prores_ks" } else { "libx264" }.into(),
                rate_mode: if master { "quality" } else { "crf" }.into(),
                crf: if id == "web" { 22 } else { 18 },
                bitrate_mbps: 20,
                audio_codec: if master { "pcm_s24le" } else { "aac" }.into(),
                audio_bitrate_kbps: 320,
                sample_rate: 48000,
                channels: 2,
            },
        }
    })
    .collect()
}
impl ExportSettingsDto {
    pub fn validate(&self) -> RenderResult<()> {
        if self.width < 64
            || self.height < 64
            || self.width > 4096
            || self.height > 4096
            || !self.width.is_multiple_of(2)
            || !self.height.is_multiple_of(2)
        {
            return Err("Resolução deve ser par, entre 64 e 4096 pixels por eixo.".into());
        }
        let fps = f64::from(self.frame_rate.numerator()) / f64::from(self.frame_rate.denominator());
        if !(1.0..=120.0).contains(&fps) {
            return Err("FPS deve estar entre 1 e 120.".into());
        }
        if !["mp4", "mov", "mkv"].contains(&self.container.as_str())
            || !["libx264", "libx265", "prores_ks"].contains(&self.codec.as_str())
        {
            return Err("Contêiner ou codec não suportado.".into());
        }
        if self.codec == "prores_ks" && (self.container != "mov" || self.rate_mode != "quality") {
            return Err("ProRes usa MOV e modo quality (422 HQ).".into());
        }
        if self.codec != "prores_ks" && !["crf", "bitrate"].contains(&self.rate_mode.as_str()) {
            return Err("Escolha CRF ou bitrate para H.264/H.265.".into());
        }
        if self.crf > 51 || !(1..=500).contains(&self.bitrate_mbps) {
            return Err("CRF deve ser 0–51; bitrate 1–500 Mbps.".into());
        }
        if !["aac", "pcm_s24le", "none"].contains(&self.audio_codec.as_str())
            || (self.audio_codec == "pcm_s24le" && self.container == "mp4")
        {
            return Err("PCM requer MOV ou MKV. MP4 usa AAC.".into());
        }
        if self.sample_rate != 48000
            || self.channels != 2
            || !(64..=320).contains(&self.audio_bitrate_kbps)
        {
            return Err("Áudio Meraki: 48 kHz, estéreo e AAC 64–320 kbps.".into());
        }
        if !self.output.is_absolute()
            || self.output.extension().and_then(|x| x.to_str()) != Some(&self.container)
        {
            return Err("Escolha pasta absoluta e extensão igual ao contêiner.".into());
        }
        if self.preset.len() > 64 {
            return Err("Nome de preset inválido".into());
        }
        Ok(())
    }
}
/// Output-only arguments, all enum-like fields are validated before process execution.
pub fn encoding_args(s: &ExportSettingsDto) -> RenderResult<Vec<String>> {
    s.validate()?;
    let mut args = vec!["-c:v".into(), s.codec.clone()];
    if s.codec == "prores_ks" {
        args.extend(["-profile:v", "3", "-pix_fmt", "yuv422p10le"].map(String::from));
    } else {
        args.extend(["-preset", "slow", "-pix_fmt", "yuv420p"].map(String::from));
        if s.codec == "libx264" {
            args.extend(["-profile:v", "high"].map(String::from));
        }
        if s.codec == "libx265" {
            args.extend(
                ["-tag:v", "hvc1", "-x265-params", "pools=2:frame-threads=2"].map(String::from),
            );
        }
        args.extend(if s.rate_mode == "crf" {
            vec!["-crf".into(), s.crf.to_string()]
        } else {
            vec!["-b:v".into(), format!("{}M", s.bitrate_mbps)]
        });
    }
    if s.audio_codec != "none" {
        args.extend([
            "-c:a".into(),
            s.audio_codec.clone(),
            "-ar".into(),
            s.sample_rate.to_string(),
            "-ac".into(),
            s.channels.to_string(),
        ]);
        if s.audio_codec == "aac" {
            args.extend(["-b:a".into(), format!("{}k", s.audio_bitrate_kbps)]);
        }
    }
    if s.container == "mp4" || s.container == "mov" {
        args.extend(["-movflags", "+faststart"].map(String::from));
    }
    args.extend(["-threads", "2", "-f"].map(String::from));
    args.push(if s.container == "mkv" {
        "matroska".into()
    } else {
        s.container.clone()
    });
    Ok(args)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> ExportSettingsDto {
        let mut s = presets(FrameRate::new(30000, 1001).unwrap())
            .remove(0)
            .settings;
        s.output = std::env::temp_dir().join("render.mp4");
        s
    }
    #[test]
    fn youtube_and_bitrate_arguments() {
        let mut s = settings();
        let a = encoding_args(&s).unwrap();
        assert!(a.windows(2).any(|a| a == ["-crf", "18"]));
        assert!(a.windows(2).any(|a| a == ["-movflags", "+faststart"]));
        s.rate_mode = "bitrate".into();
        s.bitrate_mbps = 35;
        let a = encoding_args(&s).unwrap();
        assert!(!a.contains(&"-crf".into()));
        assert!(a.windows(2).any(|a| a == ["-b:v", "35M"]));
    }
    #[test]
    fn rejects_incompatible_and_injected_options() {
        let mut s = settings();
        s.codec = "libx264 -y".into();
        assert!(s.validate().is_err());
        s.codec = "prores_ks".into();
        assert!(s.validate().is_err());
        s.container = "mov".into();
        s.output.set_extension("mov");
        s.rate_mode = "quality".into();
        assert!(encoding_args(&s).unwrap().contains(&"yuv422p10le".into()));
        s.width = 1919;
        assert!(s.validate().is_err());
    }
}
