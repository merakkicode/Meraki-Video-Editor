#![forbid(unsafe_code)]
mod editorial;
pub use editorial::*;
mod animation;
pub use animation::*;
mod mixer;
mod visual;
use meraki_core::*;
use meraki_graph::EffectGraph;
pub use mixer::*;
use std::collections::{BTreeMap, BTreeSet};
pub use visual::*;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MediaSource {
    pub asset: AssetId,
    pub stream_index: u32,
    pub kind: TrackKind,
    /// Known usable range after translating stream PTS to normalized source seconds.
    pub available_start: Time,
    pub available_duration: Time,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Clip {
    #[serde(default)]
    pub motion: Motion,
    #[serde(default)]
    pub transform: Transform,
    pub id: ClipId,
    pub name: String,
    pub source: MediaSource,
    pub timeline_start: Time,
    pub duration: Time,
    pub source_in: Time,
    pub speed: PlaybackRate,
    pub enabled: bool,
    pub link_group: Option<LinkGroupId>,
    /// MVP: video effects only. Audio DSP is a separate graph in a later milestone.
    pub effects: Option<GraphId>,
}
impl Clip {
    pub fn end(&self) -> Result<Time> {
        self.timeline_start.checked_add(self.duration)
    }
    pub fn source_at(&self, timeline_time: Time) -> Result<Time> {
        if timeline_time < self.timeline_start || timeline_time >= self.end()? {
            return Err(Error::Invalid("outside clip"));
        }
        self.source_in.checked_add(
            timeline_time
                .checked_sub(self.timeline_start)?
                .scale(self.speed.ratio())?,
        )
    }
    pub fn validate(&self) -> Result<()> {
        self.transform.validate()?;
        self.motion.validate(self.duration)?;
        if self.motion.title.is_some() && self.source.kind != TrackKind::Video {
            return Err(Error::Invalid("title must be video"));
        }
        if self.timeline_start < Time::ZERO
            || self.duration <= Time::ZERO
            || self.source.available_duration <= Time::ZERO
        {
            return Err(Error::Invalid("clip range/speed"));
        }
        self.end()?;
        if self.motion.title.is_some() {
            return Ok(());
        }
        let source_end = self
            .source_in
            .checked_add(self.duration.scale(self.speed.ratio())?)?;
        let available_end = self
            .source
            .available_start
            .checked_add(self.source.available_duration)?;
        if self.source_in < self.source.available_start || source_end > available_end {
            return Err(Error::Invalid("source handles"));
        }
        Ok(())
    }
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub locked: bool,
    pub enabled: bool,
    pub solo: bool,
    pub clips: Vec<Clip>,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Timeline {
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub mixer: MixerSettings,
    pub id: TimelineId,
    pub name: String,
    pub frame_rate: FrameRate,
    pub audio_sample_rate: u32,
    /// Video tracks composited in vector order: later video tracks are above earlier ones.
    pub tracks: Vec<Track>,
    pub graphs: BTreeMap<GraphId, EffectGraph>,
}
impl Timeline {
    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        if self.markers.len() > 10000 {
            return Err(Error::Invalid("marker limit"));
        }
        for marker in &self.markers {
            marker.validate()?;
            if !ids.insert(&marker.id) {
                return Err(Error::Invalid("duplicate marker"));
            }
        }
        self.mixer.master.validate()?;
        for (id, channel) in &self.mixer.tracks {
            channel.validate()?;
            if !self.tracks.iter().any(|t| t.id == *id) {
                return Err(Error::NotFound);
            }
        }
        if self.audio_sample_rate == 0 {
            return Err(Error::Invalid("sample rate"));
        }
        let mut tracks = BTreeSet::new();
        let mut clips = BTreeSet::new();
        for (id, graph) in &self.graphs {
            if *id != graph.id {
                return Err(Error::Invalid("graph identity"));
            }
            meraki_graph::compile(graph)?;
        }
        for track in &self.tracks {
            if !tracks.insert(track.id) {
                return Err(Error::Invalid("duplicate track"));
            }
            validate_transitions(track)?;
            let mut intervals = Vec::new();
            for clip in &track.clips {
                if !clips.insert(clip.id) || clip.source.kind != track.kind {
                    return Err(Error::Invalid("clip identity/kind"));
                }
                clip.validate()?;
                if let Some(id) = clip.effects {
                    if track.kind != TrackKind::Video || !self.graphs.contains_key(&id) {
                        return Err(Error::Invalid("clip graph"));
                    }
                }
                intervals.push((clip.timeline_start, clip.end()?));
            }
            intervals.sort();
            if intervals.windows(2).any(|w| w[0].1 > w[1].0) {
                return Err(Error::Invalid("same-track overlap"));
            }
        }
        Ok(())
    }
    /// Primitive for the engine transaction: within-track move, no implicit ripple.
    pub fn move_clip(&mut self, id: ClipId, to: Time) -> Result<()> {
        for track in &mut self.tracks {
            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == id) {
                if track.locked {
                    return Err(Error::Locked);
                }
                clip.timeline_start = to;
                return Ok(());
            }
        }
        Err(Error::NotFound)
    }
    /// Splits one clip only; grouped A/V edits must be expanded by the command layer.
    /// Static visual graphs survive a split; the engine copies shared graphs on visual edits.
    pub fn split_clip(&mut self, id: ClipId, at: Time, right_id: ClipId) -> Result<()> {
        if self
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .any(|c| c.id == right_id)
        {
            return Err(Error::Invalid("duplicate clip"));
        }
        for track in &mut self.tracks {
            if let Some(index) = track.clips.iter().position(|c| c.id == id) {
                if track.locked {
                    return Err(Error::Locked);
                }
                let original = &track.clips[index];
                if at <= original.timeline_start || at >= original.end()? {
                    return Err(Error::Invalid("split boundary"));
                }
                let mut right = original.clone();
                right.id = right_id;
                right.source_in = original.source_at(at)?;
                right.timeline_start = at;
                right.duration = original.end()?.checked_sub(at)?;
                let left_duration = at.checked_sub(original.timeline_start)?;
                right.motion.offset = original.motion.offset.checked_add(left_duration)?;
                right.motion.fade_in = Time::ZERO;
                right.motion.dissolve_in = Time::ZERO;
                right.motion.fade_out = right.motion.fade_out.min(right.duration);
                track.clips[index].motion.fade_out = Time::ZERO;
                track.clips[index].motion.fade_in =
                    track.clips[index].motion.fade_in.min(left_duration);
                track.clips[index].duration = left_duration;
                track.clips.insert(index + 1, right);
                return Ok(());
            }
        }
        Err(Error::NotFound)
    }
}
