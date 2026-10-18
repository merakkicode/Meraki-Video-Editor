use meraki_core::{AssetId, Time};
use meraki_media::*;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}
fn find_frame(asset: &Asset, target: Time) -> std::sync::Arc<VideoFrame> {
    let decoder = DecoderSession::start(&Tools::discover(), asset, target).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(Instant::now() < deadline, "seek timed out");
        match decoder.next(Duration::from_millis(200)) {
            Ok(Ok(frame)) if frame.covers(target) => return frame,
            Ok(Ok(_)) => {}
            Ok(Err(e)) => panic!("{e}"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(e) => panic!("{e}"),
        }
    }
}
#[test]
#[ignore = "requires FFmpeg and scripts/create-fixtures.ps1"]
fn real_h264_nonzero_pts_vfr_and_latest_seek() {
    let tools = Tools::discover();
    let mut observations = Vec::new();
    for (i, name) in ["sample.mp4", "offset.mp4", "vfr.mkv"].iter().enumerate() {
        let asset = probe(&tools, &fixture(name), AssetId(i as u128 + 1)).unwrap();
        assert_eq!((asset.width, asset.height), (640, 360));
        if *name == "offset.mp4" {
            assert!(asset.start_time >= Time::new(5, 1).unwrap());
        }
        let zero = find_frame(&asset, Time::ZERO);
        let target = Time::new(105, 100).unwrap();
        let started = Instant::now();
        let later = find_frame(&asset, target);
        assert!(later.covers(target));
        assert_ne!(
            &zero.pixels[..],
            &later.pixels[..],
            "real frames must differ over time"
        );
        assert_eq!(later.pixels.len(), 640 * 360 * 4);
        observations.push(format!(
            "{name}: seek {:.1}ms, frame PTS {:?}",
            started.elapsed().as_secs_f64() * 1000.,
            later.pts
        ));
    }
    let asset = probe(&tools, &fixture("sample.mp4"), AssetId(1)).unwrap();
    let worker = DecodeWorker::spawn(tools, 16, 64 * 1024 * 1024).unwrap();
    let started = Instant::now();
    for seq in 1..=80 {
        worker.request(DecodeRequest {
            generation: seq,
            sequence: seq,
            revision: 1,
            asset: Some(asset.clone()),
            source_time: Time::new(if seq == 80 { 7 } else { seq as i64 % 10 }, 1).unwrap(),
            timeline_time: Time::ZERO,
        });
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if worker.with_output(|o| {
            o.presented.as_ref().is_some_and(|p| {
                p.sequence == 80
                    && p.generation == 80
                    && p.frame.as_ref().unwrap().covers(Time::new(7, 1).unwrap())
            })
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "latest seek failed: {:?}",
            worker.with_output(|o| o.error.clone())
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(worker.with_output(|o| o.cache_bytes <= 64 * 1024 * 1024));
    println!(
        "{}\n80 seeks -> newest frame: {:.1}ms",
        observations.join("\n"),
        started.elapsed().as_secs_f64() * 1000.
    );
    // Reusing the same source after EOF must restart decoding even without a new generation.
    for (sequence, target) in [(81, Time::new(12, 1).unwrap()), (82, Time::ZERO)] {
        worker.request(DecodeRequest {
            generation: 80,
            sequence,
            revision: 1,
            asset: Some(asset.clone()),
            source_time: target,
            timeline_time: Time::new(sequence as i64, 1).unwrap(),
        });
        let deadline = Instant::now() + Duration::from_secs(15);
        while !worker.with_output(|o| {
            o.presented
                .as_ref()
                .is_some_and(|p| p.sequence == sequence && p.frame.as_ref().unwrap().covers(target))
        }) {
            assert!(
                Instant::now() < deadline,
                "repeated source failed after EOF"
            );
            thread::sleep(Duration::from_millis(5));
        }
        thread::sleep(Duration::from_millis(100));
    }
    worker.shutdown();
}
