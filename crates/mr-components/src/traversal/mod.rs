//! Traversal kernels: answer ray queries against a storage. A GPU kernel
//! provides `ctrace(o, d, max_t) -> Hit`, written only against the storage interface (the
//! capabilities it relies on are declared in the registry, `caps`). Visibility strategies
//! call it for primary rays (ray casting, discovery), shading for shadow and bounce rays
//! (`occluded`, `strace`). Each kernel instance is a shader module of its own over its
//! storage instance (`crate::shaders`). CPU kernels: `refloor::cpu` (ray packets) and
//! `incremental::cpu`.

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

#[cfg(test)]
mod tests {
    use crate::shaders::{self, KernelConsts};
    use mr_gpu::program::{COMMON, LIGHTING, Program, VOXELS};

    /// A primary walk plus a secondary walk of every other storage and kernel form one
    /// valid shader as modules, without renaming (as gpu-compute assembles them).
    #[test]
    fn secondary_walks_compile_next_to_the_primary() {
        let storages = ["bricks", "hier", "prefix", "distance", "rows", "tree64", "svo_dag", "clipmap"];
        let kernels = ["refloor", "incremental", "sphere", "rowrun"];
        for primary in storages {
            for secondary in storages {
                for k in kernels {
                    let mut p = Program::world(Some("shadow::rays"));
                    let w = shaders::walk(&mut p, "primary", &crate::storage::gpu(primary).unwrap(), 2, "refloor", &KernelConsts::defaults(false));
                    shaders::walk(&mut p, "shadow", &crate::storage::gpu(secondary).unwrap(), 4, k, &KernelConsts::defaults(false));
                    let root = "@group(0) @binding(0) var<storage, read_write> out: array<u32>;\n\
                                @compute @workgroup_size(8) fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
                                let h = ctrace(vec3<f32>(0.5), vec3<f32>(1.0, 0.0, 0.0), 100.0);\n\
                                out[id.x] = select(0u, 1u, h.hit) + select(0u, 2u, occluded(vec3<f32>(0.5), vec3<f32>(0.0, 1.0, 0.0), 100.0));\n}\n";
                    let m = p.naga(root, &[&w.kernel, "shadow::rays", LIGHTING, VOXELS, COMMON]).unwrap_or_else(|e| panic!("{primary} + refloor, shadows {secondary} + {k}: {e}"));
                    use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{primary} + refloor, shadows {secondary} + {k}: {e:?}"));
                }
            }
        }
    }
}
