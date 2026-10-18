use meraki_core::{Error, Result};
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    #[serde(default)]
    pub rotation: f32,
    pub scale: f32,
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            rotation: 0.,
            scale: 1.,
            x: 0.,
            y: 0.,
            opacity: 1.,
        }
    }
}
impl Transform {
    pub fn validate(&self) -> Result<()> {
        if !self.rotation.is_finite()
            || self.rotation.abs() > 360.
            || !self.scale.is_finite()
            || !(0.01..=4.).contains(&self.scale)
            || !self.opacity.is_finite()
            || !(0. ..=1.).contains(&self.opacity)
            || !self.x.is_finite()
            || !self.y.is_finite()
            || self.x.abs() > 16384.
            || self.y.abs() > 16384.
        {
            return Err(Error::Invalid("clip transform"));
        }
        Ok(())
    }
    /// Canvas is 1920x1080 logical pixels. Returned [a,b,c,d,tx,ty] maps source edges to output edges.
    pub fn affine(&self, source: [u32; 2], output: [u32; 2]) -> [f32; 6] {
        let density = (output[0] as f32 / 1920.).min(output[1] as f32 / 1080.);
        let fit = (1920. / source[0] as f32).min(1080. / source[1] as f32);
        let scale = fit * self.scale * density;
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let (a, b, c, d) = (scale * cos, scale * sin, -scale * sin, scale * cos);
        [
            a,
            b,
            c,
            d,
            output[0] as f32 / 2. + self.x * density
                - (source[0] as f32 * a + source[1] as f32 * c) / 2.,
            output[1] as f32 / 2. + self.y * density
                - (source[0] as f32 * b + source[1] as f32 * d) / 2.,
        ]
    }
}
