//! The walking lane (`gpu-compute`; the primary rays' kernel API `compute`): a renderer from
//! components (`mr-components`, IDEAS §12): a **storage**, a **traversal** kernel walked by
//! ray casting (the visibility strategy, `raycast`) and a **shading** component, each a WGSL
//! module from `components/`, composed into one shader, so the compiler optimises across them
//! and the split costs nothing at run time.
//!
//! Storages answer the interface in `mr_components::storage`; optional capabilities the
//! chosen storage lacks come from adapters (`mr_components::caps`), so every storage works
//! with every traversal kernel. Shadow rays use the same composed walk, or their sites' own.
//! The storages' own data (per-slot side data, or region blobs) is kept by
//! `storage::gpu_data::StorageSet`. Meshes (mesh mode) are a surface source:
//! `mr_gpu::meshes::MeshRecords` rasterises their hit records first, rays stop at them, and
//! the walk or the deferred pass shades them.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_components::shaders::{KernelInst, Walks};
use mr_components::shading::state as shade_state;
use mr_components::storage::gpu_data::Bound;
use mr_components::{Cap, Slot, caps, components, names_on, placement_param, require};
use mr_core::*;
use mr_gpu::meshes::{MESH_RECORDS_WGSL, MeshRecords};
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

use super::{Before, Cast, CastProgram, Passes, Ticket};
use crate::{Core, CoreSpec, LANE_BINDING};

pub(crate) const BACKEND: &str = "gpu-compute";

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "GPU-compute back end: renders a composition of components (storage, traversal, shading) fused into one compute shader".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "modular".into(), "wgpu".into(), "meshes".into(), "lod".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        // LOD with pool storages and refloor (cells 2x coarser per level; was gpu-dda-lod).
        max_lod: 6,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

fn describe(slot: Slot) -> String {
    components(slot).filter(|c| mr_components::unavailable("gpu-compute", slot, c.name).is_none()).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("storage", &format!("Storage component. {}", describe(Slot::Storage)), &names_on(BACKEND, Slot::Storage), "bricks", false),
        ParamSpec::choice("visibility", &format!("Visibility strategy. {}", describe(Slot::Visibility)), &names_on(BACKEND, Slot::Visibility), "raycast", false),
        ParamSpec::choice("traversal", &format!("Traversal kernel (any storage works with any; missing fast paths come from adapters). {}", describe(Slot::Traversal)), &names_on(BACKEND, Slot::Traversal), "refloor", true),
        ParamSpec::choice("shadow_storage", "What shadow and bounce rays walk: `same` (the primary storage) or another storage (its data is kept as well)", &names_on(BACKEND, Slot::ShadowStorage), "same", false),
        ParamSpec::choice("shadow_traversal", "The kernel of shadow and bounce rays: `same` (the primary kernel) or another", &names_on(BACKEND, Slot::ShadowTraversal), "same", false),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names_on(BACKEND, Slot::Shading), "direct", true),
        ParamSpec::choice("prepass", &format!("Prepass component (a stage before the walk). {}", describe(Slot::Prepass)), &names_on(BACKEND, Slot::Prepass), "none", true),
        ParamSpec::choice("meshes", &format!("Meshes component (mesh mode). {}", describe(Slot::Meshes)), &names_on(BACKEND, Slot::Meshes), "composite", true),
        ParamSpec::int("tile", "beam prepass: tile size in pixels (1 = no prepass)", 1, 64, 8, true).of("prepass", &["beam"]),
    ];
    p.extend(shade_state::params());
    p.push(placement_param(BACKEND, "inline"));
    p.push(ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell after each move (no float drift) instead of accumulating them", true, true).of("traversal", &["incremental"]));
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them"), true, true).of("traversal", &["incremental"]));
    }
    p.push(ParamSpec::choice("dispatch", "How pixels are run: `compute` (one compute thread per pixel) or `fragment` (a fragment shader over a full-screen triangle; was gpu-frag-dda)", &["compute", "fragment"], "compute", true));
    p.push(ParamSpec::choice("view", "image: the rendered scene. steps: primary-ray loop iterations per pixel on a colour scale (log; white above view_max)", &["image", "steps"], "image", true));
    p.push(ParamSpec::int("view_max", "steps view: the top of the colour scale (iterations)", 1, 16384, 256, true));
    p.extend([
        shadows_param(),
        ParamSpec::int("max_steps", "Loop iteration limit per ray", 32, 16384, 2048, true),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB); for region storages (tree64) the limit of their own data", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]);
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    visibility: String,
    traversal: String,
    shadow_storage: String,
    shadow_traversal: String,
    shading: String,
    placement: String,
    prepass: String,
    meshes: String,
    tile: u32,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    exact: bool,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    view: String,
    view_max: u32,
    dispatch: String,
    shadows: Shadows,
    pool_mb: u32,
    max_steps: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            storage: "bricks".into(),
            visibility: "raycast".into(),
            traversal: "refloor".into(),
            shadow_storage: "same".into(),
            shadow_traversal: "same".into(),
            shading: "direct".into(),
            placement: "inline".into(),
            prepass: "none".into(),
            meshes: "composite".into(),
            tile: 8,
            bounces: 1,
            spp: 1,
            accumulate: true,
            exact: true,
            skip2: true,
            skip4: true,
            skip8: true,
            skip16: true,
            view: "image".into(),
            view_max: 256,
            dispatch: "compute".into(),
            shadows: Shadows::All,
            pool_mb: 2048,
            max_steps: 2048,
            frames_in_flight: 3,
        }
    }
}

