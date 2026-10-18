#![forbid(unsafe_code)]
mod proxy;
pub use proxy::*;
mod cache;
mod decode;
mod probe;
mod worker;
pub use cache::*;
pub use decode::*;
use meraki_core::{AssetId, FrameRate, Time};
pub use probe::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
pub use worker::*;

pub type MediaResult<T> = Result<T, String>;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    #[serde(default)]
    pub audio_only: bool,
    pub id: AssetId,
    pub path: PathBuf,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub preview_width: u32,
    pub preview_height: u32,
    pub frame_rate: FrameRate,
    pub duration: Time,
    pub stream_index: u32,
    pub time_base: Time,
    /// Absolute stream timestamp corresponding to normalized source time zero.
    pub start_time: Time,
    pub codec: String,
}
impl Asset {
    pub fn source_time(&self, pts: i64) -> MediaResult<Time> {
        pts_to_time(pts, self.time_base)?
            .checked_sub(self.start_time)
            .map_err(|e| e.to_string())
    }
    pub fn validate(&self) -> MediaResult<()> {
        if self.width == 0
            || self.height == 0
            || self.width > 32768
            || self.height > 32768
            || self.preview_width == 0
            || self.preview_height == 0
            || self.preview_width > 1920
            || self.preview_height > 1080
            || self.duration <= Time::ZERO
            || self.time_base <= Time::ZERO
            || self.path.as_os_str().is_empty()
        {
            return Err("Metadados de vídeo inválidos.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetCatalog(pub BTreeMap<AssetId, Asset>);
impl AssetCatalog {
    pub fn insert(&mut self, asset: Asset) -> MediaResult<()> {
        asset.validate()?;
        if self.0.contains_key(&asset.id) {
            return Err("ID de mídia duplicado.".into());
        }
        self.0.insert(asset.id, asset);
        Ok(())
    }
    pub fn next_id(&self) -> MediaResult<AssetId> {
        self.0
            .keys()
            .next_back()
            .map_or(Some(1), |id| id.0.checked_add(1))
            .map(AssetId)
            .ok_or("Limite de IDs.".into())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8,
    Nv12,
}
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub asset: AssetId,
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pts: Time,
    pub duration: Time,
    pub pixels: Arc<Vec<u8>>,
}
impl VideoFrame {
    pub fn covers(&self, at: Time) -> bool {
        self.pts <= at
            && self
                .pts
                .checked_add(self.duration)
                .is_ok_and(|end| at < end)
    }
    pub fn validate(&self) -> MediaResult<()> {
        let min_stride = match self.format {
            PixelFormat::Rgba8 => self.width.checked_mul(4),
            PixelFormat::Nv12 => Some(self.width),
        }
        .ok_or("Stride inválido")?;
        let rows = match self.format {
            PixelFormat::Rgba8 => self.height as usize,
            PixelFormat::Nv12 => self.height as usize + self.height.div_ceil(2) as usize,
        };
        if self.width == 0
            || self.height == 0
            || self.stride < min_stride
            || self.duration <= Time::ZERO
            || (self.format == PixelFormat::Nv12
                && (!self.width.is_multiple_of(2) || !self.height.is_multiple_of(2)))
            || (self.stride as usize).checked_mul(rows) != Some(self.pixels.len())
        {
            return Err("Layout de frame inválido.".into());
        }
        Ok(())
    }
}
pub fn pts_to_time(pts: i64, time_base: Time) -> MediaResult<Time> {
    Time::new(pts, 1)
        .and_then(|t| t.scale(time_base))
        .map_err(|e| e.to_string())
}
