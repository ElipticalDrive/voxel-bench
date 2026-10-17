//! GPU-compute back end: assembles a renderer from components (`mr-components`, IDEAS
//! §12): a **storage**, a **traversal** kernel walked by ray casting (the visibility
//! strategy, `raycast`) and a **shading** component, each a WGSL module
//! from `components/`, pasted into one compute shader before compilation, so the compiler
//! optimises across them and the split costs nothing at run time.
//!
//! Storages answer the interface in `mr_components::storage`; optional capabilities the
//! chosen storage lacks come from adapters (`mr_components::caps`), so every storage works with every
//! traversal kernel. Shadow rays use the same composed walk. The storage's own data
//! (per-slot side data, or region blobs) is kept by `storage::gpu_data::StorageData`.
//! Meshes (mesh mode) are a surface source: `mr_gpu::meshes::MeshRecords` rasterises their
//! hit records first, rays stop at them, and the walk or the deferred pass shades them. Shading is placed
//! inline (in the walk's pass) or deferred (`placement`, PLAN step 12: the walk writes hit
//! records, the common pass `shading/deferred.wgsl` shades them).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_components::storage::gpu_data;
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::graph::{self, Role};
use mr_components::shaders::{KernelInst, Walks};
use mr_components::storage::gpu_data::StorageSet;
use mr_components::{Cap, caps, names_on, parse_placement, placement_param, require, Slot, components};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::meshes::{MESH_RECORDS_WGSL, MeshRecords};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuNeeds, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, d, w| Ok(Box::new(GpuCompose::new(p, d, w)?)) }
}


const BACKEND: &str = "gpu-compute";

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-compute".into(),
        description: "GPU-compute back end: renders a composition of components (storage, traversal, shading) fused into one compute shader".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "modular".into(), "wgpu".into(), "meshes".into(), "lod".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        // LOD with pool storages and refloor (cells 2x coarser per level; was gpu-dda-lod).
        max_lod: 6,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}


