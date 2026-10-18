#![forbid(unsafe_code)]
use meraki_core::{ClipId, Error, Result, Time};
use meraki_timeline::Timeline;
use std::{collections::VecDeque, sync::Arc};

#[derive(Debug, Clone)]
pub enum Edit {
    Trim {
        clip: ClipId,
        head: bool,
        at: Time,
        ripple: bool,
    },
    Marker {
        marker: meraki_timeline::Marker,
        remove: bool,
    },
    Motion {
        clip: ClipId,
        motion: meraki_timeline::Motion,
    },
    Title {
        id: ClipId,
        at: Time,
    },
    Visual {
        clip: ClipId,
        transform: meraki_timeline::Transform,
        effects: meraki_graph::EffectSettings,
        motion: Option<meraki_timeline::Motion>,
    },
    Mixer {
        track: Option<meraki_core::TrackId>,
        channel: meraki_timeline::ChannelMix,
    },
    Move {
        clip: ClipId,
        to: Time,
    },
    Split {
        clip: ClipId,
        at: Time,
        right_id: ClipId,
    },
    Insert {
        track: meraki_core::TrackId,
        clip: Box<meraki_timeline::Clip>,
    },
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub revision: u64,
    pub timeline: Arc<Timeline>,
}
/// Single-owner state machine. Host this in one engine actor; no UI/GPU/FFmpeg dependencies.
/// Cloning is a baseline correctness choice; replace with structural sharing after profiling.
pub struct Engine {
    current: Snapshot,
    undo: VecDeque<Arc<Timeline>>,
}
impl Engine {
    pub fn new(timeline: Timeline) -> Result<Self> {
        timeline.validate()?;
        Ok(Self {
            current: Snapshot {
                revision: 0,
                timeline: Arc::new(timeline),
            },
            undo: VecDeque::new(),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.current.clone()
    }
    pub fn apply(&mut self, expected_revision: u64, edit: Edit) -> Result<Snapshot> {
        if expected_revision != self.current.revision {
            return Err(Error::Conflict);
        }
        let revision = self
            .current
            .revision
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        let mut next = (*self.current.timeline).clone();
        match edit {
            Edit::Trim {
                clip,
                head,
                at,
                ripple,
            } => next.trim_clip(clip, head, at, ripple)?,
            Edit::Marker { marker, remove } => {
                marker.validate()?;
                next.markers.retain(|m| m.id != marker.id);
                if !remove {
                    next.markers.push(marker);
                    next.markers.sort_by_key(|m| m.at);
                }
            }
            Edit::Motion { clip, motion } => {
                let track = next
                    .tracks
                    .iter_mut()
                    .find(|t| t.clips.iter().any(|c| c.id == clip))
                    .ok_or(Error::NotFound)?;
                if track.locked {
                    return Err(Error::Locked);
                }
                track
                    .clips
                    .iter_mut()
                    .find(|c| c.id == clip)
                    .unwrap()
                    .motion = motion;
            }
            Edit::Title { id, at } => {
                use meraki_timeline::*;
                let track = next
                    .tracks
                    .iter_mut()
                    .filter(|t| t.kind == TrackKind::Video)
                    .nth(1)
                    .ok_or(Error::Invalid("V2 missing"))?;
                if track.locked {
                    return Err(Error::Locked);
                }
                track.clips.push(Clip {
                    id,
                    name: "Título".into(),
                    motion: Motion {
                        title: Some(Title::default()),
                        ..Default::default()
                    },
                    transform: Transform::default(),
                    source: MediaSource {
                        asset: meraki_core::AssetId(0),
                        stream_index: 0,
                        kind: TrackKind::Video,
                        available_start: Time::ZERO,
                        available_duration: Time::new(5, 1)?,
                    },
                    timeline_start: at,
                    duration: Time::new(5, 1)?,
                    source_in: Time::ZERO,
                    speed: meraki_core::PlaybackRate::new(1, 1)?,
                    enabled: true,
                    link_group: None,
                    effects: None,
                });
                track.clips.sort_by_key(|c| c.timeline_start);
            }
            Edit::Visual {
                clip,
                transform,
                effects,
                motion,
            } => {
                transform.validate()?;
                effects.validate()?;
                let old = next
                    .tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .find(|c| c.id == clip)
                    .ok_or(Error::NotFound)?
                    .effects;
                let unique = old.filter(|id| {
                    next.tracks
                        .iter()
                        .flat_map(|t| &t.clips)
                        .filter(|c| c.effects == Some(*id))
                        .count()
                        == 1
                });
                let graph_id = match unique {
                    Some(id) => id,
                    None => meraki_core::GraphId(
                        next.graphs
                            .keys()
                            .map(|id| id.0)
                            .max()
                            .unwrap_or(0)
                            .checked_add(1)
                            .ok_or(Error::Overflow)?,
                    ),
                };
                let track = next
                    .tracks
                    .iter_mut()
                    .find(|t| t.clips.iter().any(|c| c.id == clip))
                    .ok_or(Error::NotFound)?;
                if track.locked {
                    return Err(Error::Locked);
                }
                if track.kind != meraki_timeline::TrackKind::Video {
                    return Err(Error::Invalid("visual edit requires video"));
                }
                let c = track.clips.iter_mut().find(|c| c.id == clip).unwrap();
                c.transform = transform;
                if let Some(m) = motion {
                    c.motion = m;
                }
                c.effects = Some(graph_id);
                next.graphs.insert(graph_id, effects.graph(graph_id)?);
            }
            Edit::Mixer { track, channel } => {
                channel.validate()?;
                if let Some(id) = track {
                    next.mixer.tracks.insert(id, channel);
                } else {
                    next.mixer.master = channel;
                }
            }
            Edit::Move { clip, to } => next.move_clip(clip, to)?,
            Edit::Split { clip, at, right_id } => next.split_clip(clip, at, right_id)?,
            Edit::Insert { track, clip } => {
                let track = next
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == track)
                    .ok_or(Error::NotFound)?;
                if track.locked {
                    return Err(Error::Locked);
                }
                track.clips.push(*clip);
                track.clips.sort_by_key(|c| c.timeline_start);
            }
        }
        next.validate()?;
        if self.undo.len() == 64 {
            self.undo.pop_front();
        }
        self.undo.push_back(self.current.timeline.clone());
        self.current = Snapshot {
            revision,
            timeline: Arc::new(next),
        };
        Ok(self.snapshot())
    }
    pub fn undo(&mut self, expected_revision: u64) -> Result<Snapshot> {
        if expected_revision != self.current.revision {
            return Err(Error::Conflict);
        }
        let revision = self
            .current
            .revision
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        let timeline = self.undo.pop_back().ok_or(Error::NotFound)?;
        self.current = Snapshot { revision, timeline };
        Ok(self.snapshot())
    }
    /// Loading a document is a transaction with a fresh revision and a new undo history.
    pub fn replace(&mut self, expected_revision: u64, timeline: Timeline) -> Result<Snapshot> {
        if expected_revision != self.current.revision {
            return Err(Error::Conflict);
        }
        timeline.validate()?;
        let revision = self
            .current
            .revision
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        self.current = Snapshot {
            revision,
            timeline: Arc::new(timeline),
        };
        self.undo.clear();
        Ok(self.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meraki_core::*;
    use meraki_graph::EffectSettings;
    use meraki_timeline::*;
    use std::collections::BTreeMap;
    fn t(n: i64) -> Time {
        Time::new(n, 1).unwrap()
    }
    fn engine() -> Engine {
        let clip = Clip {
            motion: Default::default(),
            id: ClipId(1),
            name: "Cena".into(),
            transform: Default::default(),
            source: MediaSource {
                asset: AssetId(1),
                stream_index: 0,
                kind: TrackKind::Video,
                available_start: t(0),
                available_duration: t(100),
            },
            timeline_start: t(0),
            duration: t(10),
            source_in: t(3),
            speed: PlaybackRate::new(2, 1).unwrap(),
            enabled: true,
            link_group: None,
            effects: None,
        };
        Engine::new(Timeline {
            markers: Vec::new(),
            id: TimelineId(1),
            name: "Meraki".into(),
            frame_rate: FrameRate::new(30000, 1001).unwrap(),
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
        })
        .unwrap()
    }
    #[test]
    fn trim_preserves_source_speed_animation_and_undo() {
        let mut e = engine();
        let original = e.snapshot().timeline.tracks[0].clips[0].clone();
        let s = e
            .apply(
                0,
                Edit::Trim {
                    clip: ClipId(1),
                    head: true,
                    at: t(2),
                    ripple: false,
                },
            )
            .unwrap();
        let c = &s.timeline.tracks[0].clips[0];
        assert_eq!(c.source_in, t(7));
        assert_eq!(c.duration, t(8));
        assert_eq!(c.end().unwrap(), t(10));
        assert_eq!(c.motion.offset, t(2));
        assert_eq!(
            c.source_at(t(3)).unwrap(),
            original.source_at(t(3)).unwrap()
        );
        let s = e
            .apply(
                1,
                Edit::Trim {
                    clip: ClipId(1),
                    head: true,
                    at: t(0),
                    ripple: false,
                },
            )
            .unwrap();
        assert_eq!(s.timeline.tracks[0].clips[0], original);
        let s = e.undo(2).unwrap();
        assert_eq!(s.timeline.tracks[0].clips[0].source_in, t(7));
    }
    #[test]
    fn trim_rejects_source_overrun_zero_duration_and_stale_revision_atomically() {
        let mut e = engine();
        let before = e.snapshot();
        for (head, at) in [(false, t(49)), (false, t(0)), (true, t(10)), (true, t(-2))] {
            assert!(e
                .apply(
                    0,
                    Edit::Trim {
                        clip: ClipId(1),
                        head,
                        at,
                        ripple: false
                    }
                )
                .is_err());
            assert_eq!(e.snapshot().timeline, before.timeline);
            assert_eq!(e.snapshot().revision, 0);
        }
        assert!(e
            .apply(
                1,
                Edit::Trim {
                    clip: ClipId(1),
                    head: false,
                    at: t(5),
                    ripple: false
                }
            )
            .is_err());
        let mut tl = (*before.timeline).clone();
        tl.tracks[0].locked = true;
        let mut locked = Engine::new(tl).unwrap();
        assert!(matches!(
            locked.apply(
                0,
                Edit::Trim {
                    clip: ClipId(1),
                    head: false,
                    at: t(5),
                    ripple: false
                }
            ),
            Err(Error::Locked)
        ));
    }
    #[test]
    fn ripple_head_and_tail_move_only_following_clips() {
        for head in [false, true] {
            let mut e = engine();
            let mut next = e.snapshot().timeline.tracks[0].clips[0].clone();
            next.id = ClipId(2);
            next.timeline_start = t(10);
            e.apply(
                0,
                Edit::Insert {
                    track: TrackId(1),
                    clip: Box::new(next.clone()),
                },
            )
            .unwrap();
            let s = e
                .apply(
                    1,
                    Edit::Trim {
                        clip: ClipId(1),
                        head,
                        at: if head { t(2) } else { t(8) },
                        ripple: true,
                    },
                )
                .unwrap();
            assert_eq!(s.timeline.tracks[0].clips[0].timeline_start, t(0));
            assert_eq!(s.timeline.tracks[0].clips[0].duration, t(8));
            assert_eq!(s.timeline.tracks[0].clips[1].timeline_start, t(8));
            assert_eq!(s.timeline.tracks[0].clips[1].source_in, next.source_in);
        }
    }
    #[test]
    fn markers_are_validated_transactional_and_undoable() {
        let mut e = engine();
        let marker = Marker {
            id: "1".into(),
            at: Time::new(1001, 30000).unwrap(),
            name: "Corte".into(),
            color: "cyan".into(),
        };
        let s = e
            .apply(
                0,
                Edit::Marker {
                    marker: marker.clone(),
                    remove: false,
                },
            )
            .unwrap();
        assert_eq!(s.timeline.markers[0], marker);
        let mut bad = marker.clone();
        bad.color = "unknown".into();
        assert!(e
            .apply(
                1,
                Edit::Marker {
                    marker: bad,
                    remove: false
                }
            )
            .is_err());
        assert!(e
            .apply(
                1,
                Edit::Marker {
                    marker,
                    remove: true
                }
            )
            .unwrap()
            .timeline
            .markers
            .is_empty());
        assert_eq!(e.undo(2).unwrap().timeline.markers.len(), 1);
    }
    #[test]
    fn visual_edits_split_copy_on_write_and_undo() {
        let mut e = engine();
        let visual = |clip, opacity| Edit::Visual {
            motion: None,
            clip: ClipId(clip),
            transform: Transform {
                rotation: 0.,
                scale: 0.5,
                x: 480.,
                opacity,
                ..Default::default()
            },
            effects: EffectSettings {
                contrast: 1.4,
                radius: 12.,
                ..Default::default()
            },
        };
        let first = e.apply(0, visual(1, 0.7)).unwrap();
        assert_eq!(first.timeline.tracks[0].clips[0].transform.scale, 0.5);
        assert!(e.apply(1, visual(1, f32::NAN)).is_err());
        assert_eq!(e.snapshot().revision, 1);
        e.apply(
            1,
            Edit::Split {
                clip: ClipId(1),
                at: t(4),
                right_id: ClipId(2),
            },
        )
        .unwrap();
        let split = e.snapshot();
        let changed = e.apply(2, visual(2, 0.2)).unwrap();
        let clips = &changed.timeline.tracks[0].clips;
        assert_ne!(clips[0].effects, clips[1].effects);
        assert_eq!(clips[0].transform.opacity, 0.7);
        assert_eq!(clips[1].source_in, t(11));
        assert_eq!(*e.undo(3).unwrap().timeline, *split.timeline);
        let mut locked = (*first.timeline).clone();
        locked.tracks[0].locked = true;
        let mut locked = Engine::new(locked).unwrap();
        assert!(matches!(locked.apply(0, visual(1, 1.)), Err(Error::Locked)));
    }
    #[test]
    fn mixer_is_transactional_and_undo_preserves_revision() {
        let mut e = engine();
        let original = e.snapshot();
        let channel = ChannelMix {
            gain_db: Some(-12.),
            eq_db: [3., -2., 1.],
            ..Default::default()
        };
        let edited = e
            .apply(
                0,
                Edit::Mixer {
                    track: Some(TrackId(1)),
                    channel: channel.clone(),
                },
            )
            .unwrap();
        assert_eq!(edited.timeline.mixer.tracks[&TrackId(1)], channel);
        assert!(matches!(e.undo(0), Err(Error::Conflict)));
        assert!(e
            .apply(
                1,
                Edit::Mixer {
                    track: Some(TrackId(999)),
                    channel: channel.clone()
                }
            )
            .is_err());
        assert!(e
            .apply(
                1,
                Edit::Mixer {
                    track: None,
                    channel: ChannelMix {
                        gain_db: Some(f32::NAN),
                        ..channel
                    }
                }
            )
            .is_err());
        assert_eq!(e.snapshot().revision, 1);
        let undone = e.undo(1).unwrap();
        assert_eq!(undone.revision, 2);
        assert_eq!(*undone.timeline, *original.timeline);
    }
    #[test]
    fn split_preserves_source_mapping_and_undo() {
        let mut e = engine();
        let original = e.snapshot();
        let edited = e
            .apply(
                0,
                Edit::Split {
                    clip: ClipId(1),
                    at: t(4),
                    right_id: ClipId(2),
                },
            )
            .unwrap();
        let clips = &edited.timeline.tracks[0].clips;
        assert_eq!(clips[0].duration, t(4));
        assert_eq!(clips[1].duration, t(6));
        assert_eq!(clips[1].source_in, t(11));
        assert_eq!(
            clips[1].source_at(t(5)).unwrap(),
            original.timeline.tracks[0].clips[0]
                .source_at(t(5))
                .unwrap()
        );
        assert_eq!(*e.undo(1).unwrap().timeline, *original.timeline);
        assert_eq!(original.timeline.tracks[0].clips.len(), 1);
    }
    #[test]
    fn failed_and_stale_edits_leave_state_unchanged() {
        let mut e = engine();
        assert!(e
            .apply(
                0,
                Edit::Move {
                    clip: ClipId(1),
                    to: t(-1)
                }
            )
            .is_err());
        assert_eq!(e.snapshot().revision, 0);
        assert_eq!(
            e.snapshot().timeline.tracks[0].clips[0].timeline_start,
            t(0)
        );
        assert!(matches!(
            e.apply(
                1,
                Edit::Move {
                    clip: ClipId(1),
                    to: t(1)
                }
            ),
            Err(Error::Conflict)
        ));
        assert!(e
            .apply(
                0,
                Edit::Split {
                    clip: ClipId(1),
                    at: t(10),
                    right_id: ClipId(2)
                }
            )
            .is_err());
    }
    #[test]
    fn adjacent_clips_are_legal_but_overlap_is_atomic_failure() {
        let mut e = engine();
        e.apply(
            0,
            Edit::Split {
                clip: ClipId(1),
                at: t(4),
                right_id: ClipId(2),
            },
        )
        .unwrap();
        assert!(e
            .apply(
                1,
                Edit::Move {
                    clip: ClipId(2),
                    to: t(3)
                }
            )
            .is_err());
        assert_eq!(e.snapshot().revision, 1);
        assert_eq!(
            e.snapshot().timeline.tracks[0].clips[1].timeline_start,
            t(4)
        );
    }
}