/// The compile-time part of a composition: everything baked into the shader.
#[derive(Clone, PartialEq, Debug)]
struct Composition {
    /// The ray-casting sites' storages and kernels (PLAN step 9c): kernel 0 is the primary
    /// rays' (its constants from this back end's knobs); the other sites' kernels are their
    /// own nodes, or the primary's when the graph binds them to it.
    walks: Walks,
    /// Kernel of the shadow, bounce (pathtrace; else the shadow rays') and mesh-shadow rays.
    shadow: usize,
    bounce: usize,
    mesh_shadow: usize,
    prepass: String,
    /// Prepass tile size in pixels (1 = the prepass is skipped).
    tile: u32,
    /// `view = "steps"`: write iteration counts instead of colour.
    steps_view: bool,
    /// LOD chunks are streamed and walked (fixed at creation).
    lod: bool,
    /// `dispatch = "fragment"`: pixels in a fragment shader over a full-screen triangle.
    fragment: bool,
    /// The meshes stage draws meshes (`composite`): frames with mesh records run a variant
    /// with their records and the mesh capability of the walk's compiled in (`MESHES`).
    meshes: bool,
}

/// Bind groups of the storage instances: the primary rays' at 2 (3 is the meshes'), the
/// other sites' own after it.
const STORAGE_GROUPS: &[u32] = &[2, 4, 5, 6];

impl Composition {
    /// The composition of graph `g` with this back end's knobs `opts` (LOD if `max_lod`
    /// levels are streamed and every walk handles them).
    fn of(g: &graph::Checked, opts: &Options, max_lod: u8) -> anyhow::Result<Self> {
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        // Every site's walk; the primary kernel's constants are this back end's knobs.
        let mut walks = Walks::default();
        let site = |role, port| g.site(role, port).ok_or_else(|| anyhow::anyhow!("gpu-compute: {} `{port}` rays are not bound", role.name()));
        let primary = walks.site(&g, site(Role::Visibility, "primary")?, STORAGE_GROUPS, false);
        walks.kernels[primary].component = require(BACKEND, Slot::Traversal, &opts.traversal)?;
        walks.kernels[primary].consts = mr_components::shaders::KernelConsts { lod: false, exact: opts.exact, levels: [opts.skip2, opts.skip4, opts.skip8, opts.skip16].iter().enumerate().fold(0, |m, (i, &on)| m | (on as u32) << (i + 1)) };
        let shadow = match super::shading_site(g, "shadow") {
            Some(_) => walks.site(&g, site(Role::Shading, "shadow")?, STORAGE_GROUPS, false),
            None => primary,
        };
        let bounce = match super::shading_site(g, "bounce") {
            Some(Some(s)) => walks.site(&g, s, STORAGE_GROUPS, false),
            _ => shadow,
        };
        let mesh_shadow = g.site(Role::Meshes, "shadow").map_or(shadow, |s| walks.site(&g, s, STORAGE_GROUPS, false));
        let mut comp = Self {
            walks,
            shadow,
            bounce,
            mesh_shadow,
            prepass: require(BACKEND, Slot::Prepass, &opts.prepass)?,
            tile: opts.tile.clamp(1, 64),
            steps_view: parse_view(&opts.view)?,
            lod: false,
            fragment: parse_dispatch(&opts.dispatch)?,
            meshes: require(BACKEND, Slot::Meshes, &opts.meshes)? != "none",
        };
        if max_lod > 0 {
            comp.set_lod(true);
            if let Some(why) = lod_blocker(&comp) {
                comp.set_lod(false);
                tracing::warn!("gpu-compute: running without LOD (max_lod = {max_lod}): {why}");
            }
        }
        Ok(comp)
    }

