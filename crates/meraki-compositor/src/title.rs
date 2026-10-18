use fontdue::{
    layout::{CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign},
    Font, FontSettings,
};
use meraki_core::{AssetId, Time};
use meraki_media::{PixelFormat, VideoFrame};
use meraki_timeline::Title;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
};

type CachedTitle = (Title, Arc<VideoFrame>);
/// Bounded eight-title cache. Rasterization and disk reads are never on the audio callback/UI.
pub fn title_frame(title: &Title) -> Result<Arc<VideoFrame>, String> {
    static CACHE: OnceLock<Mutex<VecDeque<CachedTitle>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "Title cache poisoned")?;
    if let Some(i) = cache.iter().position(|(t, _)| t == title) {
        let entry = cache.remove(i).unwrap();
        let f = entry.1.clone();
        cache.push_back(entry);
        return Ok(f);
    }
    title.validate().map_err(|e| e.to_string())?;
    let file = match title.font.as_str() {
        "Arial" => "arial.ttf",
        "Georgia" => "georgia.ttf",
        _ => "segoeui.ttf",
    };
    let path = if cfg!(windows) {
        std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:/Windows".into())
            .join("Fonts")
            .join(file)
    } else {
        let family = if title.font == "Georgia" {
            "DejaVuSerif.ttf"
        } else {
            "DejaVuSans.ttf"
        };
        let mut roots = vec![];
        if let Some(root) = std::env::var_os("MERAKI_FONT_DIR") {
            roots.push(std::path::PathBuf::from(root));
        }
        if let Ok(exe) = std::env::current_exe() {
            for parent in exe.ancestors().skip(1) {
                roots.push(parent.join("share/fonts/truetype/dejavu"));
            }
        }
        roots.extend(
            [
                "/usr/share/fonts/truetype/dejavu",
                "/usr/share/fonts/dejavu",
                "/app/share/fonts/truetype/dejavu",
            ]
            .map(std::path::PathBuf::from),
        );
        roots
            .into_iter()
            .map(|root| root.join(family))
            .find(|path| path.is_file())
            .ok_or("Install DejaVu fonts or set MERAKI_FONT_DIR")?
    };
    let bytes = std::fs::read(path).map_err(|e| format!("Fonte {}: {e}", title.font))?;
    let font = Font::from_bytes(bytes, FontSettings::default()).map_err(str::to_owned)?;
    let frame = Arc::new(rasterize(title, &font)?);
    cache.push_back((title.clone(), frame.clone()));
    if cache.len() > 8 {
        cache.pop_front();
    }
    Ok(frame)
}
pub fn rasterize(title: &Title, font: &Font) -> Result<VideoFrame, String> {
    title.validate().map_err(|e| e.to_string())?;
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        x: 96.,
        y: 54.,
        max_width: Some(1728.),
        max_height: Some(972.),
        horizontal_align: match title.align.as_str() {
            "left" => HorizontalAlign::Left,
            "right" => HorizontalAlign::Right,
            _ => HorizontalAlign::Center,
        },
        vertical_align: VerticalAlign::Middle,
        ..Default::default()
    });
    layout.append(&[font], &TextStyle::new(&title.text, title.size, 0));
    let mut mask = vec![0u8; 1920 * 1080];
    for glyph in layout.glyphs() {
        // Layout retains control characters for line breaking. They have no ink;
        // rasterizing their missing-glyph index would paint a visible square.
        if glyph.parent.is_control() || glyph.parent.is_whitespace() {
            continue;
        }
        let (metrics, coverage) = font.rasterize_config(glyph.key);
        for y in 0..metrics.height {
            for x in 0..metrics.width {
                let xx = glyph.x.floor() as i32 + x as i32;
                let yy = glyph.y.floor() as i32 + y as i32;
                if (0..1920).contains(&xx) && (0..1080).contains(&yy) {
                    let at = yy as usize * 1920 + xx as usize;
                    mask[at] = mask[at].max(coverage[y * metrics.width + x]);
                }
            }
        }
    }
    let mut pixels = vec![0u8; 1920 * 1080 * 4];
    for y in 0..1080 {
        for x in 0..1920 {
            let a = mask[y * 1920 + x] as f32 / 255. * title.color[3] as f32 / 255.;
            let shadow = if title.shadow && x >= 4 && y >= 4 {
                mask[(y - 4) * 1920 + x - 4] as f32 / 255. * 0.65
            } else {
                0.
            };
            let alpha = a + shadow * (1. - a);
            let i = (y * 1920 + x) * 4;
            if alpha > 0. {
                for c in 0..3 {
                    pixels[i + c] = (title.color[c] as f32 * a / alpha).round() as u8;
                }
            }
            pixels[i + 3] = (alpha * 255.).round() as u8;
        }
    }
    Ok(VideoFrame {
        asset: AssetId(0),
        format: PixelFormat::Rgba8,
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        pts: Time::ZERO,
        duration: Time::new(86400, 1).unwrap(),
        pixels: Arc::new(pixels),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_has_transparent_background_and_antialiased_coverage() {
        let f = title_frame(&Title::default()).unwrap();
        assert_eq!(&f.pixels[..4], &[0, 0, 0, 0]);
        assert!(f
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[3] < 255));
        assert!(f.pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        assert!(Arc::ptr_eq(&f, &title_frame(&Title::default()).unwrap()));
    }

    #[test]
    fn line_breaks_and_spaces_never_paint_missing_glyph_boxes() {
        let title = Title {
            text: " \n\r\n\t ".into(),
            ..Title::default()
        };
        let frame = title_frame(&title).unwrap();
        assert!(frame.pixels.iter().all(|&channel| channel == 0));
    }
}
