//! Visibility strategies: how primary visibility is organised. `raycast` casts one ray per
//! pixel through the traversal kernel (`crate::traversal`); raster strategies
//! (`raster_wgsl`) draw the geometry component's faces and find each pixel's face by depth
//! testing; discovery (`quadtree`, `cluster`), `splat` and `columns` live in their back
//! ends for now.

pub mod drawer;

/// The face drawer's WGSL (vertex stage, `pixel_ray`, `face_plane_hit`, `face_hit`,
/// `fragment_hit`), for every back end that rasterises block faces. Fragment data comes
/// from flat `varyings` (plane, block) or from the face source's records.
pub fn faces_wgsl(varyings: bool) -> String {
    let fragment = if varyings { include_str!("faces_varyings.wgsl") } else { include_str!("faces_records.wgsl") };
    format!("{}\n{fragment}", include_str!("faces.wgsl"))
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
