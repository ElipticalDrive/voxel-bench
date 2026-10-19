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

/// What a benchmark pass renders: the lighting mode, and whether meshes are drawn with
/// the voxel world (see [`crate::mesh`]). Written `unlit`, `lit`, `unlit+meshes`,
/// `lit+meshes`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PassMode {
    pub lighting: RenderMode,
    pub meshes: bool,
}

impl PassMode {
    pub const UNLIT: Self = Self { lighting: RenderMode::Unlit, meshes: false };
    pub const LIT: Self = Self { lighting: RenderMode::Lit, meshes: false };

    pub fn name(self) -> &'static str {
        match (self.lighting, self.meshes) {
            (RenderMode::Unlit, false) => "unlit",
            (RenderMode::Lit, false) => "lit",
            (RenderMode::Unlit, true) => "unlit+meshes",
            (RenderMode::Lit, true) => "lit+meshes",
        }
    }
}

impl From<RenderMode> for PassMode {
    fn from(lighting: RenderMode) -> Self {
        Self { lighting, meshes: false }
    }
}

impl TryFrom<String> for PassMode {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let (l, meshes) = match s.strip_suffix("+meshes") {
            Some(l) => (l, true),
            None => (s.as_str(), false),
        };
        let lighting = match l {
            "unlit" => RenderMode::Unlit,
            "lit" => RenderMode::Lit,
            _ => return Err(format!("unknown mode `{s}` (unlit, lit, unlit+meshes, lit+meshes)")),
        };
        Ok(Self { lighting, meshes })
    }
}

impl From<PassMode> for String {
    fn from(m: PassMode) -> String {
        m.name().to_string()
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
    srgb_to_linear_f(c as f32 / 255.0)
}

/// sRGB (0..1, continuous) to linear: for interpolated mesh vertex colours.
#[inline]
pub fn srgb_to_linear_f(c: f32) -> f32 {
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

/// Tone mapping: the post node's components (`mr_components::post`), linear
/// colour into display range before [`encode_srgb`]. GPU twins: `components/src/post`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// Clamp to 0..1 (no tone mapper).
    #[default]
    Clamp,
    /// Reinhard on luminance: `c / (1 + L)`, hue kept, highlights compressed without a limit.
    Reinhard,
    /// ACES filmic (Narkowicz's fit of the reference rendering transform): a toe and a
    /// shoulder, saturated highlights.
    Aces,
    /// No mapping, no limit: high dynamic range output. The output is a float image
    /// ([`PixelFormat::Rgba16F`]) holding the sRGB transfer of the linear colour extended
    /// past 1 (extended sRGB), for an HDR display to show; its 8-bit form ([`hdr_to_rgba8`])
    /// is `clamp`'s.
    Hdr,
}

impl Tone {
    pub const ALL: [Tone; 4] = [Tone::Clamp, Tone::Reinhard, Tone::Aces, Tone::Hdr];

    pub fn name(self) -> &'static str {
        match self {
            Tone::Clamp => "clamp",
            Tone::Reinhard => "reinhard",
            Tone::Aces => "aces",
            Tone::Hdr => "hdr",
        }
    }

    pub fn parse(name: &str) -> Option<Tone> {
        Tone::ALL.into_iter().find(|t| t.name() == name)
    }

    /// Linear colour `c` mapped into display range (still linear; 0..1 except `clamp`'s).
    #[inline(always)]
    pub fn map(self, c: Vec3) -> Vec3 {
        match self {
            Tone::Clamp | Tone::Hdr => c,
            Tone::Reinhard => c / (1.0 + c.dot(LUMA)),
            Tone::Aces => {
                let x = c.max(Vec3::ZERO);
                (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)
            }
        }
    }

    /// The output pixel of linear colour `c`: mapped, then sRGB-encoded (as the GPU's
    /// `display`; `hdr`'s 8-bit form).
    #[inline(always)]
    pub fn encode(self, c: Vec3) -> [u8; 4] {
        encode_srgb(self.map(c))
    }

    /// The output's pixel format.
    pub fn format(self) -> PixelFormat {
        if self == Tone::Hdr { PixelFormat::Rgba16F } else { PixelFormat::Rgba8 }
    }
}

/// How a frame's pixels are stored (top row first, tightly packed).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    /// RGBA8, sRGB-encoded: every tone mapper but `hdr`.
    #[default]
    Rgba8,
    /// RGBA16 float (little-endian halves), extended sRGB: the sRGB transfer of linear light
    /// continued past 1 (`hdr`); 0..1 is what RGBA8 holds, unquantised.
    Rgba16F,
}

impl PixelFormat {
    pub fn bytes(self) -> u32 {
        match self {
            PixelFormat::Rgba8 => 4,
            PixelFormat::Rgba16F => 8,
        }
    }
}

/// The extended sRGB transfer (`hdr`'s encoding): sRGB's curve continued past 1.
#[inline]
pub fn linear_to_srgb_extended(x: f32) -> f32 {
    let c = x.max(0.0);
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Its inverse: extended sRGB to linear light.
#[inline]
pub fn srgb_extended_to_linear(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// An `Rgba16F` frame's 8-bit form: clamped and quantised, the pixels `clamp` writes (up to
/// one step where the half's rounding crosses a quantisation boundary).
pub fn hdr_to_rgba8(px: &[u8]) -> Vec<u8> {
    static TABLE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| (0..=u16::MAX).map(|h| (half_to_f32(h).clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect());
    px.chunks_exact(2).map(|h| t[u16::from_le_bytes([h[0], h[1]]) as usize]).collect()
}

/// IEEE half to f32 (subnormals included; no NaN payloads).
#[inline]
pub fn half_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let man = (h & 0x3ff) as f32;
    sign * match exp {
        0 => man * 2f32.powi(-24),
        31 => if man == 0.0 { f32::INFINITY } else { f32::NAN },
        e => (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    }
}

/// Rec. 709 luminance weights.
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);
