//! Visibility components: how a pixel's surface is found. The GPU ray walks provide
//! `ctrace(o, d, max_t) -> Hit`, written only against the storage interface; shadow rays
//! use the same walk (`occluded`). Raster visibility components (`raster_wgsl`) draw the
//! geometry component's faces instead and find each pixel's face by depth testing.

pub mod cpu;
pub mod drawer;
#[cfg(test)]
mod cpu_tests;
pub mod incremental {
    pub mod cpu;
}
pub mod refloor {
    pub mod cpu;
}

/// The face drawer's WGSL (vertex stage, `pixel_ray`, `face_plane_hit`, `face_hit`,
/// `fragment_hit`), for every back end that rasterises block faces. Fragment data comes
/// from flat `varyings` (plane, block) or from the face source's records.
pub fn faces_wgsl(varyings: bool) -> String {
    let fragment = if varyings { include_str!("faces_varyings.wgsl") } else { include_str!("faces_records.wgsl") };
    format!("{}\n{fragment}", include_str!("faces.wgsl"))
}

pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "refloor" => include_str!("refloor/gpu.wgsl"),
        "incremental" => include_str!("incremental/gpu.wgsl"),
        "sphere" => include_str!("sphere/gpu.wgsl"),
        "rowrun" => include_str!("rowrun/gpu.wgsl"),
        _ => return None,
    })
}

/// Shadow rays as the composed walk (`ctrace`) answers them: `occluded()` for lighting.
pub const OCCLUDED_WGSL: &str = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n";

/// Secondary rays (shadow and bounce rays) through a composed storage, for back ends whose
/// primary visibility is not a ray walk (raster, discovery, splat) and for passes outside
/// the main shader (meshes): the storage (`GpuStorage::source`, bind group 2) walked with
/// `refloor` (over LOD chunks with `lod`) as `ctrace`, plus `occluded()`.
pub fn occlusion_wgsl(storage: &crate::storage::GpuStorage, lod: bool) -> String {
    format!("const LOD: bool = {lod};\n{}\n{}\n{OCCLUDED_WGSL}", storage.source(), include_str!("refloor/gpu.wgsl"))
}

/// Visibility components of the gpu-rt back end.
pub fn rt_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "rayquery" => include_str!("rayquery/gpu.wgsl"),
        _ => return None,
    })
}

/// Visibility components of the gpu-raster back end.
pub fn raster_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "raster" => include_str!("raster/gpu.wgsl"),
        "visbuffer" => include_str!("visbuffer/gpu.wgsl"),
        "gbuffer" => include_str!("gbuffer/gpu.wgsl"),
        _ => return None,
    })
}
