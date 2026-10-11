//! GPU-splat back end (components, IDEAS §12): geometry `points`, visibility `splat`,
//! shading `direct`, and the **storage** lit-mode shadow rays walk (refloor; group 2).
//! Was the gpu-splat renderer.
//!
//! GPU point splatting: every face-exposed block becomes one point. A compute pass
//! projects each point and `atomicMin`s its view depth into a u32 framebuffer over a
//! square footprint sized from the projected block size (capped at `max_splat` px), a
//! second pass writes the winning point id, and a resolve pass ray-boxes the pixel
//! against the winning block to texture (and N.L-light) the hit face.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_components::storage::gpu_data::{self, StorageData};
use mr_components::{names_on, require, Slot, storage, visibility};
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuSplat::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-splat".into(),
        description: "GPU-splat back end (storage for shadow rays, points geometry, splat visibility, shading). Compute point splatting: one point per exposed block, atomicMin depth + id buffer, ray-box resolve".into(),
        tags: vec!["gpu".into(), "compute".into(), "points".into(), "splat".into(), "modular".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, vec![
            ParamSpec::choice("storage", "Storage component the lit-mode shadow rays walk (refloor)", &names_on(BACKEND, Slot::Storage), "bricks", false),
            ParamSpec::choice("geometry", "Geometry component", &names_on(BACKEND, Slot::Geometry), "points", false),
            ParamSpec::choice("visibility", "Visibility component", &names_on(BACKEND, Slot::Visibility), "splat", false),
            ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false),
            ParamSpec::int("max_splat", "Largest splat side (px)", 1, 64, 8, true).of("visibility", &["splat"]),
            shadows_param(),
            ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
            ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
        ]),
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    storage: String,
    geometry: String,
    visibility: String,
    shading: String,
    max_splat: u32,
    shadows: Shadows,
    pool_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, storage: "bricks".into(), geometry: "points".into(), visibility: "splat".into(), shading: "direct".into(), max_splat: 8, shadows: Shadows::All, pool_mb: 2048, frames_in_flight: 3 }
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
    count: u32,
    max_splat: u32,
}

type Point = [u32; 4];

/// Exposed-block points of one chunk.
fn build_points(world: &VoxelWorld, cpos: IVec3) -> Vec<Point> {
    let Some(chunk) = world.chunk(cpos) else { return vec![] };
    if chunk.is_air() {
        return vec![];
    }
    let base = cpos * CHUNK_SIZE;
    let solid = |l: IVec3| -> bool {
        if l.cmpge(IVec3::ZERO).all() && l.cmplt(IVec3::splat(CHUNK_SIZE)).all() { chunk.get(l) != BlockId::AIR } else { world.block(base + l) != BlockId::AIR }
    };
    const N: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
    let mut out = vec![];
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let l = IVec3::new(x, y, z);
                let b = chunk.get(l);
                if b == BlockId::AIR {
                    continue;
                }
                let mut mask = 0u32;
                for (i, n) in N.iter().enumerate() {
                    if !solid(l + *n) {
                        mask |= 1 << i;
                    }
                }
                if mask != 0 {
                    let p = base + l;
                    out.push([p.x as u32, p.y as u32, p.z as u32, b.0 as u32 | mask << 16]);
                }
            }
        }
    }
    out
}

const BACKEND: &str = "gpu-splat";



pub struct GpuSplat {
    gpu: Gpu,
    world: VoxelWorld,
    /// The storage's own data for shadow rays (group 2).
    data: StorageData,
    layout_st: wgpu::BindGroupLayout,
    pipes: [wgpu::ComputePipeline; 4],
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    zbuf: wgpu::Buffer,
    idbuf: wgpu::Buffer,
    points_buf: wgpu::Buffer,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    chunks: HashMap<IVec3, Vec<Point>>,
    dirty: bool,
    count: u32,
    max_splat: u32,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn pixel_buffer(gpu: &Gpu, w: u32, h: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("splat fb"), size: w as u64 * h as u64 * 4, usage: wgpu::BufferUsages::STORAGE, mapped_at_creation: false })
}

