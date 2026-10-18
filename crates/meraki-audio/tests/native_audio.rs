use meraki_audio::*;
use meraki_core::*;
use meraki_media::*;
use meraki_timeline::*;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::{Duration, Instant},
};
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        assert!(Instant::now() < end, "timeout");
        thread::sleep(Duration::from_millis(20));
    }
}
#[test]
#[ignore = "requires default stereo f32 48 kHz output, FFmpeg and create-audio-fixtures.ps1; plays test tone"]
fn native_clock_mixer_and_real_pcm() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio-sync.mp4");
    let asset = probe(&Tools::discover(), &path, AssetId(1)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut pcm = prepare(&asset, dir.path(), &AtomicBool::new(false))
        .unwrap()
        .unwrap();
    assert!(pcm.peaks.values.len() > 100);
    assert!(pcm.peaks.values.iter().any(|p| p[1] > 0.05));
    let f = pcm.at(Time::new(1, 100).unwrap()).unwrap();
    assert!(f.iter().all(|v| v.is_finite()));
    let wav = probe(
        &Tools::discover(),
        &path.with_file_name("audio-track.wav"),
        AssetId(2),
    )
    .unwrap();
    assert!(wav.audio_only);
    let clip = Clip {
        motion: Default::default(),
        id: ClipId(1),
        name: "AV".into(),
        transform: Default::default(),
        source: MediaSource {
            asset: asset.id,
            stream_index: asset.stream_index,
            kind: TrackKind::Video,
            available_start: Time::ZERO,
            available_duration: asset.duration,
        },
        timeline_start: Time::ZERO,
        duration: asset.duration,
        source_in: Time::ZERO,
        speed: PlaybackRate::new(1, 1).unwrap(),
        enabled: true,
        link_group: None,
        effects: None,
    };
    let mut timeline = Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Native test".into(),
        frame_rate: FrameRate::new(30, 1).unwrap(),
        audio_sample_rate: 48000,
        tracks: vec![Track {
            id: TrackId(1),
            name: "V1".into(),
            kind: TrackKind::Video,
            locked: false,
            enabled: true,
            solo: false,
            clips: vec![clip],
        }],
        graphs: BTreeMap::new(),
        mixer: Default::default(),
    };
    let catalog = AssetCatalog(BTreeMap::from([(asset.id, asset)]));
    let engine = AudioEngine::spawn(timeline.clone(), |_| {}).unwrap();
    engine.update(Arc::new(timeline.clone()), &catalog, true);
    until(|| engine.state().available || engine.state().error.is_some());
    assert!(engine.state().available, "{:?}", engine.state().error);
    until(|| engine.peaks().contains_key(&AssetId(1)));
    engine.start(Time::ZERO).unwrap();
    until(|| engine.time() > Time::new(1, 2).unwrap());
    let start = Instant::now();
    let time = engine.time();
    thread::sleep(Duration::from_millis(700));
    let elapsed = engine.time().checked_sub(time).unwrap();
    let seconds = elapsed.numerator() as f64 / f64::from(elapsed.denominator());
    assert!(
        (seconds - start.elapsed().as_secs_f64()).abs() < 0.12,
        "clock delta {seconds}"
    );
    let master_rms = |e: &AudioEngine| {
        e.state()
            .meters
            .iter()
            .find(|m| m.id == "master")
            .unwrap()
            .rms[0]
    };
    until(|| master_rms(&engine) > -40.);
    timeline.mixer.tracks.insert(
        TrackId(1),
        ChannelMix {
            gain_db: Some(-24.),
            ..Default::default()
        },
    );
    engine.update(Arc::new(timeline.clone()), &catalog, false);
    until(|| master_rms(&engine) < -40.);
    assert!(engine.time() > Time::new(1, 1).unwrap());
    timeline.mixer.tracks.get_mut(&TrackId(1)).unwrap().muted = true;
    engine.update(Arc::new(timeline.clone()), &catalog, false);
    until(|| master_rms(&engine) < -100.);
    engine.reset(Time::new(7, 1).unwrap(), false).unwrap();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(engine.time(), Time::new(7, 1).unwrap());
    timeline.mixer.tracks.clear();
    engine.update(Arc::new(timeline), &catalog, false);
    engine.start(Time::new(3, 1).unwrap()).unwrap();
    until(|| engine.time() > Time::new(31, 10).unwrap());
    assert!(engine.time() < Time::new(4, 1).unwrap());
    println!("Native CPAL clock, real PCM/peaks, live gain/mute and seek passed; latency {:.2}ms, underruns {}",engine.state().latency_ms,engine.state().underruns);
    engine.shutdown();
}
