//! Draw components: how a set of block faces becomes the image (the face drawer). The faces come from the visibility strategy: every face of the geometry
//! component (`faces`, the rasterisers) or the face set discovery found (`quadtree`,
//! `cluster`). `raster` depth-tests and shades each fragment; `visbuffer` writes face ids
//! (an encoding of the hit record its face source decodes) and `gbuffer` the common packed
//! record (`pack_hit`), then the common deferred pass (`crate::shading`) shades each pixel
//! (`drawer::FaceDrawer`).

pub mod drawer;

/// The face drawer's WGSL (vertex stage, `pixel_ray`, `face_plane_hit`, `face_hit`,
/// `fragment_hit`), for every back end that rasterises block faces. Fragment data comes
/// from flat `varyings` (plane, block) or from the face source's records.
pub fn faces_wgsl(varyings: bool) -> String {
    let fragment = if varyings { include_str!("faces_varyings.wgsl") } else { include_str!("faces_records.wgsl") };
    format!("{}\n{fragment}", include_str!("faces.wgsl"))
}

/// The GPU draw components.
pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "raster" => include_str!("raster/gpu.wgsl"),
        "visbuffer" => concat!(include_str!("visbuffer/gpu.wgsl"), "\n", include_str!("../shading/deferred.wgsl")),
        "gbuffer" => concat!(include_str!("gbuffer/gpu.wgsl"), "\n", include_str!("../shading/deferred.wgsl"), "\n", include_str!("../shading/packed.wgsl")),
        _ => return None,
    })
}
