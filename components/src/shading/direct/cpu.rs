//! CPU implementation of the `direct` shading component: the reference lighting model.
//! Unlit: the face texel. Lit: ambient, sun and the point lights near the point, each with
//! a shadow ray through the composed visibility walk (`occluded`).

use glam::{IVec3, Vec3};
use mr_core::lighting::{encode_srgb, point_light, srgb_to_linear, texel_coord};
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
}

/// Colour of a hit on `face` of `block` at camera-relative point `p` inside `cell`.
pub fn shade<'a, L, O>(ctx: &Ctx<'a, L, O>, block: u16, face: Face, cell: IVec3, p: Vec3, mode: RenderMode) -> [u8; 4]
where
    L: Fn(IVec3) -> Option<&'a [Light]>,
    O: Fn(Vec3, Vec3, f32) -> bool,
{
    let uv = face.uv(p - cell.as_vec3());
    let texel = ctx.blocks.texture_of(BlockId(block), face).texel(texel_coord(uv.x), texel_coord(uv.y));
    if mode == RenderMode::Unlit {
        return texel;
    }
    let albedo = Vec3::new(srgb_to_linear(texel[0]), srgb_to_linear(texel[1]), srgb_to_linear(texel[2]));
    let n = face.normal().as_vec3();
    let ps = p + n * 1e-3;
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
    encode_srgb(albedo * light * ctx.env.exposure)
}