    fn primary(&self) -> &KernelInst {
        &self.walks.kernels[0]
    }

    /// The prepass in effect: a tile of 1 pixel turns it off.
    fn prepass(&self) -> &str {
        if self.tile <= 1 { "none" } else { &self.prepass }
    }

    /// Sets LOD on every kernel (they all walk the LOD chunks).
    fn set_lod(&mut self, lod: bool) {
        self.lod = lod;
        self.walks.kernels.iter_mut().for_each(|k| k.consts.lod = lod);
    }

    /// The shader as modules (PLAN steps 9b, 9c): every storage and kernel node its own
    /// module (`Walks`), the shadow and bounce rays' services through their kernels, the
    /// shading component, and the lane's root, which casts the primary rays through kernel 0
    /// and reads its storage (the prepass); `meshed`: with the mesh capability (`MESHES`) and
    /// the mesh records at group 3.
    fn program(&self, shading: &str, meshed: bool) -> CastProgram {
        use mr_components::shaders;
        let mut program = mr_gpu::program::Program::world(Some("rays::shadow"));
        self.walks.add_to(&mut program);
        let rays = shaders::shading_rays(&mut program, &self.walks, self.shadow, self.bounce);
        shaders::shading(&mut program, shading, &rays.iter().map(String::as_str).collect::<Vec<_>>(), 0, crate::STATE_BINDING);
        // Mesh records at group 3 (the triangles at binding 3), lit through the mesh-shadow site.
        let meshes = meshed.then(|| shaders::meshes(&mut program, &self.walks, self.shadow, self.mesh_shadow, 3, 3));
        let pre = mr_components::prepass::gpu_wgsl(self.prepass()).expect("validated");
        let primary = self.primary();
        let primary_storage = primary.storage.expect("the walk's primary rays walk a storage");
        let root = format!(
            "// Composition: storage = {}, prepass = {}, traversal = {}, shading = {shading}\n\
             const PREPASS_TILE: u32 = {}u;\n\
             {}\n{}\n{}\n{pre}",
            self.walks.storages[primary_storage].component,
            self.prepass(),
            primary.component,
            self.tile.max(1),
            if meshed { MESH_RECORDS_WGSL } else { "" },
            if self.steps_view { mr_core::legend::ramp_wgsl() } else { String::new() },
            include_str!("walk.wgsl"),
        );
        let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string(), self.walks.kernel_path(0), self.walks.storage_path(primary_storage)];
        deps.extend(meshes);
        deps.extend(rays);
        deps.extend([mr_gpu::program::LIGHTING, mr_gpu::program::VOXELS, mr_gpu::program::COMMON].map(String::from));
        CastProgram { program, root, deps }
    }
}

