//! A pipeline's GPU code as modules (PLAN step 9b, `mr_gpu::program`): every storage and
//! kernel instance is its own module, and so is each ray service (`occluded`, `strace`
//! through one kernel), so the primary and the secondary rays' storages and kernels sit side
//! by side without renaming. Back ends add the instances their graph has and compile their
//! own code (the root) against them.

use mr_gpu::program::{LIGHTING, Program, VOXELS};

use crate::graph::{Checked, Node, Site};
use crate::storage::GpuStorage;
use crate::traversal;

/// The constants of a kernel instance: `LOD` (LOD chunks are streamed and walked) and, for
/// `incremental`, `EXACT` and `LEVEL_MASK` (its knobs; `levels` = the skip knobs' mask, bit
/// k = 2^k cubes allowed).
#[derive(Clone, Debug, PartialEq)]
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

    /// A kernel node's knobs (`exact`, `skip2`..`skip16`), the rest at their defaults.
    pub fn of(node: &Node, lod: bool) -> Self {
        let on = |k: &str| node.knobs.get(k).and_then(|v| v.as_bool());
        let skips = [(1, "skip2"), (2, "skip4"), (3, "skip8"), (4, "skip16")];
        let levels = if skips.iter().any(|(_, k)| on(k).is_some()) { skips.iter().fold(0, |m, &(i, k)| m | (on(k).unwrap_or(true) as u32) << i) } else { u32::MAX };
        Self { lod, exact: on("exact").unwrap_or(true), levels }
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

/// Adds the shadow-ray service `occluded()` at `path`, through kernel module `kernel`.
pub fn shadow_rays(p: &mut Program, path: &str, kernel: &str) {
    p.add(path, "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n", &[kernel, VOXELS]);
}

/// One storage instance of a pipeline (a graph node): its module at its own bind group.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageInst {
    pub node: String,
    pub component: String,
    pub group: u32,
}

/// One kernel instance (a graph node) over `storages[storage]`, or (`None`) a ray query
/// over the acceleration structures of [`Walks::geometry`].
#[derive(Clone, Debug, PartialEq)]
pub struct KernelInst {
    pub node: String,
    pub component: String,
    pub storage: Option<usize>,
    pub consts: KernelConsts,
}

/// The geometry node ray queries trace (its acceleration structures are the executor's).
#[derive(Clone, Debug, PartialEq)]
pub struct QueryGeometry {
    pub node: String,
    pub component: String,
}

/// The walks of a pipeline's GPU ray-casting sites (PLAN step 9c): one module per storage
/// node and one per kernel node, so sites bound to one node share it and sites bound to
/// their own each have theirs; every storage at its own bind group. Sites bound to a ray
/// query (step 13e4b) trace the acceleration structures built from `geometry`: their ray
/// services are virtual functions the root overrides (naga cannot write ray queries back
/// into a module).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Walks {
    pub storages: Vec<StorageInst>,
    pub kernels: Vec<KernelInst>,
    pub geometry: Option<QueryGeometry>,
}

/// A module path for node `id` under `kind` (`kernel::shadow_kernel`).
fn node_path(kind: &str, id: &str) -> String {
    let id: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    format!("{kind}::n_{id}")
}

impl Walks {
    /// Adds storage node `node` (once), at the next bind group of `groups`; its index.
    pub fn storage(&mut self, ck: &Checked, node: &str, groups: &[u32]) -> usize {
        if let Some(i) = self.storages.iter().position(|s| s.node == node) {
            return i;
        }
        let group = *groups.get(self.storages.len()).unwrap_or_else(|| panic!("more storages than bind groups ({groups:?})"));
        self.storages.push(StorageInst { node: node.into(), component: ck.node(node).component.clone(), group });
        self.storages.len() - 1
    }

    /// Adds the walk of `site` (its nodes once; a new storage at the next of `groups`),
    /// the kernel's constants from its knobs; the kernel's index.
    pub fn site(&mut self, ck: &Checked, site: Site, groups: &[u32], lod: bool) -> usize {
        if let Some(i) = self.kernels.iter().position(|k| k.node == site.kernel) {
            return i;
        }
        let n = ck.node(site.kernel);
        let st = if traversal::rt_wgsl(&n.component).is_some() {
            let g = QueryGeometry { node: site.storage.into(), component: ck.node(site.storage).component.clone() };
            match &self.geometry {
                Some(have) => assert!(*have == g, "ray queries over two geometry nodes (`{}`, `{}`): one acceleration structure per pipeline", have.node, g.node),
                None => self.geometry = Some(g),
            }
            None
        } else {
            Some(self.storage(ck, site.storage, groups))
        };
        self.kernels.push(KernelInst { node: site.kernel.into(), component: n.component.clone(), storage: st, consts: KernelConsts::of(n, lod) });
        self.kernels.len() - 1
    }

