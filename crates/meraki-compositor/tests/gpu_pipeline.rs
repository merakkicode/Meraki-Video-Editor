use meraki_compositor::*;
use meraki_core::*;
use meraki_graph::EffectSettings;
use meraki_media::*;
use meraki_timeline::*;
use std::{collections::BTreeMap, sync::Arc, time::Instant};
fn t(n: i64) -> Time {
    Time::new(n, 1).unwrap()
}
fn clip(id: u128, start: i64) -> Clip {
    Clip {
        motion: Default::default(),
        transform: Default::default(),
        id: ClipId(id),
        name: "Meraki".into(),
        source: MediaSource {
            asset: AssetId(id),
            stream_index: 0,
            kind: TrackKind::Video,
            available_start: t(0),
            available_duration: t(20),
        },
        timeline_start: t(start),
        duration: t(3),
        source_in: t(2),
        speed: PlaybackRate::new(1, 1).unwrap(),
        enabled: true,
        link_group: None,
        effects: None,
    }
}
fn timeline(clips: Vec<Clip>) -> Timeline {
    Timeline {
        markers: Vec::new(),
        mixer: Default::default(),
        id: TimelineId(1),
        name: "Meraki".into(),
        frame_rate: FrameRate::new(60, 1).unwrap(),
        audio_sample_rate: 48000,
        tracks: vec![Track {
            id: TrackId(1),
            name: "V1".into(),
            kind: TrackKind::Video,
            locked: false,
            enabled: true,
            solo: false,
            clips,
        }],
        graphs: BTreeMap::new(),
    }
}
fn frame(color: [u8; 4], w: u32, h: u32) -> Arc<VideoFrame> {
    Arc::new(VideoFrame {
        asset: AssetId(1),
        format: PixelFormat::Rgba8,
        width: w,
        height: h,
        stride: w * 4,
        pts: t(0),
        duration: t(20),
        pixels: Arc::new(color.repeat((w * h) as usize)),
    })
}
#[test]
fn animated_split_preserves_every_sample_and_volume() {
    let mut c = clip(1, 0);
    for p in [Property::Scale, Property::Volume] {
        c.motion.keys.insert(
            p,
            vec![
                Keyframe {
                    at: t(0),
                    value: 1.,
                    interpolation: Interpolation::Smooth,
                },
                Keyframe {
                    at: t(3),
                    value: 1.5,
                    interpolation: Interpolation::Linear,
                },
            ],
        );
    }
    let original = c.clone();
    let mut tl = timeline(vec![c]);
    tl.split_clip(ClipId(1), t(1), ClipId(2)).unwrap();
    tl.validate().unwrap();
    for i in 60..180 {
        let at = Time::new(i, 60).unwrap();
        let right = &tl.tracks[0].clips[1];
        assert_eq!(right.transform_at(at), original.transform_at(at));
        assert_eq!(right.volume_at(at), original.volume_at(at));
    }
}
#[test]
fn centered_dissolve_requires_handles_and_maps_both_sources() {
    let _session = CompositorSession;
    let a = clip(1, 0);
    let mut b = clip(2, 3);
    b.motion.dissolve_in = t(1);
    let mut tl = timeline(vec![a, b]);
    tl.validate().unwrap();
    let layers = active_layers(&tl, t(3));
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].1, t(5));
    assert_eq!(layers[1].1, t(2));
    let mixed = compose_at(
        &tl,
        &[
            (layers[0].0, frame([255, 0, 0, 255], 16, 9)),
            (layers[1].0, frame([0, 0, 255, 255], 16, 9)),
        ],
        [16, 9],
        t(3),
    )
    .unwrap();
    assert!((mixed[4 * 70] as i32 - 128).abs() <= 1);
    assert!((mixed[4 * 70 + 2] as i32 - 128).abs() <= 1);
    // Dissolve is a premultiplied lerp of the pair, not incoming over an unchanged outgoing clip.
    let transparent = compose_at(
        &tl,
        &[
            (layers[0].0, frame([255, 0, 0, 128], 16, 9)),
            (layers[1].0, frame([0, 0, 255, 128], 16, 9)),
        ],
        [16, 9],
        t(3),
    )
    .unwrap();
    assert!((transparent[4 * 70] as i32 - 64).abs() <= 1);
    assert!((transparent[4 * 70 + 2] as i32 - 64).abs() <= 1);
    tl.tracks[0].clips[1].source_in = t(0);
    assert!(tl.validate().is_err());
}
#[test]
fn fade_and_rotation_boundaries() {
    let mut c = clip(1, 0);
    c.motion.fade_in = t(1);
    c.motion.fade_out = t(1);
    assert_eq!(c.transform_at(t(0)).opacity, 0.);
    assert_eq!(c.transform_at(t(1)).opacity, 1.);
    assert_eq!(c.transform_at(t(3)).opacity, 0.);
    c.transform.rotation = 90.;
    let [a, b, c, d, _, _] = c.transform.affine([1920, 1080], [1920, 1080]);
    assert!(a.abs() < 1e-6 && d.abs() < 1e-6);
    assert_eq!(b, 1.);
    assert_eq!(c, -1.);
}
#[test]
#[ignore = "requires native DX12/Vulkan GPU; measures full upload/effects/readback at 1080p"]
fn native_gpu_parity_and_1080p_budget() {
    let mut gpu = Gpu::new().expect("native GPU required; fallback is not accepted by this test");
    println!("Adapter: {}", gpu.name);
    let mut c = clip(1, 0);
    c.effects = Some(GraphId(1));
    c.transform.rotation = 8.;
    c.transform.scale = 0.7;
    c.transform.opacity = 0.7;
    let mut tl = timeline(vec![c]);
    tl.graphs.insert(
        GraphId(1),
        EffectSettings {
            brightness: 0.1,
            contrast: 1.2,
            saturation: 0.4,
            radius: 12.,
            ..Default::default()
        }
        .graph(GraphId(1))
        .unwrap(),
    );
    let mut pixels = vec![0u8; 320 * 180 * 4];
    for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        p.copy_from_slice(&[
            (i % 256) as u8,
            ((i / 320) % 256) as u8,
            200,
            if i % 7 == 0 { 100 } else { 255 },
        ]);
    }
    let f = Arc::new(VideoFrame {
        pixels: Arc::new(pixels),
        ..(*frame([0; 4], 320, 180)).clone()
    });
    let frames = [(&tl.tracks[0].clips[0], f)];
    let cpu = compose_cpu(&tl, &frames, [320, 180]).unwrap();
    let native = gpu.compose(&tl, &frames, [320, 180]).unwrap();
    let mae = cpu
        .iter()
        .zip(&native)
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / cpu.len() as f64;
    println!("CPU/GPU MAE: {mae:.4}/255");
    assert!(mae < 1.5);
    let frames = [(
        &tl.tracks[0].clips[0],
        frame([128, 70, 210, 255], 1920, 1080),
    )];
    for _ in 0..10 {
        gpu.compose(&tl, &frames, [1920, 1080]).unwrap();
    }
    let mut ms = Vec::new();
    for _ in 0..120 {
        let start = Instant::now();
        gpu.compose(&tl, &frames, [1920, 1080]).unwrap();
        ms.push(start.elapsed().as_secs_f64() * 1000.);
    }
    ms.sort_by(f64::total_cmp);
    println!(
        "1080p full GPU pipeline: mean={:.2} ms p95={:.2} ms; 60fps budget met={}",
        ms.iter().sum::<f64>() / ms.len() as f64,
        ms[114],
        ms[114] <= 16.667
    );
}

