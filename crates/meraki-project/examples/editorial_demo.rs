use meraki_core::*;
use meraki_media::{probe, AssetCatalog, Tools};
use meraki_project::{save, Project};
use meraki_timeline::*;
use std::{collections::BTreeMap, path::PathBuf};
fn main() -> std::result::Result<(), String> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("Informe o destino .meraki")?);
    if output.exists() {
        return Err("Destino já existe".into());
    }
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio-sync.mp4");
    let asset = probe(&Tools::discover(), &source, AssetId(1))?;
    let time = |n| Time::new(n, 1).unwrap();
    let clips = [(1, 2, 1), (2, 7, 4)].map(|(id, start, source)| Clip {
        id: ClipId(id),
        name: format!("Cena {id}"),
        motion: Default::default(),
        transform: Default::default(),
        source: MediaSource {
            asset: asset.id,
            stream_index: asset.stream_index,
            kind: TrackKind::Video,
            available_start: Time::ZERO,
            available_duration: asset.duration,
        },
        timeline_start: time(start),
        duration: time(3),
        source_in: time(source),
        speed: PlaybackRate::new(1, 1).unwrap(),
        enabled: true,
        link_group: None,
        effects: None,
    });
    let mut tracks: Vec<Track> = (1..=4)
        .map(|id| Track {
            id: TrackId(id),
            name: format!(
                "{} {}",
                if id < 3 { "Vídeo" } else { "Áudio" },
                if id < 3 { id } else { id - 2 }
            ),
            kind: if id < 3 {
                TrackKind::Video
            } else {
                TrackKind::Audio
            },
            locked: false,
            enabled: true,
            solo: false,
            clips: vec![],
        })
        .collect();
    tracks[0].clips = clips.into();
    let timeline = Timeline {
        id: TimelineId(1),
        name: "Meraki Meraki — Edição editorial".into(),
        frame_rate: FrameRate::new(30, 1).unwrap(),
        audio_sample_rate: 48000,
        tracks,
        graphs: BTreeMap::new(),
        mixer: Default::default(),
        markers: vec![
            Marker {
                id: "1".into(),
                at: time(5),
                name: "Fim da primeira cena".into(),
                color: "cyan".into(),
            },
            Marker {
                id: "2".into(),
                at: time(7),
                name: "Entrada da segunda cena".into(),
                color: "violet".into(),
            },
        ],
    };
    let catalog = AssetCatalog(BTreeMap::from([(asset.id, asset)]));
    save(&output, &Project::new(timeline, &catalog))?;
    println!("Demo Meraki: {}", output.display());
    Ok(())
}
