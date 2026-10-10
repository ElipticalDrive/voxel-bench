//! gpu-hybrid: raster primary visibility + ray-traced secondary, deferred. Greedy meshes
//! (gpu-raster's mesher and draw path) are rasterised into a G-buffer (albedo + surface
//! point + face); in lit mode a compute pass then DDA-traces shadow rays through the GPU
//! voxel world from each G-buffer surface point, once per pixel, off the raster path.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry_hybrid() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuHybrid::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-hybrid".into(),
        description: "Hybrid: greedy-meshed raster G-buffer (albedo, position, face), lit mode shades in a compute pass with DDA-traced shadow rays".into(),
        tags: vec!["gpu".into(), "raster".into(), "hybrid".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray DDA step limit", 32, 16384, 2048, true),
        ParamSpec::bool("depth_prepass", "Lit mode: depth pre-pass so lighting runs once per pixel", true, true),
        ParamSpec::int("mesh_budget", "Chunks remeshed per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB)", 64, 6144, 1536, false),
        ParamSpec::int("quad_buffer_mb", "Quad buffer limit (MiB)", 16, 4096, 512, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    shadows: Shadows,
    /// Voxel pool for lighting rays (unused in unlit mode but always allocated).
    pool_mb: u32,
    quad_buffer_mb: u32,
    /// Lit mode: lay down depth first so the expensive fragment shader runs once per pixel.
    depth_prepass: bool,
    mesh_budget: usize,
    frames_in_flight: usize,
    max_steps: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, pool_mb: 1536, quad_buffer_mb: 512, depth_prepass: true, mesh_budget: 0, frames_in_flight: 3, max_steps: 2048 }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    cam_frac: [f32; 3],
    mode: u32,
    forward: [f32; 3],
    tan_half_fov: f32,
    right: [f32; 3],
    aspect: f32,
    up: [f32; 3],
    _p0: u32,
    size: [f32; 2],
    _p1: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

const MAX_SLOTS: usize = 1 << 16;
const QUAD_BYTES: u64 = 8;

struct ChunkMesh {
    slot: u16,
    offset: u32,
    counts: [u32; 6],
}

impl ChunkMesh {
    fn len(&self) -> u32 {
        self.counts.iter().sum()
    }
}

pub struct GpuHybrid {
    gpu: Gpu,
    world: VoxelWorld,
    opts: Options,
    pipe_fill: wgpu::RenderPipeline,
    pipe_depth: wgpu::RenderPipeline,
    pipe_gfill: wgpu::RenderPipeline,
    pipe_gequal: wgpu::RenderPipeline,
    pipe_shade: wgpu::ComputePipeline,
    shade_layout: wgpu::BindGroupLayout,
    gbuf: GBuffer,
    frame_buf: wgpu::Buffer,
    quad_buf: GrowableBuffer,
    chunk_buf: wgpu::Buffer,
    indirect: wgpu::Buffer,
    layout0: wgpu::BindGroupLayout,
    bind0: Option<wgpu::BindGroup>,
    output: wgpu::Texture,
    depth: wgpu::TextureView,
    ring: FrameRing,
    meshes: HashMap<IVec3, ChunkMesh>,
    dirty: HashSet<IVec3>,
    alloc: RangeAlloc,
    free_slots: Vec<u16>,
    width: u32,
    height: u32,
    waker: Waker,
    overflowed: bool,
}

/// Quads that fit the initial (1 MiB) quad buffer.
fn quad_buf_initial_quads() -> u64 {
    (1 << 20) / QUAD_BYTES
}

const GPOS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// Lit-mode G-buffer: albedo (sRGB texel) and camera-relative surface point + face id
/// (w = -1 where nothing was rasterised), plus the bind group the shading pass reads.
struct GBuffer {
    albedo: wgpu::TextureView,
    pos: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

impl GBuffer {
    fn new(gpu: &Gpu, layout: &wgpu::BindGroupLayout, output: &wgpu::Texture, w: u32, h: u32) -> Self {
        let tex = |label, format| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let albedo = tex("gbuffer albedo", wgpu::TextureFormat::Rgba8Unorm);
        let pos = tex("gbuffer position", GPOS_FORMAT);
        let out = output.create_view(&Default::default());
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hybrid gbuffer"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&albedo) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&pos) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&out) },
            ],
        });
        Self { albedo, pos, bind }
    }
}