#[test]
#[ignore = "requires FFmpeg and native GPU; generates a real 1080p60 fixture and measures decode + composition"]
fn real_1080p60_preview_and_latest_seek() {
    use std::{collections::BTreeSet, path::PathBuf, thread, time::Duration};
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/m6-1080p60.mp4");
    if !file.exists() {
        let status = command(&Tools::discover().ffmpeg)
            .args([
                "-v",
                "error",
                "-n",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1920x1080:rate=60",
                "-t",
                "4",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "25",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&file)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let asset = probe(&Tools::discover(), &file, AssetId(1)).unwrap();
    assert_eq!(asset.frame_rate, FrameRate::new(60, 1).unwrap());
    let mut c = clip(1, 0);
    c.source_in = t(0);
    c.source.available_duration = asset.duration;
    c.duration = asset.duration;
    c.effects = Some(GraphId(1));
    let mut tl = timeline(vec![c]);
    tl.graphs.insert(
        GraphId(1),
        EffectSettings {
            contrast: 1.2,
            radius: 12.,
            ..Default::default()
        }
        .graph(GraphId(1))
        .unwrap(),
    );
    tl.validate().unwrap();
    let tl = Arc::new(tl);
    let catalog = AssetCatalog(BTreeMap::from([(asset.id, asset)]));
    let worker = CompositeWorker::spawn().unwrap();
    let request = |seq, generation, index| CompositeRequest {
        generation,
        sequence: seq,
        revision: 1,
        timeline: tl.clone(),
        catalog: catalog.clone(),
        time: Time::new(index, 60).unwrap(),
    };
    worker.request(request(1, 1, 0));
    let deadline = Instant::now() + Duration::from_secs(20);
    while !worker.with_output(|o| o.presented.is_some()) {
        assert!(
            Instant::now() < deadline,
            "warm-up: {:?}",
            worker.with_output(|o| o.error.clone())
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !worker.with_output(|o| o.backend.starts_with("CPU")),
        "native GPU required"
    );
    let mut seen = BTreeSet::new();
    let start = Instant::now();
    let mut lag = Vec::new();
    for i in 1..=180 {
        worker.request(request(i as u64 + 1, 1, i));
        let end = start + Duration::from_secs_f64(i as f64 / 60.);
        if let Some(wait) = end.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
        worker.with_output(|o| {
            assert!(o.error.is_none(), "{:?}", o.error);
            if let Some(p) = &o.presented {
                if i > 30 {
                    seen.insert(p.timeline_time);
                    lag.push(i as f64 - meraki_timeline::seconds(p.timeline_time) * 60.);
                }
            }
        });
    }
    lag.sort_by(f64::total_cmp);
    println!(
        "1080p60 decode+GPU: {}/150 distinct samples, p95 lag {:.1} frames, backend {}",
        seen.len(),
        lag[142],
        worker.with_output(|o| o.backend.clone())
    );
    for seq in 182..222 {
        worker.request(request(seq, seq, (seq % 120) as i64));
    }
    worker.request(request(222, 222, 60));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if worker.with_output(|o| {
            o.presented
                .as_ref()
                .is_some_and(|p| p.generation == 222 && p.timeline_time == t(1))
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    worker.shutdown();
    assert!(seen.len() >= 140, "1080p60 cadence below acceptance budget");
}
