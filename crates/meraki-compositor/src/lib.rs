#![forbid(unsafe_code)]
mod gpu;
mod title;
mod worker;
pub use gpu::Gpu;
use meraki_core::Time;
use meraki_graph::{compile, Operation};
use meraki_media::VideoFrame;
use meraki_timeline::{Clip, Timeline, TrackKind, Transform};
use rayon::prelude::*;
use std::sync::{Arc, OnceLock};
pub use title::title_frame;
pub use worker::*;

pub type Pixel = [f32; 4];
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Blend {
    Over,
    Begin,
    End,
}
pub fn over(top: Pixel, bottom: Pixel) -> Pixel {
    std::array::from_fn(|i| top[i] + bottom[i] * (1. - top[3]))
}
/// Resolve centered adjacent dissolves without changing editorial clip ranges.
pub fn active_layers(t: &Timeline, at: Time) -> Vec<(&Clip, Time)> {
    let solo = t
        .tracks
        .iter()
        .any(|t| t.enabled && t.kind == TrackKind::Video && t.solo);
    let mut layers = Vec::new();
    for track in t
        .tracks
        .iter()
        .filter(|t| t.enabled && t.kind == TrackKind::Video && (!solo || t.solo))
    {
        let mut clips: Vec<_> = track.clips.iter().filter(|c| c.enabled).collect();
        clips.sort_by_key(|c| c.timeline_start);
        for (i, c) in clips.iter().enumerate() {
            let half = c
                .motion
                .dissolve_in
                .scale(Time::new(1, 2).unwrap())
                .unwrap_or(Time::ZERO);
            let tail = clips
                .get(i + 1)
                .filter(|n| c.end().ok() == Some(n.timeline_start))
                .map_or(Time::ZERO, |n| {
                    n.motion
                        .dissolve_in
                        .scale(Time::new(1, 2).unwrap())
                        .unwrap_or(Time::ZERO)
                });
            let start = c
                .timeline_start
                .checked_sub(half)
                .unwrap_or(c.timeline_start);
            let end = c
                .end()
                .and_then(|e| e.checked_add(tail))
                .unwrap_or(c.timeline_start);
            if at >= start && at < end {
                if let Ok(source) = at
                    .checked_sub(c.timeline_start)
                    .and_then(|v| v.scale(c.speed.ratio()))
                    .and_then(|v| c.source_in.checked_add(v))
                {
                    layers.push((*c, source));
                }
            }
        }
    }
    layers
}
/// Evaluate exactly once per frame; both render backends receive the same frozen values.
pub fn compose_at(
    timeline: &Timeline,
    frames: &[(&Clip, Arc<VideoFrame>)],
    size: [u32; 2],
    at: Time,
) -> Result<Vec<u8>, String> {
    let mut clips: Vec<_> = frames
        .iter()
        .map(|(c, _)| {
            let mut c = (*c).clone();
            c.transform = c.transform_at(at);
            c
        })
        .collect();
    let mut blends = vec![Blend::Over; clips.len()];
    for i in 1..clips.len() {
        let incoming = &frames[i].0;
        let outgoing = &frames[i - 1].0;
        if incoming.motion.dissolve_in <= Time::ZERO
            || outgoing.end().ok() != Some(incoming.timeline_start)
            || !timeline.tracks.iter().any(|t| {
                t.clips.iter().any(|c| c.id == incoming.id)
                    && t.clips.iter().any(|c| c.id == outgoing.id)
            })
        {
            continue;
        }
        let half = incoming
            .motion
            .dissolve_in
            .scale(Time::new(1, 2).unwrap())
            .map_err(|e| e.to_string())?;
        let elapsed = at
            .checked_sub(
                incoming
                    .timeline_start
                    .checked_sub(half)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        let weight = (meraki_timeline::seconds(elapsed)
            / meraki_timeline::seconds(incoming.motion.dissolve_in))
        .clamp(0., 1.) as f32;
        clips[i - 1].transform.opacity *= 1. - weight;
        clips[i].transform.opacity *= weight;
        blends[i - 1] = Blend::Begin;
        blends[i] = Blend::End;
    }
    let resolved: Vec<_> = clips
        .iter()
        .zip(frames)
        .map(|(c, (_, f))| (c, f.clone()))
        .collect();
    compose_plan(timeline, &resolved, size, &blends)
}
thread_local! { static BACKEND:std::cell::RefCell<Option<Result<Gpu,String>>>=const {std::cell::RefCell::new(None)}; }
/// Release native GPU objects before OS thread-local destructors run (Vulkan driver TLS).
pub struct CompositorSession;
impl Default for CompositorSession {
    fn default() -> Self {
        Self
    }
}
impl Drop for CompositorSession {
    fn drop(&mut self) {
        let _ = BACKEND.try_with(|b| {
            b.borrow_mut().take();
        });
    }
}
pub fn backend_name() -> String {
    if std::env::var("MERAKI_COMPOSITOR").as_deref() == Ok("cpu") {
        return "CPU/Rayon (forçado)".into();
    }
    BACKEND.with(|b| {
        let mut b = b.borrow_mut();
        if b.is_none() {
            *b = Some(Gpu::new());
        }
        match b.as_ref().unwrap() {
            Ok(g) => g.name.clone(),
            Err(e) => format!("CPU/Rayon: {e}"),
        }
    })
}
pub fn compose(
    timeline: &Timeline,
    frames: &[(&Clip, Arc<VideoFrame>)],
    size: [u32; 2],
) -> Result<Vec<u8>, String> {
    compose_plan(timeline, frames, size, &vec![Blend::Over; frames.len()])
}
fn compose_plan(
    timeline: &Timeline,
    frames: &[(&Clip, Arc<VideoFrame>)],
    size: [u32; 2],
    blends: &[Blend],
) -> Result<Vec<u8>, String> {
    if size.contains(&0) || size.iter().any(|d| *d > 4096) || frames.len() > 8 {
        return Err("Invalid composition dimensions/layers".into());
    }
    for (c, f) in frames {
        c.transform.validate().map_err(|e| e.to_string())?;
        f.validate()?;
        if f.format != meraki_media::PixelFormat::Rgba8 {
            return Err("RGBA required".into());
        }
    }
    if std::env::var("MERAKI_COMPOSITOR").as_deref() != Ok("cpu") {
        let result = BACKEND.with(|b| {
            let mut b = b.borrow_mut();
            if b.is_none() {
                *b = Some(Gpu::new());
            }
            if let Some(Ok(g)) = b.as_mut() {
                match g.compose_plan(timeline, frames, size, blends) {
                    Ok(p) => return Some(p),
                    Err(e) => {
                        eprintln!("Meraki fallback: {e}");
                        *b = Some(Err(e));
                    }
                }
            }
            None
        });
        if let Some(p) = result {
            return Ok(p);
        }
    }
    compose_cpu_plan(timeline, frames, size, blends)
}
fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .thread_name(|i| format!("meraki-pixels-{i}"))
            .build()
            .expect("pixel worker pool")
    })
}
fn color(p: &mut Pixel, brightness: f32, contrast: f32, saturation: f32) {
    if p[3] <= 0. {
        *p = [0.; 4];
        return;
    }
    let rgb = [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
    let luma = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    for i in 0..3 {
        p[i] = (((luma + (rgb[i] - luma) * saturation) - 0.5) * contrast + 0.5 + brightness)
            .clamp(0., 1.)
            * p[3];
    }
}
// Sliding window O(width*height), independent of radius. Clamped edge extension.
#[cfg(test)]
fn horizontal(input: &[Pixel], width: usize, radius: usize) -> Vec<Pixel> {
    let mut out = vec![[0.; 4]; input.len()];
    out.par_chunks_mut(width)
        .zip(input.par_chunks(width))
        .for_each(|(dst, src)| {
            let mut sum = [0.; 4];
            for d in -(radius as isize)..=radius as isize {
                let p = src[d.clamp(0, width as isize - 1) as usize];
                for c in 0..4 {
                    sum[c] += p[c];
                }
            }
            let count = (radius * 2 + 1) as f32;
            for (x, p) in dst.iter_mut().enumerate() {
                *p = sum.map(|v| v / count);
                let left = src[x.saturating_sub(radius)];
                let right = src[(x + radius + 1).min(width - 1)];
                for c in 0..4 {
                    sum[c] += right[c] - left[c];
                }
            }
        });
    out
}
fn transpose(input: &[Pixel], w: usize, h: usize) -> Vec<Pixel> {
    let mut out = vec![[0.; 4]; input.len()];
    out.par_chunks_mut(h).enumerate().for_each(|(x, row)| {
        for (y, p) in row.iter_mut().enumerate() {
            *p = input[y * w + x];
        }
    });
    out
}
#[cfg(test)]
fn blur(input: Vec<Pixel>, w: usize, h: usize, radius: usize) -> Vec<Pixel> {
    if radius == 0 {
        return input;
    }
    let a = horizontal(&input, w, radius);
    let t = transpose(&a, w, h);
    let b = horizontal(&t, h, radius);
    transpose(&b, h, w)
}
fn sample(src: &[Pixel], w: usize, h: usize, x: f32, y: f32) -> Pixel {
    if x < -1. || y < -1. || x >= w as f32 || y >= h as f32 {
        return [0.; 4];
    }
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let mut result = [0.; 4];
    for (dx, wx) in [(0, 1. - fx), (1, fx)] {
        for (dy, wy) in [(0, 1. - fy), (1, fy)] {
            let xx = ix + dx;
            let yy = iy + dy;
            if xx >= 0 && yy >= 0 && xx < w as i32 && yy < h as i32 {
                let p = src[yy as usize * w + xx as usize];
                for c in 0..4 {
                    result[c] += p[c] * wx * wy;
                }
            }
        }
    }
    result
}
fn place(out: &mut [Pixel], size: [u32; 2], src: &[Pixel], source: [u32; 2], transform: Transform) {
    place_mode(out, size, src, source, transform, false);
}
fn place_mode(
    out: &mut [Pixel],
    size: [u32; 2],
    src: &[Pixel],
    source: [u32; 2],
    transform: Transform,
    additive: bool,
) {
    let [a, b, c, d, tx, ty] = transform.affine(source, size);
    let det = a * d - b * c;
    let w = source[0] as usize;
    let h = source[1] as usize;
    out.par_chunks_mut(size[0] as usize)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let dx = x as f32 + 0.5 - tx;
                let dy = y as f32 + 0.5 - ty;
                let sx = (d * dx - c * dy) / det - 0.5;
                let sy = (-b * dx + a * dy) / det - 0.5;
                let top = sample(src, w, h, sx, sy).map(|v| v * transform.opacity);
                *p = if additive {
                    std::array::from_fn(|i| top[i] + p[i])
                } else {
                    over(top, *p)
                };
            }
        });
}
/// Shared SDR RGBA8 pipeline for preview and offline. Input is straight alpha;
/// internal buffers are premultiplied. Canvas is 1920x1080; export aspect changes letterbox it.
pub fn compose_cpu(
    timeline: &Timeline,
    frames: &[(&Clip, Arc<VideoFrame>)],
    size: [u32; 2],
) -> Result<Vec<u8>, String> {
    compose_cpu_plan(timeline, frames, size, &vec![Blend::Over; frames.len()])
}
fn compose_cpu_plan(
    timeline: &Timeline,
    frames: &[(&Clip, Arc<VideoFrame>)],
    size: [u32; 2],
    blends: &[Blend],
) -> Result<Vec<u8>, String> {
    if size.contains(&0) || size.iter().any(|d| *d > 4096) || frames.len() > 8 {
        return Err("Limite de composição: até 4096 px por eixo e 8 camadas".into());
    }
    pool().install(|| {
        let mut output = vec![[0., 0., 0., 1.]; size[0] as usize * size[1] as usize];
        let mut pair = Vec::new();
        for (i, (clip, frame)) in frames.iter().enumerate() {
            frame.validate()?;
            if frame.format != meraki_media::PixelFormat::Rgba8 {
                return Err("Compositor requer RGBA8".into());
            }
            clip.transform.validate().map_err(|e| e.to_string())?;
            let w = frame.width as usize;
            let h = frame.height as usize;
            let mut pixels = vec![[0.; 4]; w * h];
            pixels.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                for (x, p) in row.iter_mut().enumerate() {
                    let offset = y * frame.stride as usize + x * 4;
                    let rgba = &frame.pixels[offset..offset + 4];
                    let alpha = rgba[3] as f32 / 255.;
                    *p = [
                        rgba[0] as f32 / 255. * alpha,
                        rgba[1] as f32 / 255. * alpha,
                        rgba[2] as f32 / 255. * alpha,
                        alpha,
                    ];
                }
            });
            if let Some(id) = clip.effects {
                let graph = timeline.graphs.get(&id).ok_or("Grafo ausente")?;
                for op in compile(graph).map_err(|e| e.to_string())? {
                    match op {
                        Operation::Color {
                            brightness,
                            contrast,
                            saturation,
                        } => {
                            if brightness != 0. || contrast != 1. || saturation != 1. {
                                pixels
                                    .par_iter_mut()
                                    .for_each(|p| color(p, brightness, contrast, saturation));
                            }
                        }
                        Operation::Blur { radius } => {
                            let source_scale = (w as f32 / 1920.).max(h as f32 / 1080.);
                            pixels =
                                gaussian(pixels, w, h, (radius * source_scale).round() as usize);
                        }
                    }
                }
            }
            if blends[i] == Blend::Over {
                place(
                    &mut output,
                    size,
                    &pixels,
                    [frame.width, frame.height],
                    clip.transform,
                );
            } else {
                if blends[i] == Blend::Begin {
                    pair = vec![[0.; 4]; output.len()];
                }
                place_mode(
                    &mut pair,
                    size,
                    &pixels,
                    [frame.width, frame.height],
                    clip.transform,
                    true,
                );
                if blends[i] == Blend::End {
                    output
                        .par_iter_mut()
                        .zip(pair.par_iter())
                        .for_each(|(dst, src)| *dst = over(*src, *dst));
                }
            }
        }
        let mut bytes = vec![0u8; output.len() * 4];
        bytes
            .par_chunks_mut(4)
            .zip(output.par_iter())
            .for_each(|(b, p)| {
                for c in 0..4 {
                    b[c] = (p[c].clamp(0., 1.) * 255.).round() as u8;
                }
            });
        Ok(bytes)
    })
}