impl GpuSplat {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let st = storage::gpu(&require(BACKEND, Slot::Storage, &opts.storage)?).expect("validated");
        for (slot, name) in [(Slot::Geometry, &opts.geometry), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
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
                // Region storages (tree64, ...) hold their own data instead of the voxel pool.
                upload_voxels: matches!(st.kind, storage::Kind::Pool { .. }),
                pool_mb: opts.pool_mb,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: 2048,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        // Shadow rays walk the composed storage with refloor.
        let layout_st = gpu_data::layout(&gpu, &st, wgpu::ShaderStages::COMPUTE);
        let data = StorageData::new(&gpu, st, gpu_data::Limits { mb: opts.pool_mb, view_radius: p.view_radius_chunks, wide_ids: p.blocks.blocks.len() > 256 });
        let occlusion = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return ctrace(p, d, max_t).hit; }\n";
        let shade = mr_components::shading::gpu_wgsl(&opts.shading).expect("validated");
        let own = format!("var<private> ct_iters: u32;\nconst LOD: bool = false;\n{}\n{}\n{}\n{shade}", st.source(), visibility::gpu_wgsl("refloor").unwrap(), include_str!("splat.wgsl"));
        let source = shader_source(occlusion, &own);
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("splat"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("splat"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                storage_entry(2, true),
                storage_entry(3, false),
                storage_entry(4, false),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("splat"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(&layout_st)],
            immediate_size: 0,
        });
        let pipe = |e: &str| {
            gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(e),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(e),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipes = [pipe("clear"), pipe("splat_depth"), pipe("splat_id"), pipe("resolve")];
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let points_buf = Self::points_buffer(&gpu, 1024);
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let zbuf = pixel_buffer(&gpu, p.width, p.height);
        let idbuf = pixel_buffer(&gpu, p.width, p.height);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &points_buf, &zbuf, &idbuf);
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone());
        Ok(Self {
            data,
            layout_st,
            gpu,
            world,
            pipes,
            layout0,
            camera_buf,
            output,
            zbuf,
            idbuf,
            points_buf,
            bind0,
            ring,
            chunks: HashMap::new(),
            dirty: false,
            count: 0,
            max_splat: opts.max_splat.clamp(1, 64),
            width: p.width,
            height: p.height,
            depth,
            waker,
        })
    }

    fn points_buffer(gpu: &Gpu, n: u64) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("points"),
            size: n.max(1) * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, pts: &wgpu::Buffer, z: &wgpu::Buffer, id: &wgpu::Buffer) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("splat"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: pts.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: z.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: id.as_entire_binding() },
            ],
        })
    }

    fn rebind(&mut self) {
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.points_buf, &self.zbuf, &self.idbuf);
    }

    /// Concatenates every chunk's points into one GPU buffer (rebuilt after any change).
    fn upload(&mut self) {
        let all: Vec<Point> = self.chunks.values().flatten().copied().collect();
        self.count = all.len() as u32;
        if (all.len() as u64) * 16 > self.points_buf.size() {
            self.points_buf = Self::points_buffer(&self.gpu, all.len() as u64 * 3 / 2);
            self.rebind();
        }
        if !all.is_empty() {
            self.gpu.queue.write_buffer(&self.points_buf, 0, bytemuck::cast_slice(&all));
        }
        self.dirty = false;
    }
}

impl Renderer for GpuSplat {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        self.data.update(&self.gpu, &self.world, &changed)?;
        // Exposure depends on neighbours, so rebuild the 6-neighbourhood too.
        let mut todo = HashSet::new();
        for c in changed {
            todo.insert(c);
            for n in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                todo.insert(c + n);
            }
        }
        for c in todo {
            let pts = build_points(&self.world, c);
            if pts.is_empty() {
                self.dirty |= self.chunks.remove(&c).is_some();
            } else {
                self.chunks.insert(c, pts);
                self.dirty = true;
            }
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        if self.dirty {
            self.upload();
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
            count: self.count,
            max_splat: self.max_splat,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let storage_bg = self.data.prepare(&self.gpu, &self.world, &self.layout_st).clone();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let groups = self.count.div_ceil(64);
        let (gx, gy) = (groups.min(65535), groups.div_ceil(65535));
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("splat"), timestamp_writes: self.ring.compute_timestamps(ticket, true, true) });
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &storage_bg, &[]);
            pass.set_pipeline(&self.pipes[0]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
            if groups > 0 {
                pass.set_pipeline(&self.pipes[1]);
                pass.dispatch_workgroups(gx, gy, 1);
                pass.set_pipeline(&self.pipes[2]);
                pass.dispatch_workgroups(gx, gy, 1);
            }
            pass.set_pipeline(&self.pipes[3]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: vec![("points".into(), self.count as f64), ("dropped_chunks".into(), self.world.dropped_chunks() as f64)],
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "max_splat" => self.max_splat = value.as_i64().unwrap_or(8).clamp(1, 64) as u32,
            "shadows" => self.world.cfg.shadows.set(value)?,
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
        self.zbuf = pixel_buffer(&self.gpu, width, height);
        self.idbuf = pixel_buffer(&self.gpu, width, height);
        self.rebind();
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let cpu = self.chunks.values().map(|v| v.len() as u64 * 16).sum();
        MemoryReport { cpu_bytes: cpu, gpu_bytes: self.world.memory_bytes() + self.data.memory_bytes() + self.points_buf.size() + self.width as u64 * self.height as u64 * 12 }
    }
}
