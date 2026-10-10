//! Render modes and the reference lighting model.
//!
//! * [`RenderMode::Unlit`]: pixel = sRGB texel of the first face hit (nearest sampling,
//!   see [`crate::Face::uv`] and [`texel_coord`]); misses are [`Environment::sky`].
//!   This is strict: renderers should match the reference up to edge pixels.
//! * [`RenderMode::Lit`]: direct lighting. The reference computes, in linear space,
//!   `albedo * (ambient + sun + sum(point lights))` with hard shadows for the sun and every
//!   light (see [`point_light`]), then [`encode_srgb`]. This is a loose category: renderers
//!   may approximate (fewer shadows, baked light, GI, ...) and are scored by error.
//!
//! Lights are invisible: looking straight at one shows nothing.

use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    Unlit,
    Lit,
}

impl RenderMode {
    pub fn name(self) -> &'static str {
        match self {
            RenderMode::Unlit => "unlit",
            RenderMode::Lit => "lit",
        }
    }
}

/// A point light. It occupies no volume and is never visible itself.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Light {
    pub id: u32,
    pub position: DVec3,
    /// Linear RGB colour, multiplied by `intensity`.
    pub color: Vec3,
    pub intensity: f32,
    /// Distance beyond which the light contributes exactly nothing.
    pub radius: f32,
}

/// Directional light at infinity.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sun {
    /// Direction the light travels (pointing away from the sun). Normalised by the host.
    pub direction: Vec3,
    /// Linear RGB irradiance.
    pub color: Vec3,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    /// Colour of rays that hit nothing (linear RGB).
    pub sky: Vec3,
    /// Constant ambient term added to every lit surface (linear RGB).
    pub ambient: Vec3,
    pub sun: Option<Sun>,
    /// Multiplier applied before sRGB encoding in lit mode.
    pub exposure: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            sky: Vec3::new(0.45, 0.65, 0.95),
            ambient: Vec3::splat(0.08),
            sun: Some(Sun { direction: Vec3::new(-0.4, -1.0, -0.25).normalize(), color: Vec3::splat(1.0) }),
            exposure: 1.0,
        }
    }
}

/// Distance attenuation for point lights: inverse-square softened near the light, windowed
/// to reach exactly zero at `radius`.
#[inline]
pub fn attenuation(dist: f32, radius: f32) -> f32 {
    if dist >= radius {
        return 0.0;
    }
    let x = dist / radius;
    let w = (1.0 - x * x * x * x).clamp(0.0, 1.0);
    w * w / (dist * dist + 1.0)
}

/// Unshadowed contribution of a point light to a surface with normal `n` at `to_light`
/// (vector from surface to light).
#[inline]
pub fn point_light(n: Vec3, to_light: Vec3, light: &Light) -> Vec3 {
    let d = to_light.length();
    if d <= 1e-6 {
        return Vec3::ZERO;
    }
    let ndl = n.dot(to_light / d).max(0.0);
    light.color * (light.intensity * ndl * attenuation(d, light.radius))
}

/// Maps a texture coordinate in `[0, 1]` to a texel index.
#[inline]
pub fn texel_coord(u: f32) -> usize {
    ((u * crate::TEX_SIZE as f32) as i32).clamp(0, crate::TEX_SIZE as i32 - 1) as usize
}

#[inline]
pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Final encoding of a linear colour to an sRGB8 pixel (clamping, no tonemapper).
#[inline]
pub fn encode_srgb(c: Vec3) -> [u8; 4] {
    let q = |v: f32| (linear_to_srgb(v) * 255.0 + 0.5) as u8;
    [q(c.x), q(c.y), q(c.z), 255]
}
