//! A pipeline's GPU code as modules (PLAN step 9b, `mr_gpu::program`): every storage and
//! kernel instance is its own module, and so is each ray service (`occluded`, `strace`
//! through one kernel), so the primary and the secondary rays' storages and kernels sit side
//! by side without renaming. Back ends add the instances their graph has and compile their
//! own code (the root) against them.

use mr_gpu::program::{LIGHTING, Program, VOXELS};

use crate::storage::GpuStorage;
use crate::traversal;

/// The constants of a kernel instance: `LOD` (LOD chunks are streamed and walked) and, for
/// `incremental`, `EXACT` and `LEVEL_MASK` (its knobs; `levels` = the skip knobs' mask, bit
/// k = 2^k cubes allowed).
pub struct KernelConsts {
    pub lod: bool,
    pub exact: bool,
    pub levels: u32,
}

impl KernelConsts {
    /// The kernel's knobs at their defaults.
    pub fn defaults(lod: bool) -> Self {
        Self { lod, exact: true, levels: u32::MAX }
    }

    fn wgsl(&self) -> String {
        let mask = if self.levels == u32::MAX { "S_LEVELS".to_string() } else { format!("(S_LEVELS & {:#x}u)", self.levels) };
        format!("const LOD: bool = {};\nconst EXACT: bool = {};\nconst LEVEL_MASK: u32 = {mask} | 1u | (1u << S_TOP);\n", self.lod, self.exact)
    }
}

/// Adds storage instance `path`, its bindings at bind group `group`.
pub fn storage(p: &mut Program, path: &str, st: &GpuStorage, group: u32) {
    p.add(path, format!("const S_GROUP: u32 = {group}u;\n{}", st.source()), &[VOXELS]);
}

/// Adds kernel instance `path` (traversal component `kernel`) walking the storage module
/// `storage`; it provides `ctrace`.
pub fn kernel(p: &mut Program, path: &str, storage: &str, kernel: &str, consts: &KernelConsts) {
    let walk = traversal::gpu_wgsl(kernel).unwrap_or_else(|| panic!("no GPU kernel `{kernel}`"));
    p.add(path, format!("{}{walk}", consts.wgsl()), &[storage, VOXELS]);
}

/// Adds ray services `path` through kernel module `kernel`: `occluded()` for shadow rays,
/// `strace()` for bounces.
pub fn rays(p: &mut Program, path: &str, kernel: &str) {
    p.add(
        path,
        "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n\
         fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit { return ctrace(o, d, max_t); }\n",
        &[kernel, VOXELS],
    );
}

/// Module paths of one walk: a storage, a kernel over it, ray services through the kernel.
pub struct Walk {
    pub storage: String,
    pub kernel: String,
    pub rays: String,
}

impl Walk {
    pub fn at(prefix: &str) -> Self {
        Self { storage: format!("{prefix}::storage"), kernel: format!("{prefix}::kernel"), rays: format!("{prefix}::rays") }
    }
}

/// Adds one walk under `prefix` (`<prefix>::storage` at bind group `group`, `::kernel`,
/// `::rays`).
pub fn walk(p: &mut Program, prefix: &str, st: &GpuStorage, group: u32, k: &str, consts: &KernelConsts) -> Walk {
    let w = Walk::at(prefix);
    storage(p, &w.storage, st, group);
    kernel(p, &w.kernel, &w.storage, k, consts);
    rays(p, &w.rays, &w.kernel);
    w
}

/// The usual program of back ends whose rays all walk one storage (bind group 2) with one
/// kernel at its defaults: the world, the walk under `walk::` and lighting through its rays.
/// The root reaches `ctrace` through `walk::kernel`, `occluded` / `strace` through
/// `walk::rays`.
pub fn walk_program(st: &GpuStorage, k: &str, lod: bool) -> (Program, Walk) {
    let mut p = Program::world(Some("walk::rays"));
    let w = walk(&mut p, "walk", st, 2, k, &KernelConsts::defaults(lod));
    (p, w)
}

/// The shading state module (`sp`, `accum`; `shading::state`) at `group`, `binding`.
pub const STATE: &str = "mr::shade_state";

/// The shading component's module (`shade_surface`), with its state, lighting and the ray
/// services at `rays` (its shadow and bounce rays).
pub const SHADING: &str = "shading";

pub fn shading(p: &mut Program, name: &str, rays: &str, group: u32, binding: u32) {
    p.add(STATE, crate::shading::state::ShadeState::wgsl(group, binding), &[]);
    let src = crate::shading::gpu_wgsl(name).unwrap_or_else(|| panic!("no GPU shading `{name}`"));
    p.add(SHADING, src, &[STATE, rays, LIGHTING, VOXELS]);
}

/// The root's usual dependencies with a walk: what it casts and shades with.
pub fn root_deps<'a>(w: &'a Walk, extra: &[&'a str]) -> Vec<&'a str> {
    let mut d: Vec<&str> = extra.to_vec();
    d.extend([w.kernel.as_str(), w.storage.as_str(), w.rays.as_str(), LIGHTING, VOXELS, mr_gpu::program::COMMON]);
    d
}
