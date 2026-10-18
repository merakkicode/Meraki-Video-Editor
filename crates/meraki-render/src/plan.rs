use crate::RenderResult;
use meraki_core::{FrameRate, Time};
use meraki_timeline::Timeline;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimelineRange {
    pub start_frame: String,
    pub end_frame: String,
}
pub fn decimal_frame(s: &str) -> RenderResult<i64> {
    let v = s.parse::<i64>().map_err(|_| "Quadro inválido")?;
    if v < 0 || v.to_string() != s {
        return Err("Quadro deve ser inteiro decimal canônico não negativo".into());
    }
    Ok(v)
}
pub fn ceil_frames(t: Time, rate: FrameRate) -> RenderResult<i64> {
    let n = i128::from(t.numerator()) * i128::from(rate.numerator());
    let d = i128::from(t.denominator()) * i128::from(rate.denominator());
    i64::try_from((n + d - 1) / d).map_err(|_| "Duração excede limite".into())
}
pub fn timeline_end(t: &Timeline) -> Time {
    t.tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter_map(|c| c.end().ok())
        .max()
        .unwrap_or(Time::ZERO)
}
#[derive(Clone)]
pub struct FramePlan {
    pub start: Time,
    pub end: Time,
    pub rate: FrameRate,
    pub total: i64,
}
impl FramePlan {
    pub fn new(t: &Timeline, range: &TimelineRange, rate: FrameRate) -> RenderResult<Self> {
        let start = decimal_frame(&range.start_frame)?;
        let end = decimal_frame(&range.end_frame)?;
        if start >= end || end > ceil_frames(timeline_end(t), t.frame_rate)? {
            return Err("Intervalo de render vazio ou fora da timeline (Out exclusivo).".into());
        }
        let start = t
            .frame_rate
            .time_at_frame(start)
            .map_err(|e| e.to_string())?;
        let end = t.frame_rate.time_at_frame(end).map_err(|e| e.to_string())?;
        let total = ceil_frames(end.checked_sub(start).map_err(|e| e.to_string())?, rate)?;
        Ok(Self {
            start,
            end,
            rate,
            total,
        })
    }
    pub fn time(&self, index: i64) -> RenderResult<Time> {
        if index < 0 || index >= self.total {
            return Err("Índice de frame fora do render".into());
        }
        self.start
            .checked_add(self.rate.time_at_frame(index).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_ntsc_frame_iteration() {
        let rate = FrameRate::new(30000, 1001).unwrap();
        let p = FramePlan {
            start: Time::new(1001, 30).unwrap(),
            end: Time::new(2002, 30).unwrap(),
            rate,
            total: 1000,
        };
        let times = (0..p.total).map(|i| p.time(i).unwrap()).collect::<Vec<_>>();
        assert_eq!(times.len(), 1000);
        assert!(times
            .windows(2)
            .all(|w| w[1].checked_sub(w[0]).unwrap() == Time::new(1001, 30000).unwrap()));
        assert!(times[999] < p.end);
        assert!(p.time(1000).is_err());
        assert_eq!(
            ceil_frames(Time::new(1001, 30).unwrap(), rate).unwrap(),
            1000
        );
    }
    #[test]
    fn rejects_noncanonical_frame_numbers() {
        for n in ["-1", "01", "1.5", "90000000000000000000"] {
            assert!(decimal_frame(n).is_err());
        }
    }
}