fn role_name(r: Role) -> &'static str {
    r.name()
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
    shading: String,
    prepass: String,
    /// Prepass tile size in pixels (1 = the prepass is skipped).
    tile: u32,
    /// `view = "steps"`: write iteration counts instead of colour.
    steps_view: bool,
    /// LOD chunks are streamed and walked (fixed at creation).
    lod: bool,
    /// `dispatch = "fragment"`: pixels in a fragment shader over a full-screen triangle.
    fragment: bool,
    /// `placement = "deferred"`: the walk writes hit records, the common pass shades them
    /// (fixed at creation: only then does group 0 carry the records).
    deferred: bool,
    /// The meshes stage draws meshes (`composite`): their records and the mesh capability of
    /// the walk's are compiled in (`MESHES`).
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
        let site = |role, port| g.site(role, port).ok_or_else(|| anyhow::anyhow!("gpu-compute: {} `{port}` rays are not bound", role_name(role)));
        let primary = walks.site(&g, site(Role::Visibility, "primary")?, STORAGE_GROUPS, false);
        walks.kernels[primary].component = require(BACKEND, Slot::Traversal, &opts.traversal)?;
        walks.kernels[primary].consts = mr_components::shaders::KernelConsts { lod: false, exact: opts.exact, levels: [opts.skip2, opts.skip4, opts.skip8, opts.skip16].iter().enumerate().fold(0, |m, (i, &on)| m | (on as u32) << (i + 1)) };
        let shadow = walks.site(&g, site(Role::Shading, "shadow")?, STORAGE_GROUPS, false);
        let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(&g, s, STORAGE_GROUPS, false));
        let mesh_shadow = g.site(Role::Meshes, "shadow").map_or(shadow, |s| walks.site(&g, s, STORAGE_GROUPS, false));
        let mut comp = Self {
            walks,
            shadow,
            bounce,
            mesh_shadow,
            shading: require(BACKEND, Slot::Shading, &opts.shading)?,
            prepass: require(BACKEND, Slot::Prepass, &opts.prepass)?,
            tile: opts.tile.clamp(1, 64),
            steps_view: parse_view(&opts.view)?,
            lod: false,
            fragment: parse_dispatch(&opts.dispatch)?,
            deferred: parse_placement(&opts.placement)?,
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

    /// Whether shading is deferred this frame: `view = "steps"` shows the walk itself, so it
    /// runs inline whatever the placement.
    fn shade_deferred(&self) -> bool {
        self.deferred && !self.steps_view
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
    /// shading component, and this back end's own code as the root, which casts the primary
    /// rays through kernel 0 and reads its storage (the prepass). Returns the program, the
    /// root and its dependencies.
    fn program(&self) -> (mr_gpu::program::Program, String, Vec<String>) {
        use mr_components::shaders;
        let mut p = mr_gpu::program::Program::world(Some("rays::shadow"));
        self.walks.add_to(&mut p);
        let rays = shaders::shading_rays(&mut p, &self.walks, self.shadow, self.bounce);
        shaders::shading(&mut p, &self.shading, &rays.iter().map(String::as_str).collect::<Vec<_>>(), 0, 4);
        // Mesh records at group 3 (the triangles at binding 3), lit through the mesh-shadow site.
        let meshes = self.meshes.then(|| shaders::meshes(&mut p, &self.walks, self.shadow, self.mesh_shadow, 3, 3));
        let pre = mr_components::prepass::gpu_wgsl(self.prepass()).expect("validated");
        let primary = self.primary();
        let root = format!(
            "// Composition: storage = {}, prepass = {}, traversal = {}, shading = {}\n\
             const PREPASS_TILE: u32 = {}u;\n\
             const VIEW_STEPS: bool = {};\n\
             {}\n{}\n{}\n{pre}\n{}",
            self.walks.storages[primary.storage].component,
            self.prepass(),
            primary.component,
            self.shading,
            self.tile.max(1),
            self.steps_view,
            if self.meshes { MESH_RECORDS_WGSL } else { "" },
            mr_core::legend::ramp_wgsl(),
            include_str!("main.wgsl"),
            if self.shade_deferred() { format!("{}\n{}", include_str!("deferred.wgsl"), mr_components::shading::DEFERRED_PACKED_WGSL) } else { String::new() },
        );
        let mut deps = vec![shaders::SHADING.to_string(), shaders::STATE.to_string(), self.walks.kernel_path(0), self.walks.storage_path(primary.storage)];
        deps.extend(meshes);
        deps.extend(rays);
        deps.extend([mr_gpu::program::LIGHTING, mr_gpu::program::VOXELS, mr_gpu::program::COMMON].map(String::from));
        (p, root, deps)
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

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    mode: u32,
    origin: [f32; 3],
    sky: u32,
    width: u32,
    height: u32,
    view_max: u32,
    _pad: u32,
}

pub struct GpuCompose {
    gpu: Arc<Gpu>,
    world: VoxelWorld,
    /// Mesh mode: mesh records rasterised first, rays capped at their depth.
    meshes: MeshRecords,
    comp: Composition,
    /// Composition the pipeline was built from (rebuilt lazily when knobs change it).
    built: Composition,
    /// The walk and shading passes for frames without mesh records, and (with a meshes
    /// stage) for frames with them: the mesh capability is compiled in only there, since
    /// its code in the walk cost frames without meshes 2-2.5% with `dispatch = fragment`.
    plain: Passes,
    meshed: Option<Passes>,
    /// The prepass component's own pass, if it has one.
    prepass: Option<wgpu::ComputePipeline>,
    /// placement = deferred: the hit records the common pass reads (one `pack_hit` per
    /// pixel; created with the back end, so present whenever placement is).
    records: Option<wgpu::Buffer>,
    pipeline_layout: wgpu::PipelineLayout,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    /// Group 0 for `dispatch = "fragment"`: a 1x1 stand-in instead of the output texture.
    bind0_frag: wgpu::BindGroup,
    stand_in: wgpu::Texture,
    /// Prepass output: one f32 per tile (sized for 1-pixel tiles), and statistics.
    beam: Beam,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    /// `view = steps`: the top of the colour scale.
    view_max: u32,
    /// The storage instances' data (the primary's at group 2, the other sites' at 4..).
    storages: StorageSet,
    /// The nodes of the timed stages: meshes, prepass, walk and shade (PLAN step 11), and
    /// when deferred, walk and shade apart.
    stage_nodes: [Vec<String>; 5],
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuCompose {
    pub fn new(p: InitParams, devices: &Devices, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let g = graph::of_init(BACKEND, &p)?;
        let comp = Composition::of(&g, &opts, p.max_lod)?;
        if let Some(why) = feature_blocker(&comp) {
            anyhow::bail!(why);
        }
        // The host hands the device in (GPU slot 0); this back end only states its needs.
        let gpu = mr_gpu::gpu(devices, 0, &GpuNeeds { experimental: false, required_features: wgpu::Features::empty(), optional_features: wgpu::Features::TIMESTAMP_QUERY })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: if comp.lod { p.max_lod.min(6) } else { 0 },
                // Region storages (tree64) hold their own data instead of the voxel pool.
                upload_voxels: StorageSet::needs_pool(&comp.walks),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                // Fragment too: `dispatch = "fragment"` shades in a fragment shader.
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
            },
        )?;
        let compute = wgpu::ShaderStages::COMPUTE;
        // Fragment too: `dispatch = "fragment"` runs the pixels in a fragment shader.
        let pixel = compute | wgpu::ShaderStages::FRAGMENT;
        // Fragment too: `dispatch = "fragment"` walks the storages.
        let limits = gpu_data::Limits { mb: opts.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 };
        let storages = StorageSet::new(&gpu, &comp.walks, compute | wgpu::ShaderStages::FRAGMENT, limits);
        if storages.groups_end() > gpu.device.limits().max_bind_groups {
            anyhow::bail!("{} storages (rays walking their own) need {} bind groups; this device has {}", comp.walks.storages.len(), storages.groups_end(), gpu.device.limits().max_bind_groups);
        }
        let meshes = MeshRecords::new(&gpu, &world, p.width, p.height);
        let rw = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: pixel,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let mut entries = vec![
            rw(2),
            rw(3),
            ShadeState::layout_entries(4, pixel)[0],
            ShadeState::layout_entries(4, pixel)[1],
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: pixel,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: compute,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ];
        // placement = deferred: the hit records (inline pipelines do without).
        if comp.deferred {
            entries.push(rw(6));
        }
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("compose camera"), entries: &entries });
        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compose"),
            bind_group_layouts: &storages.layouts(&[Some(&layout0), Some(world.layout()), None, Some(meshes.layout())]),
            immediate_size: 0,
        });
        let (plain, meshed, prepass) = Self::pipelines(&gpu, &pipeline_layout, &comp)?;
        let records = comp.deferred.then(|| Self::records(&gpu, p.width, p.height));
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let beam = Beam::new(&gpu, p.width, p.height);
        let accum = ShadeState::new(&gpu, p.width, p.height, &comp.shading, opts.bounces, opts.spp, opts.accumulate);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &beam, &accum, records.as_ref());
        let stand_in = mr_gpu::output_texture(&gpu, 1, 1);
        let bind0_frag = Self::bind0(&gpu, &layout0, &camera_buf, &stand_in, &beam, &accum, records.as_ref());
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()).with_stages(p.stage_times);
        let walk = |k: usize| format!("{} over {}", comp.walks.kernels[k].component, comp.walks.storages[comp.walks.kernels[k].storage].component);
        tracing::info!("gpu-compute: primary {}, shadow {}, bounce {}, shading {}", walk(0), walk(comp.shadow), walk(comp.bounce), comp.shading);
        Ok(Self {
            gpu,
            world,
            meshes,
            built: comp.clone(),
            comp,
            plain,
            meshed,
            prepass,
            records,
            pipeline_layout,
            layout0,
            camera_buf,
            output,
            bind0,
            bind0_frag,
            stand_in,
            beam,
            accum,
            view_max: opts.view_max.clamp(1, 16384),
            storages,
            stage_nodes: [g.ids(&[Role::Meshes]), g.ids(&[Role::Prepass]), g.ids(&[Role::Visibility, Role::Shading]), g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            ring,
            width: p.width,
            height: p.height,
            depth,
            waker,
        })
    }

    /// The passes without the mesh capability, with it (when `comp` has a meshes stage), and
    /// the prepass's pass if it has one.
    fn pipelines(gpu: &Gpu, layout: &wgpu::PipelineLayout, comp: &Composition) -> anyhow::Result<(Passes, Option<Passes>, Option<wgpu::ComputePipeline>)> {
        let (main, prepass, shade) = Self::pipeline(gpu, layout, &Composition { meshes: false, ..comp.clone() })?;
        let meshed = if comp.meshes {
            let (main, _, shade) = Self::pipeline(gpu, layout, comp)?;
            Some(Passes { main, shade })
        } else {
            None
        };
        Ok((Passes { main, shade }, meshed, prepass))
    }

    /// The main pipeline (the walk, shading inline or writing records), the prepass's pass
    /// if it has one, and the deferred shading pass when shading is deferred.
    #[allow(clippy::type_complexity)]
    fn pipeline(gpu: &Gpu, layout: &wgpu::PipelineLayout, comp: &Composition) -> anyhow::Result<(Main, Option<wgpu::ComputePipeline>, Option<wgpu::ComputePipeline>)> {
        let (program, root, deps) = comp.program();
        let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = mr_gpu::program::compile(&gpu.device, "compose", &program, &root, &deps);
        let entry = |name| {
            gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(layout),
                module: &module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let deferred = comp.shade_deferred();
        let pipeline = if comp.fragment {
            // Deferred: the fragments write records, not colour (the output is attached
            // untouched).
            let write_mask = if deferred { wgpu::ColorWrites::empty() } else { wgpu::ColorWrites::ALL };
            let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask })];
            Main::Fragment(gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("compose fragment"),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(if deferred { "fs_cast" } else { "fs_full" }), compilation_options: Default::default(), targets: &targets }),
                multiview_mask: None,
                cache: None,
            }))
        } else {
            Main::Compute(entry(if deferred { "walk_records" } else { "main" }))
        };
        let prepass = mr_components::prepass::has_pass(comp.prepass()).then(|| entry("prepass"));
        let shade = deferred.then(|| entry("shade"));
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling {comp:?}: {e}");
        }
        Ok((pipeline, prepass, shade))
    }

    /// The hit records of `placement = deferred` (8 bytes per pixel).
    fn records(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("hit records"), size: (width as u64 * height as u64 * 8).max(16), usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
    }

    fn rebind(&mut self) {
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.beam, &self.accum, self.records.as_ref());
        self.bind0_frag = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.stand_in, &self.beam, &self.accum, self.records.as_ref());
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, beam: &Beam, accum: &ShadeState, records: Option<&wgpu::Buffer>) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        let mut entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 2, resource: beam.tiles.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: beam.stats.as_entire_binding() },
            accum.bind_entries(4)[0].clone(),
            accum.bind_entries(4)[1].clone(),
        ];
        entries.extend(records.map(|r| wgpu::BindGroupEntry { binding: 6, resource: r.as_entire_binding() }));
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("compose camera"), layout, entries: &entries })
    }
}

