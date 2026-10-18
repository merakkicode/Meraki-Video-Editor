use meraki_timeline::ChannelMix;
pub fn db_gain(db: Option<f32>) -> f32 {
    db.map_or(0., |d| 10f32.powf(d / 20.))
}
pub fn level_db(value: f32) -> f32 {
    (20. * value.max(1e-6).log10()).clamp(-120., 24.)
}
/// Offline encoder uses the same target biquad coefficients and channel gain.
pub fn ffmpeg_channel_filter(mix: &ChannelMix) -> String {
    let mut parts: Vec<String> = mix
        .eq_db
        .iter()
        .enumerate()
        .map(|(i, db)| {
            let [b0, b1, b2, a1, a2] = coefficients(i, *db);
            format!("biquad=b0={b0}:b1={b1}:b2={b2}:a0=1:a1={a1}:a2={a2}")
        })
        .collect();
    parts.push(format!(
        "volume={}",
        if mix.muted { 0. } else { db_gain(mix.gain_db) }
    ));
    parts.join(",")
}
/// Normalized RBJ coefficients [b0,b1,b2,a1,a2], 48 kHz.
pub fn coefficients(band: usize, db: f32) -> [f64; 5] {
    let a = 10f64.powf(f64::from(db) / 40.);
    let w = 2. * std::f64::consts::PI * [120., 1000., 8000.][band] / 48000.;
    let (sin, cos) = w.sin_cos();
    let alpha = sin / (2. * std::f64::consts::FRAC_1_SQRT_2);
    let beta = 2. * a.sqrt() * alpha;
    let (b0, b1, b2, a0, a1, a2) = match band {
        0 => (
            a * ((a + 1.) - (a - 1.) * cos + beta),
            2. * a * ((a - 1.) - (a + 1.) * cos),
            a * ((a + 1.) - (a - 1.) * cos - beta),
            (a + 1.) + (a - 1.) * cos + beta,
            -2. * ((a - 1.) + (a + 1.) * cos),
            (a + 1.) + (a - 1.) * cos - beta,
        ),
        1 => (
            1. + alpha * a,
            -2. * cos,
            1. - alpha * a,
            1. + alpha / a,
            -2. * cos,
            1. - alpha / a,
        ),
        _ => (
            a * ((a + 1.) + (a - 1.) * cos + beta),
            -2. * a * ((a - 1.) + (a + 1.) * cos),
            a * ((a + 1.) + (a - 1.) * cos - beta),
            (a + 1.) - (a - 1.) * cos + beta,
            2. * ((a - 1.) - (a + 1.) * cos),
            (a + 1.) - (a - 1.) * cos - beta,
        ),
    };
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}
pub fn response_db(eq: [f32; 3], hz: f64) -> f64 {
    let w = 2. * std::f64::consts::PI * hz / 48000.;
    eq.iter()
        .enumerate()
        .map(|(band, db)| {
            let [b0, b1, b2, a1, a2] = coefficients(band, *db);
            let power = |x0: f64, x1: f64, x2: f64| {
                (x0 + x1 * w.cos() + x2 * (2. * w).cos()).powi(2)
                    + (x1 * w.sin() + x2 * (2. * w).sin()).powi(2)
            };
            10. * (power(b0, b1, b2) / power(1., a1, a2)).log10()
        })
        .sum()
}
#[derive(Clone, Default)]
pub struct Biquad {
    z: [[f64; 2]; 2],
}
impl Biquad {
    pub fn process(&mut self, input: [f32; 2], c: [f64; 5]) -> [f32; 2] {
        let mut out = [0.; 2];
        for i in 0..2 {
            let x = f64::from(input[i]);
            let y = c[0] * x + self.z[i][0];
            self.z[i][0] = c[1] * x - c[3] * y + self.z[i][1];
            self.z[i][1] = c[2] * x - c[4] * y;
            // Avoid denormal tails on long silence.
            for z in &mut self.z[i] {
                if z.abs() < 1e-20 {
                    *z = 0.;
                }
            }
            out[i] = y as f32;
        }
        out
    }
}
#[derive(Clone)]
pub struct ChannelDsp {
    gain: f32,
    eq: [f32; 3],
    filters: [Biquad; 3],
}
impl Default for ChannelDsp {
    fn default() -> Self {
        Self {
            gain: 0.,
            eq: [0.; 3],
            filters: std::array::from_fn(|_| Biquad::default()),
        }
    }
}
impl ChannelDsp {
    /// Linear gain ramp over a block, EQ gain moves at most 0.25 dB per block.
    pub fn process(&mut self, frames: &mut [[f32; 2]], mix: &ChannelMix, audible: bool) {
        let target = if audible && !mix.muted {
            db_gain(mix.gain_db)
        } else {
            0.
        };
        let step = (target - self.gain) / frames.len().max(1) as f32;
        let coeff = std::array::from_fn::<_, 3, _>(|i| {
            self.eq[i] += (mix.eq_db[i] - self.eq[i]).clamp(-0.25, 0.25);
            coefficients(i, self.eq[i])
        });
        for frame in frames {
            for (filter, c) in self.filters.iter_mut().zip(coeff) {
                *frame = filter.process(*frame, c);
            }
            self.gain += step;
            for sample in frame {
                *sample *= self.gain;
            }
        }
        self.gain = target;
    }
}
pub fn mix_stereo(target: &mut [[f32; 2]], source: &[[f32; 2]]) {
    for (a, b) in target.iter_mut().zip(source) {
        a[0] += b[0];
        a[1] += b[1];
    }
}
pub fn levels(frames: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let mut peak = [0f32; 2];
    let mut sum = [0f64; 2];
    for f in frames {
        for c in 0..2 {
            peak[c] = peak[c].max(f[c].abs());
            sum[c] += f64::from(f[c]).powi(2);
        }
    }
    (
        peak.map(level_db),
        sum.map(|s| level_db((s / frames.len().max(1) as f64).sqrt() as f32)),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gain_and_stereo_sum() {
        assert_eq!(db_gain(None), 0.);
        assert!((db_gain(Some(6.)) - 1.9952623).abs() < 1e-6);
        let mut a = [[0.1, 0.2]; 2];
        mix_stereo(&mut a, &[[0.3, -0.1]; 2]);
        assert_eq!(a, [[0.4, 0.1]; 2]);
    }
    #[test]
    fn biquad_response_and_stability() {
        assert!(response_db([0.; 3], 1000.).abs() < 1e-8);
        assert!((response_db([0., 6., 0.], 1000.) - 6.).abs() < 0.01);
        assert!(response_db([6., 0., 0.], 20.) > 5.9);
        assert!(response_db([0., 0., 6.], 20000.) > 5.9);
        for band in 0..3 {
            for db in [-12., 12.] {
                let mut f = Biquad::default();
                let c = coefficients(band, db);
                for i in 0..48000 {
                    let y = f.process([if i == 0 { 1. } else { 0. }; 2], c);
                    assert!(y[0].is_finite() && y[0].abs() < 10.);
                }
            }
        }
    }
    #[test]
    fn gain_ramp_mute_and_rms() {
        let mut dsp = ChannelDsp::default();
        let mut f = [[0.5; 2]; 256];
        dsp.process(&mut f, &ChannelMix::default(), true);
        assert!(f[0][0] < 0.01);
        assert!((f[255][0] - 0.5).abs() < 1e-6);
        let (p, r) = levels(&[[0.5; 2]; 256]);
        assert!((p[0] + 6.0206).abs() < 0.001);
        assert_eq!(p, r);
        dsp.process(&mut f, &ChannelMix::default(), false);
        assert_eq!(f[255], [0.; 2]);
    }
}
