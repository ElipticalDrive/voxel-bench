//! Composable GPU ray caster (IDEAS §9/§12, PLAN step 6′): the world's **storage** and the
//! ray's **traversal** are separate WGSL modules behind a small interface, chosen by knobs
//! and pasted into one shader before compilation, so the compiler optimises across them
//! and the split costs nothing at run time.
//!
//! * Storage (`store_*.wgsl`): `S_LEVELS`, `s_occupied(slot, level, c)`, `s_probe(slot, l)`,
//!   `s_block(slot, l)`. `bricks` is the shared pool + 4^3 brick bits; `hier` adds an 8^3
//!   region summary per slot; `prefix` adds the user's Morton masks + popcount prefix per
//!   slot (`prefix.rs`). Side data lives in `aux`, indexed like the pool.
//! * Traversal (`trav_*.wgsl`): `ctrace(o, d, max_t) -> Hit`, written only against the
//!   storage interface and the shared chunk table. `refloor` is gpu-dda's walk;
//!   `incremental` is the per-level DDA from cpu-simd-mem.
//!
//! Shadow rays use the same composed walk. `bricks` + `refloor` is gpu-dda's algorithm on
//! gpu-dda's data, so comparing the two measures the cost of the split.

mod prefix;

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::meshes::{MESH_COMPOSITE_WGSL, MeshComposite};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, wgpu};
use serde::Deserialize;

/// A storage module: its WGSL, plus optional per-slot side data in the `aux` buffer
/// (indexed like the shared pool), built on the CPU for each dense chunk.
///
/// Every storage answers the required queries (`S_LEVELS`, `s_occupied`, `s_probe`,
/// `s_block`). Optional queries (`OPTIONAL_QUERIES`) are fast paths: a storage lists the
/// ones it implements in `provides`, and the assembler appends a generic fallback from
/// `defaults.wgsl` for the rest, so every storage works with every traversal.
struct Storage {
    name: &'static str,
    wgsl: &'static str,
    provides: &'static [&'static str],
    /// u32 words per slot (0 = no side data) and their builder.
    aux_words: usize,
    build: fn(&DenseChunk) -> Vec<u32>,
}

const STORAGES: &[Storage] = &[
    Storage { name: "bricks", wgsl: include_str!("store_bricks.wgsl"), provides: &[], aux_words: 0, build: |_| Vec::new() },
    Storage { name: "hier", wgsl: include_str!("store_hier.wgsl"), provides: &[], aux_words: 2, build: hier_summary },
    Storage { name: "prefix", wgsl: include_str!("store_prefix.wgsl"), provides: &[], aux_words: prefix::PREFIX_WORDS, build: prefix::build },
    Storage { name: "distance", wgsl: include_str!("store_distance.wgsl"), provides: &["s_dist"], aux_words: CHUNK_VOLUME / 4, build: distance_field },
    Storage { name: "rows", wgsl: include_str!("store_rows.wgsl"), provides: &["s_row"], aux_words: 1024, build: occupancy_rows },
];

/// Optional storage queries with a generic fallback in `defaults.wgsl` (`//@name`).
const OPTIONAL_QUERIES: &[&str] = &["s_dist", "s_row"];

/// The fallback WGSL of every optional query `st` does not provide.
fn fallbacks(st: &Storage) -> String {
    let src = include_str!("defaults.wgsl");
    let mut out = String::new();
    for q in OPTIONAL_QUERIES.iter().filter(|q| !st.provides.contains(q)) {
        let start = src.find(&format!("//@{q}\n")).expect("fallback missing");
        let rest = &src[start..];
        let end = rest[3..].find("//@").map_or(rest.len(), |e| e + 3);
        out += &rest[..end];
    }
    out
}

/// `distance`: gpu-sdf's Chebyshev distance field, packed four u8 per word.
fn distance_field(d: &DenseChunk) -> Vec<u32> {
    bytemuck::cast_slice(&gpu_sdf::chebyshev_df(d)).to_vec()
}