fn gaussian(input: Vec<Pixel>, w: usize, h: usize, radius: usize) -> Vec<Pixel> {
    if radius == 0 {
        return input;
    }
    let sigma = (radius as f32 / 3.).max(0.5);
    let mut weights: Vec<f32> = (0..=radius)
        .map(|i| (-(i as f32).powi(2) / (2. * sigma * sigma)).exp())
        .collect();
    let total = weights[0] + 2. * weights.iter().skip(1).sum::<f32>();
    for w in &mut weights {
        *w /= total;
    }
    fn pass(src: &[Pixel], w: usize, weights: &[f32]) -> Vec<Pixel> {
        let mut out = vec![[0.; 4]; src.len()];
        out.par_chunks_mut(w)
            .zip(src.par_chunks(w))
            .for_each(|(dst, row)| {
                for (x, p) in dst.iter_mut().enumerate() {
                    *p = row[x].map(|v| v * weights[0]);
                    for (d, weight) in weights.iter().enumerate().skip(1) {
                        for (c, v) in p.iter_mut().enumerate() {
                            *v +=
                                (row[x.saturating_sub(d)][c] + row[(x + d).min(w - 1)][c]) * weight;
                        }
                    }
                }
            });
        out
    }
    transpose(
        &pass(&transpose(&pass(&input, w, &weights), w, h), h, &weights),
        h,
        w,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn premultiplied_over() {
        assert_eq!(
            over([0.5, 0., 0., 0.5], [0., 0., 1., 1.]),
            [0.5, 0., 0.5, 1.]
        );
        assert_eq!(over([0.; 4], [0.2, 0.3, 0.4, 1.]), [0.2, 0.3, 0.4, 1.]);
    }
    #[test]
    fn pip_geometry_opacity_and_transparent_border() {
        let mut out = vec![[0., 0., 1., 1.]; 160 * 90];
        let t = Transform {
            rotation: 0.,
            scale: 0.5,
            x: 480.,
            y: 270.,
            opacity: 0.5,
        };
        place(
            &mut out,
            [160, 90],
            &vec![[1., 0., 0., 1.]; 160 * 90],
            [160, 90],
            t,
        );
        assert_eq!(out[20 * 160 + 20], [0., 0., 1., 1.]);
        assert_eq!(out[60 * 160 + 120], [0.5, 0., 0.5, 1.]);
        assert_eq!(sample(&[[0., 0., 0., 0.]], 1, 1, -2., 0.), [0.; 4]);
        // An RGBA texel with half alpha at half opacity covers one quarter.
        let mut pixel = [[0., 0., 1., 1.]];
        place(
            &mut pixel,
            [1, 1],
            &[[0.5, 0., 0., 0.5]],
            [1, 1],
            Transform {
                rotation: 0.,
                opacity: 0.5,
                ..Default::default()
            },
        );
        assert_eq!(pixel[0], [0.25, 0., 0.75, 1.]);
    }
    #[test]
    fn separable_blur_spreads_an_impulse_without_darkening_alpha() {
        let mut input = vec![[0., 0., 0., 1.]; 81];
        input[40] = [1.; 4];
        let blurred = blur(input, 9, 9, 1);
        assert!((blurred[40][0] - 1. / 9.).abs() < 1e-6);
        assert!((blurred[30][0] - 1. / 9.).abs() < 1e-6);
        assert_eq!(blurred[0], [0., 0., 0., 1.]);
        assert!((blurred.iter().map(|p| p[0]).sum::<f32>() - 1.).abs() < 1e-6);
    }
    #[test]
    fn affine_center_scale_translation() {
        let t = Transform {
            rotation: 0.,
            scale: 0.5,
            x: 480.,
            y: 270.,
            opacity: 1.,
        };
        assert_eq!(
            t.affine([1920, 1080], [1920, 1080]),
            [0.5, 0., 0., 0.5, 960., 540.]
        );
        assert_eq!(
            t.affine([1920, 1080], [960, 540]),
            [0.25, 0., 0., 0.25, 480., 270.]
        );
        let mut out = vec![[0., 0., 1., 1.]; 16];
        place(
            &mut out,
            [4, 4],
            &[[1., 0., 0., 1.]; 4],
            [2, 2],
            Transform {
                rotation: 0.,
                opacity: 0.5,
                ..Default::default()
            },
        );
        assert_eq!(out[5], [0.5, 0., 0.5, 1.]);
    }
    #[test]
    fn blur_conserves_constant_and_color_respects_alpha() {
        let p = [0.25, 0.5, 0., 0.5];
        let b = blur(vec![p; 64], 8, 8, 4);
        assert!(b.iter().all(|v| *v == p));
        let mut transparent = [0.; 4];
        color(&mut transparent, 1., 2., 2.);
        assert_eq!(transparent, [0.; 4]);
        let mut red = [0.5, 0., 0., 0.5];
        color(&mut red, 0., 1., 0.);
        assert!((red[0] - red[1]).abs() < 1e-6);
    }
}
