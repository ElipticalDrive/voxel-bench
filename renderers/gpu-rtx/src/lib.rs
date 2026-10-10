//! Hardware ray tracing (RT cores) through wgpu ray queries, in two flavours:
//!
//! * `gpu-rtx`: greedy-meshed triangles in one BLAS per chunk. Traversal and intersection
//!   are entirely in fixed-function hardware.
//! * `gpu-rtx-bricks`: one AABB per occupied 4^3 brick. The hardware walks the BVH, a
//!   tiny software DDA intersects voxels inside each candidate box (procedural geometry).
//!
//! Both rebuild the TLAS every frame with instances translated relative to the camera's
//! block, so precision never degrades far from the origin.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::chunk::cell_of_index;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;
use wgpu::util::DeviceExt;

#[derive(Copy, Clone, PartialEq, Debug)]
enum Geometry {
    Triangles,
    Bricks,
}

pub fn entry_triangles() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "gpu-rtx".into(),
            description: "Hardware ray tracing: greedy-mesh triangles per chunk BLAS, TLAS of chunks, ray queries for primary and shadow rays".into(),
            tags: vec!["gpu".into(), "raytrace".into(), "hardware-rt".into(), "wgpu".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(true),
        },
        create: |p, w| Ok(Box::new(GpuRtx::new(p, w, Geometry::Triangles)?)),
    }
}

pub fn entry_bricks() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "gpu-rtx-bricks".into(),
            description: "Hybrid hardware/software: RT-core BVH over occupied 4^3 bricks (AABBs), software voxel DDA inside each candidate".into(),
            tags: vec!["gpu".into(), "raytrace".into(), "hardware-rt".into(), "procedural".into(), "wgpu".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(false),
        },
        create: |p, w| Ok(Box::new(GpuRtx::new(p, w, Geometry::Bricks)?)),
    }
}

fn params(triangles: bool) -> Vec<ParamSpec> {
    let mut p = vec![
        shadows_param(),
        ParamSpec::bool("fast_build", "Prefer fast BVH builds over fast traversal", false, true),
        ParamSpec::int("build_budget", "Chunk BLAS rebuilds per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ];
    if triangles {
        p.push(ParamSpec::choice("cull", "Triangle facing culled for primary rays", &["back", "front", "none"], "back", true));
    } else {
        p.push(ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false));
    }
    p
}

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Cull {
    #[default]
    Back,
    Front,
    None,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    shadows: Shadows,
    frames_in_flight: usize,
    /// Voxel pool for the bricks variant.
    pool_mb: u32,
    /// Triangle facing culled for primary rays (triangles variant).
    cull: Cull,
    /// Prefer fast BVH builds over fast traversal (helps edit-heavy scenes).
    fast_build: bool,
    build_budget: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, frames_in_flight: 3, pool_mb: 2048, cull: Cull::Back, fast_build: false, build_budget: 0 }
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
    ray_flags: u32,
    _p0: u32,
}

const MAX_SLOTS: u32 = 1 << 16;
const NO_BRICK: u32 = 0xffff_ffff;

struct ChunkAs {
    slot: u32,
    blas: wgpu::Blas,
    /// Range in the shared geometry buffer (u32 units).
    range: (u32, u32),
    _inputs: Vec<wgpu::Buffer>,
}

/// CPU-side geometry for one chunk's BLAS.
enum Built {
    Triangles { vertices: Vec<[f32; 3]>, indices: Vec<u32>, quads: Vec<[u32; 2]> },
    Aabbs { boxes: Vec<[f32; 6]>, bricks: Vec<u32> },
}

pub struct GpuRtx {
    gpu: Gpu,
    world: VoxelWorld,
    geometry: Geometry,
    opts: Options,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    ring: FrameRing,
    waker: Waker,
    width: u32,
    height: u32,
    tlas: wgpu::Tlas,
    tlas_cap: u32,
    geo_buf: GrowableBuffer,
    geo_alloc: RangeAlloc,
    chunk_info: wgpu::Buffer,
    chunks: HashMap<IVec3, ChunkAs>,
    free_slots: Vec<u32>,
    dirty: HashSet<IVec3>,
    bind0: Option<wgpu::BindGroup>,
}

fn tlas(gpu: &Gpu, cap: u32) -> wgpu::Tlas {
    gpu.device.create_tlas(&wgpu::CreateTlasDescriptor {
        label: Some("chunks"),
        max_instances: cap,
        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
    })
}

