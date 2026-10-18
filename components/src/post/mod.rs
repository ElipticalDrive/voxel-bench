//! The post node (PLAN step 15b, IDEAS §18 part 8): what turns the shading's linear colour
//! into the output's display colour, written once for every pipeline. Today: tone mapping,
//! compiled into the shading's final encode (fused: no pass, no image of its own), on the
//! GPU as the `mr::post` module (`display`), on the CPU as [`Tone::encode`].
//!
//! It maps linear colours: lit radiance (after the environment's exposure) and the sky.
//! Unlit pixels are texels, already display colours, and pass through unmapped.

pub use mr_core::lighting::Tone;

/// The `mr::post` module of tone mapper `t` (`display(c) -> vec4<f32>`, as [`Tone::encode`]).
pub fn wgsl(t: Tone) -> &'static str {
    match t {
        Tone::Clamp => mr_gpu::program::POST_WGSL,
        Tone::Reinhard => include_str!("reinhard.wgsl"),
        Tone::Aces => include_str!("aces.wgsl"),
    }
}

/// Replaces the program's `mr::post` module (`Program::world` adds `clamp`'s) with `t`'s.
pub fn add_to(t: Tone, p: &mut mr_gpu::program::Program) {
    p.add(mr_gpu::program::POST, wgsl(t), &[mr_gpu::program::COMMON]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use mr_core::lighting::encode_srgb;

    #[test]
    fn tones() {
        let c = Vec3::new(0.2, 0.5, 4.0);
        assert_eq!(Tone::Clamp.encode(c), encode_srgb(c));
        for t in Tone::ALL {
            assert_eq!(Tone::parse(t.name()), Some(t));
            assert_eq!(t.map(Vec3::ZERO), Vec3::ZERO, "{t:?} keeps black");
        }
        // The curves compress: bright input stays near or below 1 (ACES's fit reaches 1.003
        // before the encode clamps) and keeps its order.
        for t in [Tone::Reinhard, Tone::Aces] {
            let (a, b) = (t.map(Vec3::splat(2.0)), t.map(Vec3::splat(8.0)));
            assert!(a.x < b.x && b.x <= 1.01, "{t:?}: {a} {b}");
        }
    }
}
