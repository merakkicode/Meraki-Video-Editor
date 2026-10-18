use meraki_core::{ClipId, Error, Time};
use meraki_engine::{Edit, Snapshot};
use meraki_graph::{EffectGraph, Interpolation, Port, Signal, Value};
use meraki_timeline::{Clip, TrackKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeDto {
    pub numerator: String,
    pub denominator: u32,
}
impl From<Time> for TimeDto {
    fn from(t: Time) -> Self {
        Self {
            numerator: t.numerator().to_string(),
            denominator: t.denominator(),
        }
    }
}
pub fn unsigned<T: std::str::FromStr>(text: &str) -> Result<T, Error> {
    if text.is_empty()
        || !text.bytes().all(|b| b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(Error::Invalid("Use inteiro decimal canônico."));
    }
    text.parse()
        .map_err(|_| Error::Invalid("Inteiro fora do intervalo permitido."))
}
impl TryFrom<TimeDto> for Time {
    type Error = Error;
    fn try_from(t: TimeDto) -> Result<Self, Error> {
        let digits = t.numerator.strip_prefix('-').unwrap_or(&t.numerator);
        unsigned::<u64>(digits)?;
        if t.numerator == "-0" {
            return Err(Error::Invalid("Zero negativo não é canônico."));
        }
        let num = t
            .numerator
            .parse::<i64>()
            .map_err(|_| Error::Invalid("Tempo fora de i64."))?;
        Time::new(num, t.denominator)
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum EditDto {
    Trim {
        clip: String,
        head: bool,
        at: TimeDto,
        ripple: bool,
    },
    Marker {
        marker: meraki_timeline::Marker,
        remove: bool,
    },
    Motion {
        clip: String,
        motion: meraki_timeline::Motion,
    },
    Title {
        id: String,
        at: TimeDto,
    },
    Visual {
        clip: String,
        transform: meraki_timeline::Transform,
        effects: meraki_graph::EffectSettings,
        #[serde(default)]
        motion: Option<meraki_timeline::Motion>,
    },
    Mixer {
        track: Option<String>,
        channel: meraki_timeline::ChannelMix,
    },
    Move {
        clip: String,
        to: TimeDto,
    },
    Split {
        clip: String,
        at: TimeDto,
        #[serde(rename = "rightId")]
        right_id: String,
    },
}
impl TryFrom<EditDto> for Edit {
    type Error = Error;
    fn try_from(edit: EditDto) -> Result<Self, Error> {
        Ok(match edit {
            EditDto::Trim {
                clip,
                head,
                at,
                ripple,
            } => Edit::Trim {
                clip: ClipId(unsigned(&clip)?),
                head,
                at: at.try_into()?,
                ripple,
            },
            EditDto::Marker { marker, remove } => Edit::Marker { marker, remove },
            EditDto::Motion { clip, motion } => Edit::Motion {
                clip: ClipId(unsigned(&clip)?),
                motion,
            },
            EditDto::Title { id, at } => Edit::Title {
                id: ClipId(unsigned(&id)?),
                at: at.try_into()?,
            },
            EditDto::Visual {
                clip,
                transform,
                effects,
                motion,
            } => Edit::Visual {
                clip: ClipId(unsigned(&clip)?),
                transform,
                effects,
                motion,
            },
            EditDto::Mixer { track, channel } => Edit::Mixer {
                track: track
                    .map(|id| unsigned(&id).map(meraki_core::TrackId))
                    .transpose()?,
                channel,
            },
            EditDto::Move { clip, to } => Edit::Move {
                clip: ClipId(unsigned(&clip)?),
                to: to.try_into()?,
            },
            EditDto::Split { clip, at, right_id } => Edit::Split {
                clip: ClipId(unsigned(&clip)?),
                at: at.try_into()?,
                right_id: ClipId(unsigned(&right_id)?),
            },
        })
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandEnvelope {
    pub protocol_version: u32,
    pub request_id: String,
    pub expected_revision: String,
    pub edit: EditDto,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoEnvelope {
    pub protocol_version: u32,
    pub request_id: String,
    pub expected_revision: String,
}
pub fn validate_envelope(version: u32, request: &str) -> Result<(), Error> {
    if version != 1 || request.is_empty() || request.len() > 128 {
        return Err(Error::Invalid("Envelope IPC inválido."));
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    Conflict,
    Invalid,
    Locked,
    NotFound,
    Unavailable,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: String,
    pub revision: String,
}
impl IpcError {
    pub fn domain(error: Error, revision: u64) -> Self {
        let (code, message) = match error {
            Error::Conflict => (
                ErrorCode::Conflict,
                "O projeto mudou. O estado foi atualizado; tente novamente.".into(),
            ),
            Error::Locked => (ErrorCode::Locked, "Esta trilha está bloqueada.".into()),
            Error::NotFound => (
                ErrorCode::NotFound,
                "Item ou histórico não encontrado.".into(),
            ),
            Error::Invalid(detail) => (ErrorCode::Invalid, format!("Edição inválida: {detail}")),
            Error::Overflow => (
                ErrorCode::Invalid,
                "O resultado excede a precisão suportada.".into(),
            ),
        };
        Self {
            code,
            message,
            revision: revision.to_string(),
        }
    }
    pub fn unavailable() -> Self {
        Self {
            code: ErrorCode::Unavailable,
            message: "O motor não está disponível.".into(),
            revision: "0".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    pub protocol_version: u32,
    pub revision: String,
    pub timeline: Json,
    pub assets: Vec<meraki_media::Asset>,
}
fn kind(k: TrackKind) -> &'static str {
    match k {
        TrackKind::Video => "video",
        TrackKind::Audio => "audio",
    }
}
fn signal(s: &Signal) -> &'static str {
    match s {
        Signal::Image => "image",
        Signal::Mask => "mask",
        Signal::Scalar => "scalar",
    }
}
fn port(p: &Port) -> Json {
    json!({"node":p.node.0.to_string(), "socket":p.socket})
}
fn value(v: &Value) -> Json {
    match v {
        Value::Scalar(v) => json!({"kind":"scalar", "value":v}),
        Value::Vec2(v) => json!({"kind":"vec2", "value":v}),
        Value::Color(v) => json!({"kind":"color", "value":v}),
        Value::Bool(v) => json!({"kind":"bool", "value":v}),
    }
}
fn graph(g: &EffectGraph) -> Json {
    let nodes: Vec<Json> = g.nodes.values().map(|n| {
        let params: serde_json::Map<String, Json> = n.parameters.iter().map(|(k,p)| {
            let keys: Vec<Json> = p.keys.iter().map(|key| json!({"at":TimeDto::from(key.at), "value":value(&key.value), "interpolation":match key.interpolation { Interpolation::Hold => "hold", Interpolation::Linear => "linear" }})).collect();
            (k.clone(), json!({"value":value(&p.value), "keys":keys}))
        }).collect();
        json!({"id":n.id.0.to_string(), "effect":n.effect, "bypass":n.bypass, "inputs":n.inputs.iter().map(signal).collect::<Vec<_>>(), "outputs":n.outputs.iter().map(signal).collect::<Vec<_>>(), "parameters":params})
    }).collect();
    json!({"id":g.id.0.to_string(), "nodes":nodes, "edges":g.edges.iter().map(|e| json!({"from":port(&e.from),"to":port(&e.to)})).collect::<Vec<_>>(), "output":port(&g.output)})
}
fn clip(c: &Clip) -> Json {
    json!({"id":c.id.0.to_string(), "name":c.name, "transform":c.transform, "motion":c.motion,
        "source":{"asset":c.source.asset.0.to_string(),"streamIndex":c.source.stream_index,"kind":kind(c.source.kind),"availableStart":TimeDto::from(c.source.available_start),"availableDuration":TimeDto::from(c.source.available_duration)},
        "timelineStart":TimeDto::from(c.timeline_start),"duration":TimeDto::from(c.duration),"sourceIn":TimeDto::from(c.source_in),"speed":TimeDto::from(c.speed.ratio()),
        "enabled":c.enabled,"linkGroup":c.link_group.map(|id|id.0.to_string()),"effects":c.effects.map(|id|id.0.to_string())})
}
impl From<Snapshot> for SnapshotDto {
    fn from(s: Snapshot) -> Self {
        let t = &s.timeline;
        Self {
            protocol_version: 1,
            revision: s.revision.to_string(),
            assets: Vec::new(),
            timeline: json!({
                "id":t.id.0.to_string(),"name":t.name,"frameRate":{"numerator":t.frame_rate.numerator(),"denominator":t.frame_rate.denominator()},"audioSampleRate":t.audio_sample_rate,
                "tracks":t.tracks.iter().map(|t| json!({"id":t.id.0.to_string(),"name":t.name,"kind":kind(t.kind),"locked":t.locked,"enabled":t.enabled,"solo":t.solo,"clips":t.clips.iter().map(clip).collect::<Vec<_>>()})).collect::<Vec<_>>(),
                "mixer":t.mixer, "markers":t.markers,
                "graphs":t.graphs.values().map(graph).collect::<Vec<_>>()
            }),
        }
    }
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewViewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub device_pixel_ratio: f64,
    pub visible: bool,
}
impl PreviewViewport {
    pub fn validate(&self) -> Result<(), Error> {
        if ![
            self.x,
            self.y,
            self.width,
            self.height,
            self.device_pixel_ratio,
        ]
        .iter()
        .all(|v| v.is_finite())
            || self.width < 0.0
            || self.height < 0.0
            || self.device_pixel_ratio <= 0.0
            || self.device_pixel_ratio > 16.0
            || self.width > 32768.0
            || self.height > 32768.0
            || self.x.abs() > 1e6
            || self.y.abs() > 1e6
        {
            return Err(Error::Invalid("Viewport inválido."));
        }
        Ok(())
    }
}
pub use meraki_render::{ExportPresetDto, ExportSettingsDto, RenderJobDto, TimelineRange};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integers_do_not_pass_through_f64() {
        assert_eq!(unsigned::<u128>(&u128::MAX.to_string()).unwrap(), u128::MAX);
        for bad in ["1e3", "+1", "01", "-1", " 1", ""] {
            assert!(unsigned::<u128>(bad).is_err());
        }
        assert!(unsigned::<u64>("18446744073709551616").is_err());
        assert_eq!(
            Time::try_from(TimeDto {
                numerator: i64::MIN.to_string(),
                denominator: 1
            })
            .unwrap()
            .numerator(),
            i64::MIN
        );
        assert!(Time::try_from(TimeDto {
            numerator: "1".into(),
            denominator: 0
        })
        .is_err());
    }
    #[test]
    fn viewport_rejects_invalid_geometry() {
        assert!(PreviewViewport {
            width: f64::INFINITY,
            device_pixel_ratio: 1.0,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
