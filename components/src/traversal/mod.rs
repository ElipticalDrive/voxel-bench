//! Traversal kernels (IDEAS §18): answer ray queries against a storage. A GPU kernel
//! provides `ctrace(o, d, max_t) -> Hit`, written only against the storage interface (the
//! capabilities it relies on are declared in the registry, `caps`). Visibility strategies
//! call it for primary rays (ray casting, discovery), shading for shadow and bounce rays
//! (`occluded`). CPU kernels: `refloor::cpu` (ray packets) and `incremental::cpu`.

pub mod cpu;
#[cfg(test)]
mod cpu_tests;
pub mod incremental {
    pub mod cpu;
}
pub mod refloor {
    pub mod cpu;
}

/// The GPU kernels that walk a storage.
pub fn gpu_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "refloor" => include_str!("refloor/gpu.wgsl"),
        "incremental" => include_str!("incremental/gpu.wgsl"),
        "sphere" => include_str!("sphere/gpu.wgsl"),
        "rowrun" => include_str!("rowrun/gpu.wgsl"),
        _ => return None,
    })
}

/// The kernels of the gpu-rt back end (hardware ray queries against its BLAS).
pub fn rt_wgsl(name: &str) -> Option<&'static str> {
    Some(match name {
        "rayquery" => include_str!("rayquery/gpu.wgsl"),
        _ => return None,
    })
}

/// Shadow rays as the composed walk (`ctrace`) answers them: `occluded()` for lighting.
pub const OCCLUDED_WGSL: &str = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n";

/// The constants of a kernel's knobs at their defaults, for back ends without those knobs
/// (gpu-compute sets them from its own).
fn default_consts(kernel: &str) -> &'static str {
    match kernel {
        "incremental" => "const EXACT: bool = true;\nconst LEVEL_MASK: u32 = S_LEVELS | 1u | (1u << S_TOP);\n",
        _ => "",
    }
}

/// Rays through a composed storage, for back ends whose primary visibility is not a ray
/// walk (raster, splat; discovery's rays use it too) and for passes outside the main
/// shader (meshes): the storage (`GpuStorage::source`, bind group 2) walked with `kernel`
/// (refloor walks LOD chunks with `lod`) as `ctrace`, plus `occluded()`.
pub fn occlusion_wgsl(storage: &crate::storage::GpuStorage, kernel: &str, lod: bool) -> String {
    let walk = gpu_wgsl(kernel).unwrap_or_else(|| panic!("no GPU kernel `{kernel}`"));
    format!("const LOD: bool = {lod};\n{}{}\n{walk}\n{OCCLUDED_WGSL}", default_consts(kernel), storage.source())
}