/// Why the features this composition turns on cannot run with its components, if they
/// can't: LOD needs every storage and kernel to handle LOD chunks and a prepass that copes
/// (none; the beam reads level 0); `view = "steps"` a primary walk that counts its steps.
fn feature_blocker(c: &Composition) -> Option<String> {
    let lod = c.lod.then(|| lod_blocker(c).map(|w| format!("this run streams LOD chunks: {w}"))).flatten();
    lod.or_else(|| (c.steps_view && !caps::provides(mr_components::Kind::Gpu, Slot::Traversal, &c.primary().component, Cap::Steps)).then(|| format!("view = steps: traversal `{}` lacks steps", c.primary().component)))
}

fn lod_blocker(c: &Composition) -> Option<String> {
    let pre = (!caps::provides(mr_components::Kind::Gpu, Slot::Prepass, c.prepass(), Cap::Lod)).then(|| format!("prepass `{}` lacks lod", c.prepass()));
    let why: Vec<String> = c.walks.lod_blocker().into_iter().chain(pre).collect();
    (!why.is_empty()).then(|| why.join("; "))
}

fn parse_dispatch(s: &str) -> anyhow::Result<bool> {
    match s {
        "compute" => Ok(false),
        "fragment" => Ok(true),
        _ => anyhow::bail!("dispatch must be `compute` or `fragment`, got `{s}`"),
    }
}

fn parse_view(s: &str) -> anyhow::Result<bool> {
    match s {
        "image" => Ok(false),
        "steps" => Ok(true),
        _ => anyhow::bail!("view must be `image` or `steps`, got `{s}`"),
    }
}

/// The walking caster's own state: the meshes' records, the prepass's buffers (the storages
/// the rays walk are the core's).
pub(crate) struct Walk {
    /// Mesh mode: mesh records rasterised first, rays capped at their depth.
    meshes: MeshRecords,
    comp: Composition,
    /// Composition the passes were compiled from (recompiled at the next frame on a change).
    built: Composition,
    /// This frame's storage bind groups.
    bound: Bound,
    /// Prepass output (one f32 per tile, sized for 1-pixel tiles) and statistics, whether this
    /// frame runs the prepass and copies its statistics out.
    beam: Beam,
    prepass: bool,
    copy_stats: bool,
    /// `view = steps`: the top of the colour scale.
    view_max: u32,
    /// The nodes of the lane's timed stages: meshes and prepass (PLAN step 11).
    stage_nodes: [Vec<String>; 2],
}

impl Walk {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &graph::Checked) -> anyhow::Result<(Core, Self, String, Option<crate::remote::Far>)> {
        let opts: Options = p.options()?;
        let comp = Composition::of(g, &opts, p.max_lod)?;
        if let Some(why) = feature_blocker(&comp) {
            anyhow::bail!(why);
        }
        let shading = require(BACKEND, Slot::Shading, &opts.shading)?;
        // Fragment too: `dispatch = "fragment"` runs the pixels (and walks the storages) in a
        // fragment shader.
        let stages = wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT;
        // The storage instances' data: the primary's at group 2, the other sites' at 4.. .
        let spec = CoreSpec {
                gpu: GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY },
                walks: &comp.walks,
                accel: crate::accel::AccelSpec::of(g, &comp.walks)?,
                stages,
                max_lod: if comp.lod { p.max_lod.min(6) } else { 0 },
                voxels: false,
                records: mr_components::parse_placement(&opts.placement)?,
                shading: &shading,
                bounces: opts.bounces,
                spp: opts.spp,
                accumulate: opts.accumulate,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                pool_mb: opts.pool_mb,
                frames_in_flight: opts.frames_in_flight,
        };
        let core = Core::new(p, devices, waker.clone(), g, spec.clone())?;
        let far = crate::remote::Far::of(p, devices, waker, g, &core, &spec)?;
        let walk = |k: usize| comp.walks.label(k);
        tracing::info!("gpu-compute: primary {}, shadow {}, bounce {}, shading {shading}", walk(0), walk(comp.shadow), walk(comp.bounce));
        let lane = Self {
            meshes: MeshRecords::new(&core.gpu, &core.world, p.width, p.height),
            built: comp.clone(),
            comp,
            bound: Bound(Vec::new()),
            beam: Beam::new(&core.gpu, p.width, p.height),
            prepass: false,
            copy_stats: false,
            view_max: opts.view_max.clamp(1, 16384),
            stage_nodes: [g.ids(&[Role::Meshes]), g.ids(&[Role::Prepass])],
        };
        Ok((core, lane, shading, far))
    }
}

