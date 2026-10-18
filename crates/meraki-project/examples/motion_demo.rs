#![forbid(unsafe_code)]
use meraki_core::*;
use meraki_media::{probe, Asset, AssetCatalog, Tools};
use meraki_project::{load, save, Project};
use meraki_timeline::*;
use std::{collections::BTreeMap, path::PathBuf};
fn t(n: i64) -> Time {
    Time::new(n, 1).unwrap()
}
fn clip(a: &Asset, id: u128, start: i64, source: i64) -> Clip {
    Clip {
        motion: Default::default(),
        transform: Default::default(),
        id: ClipId(id),
        name: a.name.clone(),
        source: MediaSource {
            asset: a.id,
            stream_index: a.stream_index,
            kind: TrackKind::Video,
            available_start: t(0),
            available_duration: a.duration,
        },
        timeline_start: t(start),
        duration: t(3),
        source_in: t(source),
        speed: PlaybackRate::new(1, 1).unwrap(),
        enabled: true,
        link_group: None,
        effects: None,
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dest = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("../meraki-m6-demo.meraki"));
    if dest.exists() {
        return Err("Destination already exists".into());
    }
    let a = probe(
        &Tools::discover(),
        &root.join("fixtures/audio-sync.mp4"),
        AssetId(1),
    )?;
    let b = probe(
        &Tools::discover(),
        &root.join("fixtures/sample.mp4"),
        AssetId(2),
    )?;
    let mut first = clip(&a, 1, 0, 1);
    first.motion.fade_in = Time::new(1, 2)?;
    first.motion.keys.insert(
        Property::Scale,
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
    first.motion.keys.insert(
        Property::Volume,
        vec![
            Keyframe {
                at: t(0),
                value: 0.25,
                interpolation: Interpolation::Linear,
            },
            Keyframe {
                at: t(3),
                value: 0.75,
                interpolation: Interpolation::Smooth,
            },
        ],
    );
    let mut second = clip(&b, 2, 3, 4);
    second.motion.dissolve_in = t(1);
    second.motion.fade_out = t(1);
    second.effects = Some(GraphId(1));
    let mut title = clip(&a, 3, 0, 0);
    title.name = "Título animado".into();
    title.duration = t(6);
    title.motion.title = Some(Title {
        text: "MERAKI\nMovimento, cor e histórias".into(),
        size: 72.,
        ..Default::default()
    });
    title.motion.fade_in = t(1);
    title.motion.fade_out = t(1);
    title.transform.y = 240.;
    title.motion.keys.insert(
        Property::Y,
        vec![
            Keyframe {
                at: t(0),
                value: 330.,
                interpolation: Interpolation::Smooth,
            },
            Keyframe {
                at: t(2),
                value: 180.,
                interpolation: Interpolation::Linear,
            },
        ],
    );
    let mut timeline = Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Meraki — Movimento, dissolvência e títulos".into(),
        frame_rate: FrameRate::new(60, 1)?,
        audio_sample_rate: 48000,
        mixer: Default::default(),
        graphs: BTreeMap::new(),
        tracks: (1..=4)
            .map(|id| Track {
                id: TrackId(id),
                name: format!(
                    "{} {}",
                    if id <= 2 { "Vídeo" } else { "Áudio" },
                    if id <= 2 { id } else { id - 2 }
                ),
                kind: if id <= 2 {
                    TrackKind::Video
                } else {
                    TrackKind::Audio
                },
                enabled: true,
                solo: false,
                locked: false,
                clips: vec![],
            })
            .collect(),
    };
    timeline.tracks[0].clips = vec![first, second];
    timeline.tracks[1].clips = vec![title];
    timeline.mixer.master.gain_db = Some(-12.);
    timeline.graphs.insert(
        GraphId(1),
        meraki_graph::EffectSettings {
            contrast: 1.2,
            saturation: 0.7,
            radius: 8.,
            ..Default::default()
        }
        .graph(GraphId(1))?,
    );
    let catalog = AssetCatalog(BTreeMap::from([(a.id, a), (b.id, b)]));
    let p = Project::new(timeline, &catalog);
    save(&dest, &p)?;
    assert_eq!(load(&dest)?, p);
    println!("{}", dest.display());
    Ok(())
}
