use meraki_core::{Error, Result, TrackId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelMix {
    /// None is minus infinity; finite values are dBFS gain, not signal level.
    pub gain_db: Option<f32>,
    pub muted: bool,
    pub solo: bool,
    /// Low shelf 120 Hz, bell 1 kHz Q=0.707, high shelf 8 kHz.
    pub eq_db: [f32; 3],
}
impl Default for ChannelMix {
    fn default() -> Self {
        Self {
            gain_db: Some(0.),
            muted: false,
            solo: false,
            eq_db: [0.; 3],
        }
    }
}
impl ChannelMix {
    pub fn validate(&self) -> Result<()> {
        if self
            .gain_db
            .is_some_and(|g| !g.is_finite() || !(-60. ..=6.).contains(&g))
            || self
                .eq_db
                .iter()
                .any(|g| !g.is_finite() || !(-12. ..=12.).contains(g))
        {
            return Err(Error::Invalid("ganho ou EQ fora dos limites"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MixerSettings {
    pub tracks: BTreeMap<TrackId, ChannelMix>,
    pub master: ChannelMix,
}
