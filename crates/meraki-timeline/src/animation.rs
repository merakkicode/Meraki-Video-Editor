use crate::{Clip, Track, TrackKind, Transform};
use meraki_core::{Error, Result, Time};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub fn seconds(t: Time) -> f64 {
    t.numerator() as f64 / t.denominator() as f64
}
fn zero() -> Time {
    Time::ZERO
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Property {
    Scale,
    X,
    Y,
    Opacity,
    Volume,
    Rotation,
}
impl Property {
    pub fn bounds(self) -> (f32, f32) {
        match self {
            Self::Scale => (0.01, 4.),
            Self::X | Self::Y => (-16384., 16384.),
            Self::Opacity => (0., 1.),
            Self::Volume => (0., 2.),
            Self::Rotation => (-360., 360.),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Interpolation {
    #[default]
    Linear,
    Smooth,
}
/// Cubic Bézier (0,0), (1/3,0), (2/3,1), (1,1). x(u)=u exactly.
pub fn ease(u: f64, interpolation: Interpolation) -> f64 {
    let u = u.clamp(0., 1.);
    match interpolation {
        Interpolation::Linear => u,
        Interpolation::Smooth => u * u * (3. - 2. * u),
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyframe {
    pub at: Time,
    pub value: f32,
    pub interpolation: Interpolation,
}
pub fn evaluate(keys: &[Keyframe], at: Time, fallback: f32) -> f32 {
    let Some(first) = keys.first() else {
        return fallback;
    };
    if at <= first.at {
        return first.value;
    }
    let i = keys.partition_point(|k| k.at <= at);
    let a = &keys[i - 1];
    let Some(b) = keys.get(i) else { return a.value };
    // Subtract rational times before converting: avoid loss with large absolute timestamps.
    let u = seconds(at.checked_sub(a.at).expect("validated animation time"))
        / seconds(b.at.checked_sub(a.at).expect("ordered keys"));
    (a.value as f64 + (b.value - a.value) as f64 * ease(u, a.interpolation)) as f32
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Title {
    pub text: String,
    pub font: String,
    pub size: f32,
    pub color: [u8; 4],
    pub align: String,
    pub shadow: bool,
}
impl Default for Title {
    fn default() -> Self {
        Self {
            text: "Seu título".into(),
            font: "Segoe UI".into(),
            size: 84.,
            color: [255; 4],
            align: "center".into(),
            shadow: true,
        }
    }
}
impl Title {
    pub fn validate(&self) -> Result<()> {
        if self.text.chars().count() > 1024
            || self.text.contains('\0')
            || !self.size.is_finite()
            || !(8. ..=300.).contains(&self.size)
            || !["Segoe UI", "Arial", "Georgia"].contains(&self.font.as_str())
            || !["left", "center", "right"].contains(&self.align.as_str())
        {
            return Err(Error::Invalid("title settings"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Motion {
    #[serde(default = "zero")]
    pub offset: Time,
    #[serde(default)]
    pub keys: BTreeMap<Property, Vec<Keyframe>>,
    #[serde(default = "zero")]
    pub fade_in: Time,
    #[serde(default = "zero")]
    pub fade_out: Time,
    #[serde(default = "zero")]
    pub dissolve_in: Time,
    #[serde(default)]
    pub title: Option<Title>,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            offset: zero(),
            keys: BTreeMap::new(),
            fade_in: zero(),
            fade_out: zero(),
            dissolve_in: zero(),
            title: None,
        }
    }
}
impl Motion {
    pub fn validate(&self, duration: Time) -> Result<()> {
        let day = Time::new(86400, 1)?;
        if self.offset < zero() || self.offset > day || self.offset.checked_add(duration)? > day {
            return Err(Error::Invalid("animation range: maximum 24h"));
        }
        for (&p, keys) in &self.keys {
            let (min, max) = p.bounds();
            if keys.len() > 1024
                || keys.iter().any(|k| {
                    k.at < zero()
                        || k.at > day
                        || !k.value.is_finite()
                        || !(min..=max).contains(&k.value)
                })
                || keys.windows(2).any(|w| w[0].at >= w[1].at)
            {
                return Err(Error::Invalid("keyframe order/range"));
            }
        }
        for t in [self.fade_in, self.fade_out, self.dissolve_in] {
            if t < zero() || t > duration {
                return Err(Error::Invalid("transition duration"));
            }
        }
        if self.fade_in.checked_add(self.fade_out)? > duration {
            return Err(Error::Invalid("overlapping fades"));
        }
        if self.dissolve_in > zero() && self.fade_in > zero() {
            return Err(Error::Invalid("fade and dissolve on same edge"));
        }
        if let Some(t) = &self.title {
            t.validate()?;
        }
        Ok(())
    }
    pub fn value(&self, p: Property, relative: Time, base: f32) -> f32 {
        let time = relative.checked_add(self.offset).unwrap_or(relative);
        self.keys.get(&p).map_or(base, |k| evaluate(k, time, base))
    }
}
impl Clip {
    pub fn transform_at(&self, at: Time) -> Transform {
        let relative = at.checked_sub(self.timeline_start).unwrap_or(Time::ZERO);
        let mut t = self.transform;
        t.scale = self.motion.value(Property::Scale, relative, t.scale);
        t.x = self.motion.value(Property::X, relative, t.x);
        t.y = self.motion.value(Property::Y, relative, t.y);
        t.rotation = self.motion.value(Property::Rotation, relative, t.rotation);
        t.opacity = self.motion.value(Property::Opacity, relative, t.opacity);
        if self.motion.fade_in > zero() {
            t.opacity *= (seconds(relative) / seconds(self.motion.fade_in)).clamp(0., 1.) as f32;
        }
        if self.motion.fade_out > zero() {
            t.opacity *= (seconds(self.duration.checked_sub(relative).unwrap_or(zero()))
                / seconds(self.motion.fade_out))
            .clamp(0., 1.) as f32;
        }
        t
    }
    pub fn volume_at(&self, at: Time) -> f32 {
        self.motion.value(
            Property::Volume,
            at.checked_sub(self.timeline_start).unwrap_or(zero()),
            1.,
        )
    }
}
/// Centered dissolve on incoming clip; requires both half-handles. No implicit freeze frames.
pub fn validate_transitions(track: &Track) -> Result<()> {
    let mut clips: Vec<_> = track.clips.iter().collect();
    clips.sort_by_key(|c| c.timeline_start);
    for (i, c) in clips.iter().enumerate() {
        if c.motion.dissolve_in == zero() {
            continue;
        }
        if track.kind != TrackKind::Video || i == 0 {
            return Err(Error::Invalid("dissolve needs previous video clip"));
        }
        let prev = clips[i - 1];
        let half = c.motion.dissolve_in.scale(Time::new(1, 2)?)?;
        if prev.end()? != c.timeline_start
            || !prev.enabled
            || !c.enabled
            || prev.motion.fade_out > zero()
            || c.motion.dissolve_in > prev.duration
            || prev
                .motion
                .dissolve_in
                .scale(Time::new(1, 2)?)?
                .checked_add(half)?
                > prev.duration
        {
            return Err(Error::Invalid("dissolve adjacency/duration"));
        }
        if c.motion.title.is_none()
            && c.source_in.checked_sub(half.scale(c.speed.ratio())?)? < c.source.available_start
        {
            return Err(Error::Invalid("incoming source handle too short"));
        }
        if prev.motion.title.is_none()
            && prev
                .source_in
                .checked_add(prev.duration.checked_add(half)?.scale(prev.speed.ratio())?)?
                > prev
                    .source
                    .available_start
                    .checked_add(prev.source.available_duration)?
        {
            return Err(Error::Invalid("outgoing source handle too short"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linear_and_bezier_are_bounded_and_exact_at_keys() {
        let keys = vec![
            Keyframe {
                at: zero(),
                value: 1.,
                interpolation: Interpolation::Smooth,
            },
            Keyframe {
                at: Time::new(2, 1).unwrap(),
                value: 1.5,
                interpolation: Interpolation::Linear,
            },
        ];
        assert_eq!(evaluate(&keys, Time::new(1, 1).unwrap(), 0.), 1.25);
        assert_eq!(evaluate(&keys, Time::new(2, 1).unwrap(), 0.), 1.5);
        assert!((ease(0.25, Interpolation::Smooth) - 0.15625).abs() < 1e-12);
        assert_eq!(ease(-1., Interpolation::Smooth), 0.);
        assert_eq!(ease(2., Interpolation::Smooth), 1.);
    }
    #[test]
    fn rejects_duplicate_times_and_nonfinite_values() {
        let mut m = Motion::default();
        let k = Keyframe {
            at: zero(),
            value: 1.,
            interpolation: Interpolation::Linear,
        };
        m.keys.insert(Property::Scale, vec![k.clone(), k]);
        assert!(m.validate(Time::new(1, 1).unwrap()).is_err());
        m.keys.get_mut(&Property::Scale).unwrap().pop();
        m.keys.get_mut(&Property::Scale).unwrap()[0].value = f32::NAN;
        assert!(m.validate(Time::new(1, 1).unwrap()).is_err());
    }
}
