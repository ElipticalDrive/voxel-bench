//! Prepass components (gpu-compute): a stage before the visibility walk. It may run its own
//! compute entry point `prepass` (one thread per tile, `PREPASS_TILE` pixels square) and
//! provides `start_t(pixel) -> f32`, the distance each primary ray may skip. The back end
//! binds `beam` (one f32 per tile) and `beam_stats` (skipped-distance sum, tile count) in
//! group 0 for it.

pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "none" => "fn start_t(px: vec2<u32>) -> f32 { return 0.0; }\n",
        "beam" => include_str!("beam/gpu.wgsl"),
        _ => return None,
    })
}

/// Whether the prepass has a `prepass` entry point to dispatch.
pub fn has_pass(name: &str) -> bool {
    name == "beam"
}
