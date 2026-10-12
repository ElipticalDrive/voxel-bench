//! Draw components: how a set of block faces becomes the image (the face drawer, IDEAS
//! §16 step 2). The faces come from the visibility strategy: every face of the geometry
//! component (`faces`, the rasterisers) or the face set discovery found (`quadtree`,
//! `cluster`). `raster` depth-tests and shades each fragment; `visbuffer` writes face ids
//! and `gbuffer` hit points, then one compute pass shades each pixel (`drawer::FaceDrawer`).

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
        "visbuffer" => include_str!("visbuffer/gpu.wgsl"),
        "gbuffer" => include_str!("gbuffer/gpu.wgsl"),
        _ => return None,
    })
}