impl Cast for Walk {
    const BACKEND: &'static str = BACKEND;

    fn layout_entries(&self, stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry> {
        let rw = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: stages,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        vec![rw(LANE_BINDING), rw(LANE_BINDING + 1)]
    }

    fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![wgpu::BindGroupEntry { binding: LANE_BINDING, resource: self.beam.tiles.as_entire_binding() }, wgpu::BindGroupEntry { binding: LANE_BINDING + 1, resource: self.beam.stats.as_entire_binding() }]
    }

    fn layouts<'a>(&'a self, core: &'a Core, [layout0, world]: [&'a wgpu::BindGroupLayout; 2]) -> Vec<Option<&'a wgpu::BindGroupLayout>> {
        core.storages.layouts(&[Some(layout0), Some(world), None, Some(self.meshes.layout())])
    }

    fn program(&self, shading: &str, meshed: bool) -> CastProgram {
        self.comp.program(shading, meshed)
    }

    fn stale(&self) -> bool {
        self.comp != self.built
    }

    fn compiled(&mut self) {
        self.built = self.comp.clone();
    }

    fn meshes(&self) -> bool {
        self.comp.meshes
    }

    fn fragment(&self) -> bool {
        self.comp.fragment
    }

    fn inline_only(&self) -> bool {
        self.comp.steps_view
    }

    fn entries(&self) -> &'static [&'static str] {
        if mr_components::prepass::has_pass(self.comp.prepass()) { &["prepass"] } else { &[] }
    }

    fn camera(&self) -> [u32; 2] {
        [self.view_max, 0]
    }

    fn updated(&mut self, _core: &Core, _changed: &[IVec3]) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.meshes.meshes.apply(update);
    }

    fn prepare(&mut self, _core: &mut Core, _passes: &mut Passes, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket, _req: &FrameRequest) {}

    fn before(&mut self, core: &mut Core, passes: &Passes, enc: &mut wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, world: &wgpu::BindGroup) -> Before {
        self.bound = core.storages.prepare(&core.gpu, &core.world);
        // The first pass carries the frame's begin timestamp: meshes, else the prepass.
        let want_meshes = req.meshes && self.built.meshes;
        let meshes_on = self.meshes.active(want_meshes);
        let ts = if meshes_on { core.ring.render_timestamps(ticket, true, false) } else { None };
        if meshes_on {
            core.ring.stage(enc, ticket, "meshes", &self.stage_nodes[0]);
        }
        self.meshes.encode(&core.gpu, enc, &req.camera, core.width as f32 / core.height as f32, req.mode, want_meshes, ts);
        let pre = passes.extra.first();
        self.prepass = pre.is_some();
        self.copy_stats = self.beam.begin(enc, self.prepass);
        if let Some(pre) = pre {
            core.ring.stage(enc, ticket, "prepass", &self.stage_nodes[1]);
            let tile = self.built.tile.max(1);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("prepass"), timestamp_writes: if meshes_on { None } else { core.ring.compute_timestamps(ticket, true, false) } });
            pass.set_pipeline(pre);
            pass.set_bind_group(0, passes.bind0(), &[]);
            pass.set_bind_group(1, world, &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            self.bound.compute(&mut pass);
            pass.dispatch_workgroups(core.width.div_ceil(tile).div_ceil(8), core.height.div_ceil(tile).div_ceil(8), 1);
        }
        Before { first: !meshes_on && !self.prepass, meshed: meshes_on }
    }

    fn groups(&self) -> impl Iterator<Item = (u32, &wgpu::BindGroup)> {
        std::iter::once((3, self.meshes.bind_group())).chain(self.bound.0.iter().map(|(g, b)| (*g, b)))
    }

    fn stats(&self, core: &Core) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>) {
        let counters = [("dropped_chunks".to_string(), core.world.dropped_chunks() as f64)]
            .into_iter()
            .chain(core.storages.counters())
            .chain(self.prepass.then(|| ("avg_skip_blocks".to_string(), self.beam.avg_skip)))
            .collect();
        (counters, self.built.steps_view.then(|| Legend::ramp("loop iterations (primary ray)", self.view_max as f64, true)), Vec::new())
    }

    fn finish(&mut self, enc: &mut wgpu::CommandEncoder) {
        if self.copy_stats {
            self.beam.copy(enc);
        }
    }

    fn submitted(&mut self) {
        if self.copy_stats {
            self.beam.read();
        }
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        let set = |c: &mut Composition| -> anyhow::Result<()> {
            match name {
                "prepass" => c.prepass = require(BACKEND, Slot::Prepass, value.as_str().unwrap_or(""))?,
                "tile" => c.tile = value.as_i64().unwrap_or(8).clamp(1, 64) as u32,
                "visibility" => drop(require(BACKEND, Slot::Visibility, value.as_str().unwrap_or(""))?),
                "traversal" => c.walks.kernels[0].component = require(BACKEND, Slot::Traversal, value.as_str().unwrap_or(""))?,
                _ => c.steps_view = parse_view(value.as_str().unwrap_or(""))?,
            }
            match feature_blocker(c) {
                Some(why) => anyhow::bail!(why),
                None => Ok(()),
            }
        };
        Some(match name {
            "visibility" | "traversal" | "prepass" | "tile" | "view" => {
                let mut c = self.comp.clone();
                set(&mut c).map(|_| self.comp = c)
            }
            "meshes" => require(BACKEND, Slot::Meshes, value.as_str().unwrap_or("")).map(|m| self.comp.meshes = m != "none"),
            "view_max" => {
                self.view_max = value.as_i64().unwrap_or(256).clamp(1, 16384) as u32;
                Ok(())
            }
            "dispatch" => parse_dispatch(value.as_str().unwrap_or("")).map(|f| self.comp.fragment = f),
            "exact" => {
                self.comp.walks.kernels[0].consts.exact = value.as_bool().unwrap_or(true);
                Ok(())
            }
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << name[4..].parse::<u32>().unwrap().trailing_zeros();
                let levels = &mut self.comp.walks.kernels[0].consts.levels;
                if value.as_bool().unwrap_or(true) {
                    *levels |= bit;
                } else {
                    *levels &= !bit;
                }
                Ok(())
            }
            _ => return None,
        })
    }

    fn resize(&mut self, core: &Core) {
        self.meshes.resize(&core.gpu, core.width, core.height);
        self.beam = Beam::new(&core.gpu, core.width, core.height);
    }

    fn memory_bytes(&self) -> u64 {
        self.beam.tiles.size()
    }
}