/// `rows`: gpu-bitmask's occupancy rows (row y * 32 + z, bit x).
fn occupancy_rows(d: &DenseChunk) -> Vec<u32> {
    let mut rows = vec![0u32; 1024];
    for (i, b) in d.0.iter().enumerate() {
        rows[i >> 5] |= (b.is_solid() as u32) << (i & 31);
    }
    rows
}

/// `hier`: one bit per 8^3 region (index `x | z << 2 | y << 4`, as cpu-simd-hier).
fn hier_summary(d: &DenseChunk) -> Vec<u32> {
    // Dense order is x + 32 z + 1024 y: the region index is bits 3-4 of each coordinate.
    // Branch-free (solidity is unpredictable in mixed chunks).
    let mut bits = 0u64;
    for (i, b) in d.0.iter().enumerate() {
        let r = (i >> 3) & 3 | ((i >> 8) & 3) << 2 | ((i >> 13) & 3) << 4;
        bits |= (b.is_solid() as u64) << r;
    }
    vec![bits as u32, (bits >> 32) as u32]
}
const TRAVERSALS: &[(&str, &str)] = &[
    ("refloor", include_str!("trav_refloor.wgsl")),
    ("incremental", include_str!("trav_incremental.wgsl")),
    ("sphere", include_str!("trav_sphere.wgsl")),
    ("rowrun", include_str!("trav_rowrun.wgsl")),
];
const CHUNK_BYTES: u64 = CHUNK_VOLUME as u64 * 2;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuCompose::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-compose".into(),
        description: "Composable compute ray caster: storage (bricks, prefix) x traversal (refloor, incremental) chosen by knobs and fused into one shader".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "modular".into(), "wgpu".into(), "meshes".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn names(list: &[(&'static str, &str)]) -> Vec<&'static str> {
    list.iter().map(|(n, _)| *n).collect()
}

fn storage_names() -> Vec<&'static str> {
    STORAGES.iter().map(|s| s.name).collect()
}

fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("storage", "World storage module. bricks: the shared u16 pool with 4^3 brick bits (levels 1, 4). hier: bricks + one bit per 8^3 region (levels 1, 4, 8). prefix: Morton occupancy masks + popcount prefix (levels 1, 2, 4, 8, 16). distance: per-voxel Chebyshev distance field (from gpu-sdf; fast s_dist). rows: 32x32 occupancy rows per chunk (from gpu-bitmask; fast s_row)", &storage_names(), "bricks", false),
        ParamSpec::choice("traversal", "Ray walk module. refloor: probe for the largest empty cube, step out, re-floor (gpu-dda). incremental: hierarchical DDA keeping per-level exit times, moving between levels incrementally. sphere: Chebyshev sphere tracing on s_dist (gpu-sdf). rowrun: refloor plus bit-scan jumps across empty row runs on s_row (gpu-bitmask). Every storage works with every traversal; missing fast paths use generic fallbacks", &names(TRAVERSALS), "refloor", true),
        ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell after each move (no float drift) instead of accumulating them", true, true),
    ];
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them"), true, true));
    }
    p.push(ParamSpec::choice("view", "image: the rendered scene. steps: primary-ray loop iterations per pixel (red = low 8 bits, green = high bits, blue = log-scale brightness)", &["image", "steps"], "image", true));
    p.extend([
        shadows_param(),
        ParamSpec::int("max_steps", "Loop iteration limit per ray", 32, 16384, 2048, true),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]);
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    storage: String,
    traversal: String,
    exact: bool,
    skip2: bool,
    skip4: bool,
    skip8: bool,
    skip16: bool,
    view: String,
    shadows: Shadows,
    pool_mb: u32,
    max_steps: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            adapter: None,
            storage: "bricks".into(),
            traversal: "refloor".into(),
            exact: true,
            skip2: true,
            skip4: true,
            skip8: true,
            skip16: true,
            view: "image".into(),
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
    storage: usize,
    traversal: usize,
    exact: bool,
    /// Levels allowed by the skip knobs (bit k = 2^k cubes).
    levels: u32,
    /// `view = "steps"`: write iteration counts instead of colour.
    steps_view: bool,
}

