//! A pipeline's GPU code as modules (`mr_gpu::program`): every storage and
//! kernel instance is its own module, and so is each ray service (`occluded`, `strace`
//! through one kernel), so the primary and the secondary rays' storages and kernels sit side
//! by side without renaming. Back ends add the instances their graph has and compile their
//! own code (the root) against them.

use mr_gpu::program::{LIGHTING, Program, VOXELS};

use crate::graph::{Checked, Node, Site};
use crate::post::Tone;
use crate::storage::GpuStorage;
use crate::traversal;

/// The constants of a kernel instance: `LOD` (LOD chunks are streamed and walked) and, for
/// `incremental`, `EXACT` and `LEVEL_MASK` (its knobs; `levels` = the skip knobs' mask, bit
/// k = 2^k cubes allowed), and every loop's `MAX_STEPS`.
#[derive(Clone, Debug, PartialEq)]
pub struct KernelConsts {
    pub lod: bool,
    pub exact: bool,
    pub levels: u32,
    /// The loop's iteration limit: a constant, not a uniform (as a uniform it
    /// cost incremental 45 % of its GPU time on NVIDIA).
    pub max_steps: u32,
}

/// The executor's `max_steps` default.
pub const MAX_STEPS: u32 = 2048;

impl KernelConsts {
    /// The kernel's knobs at their defaults.
    pub fn defaults(lod: bool) -> Self {
        Self { lod, exact: true, levels: u32::MAX, max_steps: MAX_STEPS }
    }

    /// A kernel node's knobs (`exact`, `skip2`..`skip16`), the rest at their defaults.
    pub fn of(node: &Node, lod: bool) -> Self {
        let on = |k: &str| node.knobs.get(k).and_then(|v| v.as_bool());
        let skips = [(1, "skip2"), (2, "skip4"), (3, "skip8"), (4, "skip16")];
        let levels = if skips.iter().any(|(_, k)| on(k).is_some()) { skips.iter().fold(0, |m, &(i, k)| m | (on(k).unwrap_or(true) as u32) << i) } else { u32::MAX };
        Self { lod, exact: on("exact").unwrap_or(true), levels, max_steps: MAX_STEPS }
    }

    fn wgsl(&self) -> String {
        let mask = if self.levels == u32::MAX { "S_LEVELS".to_string() } else { format!("(S_LEVELS & {:#x}u)", self.levels) };
        format!("const LOD: bool = {};\nconst EXACT: bool = {};\nconst LEVEL_MASK: u32 = {mask} | 1u | (1u << S_TOP);\nconst MAX_STEPS: u32 = {}u;\n", self.lod, self.exact, self.max_steps)
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
/// `strace()` and `btrace()` for bounces.
pub fn rays(p: &mut Program, path: &str, kernel: &str) {
    let src = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n\
               fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit { return ctrace(o, d, max_t); }\n";
    p.add(path, format!("{src}{}", btrace_wgsl(false)), &[kernel, VOXELS]);
}

/// A walk kernel's `vox_transmit` (the shadow rays' light through see-through blocks, the
/// reference's `transmittance`): with `LAYERS` the layer walk from `p`, each see-through
/// surface (entry and exit faces) letting `1 - coverage` through, an opaque block or a LOD
/// cell none; else 0 or 1 by `vox_occluded`.
const TRANSMIT_WALK: &str = "fn vox_transmit(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> f32 {
#ifdef LAYERS
    var w = layer_walk(p, ctrace(p, d, max_t));
    var through = 1.0;
    for (var i = 0u; i < 1024u; i++) {
        let l = layer_next(&w, p, d, max_t);
        if (!l.ok) { break; }
        through *= 1.0 - layer_coverage(l, layer_surface(l, p, d));
        if (through <= 0.0) { return 0.0; }
    }
    return through;
#else
    return select(1.0, 0.0, vox_occluded(p, d, max_t));
#endif
}
";

/// A ray query's `vox_transmit`: it stops at the first surface (the acceleration
/// structures are opaque), so see-through blocks shadow fully.
const TRANSMIT_QUERY: &str = "fn vox_transmit(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> f32 { return select(1.0, 0.0, vox_occluded(p, d, max_t)); }\n";

/// `btrace()` (where a bounce lands, a `Bounced`) over the module's `strace()`; with
/// `meshes` the nearer of the voxel hit and the mesh triangles' (`mesh_trace`).
fn btrace_wgsl(meshes: bool) -> String {
    let mesh = if meshes {
        "    let m = mesh_trace(o, d, select(max_t, h.t, h.hit));\n    \
         if (m.tri != MESH_NONE) {\n        let s = mesh_surface(m.tri, o, d, m.t);\n        return Bounced(true, s.p, s.n, s.color);\n    }\n"
    } else {
        ""
    };
    format!(
        "fn btrace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Bounced {{\n    let h = strace(o, d, max_t);\n{mesh}    \
         var b: Bounced;\n    b.hit = h.hit;\n    \
         if (h.hit) {{\n        let s = surface_of(h, o, d);\n        b.p = s.p;\n        b.n = s.n;\n        \
         b.albedo = surface_texel(s.block, s.face, s.local);\n    }}\n    return b;\n}}\n"
    )
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

/// The walks of a pipeline's GPU ray-casting sites: one module per storage
/// node and one per kernel node, so sites bound to one node share it and sites bound to
/// their own each have theirs; every storage at its own bind group. Sites bound to a ray
/// query trace the acceleration structures built from `geometry`: their ray
/// services are virtual functions the root overrides (naga cannot write ray queries back
/// into a module).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Walks {
    pub storages: Vec<StorageInst>,
    pub kernels: Vec<KernelInst>,
    pub geometry: Option<QueryGeometry>,
    /// The post node's tone mapper, compiled into the shading's output.
    pub post: Tone,
    /// Every kernel's iteration limit (the executor's `max_steps`).
    pub max_steps: u32,
}

/// A module path for node `id` under `kind` (`kernel::shadow_kernel`).
fn node_path(kind: &str, id: &str) -> String {
    let id: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    format!("{kind}::n_{id}")
}

impl Walks {
    /// No walks yet, for graph `ck` (its post node's tone mapper).
    pub fn of(ck: &Checked) -> Self {
        Self { post: ck.tone(), max_steps: MAX_STEPS, ..Self::default() }
    }

