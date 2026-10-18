use crate::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub id: String,
    pub at: Time,
    pub name: String,
    pub color: String,
}
impl Marker {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty()
            || self.id.len() > 128
            || self.at < Time::ZERO
            || self.name.len() > 1024
            || !["cyan", "violet", "yellow", "red"].contains(&self.color.as_str())
        {
            return Err(Error::Invalid("marker"));
        }
        Ok(())
    }
}

impl Timeline {
    /// Transaction primitive. The engine validates the complete candidate before publishing.
    /// Ripple shifts subsequent clips on this track only; other tracks remain anchored.
    pub fn trim_clip(&mut self, id: ClipId, head: bool, at: Time, ripple: bool) -> Result<()> {
        let track = self
            .tracks
            .iter_mut()
            .find(|t| t.clips.iter().any(|c| c.id == id))
            .ok_or(Error::NotFound)?;
        if track.locked {
            return Err(Error::Locked);
        }
        let index = track.clips.iter().position(|c| c.id == id).unwrap();
        let old = track.clips[index].clone();
        let old_end = old.end()?;
        let c = &mut track.clips[index];
        let delta = if head {
            let delta = at.checked_sub(old.timeline_start)?;
            c.timeline_start = if ripple { old.timeline_start } else { at };
            c.source_in = old.source_in.checked_add(delta.scale(old.speed.ratio())?)?;
            c.duration = old.duration.checked_sub(delta)?;
            // Shift the animation origin, preserving curves exactly when revealing handles.
            let offset = old.motion.offset.checked_add(delta)?;
            if offset < Time::ZERO {
                for keys in c.motion.keys.values_mut() {
                    for key in keys {
                        key.at = key.at.checked_sub(offset)?;
                    }
                }
                c.motion.offset = Time::ZERO;
            } else {
                c.motion.offset = offset;
            }
            Time::ZERO.checked_sub(delta)?
        } else {
            c.duration = at.checked_sub(old.timeline_start)?;
            at.checked_sub(old_end)?
        };
        c.validate()?;
        if ripple {
            for next in &mut track.clips {
                if next.id != id && next.timeline_start >= old_end {
                    next.timeline_start = next.timeline_start.checked_add(delta)?;
                }
            }
        }
        Ok(())
    }
}