impl Renderer for GpuCompose {
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.meshes.apply(update);
        Ok(())
    }

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        let changed = self.world.apply(&self.gpu, updates);
        self.storages.update(&self.gpu, &self.world, &changed)
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.comp != self.built {
            (self.plain, self.meshed, self.prepass) = Self::pipelines(&self.gpu, &self.pipeline_layout, &self.comp)?;
            if self.accum.set_shading(&self.gpu, &self.comp.shading) {
                self.rebind();
            }
            self.built = self.comp.clone();
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
        self.accum.begin(&self.gpu, c, req.mode);
        let cam = CameraUniform {
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            origin: frac.to_array(),
            sky: u32::from_le_bytes(encode_srgb(self.world.env.sky)),
            width: self.width,
            height: self.height,
            view_max: self.view_max,
            _pad: 0,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let bound = self.storages.prepare(&self.gpu, &self.world);
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // The first pass carries the frame's begin timestamp: meshes, else the prepass.
        let want_meshes = req.meshes && self.built.meshes;
        let meshes_on = self.meshes.active(want_meshes);
        let ts = if meshes_on { self.ring.render_timestamps(ticket, true, false) } else { None };
        if meshes_on {
            self.ring.stage(&mut enc, ticket, "meshes", &self.stage_nodes[0]);
        }
        self.meshes.encode(&self.gpu, &mut enc, c, self.width as f32 / self.height as f32, req.mode, want_meshes, ts);
        let copy_stats = self.beam.begin(&mut enc, self.prepass.is_some());
        if let Some(pre) = &self.prepass {
            self.ring.stage(&mut enc, ticket, "prepass", &self.stage_nodes[1]);
            let tile = self.comp.tile.max(1);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("prepass"),
                timestamp_writes: if meshes_on { None } else { self.ring.compute_timestamps(ticket, true, false) },
            });
            pass.set_pipeline(pre);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            bound.compute(&mut pass);
            pass.dispatch_workgroups(self.width.div_ceil(tile).div_ceil(8), self.height.div_ceil(tile).div_ceil(8), 1);
        }
        let first = !meshes_on && self.prepass.is_none();
        // The walk's pass ends the frame unless the deferred shading pass follows.
        let passes = if meshes_on { self.meshed.as_ref().unwrap_or(&self.plain) } else { &self.plain };
        let last = passes.shade.is_none();
        if last {
            self.ring.stage(&mut enc, ticket, "cast+shade", &self.stage_nodes[2]);
        } else {
            self.ring.stage(&mut enc, ticket, "cast", &self.stage_nodes[3]);
        }
        match &passes.main {
            Main::Compute(pipeline) => {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("compose"),
                    timestamp_writes: self.ring.compute_timestamps(ticket, first, last),
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind0, &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                pass.set_bind_group(3, self.meshes.bind_group(), &[]);
                bound.compute(&mut pass);
                pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
            }
            Main::Fragment(pipeline) => {
                let view = self.output.create_view(&Default::default());
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("compose fragment"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: self.ring.render_timestamps(ticket, first, last),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                // The output is the colour attachment here, so group 0 binds a stand-in
                // for the storage-texture output.
                pass.set_bind_group(0, &self.bind0_frag, &[]);
                pass.set_bind_group(1, &world_bg, &[]);
                pass.set_bind_group(3, self.meshes.bind_group(), &[]);
                bound.render(&mut pass);
                pass.draw(0..3, 0..1);
            }
        }
        if let Some(shade) = &passes.shade {
            self.ring.stage(&mut enc, ticket, "shade", &self.stage_nodes[4]);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("deferred shade"), timestamp_writes: self.ring.compute_timestamps(ticket, false, true) });
            pass.set_pipeline(shade);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            bound.compute(&mut pass);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: [("dropped_chunks".to_string(), self.world.dropped_chunks() as f64)]
                .into_iter()
                .chain(self.storages.counters())
                .chain(self.prepass.is_some().then(|| ("avg_skip_blocks".to_string(), self.beam.avg_skip)))
                .chain(self.accum.counter(req.mode))
                .collect(),
            legend: self.built.steps_view.then(|| Legend::ramp("loop iterations (primary ray)", self.view_max as f64, true)),
            stages: Vec::new(),
        };
        if copy_stats {
            self.beam.copy(&mut enc);
        }
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        if copy_stats {
            self.beam.read();
        }
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "visibility" | "traversal" | "prepass" | "tile" => {
                let mut c = self.comp.clone();
                match name {
                    "prepass" => c.prepass = require(BACKEND, Slot::Prepass, value.as_str().unwrap_or(""))?,
                    "tile" => c.tile = value.as_i64().unwrap_or(8).clamp(1, 64) as u32,
                    "visibility" => drop(require(BACKEND, Slot::Visibility, value.as_str().unwrap_or(""))?),
                    _ => c.walks.kernels[0].component = require(BACKEND, Slot::Traversal, value.as_str().unwrap_or(""))?,
                }
                if let Some(why) = feature_blocker(&c) {
                    anyhow::bail!(why);
                }
                self.comp = c;
            }
            "shading" => self.comp.shading = require(BACKEND, Slot::Shading, value.as_str().unwrap_or(""))?,
            "meshes" => self.comp.meshes = require(BACKEND, Slot::Meshes, value.as_str().unwrap_or(""))? != "none",
            "view_max" => self.view_max = value.as_i64().unwrap_or(256).clamp(1, 16384) as u32,
            "view" => {
                let mut c = self.comp.clone();
                c.steps_view = parse_view(value.as_str().unwrap_or(""))?;
                if let Some(why) = feature_blocker(&c) {
                    anyhow::bail!(why);
                }
                self.comp = c;
            }
            "dispatch" => self.comp.fragment = parse_dispatch(value.as_str().unwrap_or(""))?,
            "exact" => self.comp.walks.kernels[0].consts.exact = value.as_bool().unwrap_or(true),
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << name[4..].parse::<u32>()?.trailing_zeros();
                let levels = &mut self.comp.walks.kernels[0].consts.levels;
                if value.as_bool().unwrap_or(true) {
                    *levels |= bit;
                } else {
                    *levels &= !bit;
                }
            }
            _ if self.accum.set_param(name, value).is_some() => {}
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.meshes.resize(&self.gpu, width, height);
        self.beam = Beam::new(&self.gpu, width, height);
        self.accum.resize(&self.gpu, width, height);
        if self.records.is_some() {
            self.records = Some(Self::records(&self.gpu, width, height));
        }
        self.rebind();
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone()).with_stages(self.ring.stages());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.storages.memory_bytes() + self.accum.memory_bytes() + self.beam.tiles.size() + self.records.as_ref().map_or(0, |r| r.size()) + self.width as u64 * self.height as u64 * 4 }
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

/// The per-pixel pipeline: compute, or a fragment shader over a full-screen triangle.
/// One variant of the walk's pass and the deferred shading pass after it.
struct Passes {
    main: Main,
    shade: Option<wgpu::ComputePipeline>,
}

enum Main {
    Compute(wgpu::ComputePipeline),
    Fragment(wgpu::RenderPipeline),
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
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
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
                                // common pass), with meshes' records and without.
                                for (deferred, meshes) in [(false, true), (true, true), (false, false), (true, false)] {
                                    let mut dc = c.clone();
                                    dc.meshes = meshes;
                                    if deferred {
                                        (dc.deferred, dc.steps_view) = (true, false);
                                    }
                                    let (p, root, deps) = dc.program();
                                    let deps: Vec<&str> = deps.iter().map(String::as_str).collect();
                                    let m = p.naga(&root, &deps).unwrap_or_else(|e| panic!("{dc:?}: {e}"));
                                    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("{dc:?}: {e:?}"));
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