fn depth_view(gpu: &Gpu, w: u32, h: u32) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl GpuHybrid {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let gpu = Gpu::new(GpuRequest {
            experimental: false,
            adapter: opts.adapter.as_deref(),
            required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE,
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("hybrid_raster.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("raster"), source: wgpu::ShaderSource::Wgsl(source.into()) });

        let buf_entry = |binding, ty, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raster frame"),
            entries: &[
                buf_entry(0, wgpu::BufferBindingType::Uniform, wgpu::ShaderStages::VERTEX_FRAGMENT),
                buf_entry(1, ro, wgpu::ShaderStages::VERTEX),
                buf_entry(2, ro, wgpu::ShaderStages::VERTEX),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raster"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = |label, write: bool, compare, fragment: bool, g: bool| {
            let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
            let targets = if g { vec![target(wgpu::TextureFormat::Rgba8Unorm), target(GPOS_FORMAT)] } else { vec![target(wgpu::TextureFormat::Rgba8Unorm)] };
            gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: fragment.then(|| wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(if g { "fs_g" } else { "fs" }),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipe_fill = pipeline("fill", true, wgpu::CompareFunction::Greater, true, false);
        let pipe_depth = pipeline("depth prepass", true, wgpu::CompareFunction::Greater, false, false);
        let pipe_gfill = pipeline("gbuffer fill", true, wgpu::CompareFunction::Greater, true, true);
        let pipe_gequal = pipeline("gbuffer equal", false, wgpu::CompareFunction::Equal, true, true);
        let shade_module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hybrid shade"),
            source: wgpu::ShaderSource::Wgsl(shader_source(DDA_OCCLUSION_WGSL, include_str!("hybrid_shade.wgsl")).into()),
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let shade_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hybrid gbuffer"),
            entries: &[
                tex_entry(0),
                tex_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let shade_pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hybrid shade"),
            bind_group_layouts: &[Some(&shade_layout), Some(world.layout())],
            immediate_size: 0,
        });
        let pipe_shade = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("hybrid shade"),
            layout: Some(&shade_pl),
            module: &shade_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let max_bytes = (gpu.device.limits().max_storage_buffer_binding_size as u64).min(gpu.device.limits().max_buffer_size);
        let quad_capacity = ((opts.quad_buffer_mb as u64 * 1024 * 1024).min(max_bytes) / QUAD_BYTES) as u32;
        let storage = |label, size: u64, extra| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
                mapped_at_creation: false,
            })
        };
        let frame_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad_buf = GrowableBuffer::new(&gpu, "quads", 1 << 20, quad_capacity as u64 * QUAD_BYTES, wgpu::BufferUsages::STORAGE);
        let chunk_buf = storage("chunk positions", MAX_SLOTS as u64 * 16, wgpu::BufferUsages::empty());
        let indirect = storage("indirect", 6 * 16 * 4096, wgpu::BufferUsages::INDIRECT);
        let depth = opts.frames_in_flight.max(1);
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let gbuf = GBuffer::new(&gpu, &shade_layout, &output, p.width, p.height);
        Ok(Self {
            depth: depth_view(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            output,
            gpu,
            world,
            opts,
            pipe_fill,
            pipe_depth,
            pipe_gfill,
            pipe_gequal,
            pipe_shade,
            gbuf,
            shade_layout,
            frame_buf,
            quad_buf,
            chunk_buf,
            indirect,
            layout0,
            bind0: None,
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            alloc: RangeAlloc::new((quad_buf_initial_quads()) as u32),
            free_slots: (0..MAX_SLOTS as u32).rev().map(|s| s as u16).collect(),
            width: p.width,
            height: p.height,
            waker,
            overflowed: false,
        })
    }

    fn drop_mesh(&mut self, pos: IVec3) {
        if let Some(m) = self.meshes.remove(&pos) {
            self.alloc.free(m.offset, m.len());
            self.free_slots.push(m.slot);
        }
    }

    /// Re-meshes dirty chunks in parallel and uploads the results.
    /// Remeshes dirty chunks, nearest to `near` first, at most `mesh_budget` per call.
    fn remesh(&mut self, near: IVec3) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.opts.mesh_budget > 0 && dirty.len() > self.opts.mesh_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.opts.mesh_budget) {
                self.dirty.insert(p);
            }
        }
        let mut work = vec![];
        for pos in dirty {
            if self.world.chunk(pos).is_none() {
                self.drop_mesh(pos);
                continue;
            }
            let slot = match self.meshes.get(&pos) {
                Some(m) => m.slot,
                None => match self.free_slots.pop() {
                    Some(s) => {
                        self.meshes.insert(pos, ChunkMesh { slot: s, offset: 0, counts: [0; 6] });
                        self.gpu.queue.write_buffer(&self.chunk_buf, s as u64 * 16, bytemuck::cast_slice(&[pos.x, pos.y, pos.z, 0]));
                        s
                    }
                    None => continue,
                },
            };
            work.push((pos, slot));
        }
        let world = &self.world;
        let results: Vec<(IVec3, mr_mesh::Mesh)> = work
            .par_iter()
            .map(|&(pos, slot)| {
                let center = world.chunk(pos).unwrap();
                let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
                (pos, mr_mesh::mesh_chunk(center, neighbors, slot))
            })
            .collect();
        let n = results.len();
        for (pos, mesh) in results {
            let m = self.meshes.get_mut(&pos).unwrap();
            let old = (m.offset, m.len());
            m.counts = [0; 6];
            self.alloc.free(old.0, old.1);
            let len = mesh.quads.len() as u32;
            if len == 0 {
                continue;
            }
            let off = match self.quad_buf.alloc(&self.gpu, &mut self.alloc, len, QUAD_BYTES) {
                Ok((off, grew)) => {
                    if grew {
                        self.bind0 = None;
                    }
                    off
                }
                Err(e) => {
                    if !self.overflowed {
                        tracing::error!("{e:#}; raise `quad_buffer_mb`");
                        self.overflowed = true;
                    }
                    continue;
                }
            };
            let m = self.meshes.get_mut(&pos).unwrap();
            m.offset = off;
            m.counts = mesh.counts;
            self.gpu.queue.write_buffer(self.quad_buf.buffer(), off as u64 * QUAD_BYTES, bytemuck::cast_slice(&mesh.quads));
        }
        n
    }
}