/// The prepass's buffers: one f32 start distance per tile and the statistics (sum of
/// skipped blocks, tiles), read back without blocking, a few frames late.
struct Beam {
    tiles: wgpu::Buffer,
    stats: wgpu::Buffer,
    read: wgpu::Buffer,
    /// 0 idle, 1 mapping, 2 mapped.
    state: Arc<AtomicU32>,
    avg_skip: f64,
}

impl Beam {
    fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let buf = |label, size: u64, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        Self {
            tiles: buf("beam tiles", (width as u64 * height as u64 * 4).max(16), wgpu::BufferUsages::STORAGE),
            stats: buf("beam stats", 16, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST),
            read: buf("beam stats readback", 16, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
            state: Arc::new(AtomicU32::new(0)),
            avg_skip: 0.0,
        }
    }

    /// Takes in a finished readback and clears the sums for this frame. Returns whether
    /// this frame's sums should be copied out.
    fn begin(&mut self, enc: &mut wgpu::CommandEncoder, active: bool) -> bool {
        if self.state.load(Ordering::Acquire) == 2 {
            if let Ok(data) = self.read.slice(..).get_mapped_range() {
                let v: &[u32] = bytemuck::cast_slice(&data[..8]);
                if v[1] > 0 {
                    self.avg_skip = v[0] as f64 / v[1] as f64;
                }
            }
            self.read.unmap();
            self.state.store(0, Ordering::Release);
        }
        if active {
            enc.clear_buffer(&self.stats, 0, None);
        }
        active && self.state.load(Ordering::Acquire) == 0
    }

