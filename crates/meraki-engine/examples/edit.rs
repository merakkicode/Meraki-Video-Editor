use meraki_core::*;
use meraki_engine::{Edit, Engine};
use meraki_timeline::*;
use std::collections::BTreeMap;

fn main() -> Result<()> {
    let seconds = |n| Time::new(n, 1);
    let clip = Clip {
        motion: Default::default(),
        id: ClipId(1),
        name: "Cena_01".into(),
        transform: Default::default(),
        source: MediaSource {
            asset: AssetId(1),
            stream_index: 0,
            kind: TrackKind::Video,
            available_start: Time::ZERO,
            available_duration: seconds(60)?,
        },
        timeline_start: Time::ZERO,
        duration: seconds(10)?,
        source_in: seconds(2)?,
        speed: PlaybackRate::new(1, 1)?,
        enabled: true,
        link_group: None,
        effects: None,
    };
    let timeline = Timeline {
        markers: Vec::new(),
        id: TimelineId(1),
        name: "Primeiro corte".into(),
        frame_rate: FrameRate::new(30000, 1001)?,
        audio_sample_rate: 48000,
        tracks: vec![Track {
            id: TrackId(1),
            name: "Video 1".into(),
            kind: TrackKind::Video,
            locked: false,
            enabled: true,
            solo: false,
            clips: vec![clip],
        }],
        mixer: Default::default(),
        graphs: BTreeMap::new(),
    };
    let mut engine = Engine::new(timeline)?;
    let split = engine.apply(
        0,
        Edit::Split {
            clip: ClipId(1),
            at: seconds(4)?,
            right_id: ClipId(2),
        },
    )?;
    println!(
        "Revision {}: {} clips",
        split.revision,
        split.timeline.tracks[0].clips.len()
    );
    let undo = engine.undo(split.revision)?;
    println!(
        "Revision {}: {} clip",
        undo.revision,
        undo.timeline.tracks[0].clips.len()
    );
    Ok(())
}
