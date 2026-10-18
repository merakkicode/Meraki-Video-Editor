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
        .unwrap_or_else(|| root.join("../meraki-m5-demo.meraki"));
    if destination.exists() {
        return Err("Destino já existe; escolha outro nome.".into());
    }
    let video = probe(
        &Tools::discover(),
        &root.join("fixtures/audio-sync.mp4"),
        AssetId(1),
    )?;
    let overlay = probe(
        &Tools::discover(),
        &root.join("fixtures/sample.mp4"),
        AssetId(2),
    )?;
    let mut timeline = Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Meraki — Picture-in-Picture e Trama".into(),
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
    let mut top = clip(&overlay, 2, 0, 1, 10);
    top.transform = Transform {
        rotation: 0.,
        scale: 0.5,
        x: 420.,
        y: 210.,
        opacity: 0.8,
    };
    top.effects = Some(GraphId(1));
    timeline.tracks[1].clips = vec![top];
    timeline.graphs.insert(
        GraphId(1),
        meraki_graph::EffectSettings {
            brightness: 0.03,
            contrast: 1.25,
            saturation: 0.65,
            radius: 6.,
            ..Default::default()
        }
        .graph(GraphId(1))
        .map_err(|e| e.to_string())?,
    );
    timeline.mixer.master.gain_db = Some(-12.);
    let catalog = AssetCatalog(BTreeMap::from([(video.id, video), (overlay.id, overlay)]));
    let project = Project::new(timeline, &catalog);
    save(&destination, &project)?;
    if load(&destination)? != project {
        return Err("Roundtrip divergente".into());
    }
    println!("Projeto validado e salvo: {}", destination.display());
    Ok(())
}