    fn copy(&self, enc: &mut wgpu::CommandEncoder) {
        enc.copy_buffer_to_buffer(&self.stats, 0, &self.read, 0, 16);
    }

    fn read(&self) {
        self.state.store(1, Ordering::Release);
        let state = self.state.clone();
        self.read.slice(..).map_async(wgpu::MapMode::Read, move |r| state.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every composition's shader composes from its modules and validates (no GPU needed):
    /// every storage and kernel, shadow, bounce and mesh-shadow rays through the primary
    /// walk or their own (shared or separate storage nodes), every shading and prepass, LOD
    /// on and off.
    #[test]
    fn programs_compose() {
        use mr_components::graph::{Graph, Node};
        let storages = names_on(BACKEND, Slot::Storage);
        let kernels = names_on(BACKEND, Slot::Traversal);
        let mut n = 0;
        for st in &storages {
            for k in &kernels {
                for (ss, sk) in [("same", "same"), ("tree64", "same"), ("same", "sphere"), ("distance", "incremental")] {
                    for shading in names_on(BACKEND, Slot::Shading) {
                        for (prepass, tile) in [("none", 1), ("beam", 8)] {
                            let c = mr_components::Composition { storage: st.to_string(), traversal: k.to_string(), shadow_storage: ss.into(), shadow_traversal: sk.into(), shading: shading.to_string(), prepass: prepass.into(), ..Default::default() };
                            let mut graphs = vec![Graph::from_composition(&c).unwrap()];
                            // Bounces and mesh shadows through walks of their own.
                            if shading == "pathtrace" && *k == "refloor" {
                                let mut g = graphs[0].clone();
                                g.nodes.insert("b.storage".into(), Node::new(Role::Storage, "rows"));
                                let mut bk = Node::new(Role::Traversal, "incremental");
                                bk.inputs.insert("storage".into(), "b.storage".into());
                                bk.knobs.insert("exact".into(), toml::Value::Boolean(false));
                                g.nodes.insert("b.kernel".into(), bk);
                                let mut mk = Node::new(Role::Traversal, "rowrun");
                                mk.inputs.insert("storage".into(), "storage".into());
                                g.nodes.insert("m.kernel".into(), mk);
                                g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "b.kernel".into());
                                g.nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), "m.kernel".into());
                                graphs.push(g);
                            }
                            for (gi, g) in graphs.into_iter().enumerate() {
                                assert!(g.validate().is_ok(), "{g:?}");
                                let ck = g.check().unwrap();
                                let opts = Options { storage: st.to_string(), traversal: k.to_string(), shading: shading.to_string(), prepass: prepass.into(), tile, exact: false, skip4: false, view: "steps".into(), ..Default::default() };
                                let lod = *st == "bricks" && *k == "refloor" && ss == "same" && sk == "same";
                                let c = Composition::of(&ck, &opts, if lod { 4 } else { 0 }).unwrap_or_else(|e| panic!("{g:?}: {e}"));
                                // The walks of their own lack LOD.
                                assert_eq!(c.lod, lod && prepass == "none" && gi == 0, "{c:?}");
                                // Inline and deferred shading (the record-writing walk and the
                                // common pass), with meshes' records and without, in compute
                                // and fragment shaders.
                                for (deferred, meshed, fragment) in [(false, true, false), (true, true, true), (false, false, true), (true, false, false)] {
                                    let what = format!("{c:?} {shading} deferred {deferred} meshed {meshed} fragment {fragment}");
                                    let steps = c.steps_view && !deferred;
                                    super::super::tests::validate(c.program(shading, meshed), deferred, steps, fragment, &what);
                                }
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(n > 100, "{n}");
    }
}