impl Composition {
    fn source(&self) -> String {
        let st = &STORAGES[self.storage];
        let (sname, store) = (st.name, st.wgsl);
        let (tname, trav) = TRAVERSALS[self.traversal];
        let own = format!(
            "// Composition: storage = {sname}, traversal = {tname}\n\
             @group(2) @binding(0) var<storage, read> aux: array<u32>;\n\
             const AUX_WORDS: u32 = {}u;\n\
             const EXACT: bool = {};\n\
             const VIEW_STEPS: bool = {};\n\
             const LEVEL_MASK: u32 = (S_LEVELS & {:#x}u) | 0x21u;\n\
             {MESH_COMPOSITE_WGSL}\n{store}\n{}\n{trav}\n{}",
            st.aux_words.max(1),
            self.exact,
            self.steps_view,
            self.levels,
            fallbacks(st),
            include_str!("main.wgsl"),
        );
        let occlusion = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n";
        shader_source(occlusion, &own)
    }
}

fn parse_view(s: &str) -> anyhow::Result<bool> {
    match s {
        "image" => Ok(false),
        "steps" => Ok(true),
        _ => anyhow::bail!("view must be `image` or `steps`, got `{s}`"),
    }
}

fn pick(list: &[(&str, &str)], name: &str, what: &str) -> anyhow::Result<usize> {
    list.iter().position(|(n, _)| *n == name).ok_or_else(|| anyhow::anyhow!("unknown {what} `{name}` (have: {})", list.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")))
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
    _pad: [u32; 2],
}

pub struct GpuCompose {
    gpu: Gpu,
    world: VoxelWorld,
    /// Mesh mode: meshes rasterised first, rays capped at their depth.
    meshes: MeshComposite,
    comp: Composition,
    /// Composition the pipeline was built from (rebuilt lazily when knobs change it).
    built: Composition,
    pipeline: wgpu::ComputePipeline,
    pipeline_layout: wgpu::PipelineLayout,
    layout0: wgpu::BindGroupLayout,
    layout2: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    /// Per-slot storage data (`prefix`), or a placeholder.
    aux: GrowableBuffer,
    bind2: Option<wgpu::BindGroup>,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuCompose {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let comp = Composition {
            storage: STORAGES.iter().position(|s| s.name == opts.storage).ok_or_else(|| anyhow::anyhow!("unknown storage `{}` (have: {})", opts.storage, storage_names().join(", ")))?,
            traversal: pick(TRAVERSALS, &opts.traversal, "traversal")?,
            exact: opts.exact,
            steps_view: parse_view(&opts.view)?,
            levels: [opts.skip2, opts.skip4, opts.skip8, opts.skip16].iter().enumerate().fold(0, |m, (i, &on)| m | (on as u32) << (i + 1)),
        };
        let gpu = Gpu::new(GpuRequest {
            experimental: false,
            adapter: opts.adapter.as_deref(),
            required_features: wgpu::Features::empty(),
            optional_features: wgpu::Features::TIMESTAMP_QUERY,
            limits: &|a| a.clone(),
        })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                upload_voxels: true,
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                // Fragment too: the mesh raster pass shades with voxel shadow rays.
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
            },
        )?;
        let meshes = MeshComposite::new(&gpu, &world, p.width, p.height);
        let compute = wgpu::ShaderStages::COMPUTE;
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose camera"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: compute,
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
            ],
        });
        let layout2 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose storage"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: compute,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compose"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(&layout2), Some(meshes.layout())],
            immediate_size: 0,
        });
        let pipeline = Self::pipeline(&gpu, &pipeline_layout, &comp)?;
        // Side data covers the same slots as the pool, up to the same limit.
        let max_slots = (opts.pool_mb as u64 * 1024 * 1024 / CHUNK_BYTES).max(1);
        let aux_bytes = STORAGES[comp.storage].aux_words as u64 * 4;
        let aux = GrowableBuffer::new(&gpu, "compose aux", 256, (max_slots * aux_bytes).max(256), wgpu::BufferUsages::STORAGE);
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output);
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone());
        tracing::info!("gpu-compose: storage {}, traversal {}", STORAGES[comp.storage].name, TRAVERSALS[comp.traversal].0);
        Ok(Self {
            gpu,
            world,
            meshes,
            built: comp.clone(),
            comp,
            pipeline,
            pipeline_layout,
            layout0,
            layout2,
            camera_buf,
            output,
            bind0,
            aux,
            bind2: None,
            ring,
            width: p.width,
            height: p.height,
            depth,
            waker,
        })
    }

    fn pipeline(gpu: &Gpu, layout: &wgpu::PipelineLayout, comp: &Composition) -> anyhow::Result<wgpu::ComputePipeline> {
        let source = comp.source();
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("compose"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compose"),
            layout: Some(layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            anyhow::bail!("compiling {comp:?}: {e}");
        }
        Ok(pipeline)
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compose camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        })
    }

    /// Bytes of side data per pool slot for the chosen storage (0 = none).
    fn aux_bytes(&self) -> u64 {
        STORAGES[self.comp.storage].aux_words as u64 * 4
    }
}