/// Frustum planes (left, right, bottom, top, near) from a reverse-Z infinite projection.
fn frustum_planes(m: Mat4) -> [Vec4; 5] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2]
}

fn aabb_visible(planes: &[Vec4; 5], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = Vec3::new(if p.x >= 0.0 { max.x } else { min.x }, if p.y >= 0.0 { max.y } else { min.y }, if p.z >= 0.0 { max.z } else { min.z });
        p.truncate().dot(v) + p.w >= 0.0
    })
}

impl Renderer for GpuHybrid {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        for pos in self.world.apply(&self.gpu, updates) {
            self.dirty.insert(pos);
            for f in Face::ALL {
                let nb = pos + f.normal();
                if self.meshes.contains_key(&nb) || self.world.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let remeshed = self.remesh(req.camera.split_position().0 >> 5);
        let mesh_ms = start.elapsed().as_secs_f64() * 1e3;
        let c = &req.camera;
        let (cam_block, frac) = c.split_position();
        let view_proj = c.projection(self.width as f32 / self.height as f32) * c.view_rotation();
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_frac: frac.to_array(),
            mode: (req.mode == RenderMode::Lit) as u32,
            forward: c.forward().to_array(),
            tan_half_fov: (c.fov_y * 0.5).tan(),
            right: c.right().to_array(),
            aspect: self.width as f32 / self.height as f32,
            up: c.up().to_array(),
            _p0: 0,
            size: [self.width as f32, self.height as f32],
            _p1: [0.0; 2],
        };
        self.gpu.queue.write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();

