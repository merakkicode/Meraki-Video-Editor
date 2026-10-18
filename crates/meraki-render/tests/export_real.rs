use meraki_core::*;
use meraki_media::*;
use meraki_project::Project;
use meraki_render::*;
use meraki_timeline::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sample.mp4")
}
fn project(asset: Asset) -> Project {
    let clip = |id, start, source| Clip {
        motion: Default::default(),
        id: ClipId(id),
        name: "Trecho".into(),
        transform: Default::default(),
        source: MediaSource {
            asset: asset.id,
            stream_index: asset.stream_index,
            kind: TrackKind::Video,
            available_start: Time::ZERO,
            available_duration: asset.duration,
        },
        timeline_start: Time::new(start, 1).unwrap(),
        duration: Time::new(1, 1).unwrap(),
        source_in: Time::new(source, 1).unwrap(),
        speed: PlaybackRate::new(1, 1).unwrap(),
        enabled: true,
        link_group: None,
        effects: None,
    };
    let timeline = Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Teste offline".into(),
        frame_rate: FrameRate::new(30, 1).unwrap(),
        audio_sample_rate: 48000,
        mixer: Default::default(),
        graphs: BTreeMap::new(),
        tracks: vec![Track {
            id: TrackId(1),
            name: "V1".into(),
            kind: TrackKind::Video,
            enabled: true,
            solo: false,
            locked: false,
            clips: vec![clip(1, 0, 0), clip(2, 2, 4)],
        }],
    };
    let mut catalog = AssetCatalog::default();
    catalog.insert(asset).unwrap();
    Project::new(timeline, &catalog)
}
fn inspect(path: &Path) -> serde_json::Value {
    let out=command(&Tools::discover().ffprobe).args(["-v","error","-count_frames","-show_entries","stream=codec_name,profile,width,height,r_frame_rate,nb_read_frames,sample_rate,channels","-of","json"]).arg(path).output().unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn frame_at(asset: &Asset, target: Time) -> Arc<VideoFrame> {
    let s = DecoderSession::start(&Tools::discover(), asset, target).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(Instant::now() < deadline);
        if let Ok(Ok(f)) = s.next(Duration::from_millis(100)) {
            if f.covers(target) {
                return f;
            }
        }
    }
}
fn audio_energy(path: &Path, start: &str) -> f64 {
    let o = command(&Tools::discover().ffmpeg)
        .args(["-v", "error", "-ss", start, "-i"])
        .arg(path)
        .args(["-t", "0.1", "-vn", "-ac", "1", "-f", "f32le", "pipe:1"])
        .output()
        .unwrap();
    assert!(o.status.success());
    let values = o
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b).abs() as f64)
        .collect::<Vec<_>>();
    assert!(!values.is_empty());
    values.iter().sum::<f64>() / values.len() as f64
}
#[test]
#[ignore = "requires FFmpeg/GPU; verifies proxy preview, original-only export, cancellation and offline relink"]
fn m7_proxy_preview_original_export_and_relink() {
    let _session = meraki_compositor::CompositorSession;
    let dir = tempfile::tempdir().unwrap();
    let tools = Tools::discover();
    let original = dir.path().join("red.mp4");
    let make = |path: &Path, color: &str, size: &str| {
        assert!(command(&tools.ffmpeg)
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("color={color}:size={size}:rate=30:duration=1"),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p"
            ])
            .arg(path)
            .status()
            .unwrap()
            .success());
    };
    make(&original, "red", "1920x1080");
    let asset = probe(&tools, &original, AssetId(1)).unwrap();
    let cache = dir.path().join(".meraki/proxies");
    generate_proxy(&asset, &cache, &AtomicBool::new(false)).unwrap();
    enable_proxies(true);
    let proxy = preview_asset(&asset);
    assert_ne!(proxy.path, asset.path);
    assert_eq!((proxy.width, proxy.height), (1280, 720));
    // Deliberately different pixels prove that export cannot accidentally use the proxy.
    make(&proxy.path, "blue", "1280x720");
    let preview = frame_at(&preview_asset(&asset), Time::new(1, 2).unwrap());
    assert!(preview.pixels[2] > 240 && preview.pixels[0] < 10);
    let mut p = project(asset.clone());
    p.timeline.tracks[0].clips.truncate(1);
    p.validate().unwrap();
    let mut settings = presets(p.timeline.frame_rate).remove(0).settings;
    settings.output = dir.path().join("original-export.mp4");
    settings.audio_codec = "none".into();
    render(
        &p,
        &settings,
        &TimelineRange {
            start_frame: "0".into(),
            end_frame: "30".into(),
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_, _, _, _| {}),
    )
    .unwrap();
    let result = probe(&tools, &settings.output, AssetId(2)).unwrap();
    assert_eq!((result.width, result.height), (1920, 1080));
    let frame = frame_at(&result, Time::new(1, 2).unwrap());
    assert!(frame.pixels[0] > 240 && frame.pixels[2] < 10);
    let moved = dir.path().join("moved.mp4");
    std::fs::rename(&original, &moved).unwrap();
    assert!(!p.assets[0].path.is_file());
    let linked = p
        .relink(probe(&tools, &moved, AssetId(1)).unwrap())
        .unwrap();
    assert_eq!(linked.timeline, p.timeline);
    enable_proxies(false);
    assert_eq!(preview_asset(&linked.assets[0]), linked.assets[0]);
    let cancelled = dir.path().join("cancelled");
    assert!(generate_proxy(&linked.assets[0], &cancelled, &AtomicBool::new(true)).is_err());
    assert!(std::fs::read_dir(cancelled).unwrap().all(|e| !e
        .unwrap()
        .path()
        .extension()
        .is_some_and(|e| e == "mp4")));
    println!(
        "Meraki: 720p blue proxy in preview; original red 1080p export; relink and cancellation passed"
    );
}
#[test]
#[ignore = "requires FFmpeg/GPU and audio-sync.mp4; validates Meraki animation, titles, dissolve and persistence through real MP4"]
fn m6_animated_title_transition_export() {
    let _session = meraki_compositor::CompositorSession;
    let asset = probe(
        &Tools::discover(),
        &fixture().with_file_name("audio-sync.mp4"),
        AssetId(1),
    )
    .unwrap();
    let mut p = project(asset.clone());
    p.timeline.frame_rate = FrameRate::new(60, 1).unwrap();
    let first = &mut p.timeline.tracks[0].clips[0];
    first.source_in = Time::new(2, 1).unwrap();
    first.motion.keys.insert(
        Property::Scale,
        vec![
            Keyframe {
                at: Time::ZERO,
                value: 1.,
                interpolation: Interpolation::Smooth,
            },
            Keyframe {
                at: Time::new(1, 1).unwrap(),
                value: 1.5,
                interpolation: Interpolation::Linear,
            },
        ],
    );
    first.motion.keys.insert(
        Property::Volume,
        vec![
            Keyframe {
                at: Time::ZERO,
                value: 0.2,
                interpolation: Interpolation::Smooth,
            },
            Keyframe {
                at: Time::new(1, 1).unwrap(),
                value: 0.8,
                interpolation: Interpolation::Linear,
            },
        ],
    );
    let second = &mut p.timeline.tracks[0].clips[1];
    second.timeline_start = Time::new(1, 1).unwrap();
    second.motion.dissolve_in = Time::new(1, 1).unwrap();
    let mut title = p.timeline.tracks[0].clips[0].clone();
    title.id = ClipId(99);
    title.duration = Time::new(2, 1).unwrap();
    title.motion = Motion {
        title: Some(Title {
            text: "MERAKI Meraki".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut track = p.timeline.tracks[0].clone();
    track.id = TrackId(2);
    track.clips = vec![title];
    p.timeline.tracks.push(track);
    p.validate().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let project_file = dir.path().join("m6.meraki");
    meraki_project::save(&project_file, &p).unwrap();
    assert_eq!(meraki_project::load(&project_file).unwrap(), p);
    let mut settings = presets(p.timeline.frame_rate).remove(0).settings;
    settings.output = dir.path().join("m6.mp4");
    render(
        &p,
        &settings,
        &TimelineRange {
            start_frame: "0".into(),
            end_frame: "120".into(),
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_, _, _, _| {}),
    )
    .unwrap();
    let meta = inspect(&settings.output);
    assert_eq!(meta["streams"][0]["nb_read_frames"], "120");
    assert_eq!(meta["streams"][0]["r_frame_rate"], "60/1");
    for time in [
        Time::new(1, 2).unwrap(),
        Time::new(1, 1).unwrap(),
        Time::new(3, 2).unwrap(),
    ] {
        let frames = meraki_compositor::active_layers(&p.timeline, time)
            .iter()
            .map(|(c, source)| {
                (
                    *c,
                    if let Some(title) = &c.motion.title {
                        meraki_compositor::title_frame(title).unwrap()
                    } else {
                        frame_at(&asset, *source)
                    },
                )
            })
            .collect::<Vec<_>>();
        let expected =
            meraki_compositor::compose_at(&p.timeline, &frames, [1920, 1080], time).unwrap();
        let out = command(&Tools::discover().ffmpeg)
            .args(["-v", "error", "-ss", &seek_decimal(time), "-i"])
            .arg(&settings.output)
            .args([
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), expected.len());
        let mae = out
            .stdout
            .iter()
            .zip(expected)
            .map(|(a, b)| (*a as f64 - b as f64).abs())
            .sum::<f64>()
            / out.stdout.len() as f64;
        println!("Meraki frame {time:?}: encoded MAE {mae:.3}/255");
        assert!(mae < 6.);
    }
    assert!(audio_energy(&settings.output, "0.5") > 0.0001);
    let gain_ratio = audio_energy(&settings.output, "0.8") / audio_energy(&settings.output, "0.1");
    println!("Meraki automated audio late/early energy: {gain_ratio:.3}");
    assert!(gain_ratio > 2., "export must apply the animated clip gain");
}
#[test]
#[ignore = "requires FFmpeg and fixtures/sample.mp4; exercises Meraki preview and real export"]
fn composition_preview_export_and_latest_revision() {
    let _session = meraki_compositor::CompositorSession;
    std::env::set_var("MERAKI_PREVIEW", "540");
    use meraki_compositor::{compose, CompositeRequest, CompositeWorker};
    use meraki_graph::EffectSettings;
    let tools = Tools::discover();
    let mut p = project(probe(&tools, &fixture(), AssetId(1)).unwrap());
    let mut top = p.timeline.tracks[0].clips[0].clone();
    top.id = ClipId(3);
    top.source_in = Time::new(4, 1).unwrap();
    top.transform = Transform {
        rotation: 0.,
        scale: 0.5,
        x: 420.,
        y: 210.,
        opacity: 0.65,
    };
    top.effects = Some(GraphId(1));
    let fx = EffectSettings {
        brightness: 0.08,
        contrast: 1.4,
        saturation: 0.2,
        radius: 15.,
        ..Default::default()
    };
    p.timeline
        .graphs
        .insert(GraphId(1), fx.graph(GraphId(1)).unwrap());
    let mut track = p.timeline.tracks[0].clone();
    track.id = TrackId(2);
    track.clips = vec![top];
    p.timeline.tracks.push(track);
    let catalog = p.validate().unwrap();
    let target = Time::new(1, 2).unwrap();
    let bottom = frame_at(&p.assets[0], target);
    let top = frame_at(&p.assets[0], Time::new(9, 2).unwrap());
    let frames = vec![
        (&p.timeline.tracks[0].clips[0], bottom),
        (&p.timeline.tracks[1].clips[0], top),
    ];
    let expected = compose(&p.timeline, &frames, [960, 540]).unwrap();
    let plain = compose(&p.timeline, &frames[..1], [960, 540]).unwrap();
    assert_ne!(expected, plain);
    let mut bypass = p.timeline.clone();
    bypass.graphs.insert(
        GraphId(1),
        EffectSettings {
            color_bypass: true,
            blur_bypass: true,
            ..fx
        }
        .graph(GraphId(1))
        .unwrap(),
    );
    let bypassed = compose(&bypass, &frames, [960, 540]).unwrap();
    assert_ne!(bypassed, expected);
    bypass.graphs.insert(
        GraphId(1),
        EffectSettings {
            color_connected: false,
            blur_connected: false,
            ..fx
        }
        .graph(GraphId(1))
        .unwrap(),
    );
    assert_eq!(compose(&bypass, &frames, [960, 540]).unwrap(), bypassed);
    let worker = CompositeWorker::spawn().unwrap();
    // Latest-only mailbox: old seeks/revisions cannot replace the final composition.
    for n in 1..=40 {
        worker.request(CompositeRequest {
            generation: n,
            sequence: n,
            revision: n,
            timeline: Arc::new(p.timeline.clone()),
            catalog: catalog.clone(),
            time: Time::new(n as i64, 30).unwrap(),
        });
    }
    worker.request(CompositeRequest {
        generation: 41,
        sequence: 41,
        revision: 41,
        timeline: Arc::new(p.timeline.clone()),
        catalog: catalog.clone(),
        time: target,
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let done = worker.with_output(|o| {
            assert!(o.error.is_none(), "{:?}", o.error);
            o.presented
                .as_ref()
                .filter(|f| f.generation == 41)
                .map(|f| {
                    assert_eq!(f.sequence, 41);
                    assert_eq!(*f.frame.as_ref().unwrap().pixels, expected);
                })
                .is_some()
        });
        if done {
            break;
        }
        assert!(Instant::now() < deadline, "preview timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    worker.shutdown();
    // Exercise sustained scheduling, not only paused seeks. Measure presentation age
    // before WebView upload; the audio device remains the production master clock.
    let mut live = p.timeline.clone();
    live.tracks[0].clips.truncate(1);
    live.tracks[0].clips[0].duration = Time::new(5, 1).unwrap();
    live.tracks[1].clips[0].duration = Time::new(5, 1).unwrap();
    let live = Arc::new(live);
    let playback = CompositeWorker::spawn().unwrap();
    let began = Instant::now();
    let mut ages = vec![];
    let mut sequences = std::collections::BTreeSet::new();
    for index in 0..90 {
        let target = Time::new(index, 30).unwrap();
        playback.request(CompositeRequest {
            generation: 1,
            sequence: index as u64 + 1,
            revision: 1,
            timeline: live.clone(),
            catalog: catalog.clone(),
            time: target,
        });
        let next = began + Duration::from_secs_f64((index + 1) as f64 / 30.);
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        playback.with_output(|o| {
            if index % 15 == 0 {
                println!(
                    "schedule {index}, presented {:?}, decoded {}, compose {:?} ms",
                    o.presented.as_ref().map(|p| p.sequence),
                    o.decoded,
                    o.presented.as_ref().map(|p| p.decode_ms)
                );
            }
            assert!(o.error.is_none(), "{:?}", o.error);
            if index >= 15 {
                if let Some(p) = &o.presented {
                    sequences.insert(p.sequence);
                    let t = p.timeline_time;
                    ages.push(
                        (index as f64 / 30. - t.numerator() as f64 / t.denominator() as f64)
                            * 1000.,
                    );
                }
            }
        });
    }
    playback.shutdown();
    ages.sort_by(f64::total_cmp);
    println!(
        "continuous samples {}, age p95 {:?}",
        sequences.len(),
        ages.get(ages.len() * 95 / 100)
    );
    assert!(
        sequences.len() >= 40,
        "too few presented frames: {}",
        sequences.len()
    );
    assert!(!ages.is_empty());
    println!(
        "Meraki sustained 30 fps requests: {} distinct samples in 2.5 s; presentation age p95 {:.1} ms",
        sequences.len(),
        ages[ages.len() * 95 / 100]
    );
    assert!(
        ages[ages.len() * 95 / 100] < 200.,
        "presentation fell behind master clock"
    );
    let mut timings = vec![];
    for _ in 0..60 {
        let now = Instant::now();
        std::hint::black_box(compose(&p.timeline, &frames, [960, 540]).unwrap());
        timings.push(now.elapsed().as_secs_f64() * 1000.);
    }
    timings.sort_by(f64::total_cmp);
    println!(
        "Shared compositor 2 x 640x360 -> 960x540, color + blur: mean {:.2} ms, p95 {:.2} ms",
        timings.iter().sum::<f64>() / 60.,
        timings[56]
    );
    let dir = tempfile::tempdir().unwrap();
    let mut settings = presets(p.timeline.frame_rate).remove(0).settings;
    settings.output = std::env::var_os("MERAKI_M5_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.path().join("m5-pip.mp4"));
    settings.audio_codec = "none".into();
    render(
        &p,
        &settings,
        &TimelineRange {
            start_frame: "0".into(),
            end_frame: "90".into(),
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_, _, _, _| {}),
    )
    .unwrap();
    let metadata = inspect(&settings.output);
    assert_eq!(metadata["streams"][0]["nb_read_frames"], "90");
    assert_eq!(metadata["streams"][0]["width"], 1920);
    assert_eq!(metadata["streams"][0]["height"], 1080);
    assert_eq!(metadata["streams"][0]["r_frame_rate"], "30/1");
    let encoded = probe(&tools, &settings.output, AssetId(2)).unwrap();
    let session = DecoderSession::start_composition(&tools, &encoded, target).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let output = loop {
        assert!(Instant::now() < deadline);
        if let Ok(Ok(f)) = session.next(Duration::from_millis(100)) {
            if f.covers(target) {
                break f;
            }
        }
    };
    let full = compose(&p.timeline, &frames, [1920, 1080]).unwrap();
    let mae = full
        .iter()
        .zip(output.pixels.iter())
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / full.len() as f64;
    assert!(mae < 6., "encoded composition error {mae}");
    println!("Meraki preview exact shared-pipeline match; latest generation 41; 1080p MP4 90/90 frames; encoded RGBA MAE {mae:.3}/255; {}",settings.output.display());
}
#[test]
#[ignore = "requires FFmpeg 9.x and scripts/create-fixtures.ps1"]
fn real_export_frames_audio_presets_and_cancellation() {
    let tools = Tools::discover();
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("with-audio.mp4");
    let o = command(&tools.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(fixture())
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-shortest",
            "-y",
        ])
        .arg(&input)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let project = project(probe(&tools, &input, AssetId(1)).unwrap());
    let range = TimelineRange {
        start_frame: "0".into(),
        end_frame: "90".into(),
    };
    let mut s = presets(project.timeline.frame_rate).remove(0).settings;
    s.output = dir.path().join("youtube.mp4");
    let progress = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observations = progress.clone();
    render(
        &project,
        &s,
        &range,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |f, t, _, _| {
            assert!(f <= t);
            observations.lock().unwrap().push(f);
        }),
    )
    .unwrap();
    let meta = inspect(&s.output);
    let v = &meta["streams"][0];
    assert_eq!(v["codec_name"], "h264");
    assert_eq!(v["profile"], "High");
    assert_eq!(v["width"], 1920);
    assert_eq!(v["height"], 1080);
    assert_eq!(v["r_frame_rate"], "30/1");
    assert_eq!(v["nb_read_frames"], "90");
    assert_eq!(meta["streams"][1]["codec_name"], "aac");
    assert_eq!(meta["streams"][1]["sample_rate"], "48000");
    assert_eq!(meta["streams"][1]["channels"], 2);
    assert!(progress.lock().unwrap().windows(2).all(|w| w[0] <= w[1]));
    let encoded = probe(&tools, &s.output, AssetId(2)).unwrap();
    let gap = frame_at(&encoded, Time::new(3, 2).unwrap());
    assert!(gap
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| p[0] < 4 && p[1] < 4 && p[2] < 4));
    assert!(audio_energy(&s.output, "0.5") > 0.01);
    assert!(audio_energy(&s.output, "1.5") < 0.001);
    assert!(audio_energy(&s.output, "2.5") > 0.01);
    let original_energy = audio_energy(&s.output, "0.5");
    for (label, gain, muted) in [("quiet", Some(-12.), false), ("muted", Some(0.), true)] {
        let mut mixed = project.clone();
        mixed.timeline.mixer.master = ChannelMix {
            gain_db: gain,
            muted,
            ..Default::default()
        };
        let mut settings = s.clone();
        settings.width = 128;
        settings.height = 72;
        settings.output = dir.path().join(format!("{label}.mp4"));
        render(
            &mixed,
            &settings,
            &range,
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _, _, _| {}),
        )
        .unwrap();
        let ratio = audio_energy(&settings.output, "0.5") / original_energy;
        if muted {
            assert!(ratio < 0.001);
        } else {
            assert!(
                (ratio - 10f64.powf(-12. / 20.)).abs() < 0.02,
                "gain ratio {ratio}"
            );
        }
    }
    let source = frame_at(&project.assets[0], Time::new(9, 2).unwrap());
    let output = frame_at(&encoded, Time::new(5, 2).unwrap());
    // Compare through a common decoder output size; frame at timeline 2.5s maps to source 4.5s.
    let mut resized = project.assets[0].clone();
    resized.preview_width = encoded.preview_width;
    resized.preview_height = encoded.preview_height;
    let source = if source.width != output.width {
        frame_at(&resized, Time::new(9, 2).unwrap())
    } else {
        source
    };
    let error = source
        .pixels
        .iter()
        .zip(output.pixels.iter())
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / output.pixels.len() as f64;
    assert!(error < 12., "source mapping error {error}");
    let bytes = std::fs::read(&s.output).unwrap();
    assert!(render(
        &project,
        &s,
        &range,
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_, _, _, _| {})
    )
    .is_err());
    assert_eq!(std::fs::read(&s.output).unwrap(), bytes);
    for (codec, container, width, height) in [
        ("libx264", "mp4", 1080, 1920),
        ("prores_ks", "mov", 128, 72),
        ("libx265", "mkv", 128, 72),
        ("libx264", "mp4", 3840, 2160),
    ] {
        let mut s = s.clone();
        s.codec = codec.into();
        s.container = container.into();
        s.width = width;
        s.height = height;
        s.output = dir.path().join(format!("{codec}-{width}.{container}"));
        s.audio_codec = if codec == "prores_ks" {
            "pcm_s24le"
        } else {
            "aac"
        }
        .into();
        s.rate_mode = if codec == "prores_ks" {
            "quality"
        } else {
            "crf"
        }
        .into();
        s.frame_rate = FrameRate::new(30000, 1001).unwrap();
        render(
            &project,
            &s,
            &TimelineRange {
                start_frame: "15".into(),
                end_frame: "18".into(),
            },
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _, _, _| {}),
        )
        .unwrap();
        let m = inspect(&s.output);
        assert_eq!(m["streams"][0]["width"], width);
        assert_eq!(m["streams"][0]["height"], height);
        assert_eq!(m["streams"][0]["r_frame_rate"], "30000/1001");
        assert_eq!(m["streams"][0]["nb_read_frames"], "3");
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    s.output = dir.path().join("cancel.mp4");
    s.width = 3840;
    s.height = 2160;
    let before = Instant::now();
    let result = render(
        &project,
        &s,
        &range,
        cancel.clone(),
        Arc::new(move |f, _, _, _| {
            if f >= 1 {
                trigger.store(true, Ordering::Release);
            }
        }),
    );
    assert!(result.is_err());
    assert!(cancel.load(Ordering::Acquire));
    assert!(!s.output.exists());
    assert!(before.elapsed() < Duration::from_secs(30));
    assert!(!std::fs::read_dir(dir.path()).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".meraki-")));
    let queue = RenderQueue::spawn(|_| {}).unwrap();
    s.width = 128;
    s.height = 72;
    s.output = dir.path().join("youtube.mp4");
    queue
        .add(project.clone(), s.clone(), range.clone())
        .unwrap(); // existing destination: controlled error
    s.output = dir.path().join("queued.mp4");
    queue
        .add(project.clone(), s.clone(), range.clone())
        .unwrap();
    assert!(queue
        .add(project.clone(), s.clone(), range.clone())
        .is_err());
    s.output = dir.path().join("cancelled-before-start.mp4");
    let queued = queue
        .add(project.clone(), s.clone(), range.clone())
        .unwrap();
    queue.cancel(&queued.id).unwrap();
    assert_eq!(queue.jobs()[0].status, RenderStatus::Ready);
    queue.start();
    let deadline = Instant::now() + Duration::from_secs(20);
    while queue
        .jobs()
        .iter()
        .any(|j| matches!(j.status, RenderStatus::Ready | RenderStatus::Rendering))
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let jobs = queue.jobs();
    assert_eq!(jobs[0].status, RenderStatus::Error);
    assert_eq!(jobs[1].status, RenderStatus::Completed);
    assert_eq!(jobs[1].percentage, 100.);
    assert_eq!(jobs[2].status, RenderStatus::Cancelled);
    queue.shutdown();
    println!("1080p H.264: 90/90 frames; real audio + silent gap; vertical, ProRes, H.265, 4K and NTSC: passed; cancellation cleanup and serial queue recovery: passed.");
}
