//! Colour scales for diagnostic views (`view` knobs): what a view's colours mean, so a UI
//! can draw a legend and read values back from pixels, like a thermal camera.
//!
//! Numeric views map each pixel's value through one shared ramp (Turbo, a rainbow ramp
//! made for reading values) with [`ramp_wgsl`] on the GPU or [`Legend::color`] on the CPU;
//! both evaluate the same polynomial. Values above the scale's `max` are drawn white.
//! Renderers report the active legend with every frame ([`crate::FrameStats::legend`]).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Legend {
    /// Each pixel's value through the shared ramp: 0 at the bottom, `max` at the top
    /// (above `max`: white), on a log scale (`log2(1 + v)`) when `log`.
    Ramp { quantity: String, max: f64, log: bool },
    /// Fixed colours and what they mean; `None`: a note without a swatch.
    Categories(Vec<(String, Option<[u8; 3]>)>),
}

/// What a pixel of a ramp view says.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Reading {
    /// About this value (the ramp has 256 steps).
    Value(f64),
    /// Above the scale's maximum (white).
    Over,
}

impl Legend {
    pub fn ramp(quantity: &str, max: f64, log: bool) -> Self {
        Legend::Ramp { quantity: quantity.into(), max: max.max(1e-9), log }
    }

    pub fn categories(items: &[(&str, Option<[u8; 3]>)]) -> Self {
        Legend::Categories(items.iter().map(|(s, c)| (s.to_string(), *c)).collect())
    }

    /// Ramp position (0..1) of value `v`, above 1 when over the maximum.
    pub fn position(&self, v: f64) -> f64 {
        match self {
            Legend::Ramp { max, log: true, .. } => (1.0 + v.max(0.0)).log2() / (1.0 + max).log2(),
            Legend::Ramp { max, .. } => v.max(0.0) / max,
            Legend::Categories(_) => 0.0,
        }
    }

    /// Value at ramp position `t` (0..1).
    pub fn value_at(&self, t: f64) -> f64 {
        match self {
            Legend::Ramp { max, log: true, .. } => (1.0 + max).powf(t) - 1.0,
            Legend::Ramp { max, .. } => t * max,
            Legend::Categories(_) => 0.0,
        }
    }

    /// sRGB colour of value `v` (white above the maximum).
    pub fn color(&self, v: f64) -> [u8; 3] {
        let t = self.position(v);
        if t > 1.0 { [255; 3] } else { ramp_rgb8(t as f32) }
    }

    /// Reads a ramp view's pixel back: the nearest ramp colour (`None` if no ramp colour is
    /// close, e.g. an overlay blended with the image, or a category view).
    pub fn read(&self, rgb: [u8; 3]) -> Option<Reading> {
        if !matches!(self, Legend::Ramp { .. }) {
            return None;
        }
        if rgb == [255; 3] {
            return Some(Reading::Over);
        }
        let d2 = |c: [u8; 3]| (0..3).map(|i| (c[i] as i32 - rgb[i] as i32).pow(2)).sum::<i32>();
        let (best, dist) = (0..256).map(|i| (i, d2(ramp_rgb8(i as f32 / 255.0)))).min_by_key(|&(_, d)| d)?;
        (dist <= 48).then(|| Reading::Value(self.value_at(best as f64 / 255.0)))
    }
}

/// The shared ramp (Turbo, polynomial fit by Anton Mikhailov, Google 2019): `t` in 0..1 to
/// sRGB in 0..1, near-black through blue, cyan, green and yellow to dark red.
pub fn ramp(t: f32) -> [f32; 3] {
    let x = t.clamp(0.0, 1.0);
    let v4 = [1.0, x, x * x, x * x * x];
    let v2 = [v4[2] * v4[2], v4[3] * v4[2]];
    let dot = |k4: [f32; 4], k2: [f32; 2]| (k4[0] * v4[0] + k4[1] * v4[1] + k4[2] * v4[2] + k4[3] * v4[3] + k2[0] * v2[0] + k2[1] * v2[1]).clamp(0.0, 1.0);
    [dot(R4, R2), dot(G4, G2), dot(B4, B2)]
}

