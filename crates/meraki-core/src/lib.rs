#![forbid(unsafe_code)]
use std::cmp::Ordering;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Invalid(&'static str),
    Overflow,
    NotFound,
    Conflict,
    Locked,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

macro_rules! ids {
    ($($name:ident),*) => { $(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u128);
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> { serializer.serialize_str(&self.0.to_string()) }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
                let text = <String as serde::Deserialize>::deserialize(deserializer)?;
                let value = text.parse::<u128>().map_err(serde::de::Error::custom)?;
                if text != value.to_string() { return Err(serde::de::Error::custom("noncanonical ID")); }
                Ok(Self(value))
            }
        }
    )* };
}
ids!(
    TimelineId,
    TrackId,
    ClipId,
    AssetId,
    GraphId,
    NodeId,
    LinkGroupId
);

/// Canonical rational seconds. No floating-point time in the editing model.
/// Numerator i64 and denominator u32 bound the representable domain; overflow is an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "TimeWire", into = "TimeWire")]
pub struct Time {
    num: i64,
    den: u32,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeWire {
    numerator: String,
    denominator: u32,
}
impl From<Time> for TimeWire {
    fn from(t: Time) -> Self {
        Self {
            numerator: t.num.to_string(),
            denominator: t.den,
        }
    }
}
impl TryFrom<TimeWire> for Time {
    type Error = Error;
    fn try_from(w: TimeWire) -> Result<Self> {
        let n = w
            .numerator
            .parse::<i64>()
            .map_err(|_| Error::Invalid("time numerator"))?;
        if n.to_string() != w.numerator {
            return Err(Error::Invalid("noncanonical time"));
        }
        Self::new(n, w.denominator)
    }
}
impl Time {
    pub const ZERO: Self = Self { num: 0, den: 1 };
    pub fn new(num: i64, den: u32) -> Result<Self> {
        Self::reduce(i128::from(num), i128::from(den))
    }
    fn reduce(num: i128, den: i128) -> Result<Self> {
        if den <= 0 {
            return Err(Error::Invalid("denominator"));
        }
        let (mut a, mut b) = (num.abs(), den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Ok(Self {
            num: i64::try_from(num / a).map_err(|_| Error::Overflow)?,
            den: u32::try_from(den / a).map_err(|_| Error::Overflow)?,
        })
    }
    pub fn numerator(self) -> i64 {
        self.num
    }
    pub fn denominator(self) -> u32 {
        self.den
    }
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Self::reduce(
            i128::from(self.num) * i128::from(rhs.den) + i128::from(rhs.num) * i128::from(self.den),
            i128::from(self.den) * i128::from(rhs.den),
        )
    }
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Self::reduce(
            i128::from(self.num) * i128::from(rhs.den) - i128::from(rhs.num) * i128::from(self.den),
            i128::from(self.den) * i128::from(rhs.den),
        )
    }
    /// Scale by a dimensionless rational (speed); zero/reverse supported mathematically,
    /// but the MVP Clip validator only accepts strictly positive playback speeds.
    pub fn scale(self, factor: Self) -> Result<Self> {
        Self::reduce(
            i128::from(self.num) * i128::from(factor.num),
            i128::from(self.den) * i128::from(factor.den),
        )
    }
}
impl Ord for Time {
    fn cmp(&self, rhs: &Self) -> Ordering {
        (i128::from(self.num) * i128::from(rhs.den))
            .cmp(&(i128::from(rhs.num) * i128::from(self.den)))
    }
}
impl PartialOrd for Time {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

/// Positive dimensionless playback speed. Reverse/freeze require a separate time-map model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "Time", into = "Time")]
pub struct PlaybackRate(Time);
impl From<PlaybackRate> for Time {
    fn from(r: PlaybackRate) -> Self {
        r.0
    }
}
impl TryFrom<Time> for PlaybackRate {
    type Error = Error;
    fn try_from(t: Time) -> Result<Self> {
        Self::new(
            u32::try_from(t.numerator()).map_err(|_| Error::Invalid("speed"))?,
            t.denominator(),
        )
    }
}
impl PlaybackRate {
    pub fn new(num: u32, den: u32) -> Result<Self> {
        if num == 0 {
            return Err(Error::Invalid("playback rate"));
        }
        Ok(Self(Time::new(i64::from(num), den)?))
    }
    pub fn ratio(self) -> Time {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RateWire", into = "RateWire")]
pub struct FrameRate {
    num: u32,
    den: u32,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RateWire {
    numerator: u32,
    denominator: u32,
}
impl From<FrameRate> for RateWire {
    fn from(r: FrameRate) -> Self {
        Self {
            numerator: r.num,
            denominator: r.den,
        }
    }
}
impl TryFrom<RateWire> for FrameRate {
    type Error = Error;
    fn try_from(w: RateWire) -> Result<Self> {
        Self::new(w.numerator, w.denominator)
    }
}
impl FrameRate {
    pub fn new(num: u32, den: u32) -> Result<Self> {
        if num == 0 || den == 0 {
            return Err(Error::Invalid("frame rate"));
        }
        let (mut a, mut b) = (num, den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Ok(Self {
            num: num / a,
            den: den / a,
        })
    }
    pub fn numerator(self) -> u32 {
        self.num
    }
    pub fn denominator(self) -> u32 {
        self.den
    }
    pub fn time_at_frame(self, index: i64) -> Result<Time> {
        Time::reduce(
            i128::from(index) * i128::from(self.den),
            i128::from(self.num),
        )
    }
}

/// A display label descriptor, NOT the source PTS nor the canonical project clock.
/// SMPTE string parsing/formatting and drop-frame label skipping are deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timecode {
    frame_number: i64,
    rate: FrameRate,
    drop_frame: bool,
}
impl Timecode {
    pub fn new(frame_number: i64, rate: FrameRate, drop_frame: bool) -> Result<Self> {
        if drop_frame && !(rate.den == 1001 && (rate.num == 30000 || rate.num == 60000)) {
            return Err(Error::Invalid("unsupported drop-frame rate"));
        }
        Ok(Self {
            frame_number,
            rate,
            drop_frame,
        })
    }
    pub fn frame_number(self) -> i64 {
        self.frame_number
    }
    pub fn rate(self) -> FrameRate {
        self.rate
    }
    pub fn drop_frame(self) -> bool {
        self.drop_frame
    }
    pub fn elapsed(self) -> Result<Time> {
        self.rate.time_at_frame(self.frame_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ntsc_and_audio_are_exact() {
        let frame = FrameRate::new(30000, 1001)
            .unwrap()
            .time_at_frame(1)
            .unwrap();
        assert_eq!(frame, Time::new(1001, 30000).unwrap());
        let samples = Time::new(16016, 48000).unwrap();
        assert_eq!(frame.scale(Time::new(10, 1).unwrap()).unwrap(), samples);
    }
    #[test]
    fn comparison_is_numeric_and_overflow_is_explicit() {
        assert!(Time::new(1, 2).unwrap() > Time::new(1, 3).unwrap());
        assert_eq!(
            Time::new(i64::MAX, 1)
                .unwrap()
                .checked_add(Time::new(1, 1).unwrap()),
            Err(Error::Overflow)
        );
        assert!(Time::new(1, 0).is_err());
        assert_eq!(Time::new(-2, 4).unwrap(), Time::new(-1, 2).unwrap());
    }
    #[test]
    fn drop_frame_does_not_change_elapsed_time() {
        let rate = FrameRate::new(30000, 1001).unwrap();
        assert_eq!(
            Timecode::new(1800, rate, true).unwrap().elapsed().unwrap(),
            rate.time_at_frame(1800).unwrap()
        );
        assert!(Timecode::new(1, FrameRate::new(24, 1).unwrap(), true).is_err());
    }
}