    /// Kernel `k` and what it walks or traces, for logs (`refloor over bricks`).
    pub fn label(&self, k: usize) -> String {
        let ki = &self.kernels[k];
        let over = ki.storage.map_or_else(|| self.geometry.as_ref().map_or("-", |g| g.component.as_str()).to_string(), |st| self.storages[st].component.clone());
        format!("{} over {over}", ki.component)
    }

    /// Kernel `k` is a ray query.
    pub fn is_query(&self, k: usize) -> bool {
        self.kernels[k].storage.is_none()
    }

    /// Adds ray services `path` through kernel `k` (`occluded()` for shadow rays and/or
    /// `strace()` for bounces): a module over the kernel's walk, or for a ray query virtual
    /// functions the program's root overrides with the trace.
    pub fn services(&self, p: &mut Program, path: &str, k: usize, occluded: bool, strace: bool) {
        const OCCLUDED: &str = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool";
        const STRACE: &str = "fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit";
        let mut src = String::new();
        if self.is_query(k) {
            let mut over = String::new();
            if occluded {
                src += &format!("virtual {OCCLUDED} {{ return false; }}\n");
                over += &format!("override {} {{ return g_rt_trace(p, d, max_t, true).hit; }}\n", OCCLUDED.replacen("fn ", &format!("fn {path}::"), 1));
            }
            if strace {
                src += &format!("virtual {STRACE} {{ var h: Hit; return h; }}\n");
                over += &format!("override {} {{ return g_rt_trace(o, d, max_t, false); }}\n", STRACE.replacen("fn ", &format!("fn {path}::"), 1));
            }
            p.add(path, src, &[VOXELS]);
            p.root(over);
        } else {
            if occluded {
                src += &format!("{OCCLUDED} {{ return ctrace(p, d, max_t).hit; }}\n");
            }
            if strace {
                src += &format!("{STRACE} {{ return ctrace(o, d, max_t); }}\n");
            }
            p.add(path, src, &[&self.kernel_path(k), VOXELS]);
        }
    }

    pub fn gpu_storage(&self, i: usize) -> GpuStorage {
        crate::storage::gpu(&self.storages[i].component).unwrap_or_else(|| panic!("no GPU storage `{}`", self.storages[i].component))
    }

    pub fn storage_path(&self, i: usize) -> String {
        node_path("storage", &self.storages[i].node)
    }

    pub fn kernel_path(&self, i: usize) -> String {
        node_path("kernel", &self.kernels[i].node)
    }

    /// Adds every storage and kernel module to `p`.
    pub fn add_to(&self, p: &mut Program) {
        for (i, s) in self.storages.iter().enumerate() {
            storage(p, &self.storage_path(i), &self.gpu_storage(i), s.group);
        }
        for (i, k) in self.kernels.iter().enumerate() {
            if let Some(st) = k.storage {
                kernel(p, &self.kernel_path(i), &self.storage_path(st), &k.component, &k.consts);
            }
        }
    }

    /// Every storage has LOD chunks and every kernel walks them, else why not.
    pub fn lod_blocker(&self) -> Option<String> {
        use crate::{Cap, Kind::Gpu, Slot, caps::provides};
        let why: Vec<String> = (self.storages.iter().filter(|s| !provides(Gpu, Slot::Storage, &s.component, Cap::Lod)).map(|s| format!("storage `{}` (`{}`) lacks lod", s.component, s.node)))
            .chain(self.kernels.iter().filter(|k| k.storage.is_none() || !provides(Gpu, Slot::Traversal, &k.component, Cap::Lod)).map(|k| format!("traversal `{}` (`{}`) lacks lod", k.component, k.node)))
            .collect();
        (!why.is_empty()).then(|| why.join("; "))
    }

