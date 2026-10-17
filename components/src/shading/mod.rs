//! Shading components: the colour of a surface point, `shade_surface(s, d, lit, px)` on the
//! GPU (`s`: the `Surface` a visibility strategy found, ray walks via `surface_of`; `d`: the
//! view ray; `px`: the pixel, for per-pixel state such as random sequences and
//! accumulation) and `shade` on the CPU (`direct::cpu`). Lit / unlit is
//! the pass mode, not a component; a shading component decides how either is computed.
//!
//! Placement (PLAN step 12): inline, called where the strategy finds the hit, or deferred,
//! the common pass over the hit records the strategy wrote (`deferred.wgsl` on the GPU,
//! [`deferred`] on the CPU).

pub mod deferred;
pub mod direct {
    pub mod cpu;
}
pub mod state;

/// The common deferred shading pass (entry point `shade`; see the file for what it expects
/// of the record's encoding and of the back end).
pub const DEFERRED_WGSL: &str = include_str!("deferred.wgsl");
/// The deferred pass over the common packed record (`pack_hit`; the back end provides
/// `record_at`, its view and `view_surface`).
pub const DEFERRED_PACKED_WGSL: &str = concat!(include_str!("deferred.wgsl"), "\n", include_str!("packed.wgsl"));

pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "direct" => include_str!("direct/gpu.wgsl"),
        "pathtrace" => include_str!("pathtrace/gpu.wgsl"),
        _ => return None,
    })
}
