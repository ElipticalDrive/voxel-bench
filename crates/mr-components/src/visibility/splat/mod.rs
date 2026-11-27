//! Visibility `splat` (point splatting in compute): each
//! point of geometry `points` (`crate::geometry::points`) is projected and, over the screen
//! rectangle of its block, `atomicMin`s the view depth where each pixel's ray enters the
//! block (a point under 2 px also covers the pixels it misses, at its centre's depth); a
//! second pass writes the winning point id (equal depths: the block last in position
//! order, so the points' array may be in any order and hold holes), and the common deferred pass is the resolve: it
//! decodes the id (ray-boxes the pixel against the winning block) and shades. One thread
//! splats a point up to `max_splat` px on a side; larger ones go to a list that a workgroup
//! per point splats. GPU only: [`GPU_WGSL`] (the root: entries `clear`, `splat_depth`,
//! `splat_big_depth`, `splat_id`, `splat_big_id`, and the record encoding of the deferred
//! pass, entry `shade`). Its `Camera` uniform (group 0, binding 0) is the GPU executor's
//! with `count` and `max_splat` as the last two words; its buffers at group 0, bindings 8-11
//! (points, depth, ids, the large points' list: a count, then ids).

pub const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// [`GPU_WGSL`] with the constants it reads (`CHUNK_BITS`).
pub fn gpu_wgsl() -> String {
    format!(
        "const CHUNK_BITS: u32 = {}u;\n{GPU_WGSL}",
        mr_core::chunk::CHUNK_BITS
    )
}

/// The points' read in [`GPU_WGSL`] with one binding.
const POINT_READ: &str = "fn point_at(i: u32) -> Point { return points[i]; }\n";

/// `own` (the splat shader) reading its points over several bindings: `points` (the first,
/// `per` points) and one more at each of `bindings` (none: `own` as it is).
pub fn paged(own: &str, bindings: &[u32], per: u64) -> String {
    if bindings.is_empty() {
        return own.to_string();
    }
    assert!(own.contains(POINT_READ), "splat's gpu.wgsl: `point_at` changed");
    let decls: String = bindings.iter().enumerate().map(|(i, b)| format!("@group(0) @binding({b}) var<storage, read> points{}: array<Point>;\n", i + 1)).collect();
    let cases: String = (1..=bindings.len()).map(|p| format!("        case {p}u: {{ return points{p}[at]; }}\n")).collect();
    let read = format!("let page = i / {per}u;\n    let at = i - page * {per}u;\n    switch page {{\n{cases}        default: {{ return points[at]; }}\n    }}");
    own.replace(POINT_READ, &format!("{decls}{}", POINT_READ.replace("return points[i];", &format!("\n    {read}\n"))))
}

/// The entry points in dispatch order: clear (per pixel), the depth splats (per point, then
/// per large point), the id splats (the same), the resolve (per pixel).
pub const ENTRIES: [&str; 6] = [
    "clear",
    "splat_depth",
    "splat_big_depth",
    "splat_id",
    "splat_big_id",
    "shade",
];

/// Workgroups of the large points' passes (each takes every `BIG_GROUPS`-th large point).
pub const BIG_GROUPS: u32 = 1024;
