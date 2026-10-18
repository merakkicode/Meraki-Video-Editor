#![forbid(unsafe_code)]
mod journal;
pub use journal::*;
use meraki_core::Time;
use meraki_media::{Asset, AssetCatalog};
use meraki_timeline::{Timeline, TrackKind};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};
pub type ProjectResult<T> = Result<T, String>;
pub const SCHEMA_VERSION: u32 = 3;
pub const MAX_PROJECT_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub timeline: Timeline,
    pub assets: Vec<Asset>,
}
impl Project {
    /// Relink preserves editorial timing and IDs; reject replacements without required handles.
    pub fn relink(&self, asset: Asset) -> ProjectResult<Self> {
        let old = self
            .assets
            .iter()
            .find(|a| a.id == asset.id)
            .ok_or("Mídia ausente")?;
        if old.audio_only != asset.audio_only {
            return Err("Tipo de mídia incompatível".into());
        }
        let mut next = self.clone();
        for clip in next
            .timeline
            .tracks
            .iter_mut()
            .flat_map(|t| &mut t.clips)
            .filter(|c| c.motion.title.is_none() && c.source.asset == asset.id)
        {
            clip.source.stream_index = asset.stream_index;
            clip.source.available_duration = asset.duration;
        }
        let id = asset.id;
        *next.assets.iter_mut().find(|a| a.id == id).unwrap() = asset;
        next.validate()?;
        Ok(next)
    }
    pub fn new(timeline: Timeline, catalog: &AssetCatalog) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            timeline,
            assets: catalog.0.values().cloned().collect(),
        }
    }
    pub fn validate(&self) -> ProjectResult<AssetCatalog> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "Versão de projeto não suportada: {}",
                self.schema_version
            ));
        }
        self.timeline.validate().map_err(|e| e.to_string())?;
        let mut catalog = AssetCatalog::default();
        for asset in &self.assets {
            catalog.insert(asset.clone())?;
        }
        for clip in self.timeline.tracks.iter().flat_map(|t| &t.clips) {
            if clip.motion.title.is_some() {
                continue;
            }
            let asset = catalog
                .0
                .get(&clip.source.asset)
                .ok_or("Clipe referencia mídia ausente do catálogo")?;
            if clip.source.kind
                != (if asset.audio_only {
                    TrackKind::Audio
                } else {
                    TrackKind::Video
                })
                || clip.source.stream_index != asset.stream_index
                || clip.source.available_start != Time::ZERO
                || clip.source.available_duration != asset.duration
            {
                return Err("Referência de stream ou duração inconsistente".into());
            }
        }
        Ok(catalog)
    }
}
/// Blocking disk operation. Sibling temp + atomic replacement preserves the old file on failure.
pub fn save(path: &Path, project: &Project) -> ProjectResult<()> {
    project.validate()?;
    let bytes = serde_json::to_vec_pretty(project).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Projeto excede 16 MiB".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn load(path: &Path) -> ProjectResult<Project> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_PROJECT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Projeto excede 16 MiB".into());
    }
    let mut project: Project =
        serde_json::from_slice(&bytes).map_err(|e| format!("Projeto inválido: {e}"))?;
    if matches!(project.schema_version, 1 | 2) {
        project.schema_version = SCHEMA_VERSION;
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    for asset in &mut project.assets {
        if asset.path.is_relative() {
            asset.path = parent.join(&asset.path);
        }
    }
    project.validate()?;
    Ok(project)
}
#[cfg(test)]
mod tests {
    use super::*;
    use meraki_core::*;
    use meraki_timeline::*;
    use std::collections::BTreeMap;
    pub(crate) fn sample() -> Project {
        let asset = Asset {
            audio_only: false,
            id: AssetId(u128::MAX),
            path: "sample.mp4".into(),
            name: "sample.mp4".into(),
            width: 640,
            height: 360,
            preview_width: 640,
            preview_height: 360,
            frame_rate: FrameRate::new(30000, 1001).unwrap(),
            duration: Time::new(1001, 100).unwrap(),
            stream_index: 0,
            time_base: Time::new(1, 90000).unwrap(),
            start_time: Time::new(1, 1).unwrap(),
            codec: "h264".into(),
        };
        let clip = Clip {
            motion: Default::default(),
            id: ClipId(u128::MAX - 1),
            name: "Real".into(),
            transform: Default::default(),
            source: MediaSource {
                asset: asset.id,
                stream_index: 0,
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
        Project {
            schema_version: SCHEMA_VERSION,
            assets: vec![asset],
            timeline: Timeline {
                markers: Vec::new(),
                id: TimelineId(1),
                name: "Teste".into(),
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
                mixer: Default::default(),
                graphs: BTreeMap::new(),
            },
        }
    }
    #[test]
    fn legacy_project_defaults_to_unity_mixer() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["timeline"].as_object_mut().unwrap().remove("mixer");
        value["timeline"]["tracks"][0]["clips"][0]
            .as_object_mut()
            .unwrap()
            .remove("transform");
        value["assets"][0]
            .as_object_mut()
            .unwrap()
            .remove("audio_only");
        let project: Project = serde_json::from_value(value).unwrap();
        project.validate().unwrap();
        assert_eq!(project.timeline.mixer, MixerSettings::default());
        assert!(!project.assets[0].audio_only);
        assert_eq!(
            project.timeline.tracks[0].clips[0].transform,
            Transform::default()
        );
    }
    #[test]
    fn m6_motion_title_roundtrip_and_v1_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m6.meraki");
        let mut p = sample();
        p.assets[0].path = dir.path().join("sample.mp4");
        let c = &mut p.timeline.tracks[0].clips[0];
        c.motion.title = Some(Title::default());
        c.motion.keys.insert(
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
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), p);
        let mut legacy = serde_json::to_value(sample()).unwrap();
        legacy["schema_version"] = 1.into();
        legacy["timeline"]["tracks"][0]["clips"][0]
            .as_object_mut()
            .unwrap()
            .remove("motion");
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let migrated = load(&path).unwrap();
        assert_eq!(migrated.schema_version, SCHEMA_VERSION);
        assert_eq!(
            migrated.timeline.tracks[0].clips[0].motion,
            Motion::default()
        );
    }
    #[test]
    fn relink_is_atomic_preserves_timing_and_rejects_short_sources() {
        let p = sample();
        let mut a = p.assets[0].clone();
        a.path = std::env::temp_dir().join("moved/sample.mp4");
        let linked = p.relink(a.clone()).unwrap();
        assert_eq!(linked.timeline, p.timeline);
        assert_eq!(linked.assets[0].path, a.path);
        a.duration = Time::new(1, 100).unwrap();
        assert!(p.relink(a).is_err());
        assert_ne!(p.assets[0].path, linked.assets[0].path);
    }
    #[test]
    fn exact_roundtrip_and_atomic_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.meraki");
        let mut p = sample();
        p.assets[0].path = dir.path().join("sample.mp4");
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), p);
        p.timeline.name = "Editado".into();
        p.timeline.mixer.master.gain_db = Some(-12.);
        p.timeline.mixer.master.eq_db = [3., -2., 1.];
        p.timeline.tracks[0].clips[0].transform = Transform {
            rotation: 0.,
            scale: 0.5,
            x: 320.,
            y: 140.,
            opacity: 0.65,
        };
        p.timeline.tracks[0].clips[0].effects = Some(GraphId(1));
        p.timeline.graphs.insert(
            GraphId(1),
            meraki_graph::EffectSettings {
                contrast: 1.3,
                radius: 15.,
                blur_bypass: true,
                ..Default::default()
            }
            .graph(GraphId(1))
            .unwrap(),
        );
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), p);
    }
    #[test]
    fn rejects_version_missing_asset_and_bad_time() {
        let mut p = sample();
        p.schema_version = 99;
        assert!(p.validate().is_err());
        p.schema_version = SCHEMA_VERSION;
        p.assets.clear();
        assert!(p.validate().is_err());
        let mut v = serde_json::to_value(sample()).unwrap();
        v["timeline"]["tracks"][0]["clips"][0]["duration"]["denominator"] = 0.into();
        assert!(serde_json::from_value::<Project>(v).is_err());
    }
    #[test]
    fn split_move_and_reload_preserve_exact_source_mapping() {
        let mut p = sample();
        p.timeline
            .split_clip(ClipId(u128::MAX - 1), Time::new(5, 1).unwrap(), ClipId(2))
            .unwrap();
        p.timeline
            .move_clip(ClipId(2), Time::new(7, 1).unwrap())
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edited.meraki");
        save(&path, &p).unwrap();
        let loaded = load(&path).unwrap();
        let right = &loaded.timeline.tracks[0].clips[1];
        assert_eq!(right.source_in, Time::new(5, 1).unwrap());
        assert_eq!(
            right.source_at(Time::new(8, 1).unwrap()).unwrap(),
            Time::new(6, 1).unwrap()
        );
        assert_eq!(
            right.duration,
            p.assets[0]
                .duration
                .checked_sub(Time::new(5, 1).unwrap())
                .unwrap()
        );
    }
}