    /// Sets every kernel's iteration limit (those added later too).
    pub fn set_max_steps(&mut self, n: u32) {
        self.max_steps = n;
        for k in &mut self.kernels {
            k.consts.max_steps = n;
        }
    }

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
        self.kernels.push(KernelInst { node: site.kernel.into(), component: n.component.clone(), storage: st, consts: KernelConsts { max_steps: self.max_steps, ..KernelConsts::of(n, lod) } });
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
    /// `strace()` and `btrace()` for bounces): a module over the kernel's walk, or for a
    /// ray query virtual functions the program's root overrides with the trace. With
    /// `meshes` the rays also meet the mesh triangles (the program has [`meshes`]'s
    /// mesh-surface module): `occluded` by either, `btrace` the nearer. With shadow rays
    /// also `transmit()`, the light they carry: with shader def `LAYERS` (transparency)
    /// through see-through blocks attenuated (the layer walk over a walk kernel; a ray query
    /// stops at the first surface), else `occluded`'s 0 or 1.
    pub fn services(&self, p: &mut Program, path: &str, k: usize, occluded: bool, strace: bool, meshes: bool) {
        const OCCLUDED: &str = "fn vox_occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool";
        const STRACE: &str = "fn strace(o: vec3<f32>, d: vec3<f32>, max_t: f32) -> Hit";
        let mut src = String::new();
        let kernel = self.kernel_path(k);
        let mut deps = vec![VOXELS];
        if self.is_query(k) {
            let mut over = String::new();
            if occluded {
                src += &format!("virtual {OCCLUDED} {{ return false; }}\n");
                src += TRANSMIT_QUERY;
                over += &format!("override {} {{ return g_rt_trace(p, d, max_t, true).hit; }}\n", OCCLUDED.replacen("fn ", &format!("fn {path}::"), 1));
            }
            if strace {
                src += &format!("virtual {STRACE} {{ var h: Hit; return h; }}\n");
                over += &format!("override {} {{ return g_rt_trace(o, d, max_t, false); }}\n", STRACE.replacen("fn ", &format!("fn {path}::"), 1));
            }
            p.root(over);
        } else {
            if occluded {
                src += &format!("{OCCLUDED} {{ return ctrace(p, d, max_t).hit; }}\n");
                src += &format!("#ifdef LAYERS\n{}#endif\n", crate::visibility::LAYER_WALK_WGSL);
                src += TRANSMIT_WALK;
            }
            if strace {
                src += &format!("{STRACE} {{ return ctrace(o, d, max_t); }}\n");
            }
            deps.insert(0, &kernel);
        }
        let mesh_occ = if meshes { "mesh_occluded(p, d, max_t) || " } else { "" };
        if occluded {
            src += &format!("fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool {{ return {mesh_occ}vox_occluded(p, d, max_t); }}\n");
            let mesh = if meshes { "    if (mesh_occluded(p, d, max_t)) { return 0.0; }\n" } else { "" };
            src += &format!("fn transmit(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> f32 {{\n{mesh}    return vox_transmit(p, d, max_t);\n}}\n");
        }
        if strace {
            src += &btrace_wgsl(meshes);
        }
        if meshes {
            deps.push(mr_gpu::meshes::MESH_SURFACE);
        }
        p.add(path, src, &deps);
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

    /// Adds every storage and kernel module and the post module to `p`.
    pub fn add_to(&self, p: &mut Program) {
        crate::post::add_to(self.post, p);
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
        use crate::{Cap, Kind::Gpu, Role, caps::provides};
        let why: Vec<String> = (self.storages.iter().filter(|s| !provides(Gpu, Role::Storage, &s.component, Cap::Lod)).map(|s| format!("storage `{}` (`{}`) lacks lod", s.component, s.node)))
            .chain(self.kernels.iter().filter(|k| k.storage.is_none() || !provides(Gpu, Role::Traversal, &k.component, Cap::Lod)).map(|k| format!("traversal `{}` (`{}`) lacks lod", k.component, k.node)))
            .collect();
        (!why.is_empty()).then(|| why.join("; "))
    }

}

/// The ray services of a shading node's sites through `walks`' kernels: shadow rays and
/// bounces (pathtrace's; without a bounce site of their own, through the shadow rays'
/// kernel, so a live switch to pathtrace finds them). Adds them to `p` and returns their
/// module paths.
pub fn shading_rays(p: &mut Program, walks: &Walks, shadow: usize, bounce: usize) -> Vec<String> {
    walks.services(p, "rays::shadow", shadow, true, false, false);
    walks.services(p, "rays::bounce", bounce, false, true, false);
    vec!["rays::shadow".to_string(), "rays::bounce".to_string()]
}

/// Adds mesh surfaces to `p` (after [`shading`]): the mesh-surface module (`mesh_surface`)
/// with the triangles at `group`, `binding`; [`SHADING_MESH`] (`shade_mesh`), which shades
/// them with the shading component `shading` lit through the mesh-shadow site's kernel
/// `mesh_shadow` (the program's shading instance when that is the shadow rays' kernel
/// `shadow`, else an instance of its own with a lighting of its own, its bounces the
/// shading's); and shader def `MESHES`, which compiles in the record's mesh capability.
/// With `rays` the shading's rays (`shadow`, `bounce`: [`shading_rays`]'s, replaced) and the
/// mesh-shadow site's meet the triangles too (meshes cast shadows and take bounces);
/// without, they see voxels only (programs whose stage lacks the triangles' binding).
/// Returns the modules' paths for the root's dependencies.
pub fn meshes(p: &mut Program, walks: &Walks, shading: &str, [shadow, bounce]: [usize; 2], mesh_shadow: usize, group: u32, binding: u32, rays: bool) -> Vec<String> {
    use mr_gpu::meshes::{MESH_SURFACE, surface_wgsl};
    p.add(MESH_SURFACE, surface_wgsl(group, binding), &[VOXELS, mr_gpu::program::COMMON]);
    if rays {
        walks.services(p, "rays::shadow", shadow, true, false, true);
        walks.services(p, "rays::bounce", bounce, false, true, true);
    }
    let shade = if mesh_shadow == shadow {
        SHADING
    } else {
        walks.services(p, "rays::mesh", mesh_shadow, true, false, rays);
        p.add("lighting::mesh", mr_gpu::voxels::LIGHTING_WGSL, &["rays::mesh", VOXELS, mr_gpu::program::POST]);
        shading_instance(p, "shading::meshlit", shading, &["rays::mesh", "rays::bounce"], "lighting::mesh");
        "shading::meshlit"
    };
    p.add(SHADING_MESH, crate::shading::SHADE_MESH_WGSL, &[shade, MESH_SURFACE]);
    p.define("MESHES");
    vec![SHADING_MESH.to_string(), MESH_SURFACE.to_string()]
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
/// Mesh surfaces through the shading component (`shade_mesh`; [`meshes`]).
pub const SHADING_MESH: &str = "shading::mesh";

pub fn shading(p: &mut Program, name: &str, rays: &[&str], group: u32, binding: u32) {
    p.add(STATE, crate::shading::state::ShadeState::wgsl(group, binding), &[]);
    shading_instance(p, SHADING, name, rays, LIGHTING);
}

/// An instance of shading component `name` at `path`: over the state, its rays and the
/// lighting module `lighting` (a second instance lights through other shadow rays).
fn shading_instance(p: &mut Program, path: &str, name: &str, rays: &[&str], lighting: &str) {
    let src = crate::shading::gpu_wgsl(name).unwrap_or_else(|| panic!("no GPU shading `{name}`"));
    let mut deps = vec![STATE];
    deps.extend(rays);
    deps.extend([lighting, VOXELS]);
    p.add(path, src, &deps);
}

/// The root's usual dependencies with a walk: what it casts and shades with.
pub fn root_deps<'a>(w: &'a Walk, extra: &[&'a str]) -> Vec<&'a str> {
    let mut d: Vec<&str> = extra.to_vec();
    d.extend([w.kernel.as_str(), w.storage.as_str(), w.rays.as_str(), LIGHTING, VOXELS, mr_gpu::program::COMMON]);
    d
}
