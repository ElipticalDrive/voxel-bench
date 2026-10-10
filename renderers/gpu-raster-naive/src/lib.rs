//! Naive "Minecraft-style" GPU rasteriser: one instanced quad per visible block face
//! (face culling against neighbours, NO greedy merging; `cull=false` emits all six faces of
//! every solid block). Structure shared with `gpu-raster`: packed per-face
//! instances, drawn with vertex pulling and one multi-draw-indirect call after per-chunk
//! frustum and per-face-direction culling. Lit mode is hybrid: rasterised visibility,
//! ray-traced shadows from the fragment shader through the GPU voxel world, with an
//! optional depth pre-pass so lighting runs once per pixel.

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

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuRasterNaive::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-raster-naive".into(),
        description: "Naive per-face mesh rasteriser (one instanced quad per block face, no greedy merge; cull=false draws every face) ; vertex pulling, multi-draw-indirect, chunk frustum culling; lit mode adds ray-traced shadows in the fragment shader".into(),
        tags: vec!["gpu".into(), "raster".into(), "hybrid".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::bool("cull", "Skip faces hidden by a solid neighbour (false = all 6 faces of every block)", true, true),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray DDA step limit", 32, 16384, 2048, true),
        ParamSpec::bool("depth_prepass", "Lit mode: depth pre-pass so lighting runs once per pixel", true, true),
        ParamSpec::int("mesh_budget", "Chunks remeshed per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB)", 64, 6144, 1536, false),
        ParamSpec::int("quad_buffer_mb", "Face buffer limit (MiB; grows on demand)", 16, 2040, 1024, false),
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
    /// Hidden-face culling against solid neighbours.
    cull: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, pool_mb: 1536, quad_buffer_mb: 1024, depth_prepass: true, mesh_budget: 0, frames_in_flight: 3, max_steps: 2048, cull: true }
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

pub struct GpuRasterNaive {
    gpu: Gpu,
    world: VoxelWorld,
    opts: Options,
    pipe_fill: wgpu::RenderPipeline,
    pipe_depth: wgpu::RenderPipeline,
    pipe_equal: wgpu::RenderPipeline,
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

impl GpuRasterNaive {
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            },
        )?;
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("raster.wgsl"));
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
        let pipeline = |label, write: bool, compare, fragment: bool| {
            let targets = [Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })];
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
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipe_fill = pipeline("fill", true, wgpu::CompareFunction::Greater, true);
        let pipe_depth = pipeline("depth prepass", true, wgpu::CompareFunction::Greater, false);
        let pipe_equal = pipeline("shade equal", false, wgpu::CompareFunction::Equal, true);

        let max_bytes = (gpu.device.limits().max_storage_buffer_binding_size as u64).min(gpu.device.limits().max_buffer_size);
        // Capped below 2 GiB: GrowableBuffer rounds sizes up to 16 B, which would overshoot
        // a 2^31-4 storage-binding limit.
        let quad_capacity = ((opts.quad_buffer_mb.min(2040) as u64 * 1024 * 1024).min(max_bytes) / QUAD_BYTES) as u32;
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
        Ok(Self {
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            depth: depth_view(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            gpu,
            world,
            opts,
            pipe_fill,
            pipe_depth,
            pipe_equal,
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
        let cull = self.opts.cull;
        let results: Vec<(IVec3, mr_mesh::Mesh)> = work
            .par_iter()
            .map(|&(pos, slot)| {
                let center = world.chunk(pos).unwrap();
                let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
                (pos, naive_mesh(center, neighbors, slot, cull))
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

impl Renderer for GpuRasterNaive {
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
        let mut faces_total = 0u64;
        for (pos, m) in &self.meshes {
            faces_total += m.len() as u64;
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
        let prepass = self.opts.depth_prepass && req.mode == RenderMode::Lit;
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
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if prepass { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(0.0) },
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.ring.render_timestamps(ticket, !prepass, true),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if prepass { &self.pipe_equal } else { &self.pipe_fill });
            pass.set_bind_group(0, &bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            if n > 0 {
                pass.multi_draw_indirect(&self.indirect, 0, n);
            }
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![
                ("faces".into(), quads as f64),
                ("faces_total".into(), faces_total as f64),
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
            "cull" => {
                let c = value.as_bool().unwrap_or(true);
                if c != self.opts.cull {
                    self.opts.cull = c;
                    let all: Vec<IVec3> = self.meshes.keys().copied().collect();
                    self.dirty.extend(all);
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
        self.depth = depth_view(&self.gpu, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let gpu = self.world.memory_bytes() + self.quad_buf.size() + self.chunk_buf.size() + self.indirect.size() + self.width as u64 * self.height as u64 * 8;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}

/// Per-face mesher: one 1x1 quad per block face (same packed encoding as `mr_mesh`, sorted
/// by face). With `cull`, faces whose neighbour is solid are skipped; otherwise all six
/// faces of every solid block are emitted.
fn naive_mesh(center: &ChunkData, neighbors: [Option<&ChunkData>; 6], slot: u16, cull: bool) -> mr_mesh::Mesh {
    let mut quads = vec![];
    let mut counts = [0u32; 6];
    if center.is_air() {
        return mr_mesh::Mesh { quads, counts };
    }
    if cull {
        if let ChunkData::Uniform(_) = center {
            if neighbors.iter().all(|n| matches!(n, Some(ChunkData::Uniform(b)) if b.is_solid())) {
                return mr_mesh::Mesh { quads, counts };
            }
        }
    }
    let get = |p: IVec3| -> BlockId {
        let out = |v: i32| !(0..32).contains(&v);
        if !(out(p.x) || out(p.y) || out(p.z)) {
            return center.get(p);
        }
        let f = if p.x > 31 {
            Face::PosX
        } else if p.x < 0 {
            Face::NegX
        } else if p.y > 31 {
            Face::PosY
        } else if p.y < 0 {
            Face::NegY
        } else if p.z > 31 {
            Face::PosZ
        } else {
            Face::NegZ
        };
        match neighbors[f as usize] {
            Some(c) => c.get(p & 31),
            None => BlockId::AIR,
        }
    };
    let uniform = matches!(center, ChunkData::Uniform(_));
    for face in Face::ALL {
        let n = face.normal();
        let before = quads.len();
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let p = IVec3::new(x, y, z);
                    // Uniform solid chunks: only boundary blocks can have visible faces.
                    let q = p + n;
                    if cull && uniform && (0..32).contains(&q.x) && (0..32).contains(&q.y) && (0..32).contains(&q.z) {
                        continue;
                    }
                    let b = center.get(p);
                    if !b.is_solid() || (cull && !get(p + n).is_air()) {
                        continue;
                    }
                    let w0 = x as u32 | (y as u32) << 5 | (z as u32) << 10 | (face as u32) << 25;
                    quads.push([w0, b.0 as u32 | (slot as u32) << 16]);
                }
            }
        }
        counts[face as usize] = (quads.len() - before) as u32;
    }
    mr_mesh::Mesh { quads, counts }
}