pub fn ramp_rgb8(t: f32) -> [u8; 3] {
    ramp(t).map(|c| (c * 255.0).round() as u8)
}

const R4: [f32; 4] = [0.135_721_38, 4.615_392_6, -42.660_32, 132.131_08];
const G4: [f32; 4] = [0.091_402_61, 2.194_188_4, 4.842_966_6, -14.185_033];
const B4: [f32; 4] = [0.106_673_3, 12.641_946, -60.582_05, 110.362_77];
const R2: [f32; 2] = [-152.942_4, 59.286_38];
const G2: [f32; 2] = [4.277_298_6, 2.829_566];
const B2: [f32; 2] = [-89.903_11, 27.348_25];

/// WGSL of the ramp: `ramp(t) -> vec3<f32>` and `ramp_value(v, max, log) -> vec4<f32>`
/// (opaque; white above `max`), the same mapping as [`Legend::color`].
pub fn ramp_wgsl() -> String {
    let v4 = |k: [f32; 4]| format!("vec4<f32>({:?}, {:?}, {:?}, {:?})", k[0], k[1], k[2], k[3]);
    let v2 = |k: [f32; 2]| format!("vec2<f32>({:?}, {:?})", k[0], k[1]);
    format!(
        "// Diagnostic colour ramp (mr_core::legend; Turbo).\n\
         fn ramp(t: f32) -> vec3<f32> {{\n\
         \x20   let x = clamp(t, 0.0, 1.0);\n\
         \x20   let v4 = vec4<f32>(1.0, x, x * x, x * x * x);\n\
         \x20   let v2 = v4.zw * v4.z;\n\
         \x20   return clamp(vec3<f32>(dot(v4, {}) + dot(v2, {}), dot(v4, {}) + dot(v2, {}), dot(v4, {}) + dot(v2, {})), vec3<f32>(0.0), vec3<f32>(1.0));\n\
         }}\n\
         fn ramp_value(v: f32, max_v: f32, log_scale: bool) -> vec4<f32> {{\n\
         \x20   let t = select(max(v, 0.0) / max_v, log2(1.0 + max(v, 0.0)) / log2(1.0 + max_v), log_scale);\n\
         \x20   if (t > 1.0) {{ return vec4<f32>(1.0); }}\n\
         \x20   return vec4<f32>(ramp(t), 1.0);\n\
         }}\n",
        v4(R4),
        v2(R2),
        v4(G4),
        v2(G2),
        v4(B4),
        v2(B2)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_reads_back() {
        let l = Legend::ramp("steps", 1000.0, true);
        for v in [0.0, 3.0, 40.0, 300.0, 999.0] {
            let Some(Reading::Value(r)) = l.read(l.color(v)) else { panic!("{v}") };
            // 256 steps over log2(1001) ≈ 10 doublings: within ~3% (+1 near zero).
            assert!((r - v).abs() <= 0.03 * v + 1.0, "{v} read as {r}");
        }
        assert_eq!(l.read(l.color(5000.0)), Some(Reading::Over));
        let lin = Legend::ramp("fragments", 16.0, false);
        let Some(Reading::Value(r)) = lin.read(lin.color(7.0)) else { panic!() };
        assert!((r - 7.0).abs() < 0.1, "{r}");
        assert_eq!(Legend::categories(&[("a", Some([1, 2, 3]))]).read([1, 2, 3]), None);
        // Near-black, then blue low and dark red at the top.
        assert!(ramp_rgb8(0.0).iter().all(|&c| c < 40));
        assert!(ramp_rgb8(0.1)[2] > ramp_rgb8(0.1)[0] && ramp_rgb8(1.0)[0] > ramp_rgb8(1.0)[2]);
        assert!(ramp_wgsl().contains("fn ramp_value"));
    }
}
