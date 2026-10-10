//! Visibility components: how a pixel's surface is found. The GPU ray walks provide
//! `ctrace(o, d, max_t) -> Hit`, written only against the storage interface; shadow rays
//! use the same walk (`occluded`). Raster visibility components (`raster_wgsl`) draw the
//! geometry component's faces instead and find each pixel's face by depth testing.

pub mod cpu;
#[cfg(test)]
mod cpu_tests;
pub mod incremental {
    pub mod cpu;
}
pub mod refloor {
    pub mod cpu;
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

/// Visibility components of the gpu-raster back end.
pub fn raster_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "raster" => include_str!("raster/gpu.wgsl"),
        "visbuffer" => include_str!("visbuffer/gpu.wgsl"),
        _ => return None,
    })
}