impl GpuRtx {
    fn new(p: InitParams, waker: Waker, geometry: Geometry) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let gpu = Gpu::new(GpuRequest {
            experimental: true,
            adapter: opts.adapter.as_deref(),
            required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY,
            optional_features: wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
            limits: &|a| a.clone(),
        })?;
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                upload_voxels: geometry == Geometry::Bricks,
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let own = format!("{}\n{}", include_str!("rtx.wgsl"), if geometry == Geometry::Triangles { include_str!("tri.wgsl") } else { include_str!("bricks.wgsl") });
        let source = format!("enable wgpu_ray_query;\n{}", shader_source("", &own));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("rtx"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let storage = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rtx"),
            entries: &[
                e(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                e(
                    1,
                    wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                ),
                e(2, wgpu::BindingType::AccelerationStructure { vertex_return: false }),
                e(3, storage),
                e(4, storage),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rtx"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("rtx"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let buf = |label, size: u64, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let camera_buf = buf("camera", size_of::<CameraUniform>() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let geo_buf = GrowableBuffer::new(&gpu, "geometry", 1 << 20, 1 << 30, wgpu::BufferUsages::STORAGE);
        let geo_words = (geo_buf.size() / 4) as u32;
        let chunk_info = buf("chunk info", MAX_SLOTS as u64 * 16, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let depth = opts.frames_in_flight.max(1);
        let tlas_cap = 4096;
        Ok(Self {
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            tlas: tlas(&gpu, tlas_cap),
            tlas_cap,
            geo_buf,
            gpu,
            world,
            geometry,
            opts,
            pipeline,
            layout0,
            camera_buf,
            waker,
            width: p.width,
            height: p.height,
            geo_alloc: RangeAlloc::new(geo_words),
            chunk_info,
            chunks: HashMap::new(),
            free_slots: (0..MAX_SLOTS).rev().collect(),
            dirty: HashSet::new(),
            bind0: None,
        })
    }

    fn drop_chunk(&mut self, pos: IVec3) {
        if let Some(c) = self.chunks.remove(&pos) {
            self.geo_alloc.free(c.range.0, c.range.1);
            self.free_slots.push(c.slot);
        }
    }

    fn build_geometry(&self, pos: IVec3) -> Option<Built> {
        match self.geometry {
            Geometry::Triangles => {
                let mesh = mr_mesh::mesh_at(pos, |p| self.world.chunk(p), 0)?;
                if mesh.quads.is_empty() {
                    return None;
                }
                let mut vertices = Vec::with_capacity(mesh.quads.len() * 4);
                let mut indices = Vec::with_capacity(mesh.quads.len() * 6);
                for q in &mesh.quads {
                    let base = vertices.len() as u32;
                    vertices.extend(mr_mesh::Quad::decode(*q).corners().map(|c| c.to_array()));
                    indices.extend([base, base + 1, base + 2, base + 2, base + 1, base + 3]);
                }
                Some(Built::Triangles { vertices, indices, quads: mesh.quads })
            }
            Geometry::Bricks => match self.world.chunk(pos)? {
                ChunkData::Uniform(b) if b.is_air() => None,
                ChunkData::Uniform(_) => Some(Built::Aabbs { boxes: vec![[0.0, 0.0, 0.0, 32.0, 32.0, 32.0]], bricks: vec![NO_BRICK] }),
                ChunkData::Dense(d) => {
                    let mut occupied = [false; 512];
                    for (i, b) in d.0.iter().enumerate() {
                        if b.is_solid() {
                            let l: IVec3 = cell_of_index(i) >> 2;
                            occupied[(l.x | (l.z << 3) | (l.y << 6)) as usize] = true;
                        }
                    }
                    let mut boxes = vec![];
                    let mut bricks = vec![];
                    for (bi, _) in occupied.iter().enumerate().filter(|(_, o)| **o) {
                        let b = bi as i32;
                        let lo = IVec3::new(b & 7, (b >> 6) & 7, (b >> 3) & 7).as_vec3() * 4.0;
                        boxes.push([lo.x, lo.y, lo.z, lo.x + 4.0, lo.y + 4.0, lo.z + 4.0]);
                        bricks.push(bi as u32);
                    }
                    (!boxes.is_empty()).then_some(Built::Aabbs { boxes, bricks })
                }
            },
        }
    }

    /// Rebuilds BLASes of dirty chunks into `enc`. Returns how many were rebuilt.
    fn rebuild(&mut self, enc: &mut wgpu::CommandEncoder, near: IVec3) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.opts.build_budget > 0 && dirty.len() > self.opts.build_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.opts.build_budget) {
                self.dirty.insert(p);
            }
        }
        let built: Vec<(IVec3, Option<Built>)> = dirty.par_iter().map(|&p| (p, self.build_geometry(p))).collect();
        let flags = if self.opts.fast_build { wgpu::AccelerationStructureFlags::PREFER_FAST_BUILD } else { wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE };
        // Everything referenced by the build entries must outlive the build call.
        let mut staged: Vec<(IVec3, u32, wgpu::Blas, wgpu::BlasGeometrySizeDescriptors, Vec<wgpu::Buffer>, (u32, u32))> = vec![];
        for (pos, b) in built {
            let old_slot = self.chunks.get(&pos).map(|c| c.slot);
            if let Some(c) = self.chunks.remove(&pos) {
                self.geo_alloc.free(c.range.0, c.range.1);
                if b.is_none() {
                    self.free_slots.push(c.slot);
                }
            }
            let Some(b) = b else { continue };
            let Some(slot) = old_slot.or_else(|| self.free_slots.pop()) else { continue };
            let usage = wgpu::BufferUsages::BLAS_INPUT;
            let (sizes, inputs, words, extra) = match &b {
                Built::Triangles { vertices, indices, quads } => {
                    let vb = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(vertices), usage });
                    let ib = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(indices), usage });
                    let sizes = wgpu::BlasGeometrySizeDescriptors::Triangles {
                        descriptors: vec![wgpu::BlasTriangleGeometrySizeDescriptor {
                            vertex_format: wgpu::VertexFormat::Float32x3,
                            vertex_count: vertices.len() as u32,
                            index_format: Some(wgpu::IndexFormat::Uint32),
                            index_count: Some(indices.len() as u32),
                            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
                        }],
                    };
                    (sizes, vec![vb, ib], bytemuck::cast_slice::<[u32; 2], u32>(quads).to_vec(), 0u32)
                }
                Built::Aabbs { boxes, bricks } => {
                    let ab = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(boxes), usage });
                    let sizes = wgpu::BlasGeometrySizeDescriptors::AABBs {
                        descriptors: vec![wgpu::BlasAABBGeometrySizeDescriptor {
                            primitive_count: boxes.len() as u32,
                            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
                        }],
                    };
                    let value = self.world.table_lookup(pos).unwrap_or(0xffff_ffff);
                    (sizes, vec![ab], bricks.clone(), value)
                }
            };
            let off = match self.geo_buf.alloc(&self.gpu, &mut self.geo_alloc, words.len() as u32, 4) {
                Ok((off, grew)) => {
                    if grew {
                        self.bind0 = None;
                    }
                    off
                }
                Err(e) => {
                    tracing::error!("{e:#}; chunk {pos} skipped");
                    self.free_slots.push(slot);
                    continue;
                }
            };
            self.gpu.queue.write_buffer(self.geo_buf.buffer(), off as u64 * 4, bytemuck::cast_slice(&words));
            let info = match self.geometry {
                Geometry::Triangles => [off / 2, 0, 0, 0],
                Geometry::Bricks => [extra, off, 0, 0],
            };
            self.gpu.queue.write_buffer(&self.chunk_info, slot as u64 * 16, bytemuck::cast_slice(&info));
            let blas = self.gpu.device.create_blas(&wgpu::CreateBlasDescriptor { label: None, flags, update_mode: wgpu::AccelerationStructureUpdateMode::Build }, sizes.clone());
            staged.push((pos, slot, blas, sizes, inputs, (off, words.len() as u32)));
        }
        let n = staged.len();
        {
            let tri_geo: Vec<Vec<wgpu::BlasTriangleGeometry>> = staged
                .iter()
                .map(|(_, _, _, sizes, inputs, _)| match sizes {
                    wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors } => vec![wgpu::BlasTriangleGeometry {
                        size: &descriptors[0],
                        vertex_buffer: &inputs[0],
                        first_vertex: 0,
                        vertex_stride: 12,
                        index_buffer: Some(&inputs[1]),
                        first_index: Some(0),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    }],
                    _ => vec![],
                })
                .collect();
            let entries: Vec<wgpu::BlasBuildEntry> = staged
                .iter()
                .zip(tri_geo)
                .map(|((_, _, blas, sizes, inputs, _), tri)| wgpu::BlasBuildEntry {
                    blas,
                    geometry: match sizes {
                        wgpu::BlasGeometrySizeDescriptors::Triangles { .. } => wgpu::BlasGeometries::TriangleGeometries(tri),
                        wgpu::BlasGeometrySizeDescriptors::AABBs { descriptors } => wgpu::BlasGeometries::AabbGeometries(vec![wgpu::BlasAabbGeometry {
                            size: &descriptors[0],
                            stride: 24,
                            aabb_buffer: &inputs[0],
                            primitive_offset: 0,
                        }]),
                    },
                })
                .collect();
            enc.build_acceleration_structures(entries.iter(), std::iter::empty());
        }
        for (pos, slot, blas, _, inputs, range) in staged {
            self.chunks.insert(pos, ChunkAs { slot, blas, range, _inputs: inputs });
        }
        n
    }
}