impl Renderer for GpuCompose {
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.meshes.apply(update);
        Ok(())
    }

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        let unit = self.aux_bytes();
        if unit > 0 {
            // Keep the side data indexed like the pool: every changed dense chunk rewrites
            // its slot (slots of unloaded chunks are simply no longer referenced).
            if self.aux.reserve(&self.gpu, self.world.pool_slots() as u64 * unit)? {
                self.bind2 = None;
            }
            let build = STORAGES[self.comp.storage].build;
            for pos in changed {
                if let (Some(slot), Some(ChunkData::Dense(d))) = (self.world.slot_of(pos), self.world.chunk(pos)) {
                    self.gpu.queue.write_buffer(self.aux.buffer(), slot as u64 * unit, bytemuck::cast_slice(&build(d)));
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.comp != self.built {
            self.pipeline = Self::pipeline(&self.gpu, &self.pipeline_layout, &self.comp)?;
            self.built = self.comp.clone();
        }
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
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
            _pad: [0; 2],
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if self.bind2.is_none() {
            self.bind2 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compose storage"),
                layout: &self.layout2,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: self.aux.buffer().as_entire_binding() }],
            }));
        }
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // With meshes, the mesh pass carries the frame's begin timestamp.
        let meshes_on = self.meshes.active(req.meshes);
        let ts = if meshes_on { self.ring.render_timestamps(ticket, true, false) } else { None };
        self.meshes.encode(&self.gpu, &mut enc, c, self.width as f32 / self.height as f32, req.mode, req.meshes, &world_bg, ts);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compose"),
                timestamp_writes: self.ring.compute_timestamps(ticket, !meshes_on, true),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, self.bind2.as_ref(), &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![("dropped_chunks".into(), self.world.dropped_chunks() as f64)],
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
            "traversal" => self.comp.traversal = pick(TRAVERSALS, value.as_str().unwrap_or(""), "traversal")?,
            "view" => self.comp.steps_view = parse_view(value.as_str().unwrap_or(""))?,
            "exact" => self.comp.exact = value.as_bool().unwrap_or(true),
            "skip2" | "skip4" | "skip8" | "skip16" => {
                let bit = 1 << name[4..].parse::<u32>()?.trailing_zeros();
                if value.as_bool().unwrap_or(true) {
                    self.comp.levels |= bit;
                } else {
                    self.comp.levels &= !bit;
                }
            }
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
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let aux = if self.aux_bytes() > 0 { self.aux.size() } else { 0 };
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + aux + self.width as u64 * self.height as u64 * 4 }
    }
}
