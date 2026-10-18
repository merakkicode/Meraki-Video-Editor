#![forbid(unsafe_code)]
use meraki_core::*;
use meraki_media::{probe, Asset, AssetCatalog, Tools};
use meraki_project::{load, save, Project};
use meraki_timeline::*;
use std::{collections::BTreeMap, path::PathBuf};

fn clip(asset: &Asset, id: u128, start: i64, source: i64, duration: i64) -> Clip {
    Clip {
        motion: Default::default(),
        id: ClipId(id),
        name: asset.name.clone(),
        transform: Default::default(),
        source: MediaSource {
            asset: asset.id,
            stream_index: asset.stream_index,
            kind: if asset.audio_only {
                TrackKind::Audio
            } else {
                TrackKind::Video
            },
            available_start: Time::ZERO,
            available_duration: asset.duration,
        },
        timeline_start: Time::new(start, 1).unwrap(),
        source_in: Time::new(source, 1).unwrap(),
        duration: Time::new(duration, 1).unwrap(),
        speed: PlaybackRate::new(1, 1).unwrap(),
        enabled: true,
        link_group: None,
        effects: None,
    }
}
fn main() -> std::result::Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let destination = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("../meraki-m4-demo.meraki"));
    if destination.exists() {
        return Err("Destino já existe; escolha outro nome.".into());
    }
    let video = probe(
        &Tools::discover(),
        &root.join("fixtures/audio-sync.mp4"),
        AssetId(1),
    )?;
    let audio = probe(
        &Tools::discover(),
        &root.join("fixtures/audio-track.wav"),
        AssetId(2),
    )?;
    let mut timeline = Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Meraki — áudio e sincronismo".into(),
        frame_rate: FrameRate::new(30, 1).unwrap(),
        audio_sample_rate: 48000,
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
                locked: false,
                enabled: true,
                solo: false,
                clips: vec![],
            })
            .collect(),
        mixer: Default::default(),
        graphs: BTreeMap::new(),
    };
    timeline.tracks[0].clips = vec![clip(&video, 1, 0, 0, 20)];
    timeline.tracks[2].clips = vec![clip(&audio, 2, 0, 0, 8), clip(&audio, 3, 10, 8, 8)];
    timeline.tracks[3].clips = vec![clip(&audio, 4, 4, 4, 12)];
    for id in [TrackId(3), TrackId(4)] {
        timeline.mixer.tracks.insert(
            id,
            ChannelMix {
                muted: true,
                gain_db: Some(-12.),
                eq_db: [0., 3., 0.],
                ..Default::default()
            },
        );
    }
    let catalog = AssetCatalog(BTreeMap::from([(video.id, video), (audio.id, audio)]));
    let project = Project::new(timeline, &catalog);
    save(&destination, &project)?;
    if load(&destination)? != project {
        return Err("Roundtrip divergente".into());
    }
    println!("Projeto validado e salvo: {}", destination.display());
    Ok(())
}