impl Renderer for GpuRtx {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        for pos in self.world.apply(&self.gpu, updates) {
            self.dirty.insert(pos);
            if self.geometry == Geometry::Triangles {
                // Face culling depends on neighbours.
                for f in Face::ALL {
                    let nb = pos + f.normal();
                    if self.chunks.contains_key(&nb) || self.world.chunk(nb).is_some() {
                        self.dirty.insert(nb);
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (cam_block, frac) = c.split_position();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // GPU time should include the acceleration-structure builds, which happen outside
        // any pass: start the clock in the encoder when the device allows it.
        let encoder_ts = self.gpu.device.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
        if let (true, Some((qs, begin, _))) = (encoder_ts, self.ring.timestamps(ticket)) {
            enc.write_timestamp(qs, begin);
        }
        let rebuilt = self.rebuild(&mut enc, cam_block >> 5);
        let build_ms = start.elapsed().as_secs_f64() * 1e3;

        // TLAS: one instance per chunk, translated relative to the camera block.
        let n = self.chunks.len() as u32;
        if n > self.tlas_cap {
            self.tlas_cap = n.next_power_of_two();
            self.tlas = tlas(&self.gpu, self.tlas_cap);
            self.bind0 = None;
        }
        for (k, (pos, ch)) in self.chunks.iter().enumerate() {
            let t = (*pos * 32 - cam_block).as_vec3();
            let transform = [1.0, 0.0, 0.0, t.x, 0.0, 1.0, 0.0, t.y, 0.0, 0.0, 1.0, t.z];
            self.tlas[k] = Some(wgpu::TlasInstance::new(&ch.blas, transform, ch.slot, 0xff));
        }
        let used = self.tlas.get().iter().filter(|i| i.is_some()).count() as u32;
        for k in n..used.max(n) {
            self.tlas[k as usize] = None;
        }
        enc.build_acceleration_structures(std::iter::empty(), std::iter::once(&self.tlas));

        let cull = match self.opts.cull {
            Cull::Back => 0x10,
            Cull::Front => 0x20,
            Cull::None => 0,
        };
        let cu = CameraUniform {
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
            ray_flags: if self.geometry == Geometry::Triangles { cull } else { 0 },
            _p0: 0,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if self.bind0.is_none() {
            let view = self.output.create_view(&Default::default());
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rtx"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::AccelerationStructure(&self.tlas) },
                    wgpu::BindGroupEntry { binding: 3, resource: self.geo_buf.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.chunk_info.as_entire_binding() },
                ],
            }));
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("rtx"), timestamp_writes: self.ring.compute_timestamps(ticket, !encoder_ts, true) });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![("instances".into(), n as f64), ("blas_rebuilt".into(), rebuilt as f64), ("build_ms".into(), build_ms)],
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "fast_build" => self.opts.fast_build = value.as_bool().unwrap_or(false),
            "build_budget" => self.opts.build_budget = value.as_i64().unwrap_or(0) as usize,
            "cull" => {
                self.opts.cull = match value.as_str() {
                    Some("front") => Cull::Front,
                    Some("none") => Cull::None,
                    _ => Cull::Back,
                }
            }
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        Ok(())
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        self.bind0 = None;
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        // BLAS sizes are driver-internal; BLAS input buffers are a lower bound.
        let inputs: u64 = self.chunks.values().flat_map(|c| c._inputs.iter()).map(|b| b.size()).sum();
        let gpu = self.world.memory_bytes() + self.geo_buf.size() + self.chunk_info.size() + inputs;
        MemoryReport { cpu_bytes: 0, gpu_bytes: gpu }
    }
}

impl Drop for GpuRtx {
    fn drop(&mut self) {
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let positions: Vec<IVec3> = self.chunks.keys().copied().collect();
        for p in positions {
            self.drop_chunk(p);
        }
    }
}