        // Cull chunks and face directions; build indirect draws.
        let planes = frustum_planes(view_proj);
        let mut draws: Vec<DrawArgs> = Vec::with_capacity(self.meshes.len() * 3);
        let mut quads = 0u64;
        for (pos, m) in &self.meshes {
            if m.len() == 0 {
                continue;
            }
            let min = (*pos * 32 - cam_block).as_vec3() - frac;
            let max = min + 32.0;
            if !aabb_visible(&planes, min, max) {
                continue;
            }
            let mut off = m.offset;
            for (f, &n) in m.counts.iter().enumerate() {
                let axis = f / 2;
                // Faces pointing +axis are only visible from beyond the chunk's min plane.
                let visible = if f % 2 == 0 { 0.0 > min[axis] } else { 0.0 < max[axis] };
                if n > 0 && visible {
                    draws.push(DrawArgs { vertex_count: 4, instance_count: n, first_vertex: 0, first_instance: off });
                    quads += n as u64;
                }
                off += n;
            }
        }
        let bytes = (draws.len() * 16) as u64;
        if bytes > self.indirect.size() {
            self.indirect = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("indirect"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });
        }
        if !draws.is_empty() {
            self.gpu.queue.write_buffer(&self.indirect, 0, bytemuck::cast_slice(&draws));
        }

        if self.bind0.is_none() {
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("raster frame"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.frame_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.quad_buf.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.chunk_buf.as_entire_binding() },
                ],
            }));
        }
        let bind0 = self.bind0.clone().unwrap();
        let sky = encode_srgb(self.world.env.sky);
        let clear = wgpu::Color { r: sky[0] as f64 / 255.0, g: sky[1] as f64 / 255.0, b: sky[2] as f64 / 255.0, a: 1.0 };
        let out_view = self.output.create_view(&Default::default());
        let lit = req.mode == RenderMode::Lit;
        let prepass = self.opts.depth_prepass && lit;
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let n = draws.len() as u32;
        if prepass {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.ring.render_timestamps(ticket, true, false),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe_depth);
            pass.set_bind_group(0, &bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            if n > 0 {
                pass.multi_draw_indirect(&self.indirect, 0, n);
            }
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shade"),
                color_attachments: &if lit {
                    vec![
                        Some(wgpu::RenderPassColorAttachment {
                            view: &self.gbuf.albedo,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &self.gbuf.pos,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: -1.0 }), store: wgpu::StoreOp::Store },
                        }),
                    ]
                } else {
                    vec![Some(wgpu::RenderPassColorAttachment {
                        view: &out_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                    })]
                },
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if prepass { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) },
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                // Lit: the frame ends in the compute shading pass; with a pre-pass this pass needs none.
                timestamp_writes: if prepass { None } else { self.ring.render_timestamps(ticket, true, !lit) },
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(match (lit, prepass) {
                (false, _) => &self.pipe_fill,
                (true, true) => &self.pipe_gequal,
                (true, false) => &self.pipe_gfill,
            });
            pass.set_bind_group(0, &bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            if n > 0 {
                pass.multi_draw_indirect(&self.indirect, 0, n);
            }
        }
        if lit {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("hybrid shade"),
                timestamp_writes: self.ring.compute_timestamps(ticket, false, true),
            });
            pass.set_pipeline(&self.pipe_shade);
            pass.set_bind_group(0, &self.gbuf.bind, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![
                ("quads".into(), quads as f64),
                ("draws".into(), n as f64),
                ("remeshed".into(), remeshed as f64),
                ("mesh_ms".into(), mesh_ms),
            ],
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
            "depth_prepass" => self.opts.depth_prepass = value.as_bool().unwrap_or(true),
            "mesh_budget" => self.opts.mesh_budget = value.as_i64().unwrap_or(0) as usize,
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
        self.depth = depth_view(&self.gpu, width, height);
        self.gbuf = GBuffer::new(&self.gpu, &self.shade_layout, &self.output, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let gpu = self.world.memory_bytes() + self.quad_buf.size() + self.chunk_buf.size() + self.indirect.size() + self.width as u64 * self.height as u64 * 28;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}