    /// A separate program for one site's shadow rays (meshes' own pipelines): the world,
    /// kernel `k`'s walk with its storage at bind group 2, lighting through `occluded`.
    /// Returns the program and the shadow rays' module path.
    pub fn shadow_program(&self, k: usize) -> (Program, String) {
        let rays = "walk::rays".to_string();
        let mut p = Program::world(Some(&rays));
        let ki = &self.kernels[k];
        let st = ki.storage.expect("mesh shadows walk a storage");
        storage(&mut p, "walk::storage", &self.gpu_storage(st), 2);
        kernel(&mut p, "walk::kernel", "walk::storage", &ki.component, &ki.consts);
        shadow_rays(&mut p, &rays, "walk::kernel");
        (p, rays)
    }
}

/// The ray services of a shading node's sites through `walks`' kernels: shadow rays and
/// bounces (pathtrace's; without a bounce site of their own, through the shadow rays'
/// kernel, so a live switch to pathtrace finds them). Adds them to `p` and returns their
/// module paths.
pub fn shading_rays(p: &mut Program, walks: &Walks, shadow: usize, bounce: usize) -> Vec<String> {
    walks.services(p, "rays::shadow", shadow, true, false);
    walks.services(p, "rays::bounce", bounce, false, true);
    vec!["rays::shadow".to_string(), "rays::bounce".to_string()]
}

/// Adds mesh surfaces to `p` (PLAN step 12d): the mesh-surface module (`mesh_surface`,
/// `shade_mesh`) with the triangles at `group`, `binding`, lit through the mesh-shadow
/// site's kernel `mesh_shadow` (the program's lighting when that is the shadow rays' kernel
/// `shadow`, else a lighting instance of its own), and shader def `MESHES`, which compiles in
/// the record's mesh capability. Returns the module's path for the root's dependencies.
pub fn meshes(p: &mut Program, walks: &Walks, shadow: usize, mesh_shadow: usize, group: u32, binding: u32) -> String {
    use mr_gpu::meshes::{MESH_SURFACE, surface_wgsl};
    let lighting = if mesh_shadow == shadow {
        LIGHTING
    } else {
        walks.services(p, "rays::mesh", mesh_shadow, true, false);
        p.add("lighting::mesh", mr_gpu::voxels::LIGHTING_WGSL, &["rays::mesh", VOXELS]);
        "lighting::mesh"
    };
    p.add(MESH_SURFACE, surface_wgsl(group, binding), &[lighting, VOXELS, mr_gpu::program::COMMON]);
    p.define("MESHES");
    MESH_SURFACE.to_string()
}

/// The usual program of back ends whose root (`lead`) casts through a kernel and reads a
/// storage of `walks` (kernel, storage) and that shade with shadow (and bounce) rays through
/// theirs: the world, every walk of `walks`, the rays' services and lighting through the
/// shadow rays. Returns the program and the root's dependencies (nearest first: the root's
/// own kernel and storage).
pub fn sites_program(walks: &Walks, lead: Option<(usize, usize)>, shadow: usize, bounce: usize) -> (Program, Vec<String>) {
    let mut p = Program::world(Some("rays::shadow"));
    walks.add_to(&mut p);
    let mut deps: Vec<String> = lead.map_or(vec![], |(k, st)| vec![walks.kernel_path(k), walks.storage_path(st)]);
    deps.extend(shading_rays(&mut p, walks, shadow, bounce));
    deps.extend([LIGHTING, VOXELS, mr_gpu::program::COMMON].map(String::from));
    (p, deps)
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

pub fn shading(p: &mut Program, name: &str, rays: &[&str], group: u32, binding: u32) {
    p.add(STATE, crate::shading::state::ShadeState::wgsl(group, binding), &[]);
    let src = crate::shading::gpu_wgsl(name).unwrap_or_else(|| panic!("no GPU shading `{name}`"));
    let mut deps = vec![STATE];
    deps.extend(rays);
    deps.extend([LIGHTING, VOXELS]);
    p.add(SHADING, src, &deps);
}

/// The root's usual dependencies with a walk: what it casts and shades with.
pub fn root_deps<'a>(w: &'a Walk, extra: &[&'a str]) -> Vec<&'a str> {
    let mut d: Vec<&str> = extra.to_vec();
    d.extend([w.kernel.as_str(), w.storage.as_str(), w.rays.as_str(), LIGHTING, VOXELS, mr_gpu::program::COMMON]);
    d
}
