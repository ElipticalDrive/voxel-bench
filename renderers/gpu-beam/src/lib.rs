//! gpu-dda plus a beam prepass: one conservative cone per N x N tile finds the depth up to
//! which every ray of the tile is guaranteed to cross only empty chunks; full-resolution
//! rays then start their DDA there. Exact (identical image to gpu-dda).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuBeam::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-beam".into(),
        description: "gpu-dda with a per-tile beam prepass: rays start at the distance their tile's cone is provably empty (chunk level)".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "beam".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("tile", "Beam tile size in pixels (1 = no prepass)", 1, 64, 8, true),
        ParamSpec::int("max_steps", "DDA step limit per ray", 32, 16384, 2048, true),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    /// Adapter name substring (default: prefer a discrete GPU).
    adapter: Option<String>,
    shadows: Shadows,
    pool_mb: u32,
    max_steps: u32,
    frames_in_flight: usize,
    tile: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, pool_mb: 2048, max_steps: 2048, frames_in_flight: 3, tile: 8 }
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
    tile: u32,
    tiles_x: u32,
}

pub struct GpuBeam {
    gpu: Gpu,
    world: VoxelWorld,
    pipeline: wgpu::ComputePipeline,
    prepass: wgpu::ComputePipeline,
    beam_buf: wgpu::Buffer,
    stats_buf: wgpu::Buffer,
    stats_read: wgpu::Buffer,
    /// 0 idle (copy allowed), 1 mapping in flight, 2 mapped and ready to read.
    stats_state: Arc<AtomicU32>,
    avg_skip: f64,
    tile: u32,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuBeam {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
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
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("beam.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("beam"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beam camera"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beam"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let prepass = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam prepass"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("prepass"),
            compilation_options: Default::default(),
            cache: None,
        });
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let beam_buf = Self::beam_buf(&gpu, p.width, p.height);
        let stats_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam stats"),
            size: 8,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stats_read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam stats readback"),
            size: 8,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &beam_buf, &stats_buf);
        let ring = FrameRing::new(&gpu, p.width, p.height, opts.frames_in_flight.max(1), waker.clone());
        Ok(Self { gpu, world, pipeline, prepass, beam_buf, stats_buf, stats_read, stats_state: Arc::new(AtomicU32::new(0)), avg_skip: 0.0, tile: opts.tile.clamp(1, 64), layout0, camera_buf, output, bind0, ring, width: p.width, height: p.height, depth: opts.frames_in_flight.max(1), waker })
    }

    /// One f32 per tile at the smallest tile size (1 px), so any `tile` fits.
    fn beam_buf(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam"),
            size: (width.max(1) as u64 * height.max(1) as u64 * 4).max(16),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, beam: &wgpu::Buffer, stats: &wgpu::Buffer) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beam camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: beam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: stats.as_entire_binding() },
            ],
        })
    }
}

impl Renderer for GpuBeam {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        self.world.apply(&self.gpu, updates);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
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
            tile: self.tile,
            tiles_x: self.width.div_ceil(self.tile),
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        // Average skipped distance: lagging, non-blocking readback of the prepass sums.
        if self.stats_state.load(Ordering::Acquire) == 2 {
            if let Ok(data) = self.stats_read.slice(..).get_mapped_range() {
                let v: &[u32] = bytemuck::cast_slice(&data[..]);
                if v[1] > 0 {
                    self.avg_skip = v[0] as f64 / v[1] as f64;
                }
            }
            self.stats_read.unmap();
            self.stats_state.store(0, Ordering::Release);
        }
        let copy_stats = self.tile > 1 && self.stats_state.load(Ordering::Acquire) == 0;
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        if self.tile > 1 {
            enc.clear_buffer(&self.stats_buf, 0, None);
        } else {
            self.avg_skip = 0.0;
        }
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("beam"),
                timestamp_writes: self.ring.compute_timestamps(ticket, true, true),
            });
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            if self.tile > 1 {
                pass.set_pipeline(&self.prepass);
                pass.dispatch_workgroups(self.width.div_ceil(self.tile).div_ceil(8), self.height.div_ceil(self.tile).div_ceil(8), 1);
            }
            pass.set_pipeline(&self.pipeline);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        if copy_stats {
            enc.copy_buffer_to_buffer(&self.stats_buf, 0, &self.stats_read, 0, 8);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![("dropped_chunks".into(), self.world.dropped_chunks() as f64), ("avg_skip_blocks".into(), self.avg_skip)],
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        if copy_stats {
            self.stats_state.store(1, Ordering::Release);
            let state = self.stats_state.clone();
            self.stats_read.slice(..).map_async(wgpu::MapMode::Read, move |r| state.store(if r.is_ok() { 2 } else { 0 }, Ordering::Release));
        }
        self.ring.poll(&self.gpu, sink, false)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "tile" => self.tile = value.as_i64().unwrap_or(8).clamp(1, 64) as u32,
            "max_steps" => self.world.cfg.max_steps = value.as_i64().unwrap_or(2048) as u32,
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
        self.beam_buf = Self::beam_buf(&self.gpu, width, height);
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.beam_buf, &self.stats_buf);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.width as u64 * self.height as u64 * 8 }
    }
}
