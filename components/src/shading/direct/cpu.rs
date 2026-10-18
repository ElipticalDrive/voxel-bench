//! CPU implementation of the `direct` shading component: the reference lighting model.
//! Unlit: the face texel. Lit: ambient, sun and the point lights near the point, each with
//! a shadow ray through the composed visibility walk (`occluded`).

use glam::{IVec3, Vec3};
use mr_core::lighting::{Tone, point_light, srgb_to_linear, texel_coord};
use mr_core::{BlockId, BlockRegistry, Environment, Face, Light, RenderMode};

/// What the shading needs from its back end.
pub struct Ctx<'a, L, O> {
    pub blocks: &'a BlockRegistry,
    pub env: &'a Environment,
    /// Absolute block of the camera; positions below are relative to it.
    pub cam: IVec3,
    pub sun_shadows: bool,
    pub light_shadows: bool,
    /// Point lights that may reach the absolute position (a coarse candidate list).
    pub lights_near: L,
    /// Whether a ray from `p` along unit `d` hits something within `dist`.
    pub occluded: O,
    /// The post's tone mapper, compiled into the output (PLAN step 15b).
    pub tone: Tone,
}

/// What shading reads from visibility (as the GPU's `Surface`): the camera-relative
/// surface point, its normal (the face's, unless a visibility strategy has a better one),
/// the point within the block (texture lookup), the block and its face.
#[derive(Copy, Clone, Debug)]
pub struct Surface {
    pub p: Vec3,
    pub n: Vec3,
    pub local: Vec3,
    pub block: u16,
    pub face: Face,
}

impl Surface {
    /// A ray walk's hit: point `p` on `face` of `block` in `cell` (camera-relative).
    #[inline]
    pub fn at_cell(block: u16, face: Face, cell: IVec3, p: Vec3) -> Self {
        Self { p, n: face.normal().as_vec3(), local: p - cell.as_vec3(), block, face }
    }
}

/// Colour of surface `s`.
pub fn shade<'a, L, O>(ctx: &Ctx<'a, L, O>, s: &Surface, mode: RenderMode) -> [u8; 4]
where
    L: Fn(IVec3) -> Option<&'a [Light]>,
    O: Fn(Vec3, Vec3, f32) -> bool,
{
    let uv = s.face.uv(s.local);
    let texel = ctx.blocks.texture_of(BlockId(s.block), s.face).texel(texel_coord(uv.x), texel_coord(uv.y));
    if mode == RenderMode::Unlit {
        return texel;
    }
    let albedo = Vec3::new(srgb_to_linear(texel[0]), srgb_to_linear(texel[1]), srgb_to_linear(texel[2]));
    let n = s.n;
    let ps = s.p + n * 1e-3;
    let mut light = ctx.env.ambient;
    if let Some(sun) = ctx.env.sun {
        let ndl = n.dot(-sun.direction);
        if ndl > 0.0 && (!ctx.sun_shadows || !(ctx.occluded)(ps, -sun.direction, 1e7)) {
            light += sun.color * ndl;
        }
    }
    let world = (ctx.cam.as_dvec3() + ps.as_dvec3()).floor().as_ivec3();
    if let Some(ls) = (ctx.lights_near)(world) {
        for l in ls {
            let to = (l.position - ctx.cam.as_dvec3()).as_vec3() - ps;
            let c = point_light(n, to, l);
            if c != Vec3::ZERO {
                let dist = to.length();
                if !ctx.light_shadows || !(ctx.occluded)(ps, to / dist, dist) {
                    light += c;
                }
            }
        }
    }
    ctx.tone.encode(albedo * light * ctx.env.exposure)
}
