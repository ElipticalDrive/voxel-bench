//! GPU path tracer over the shared DDA voxel world: unlit = primary-hit textures; lit =
//! diffuse path tracing (sun/point-light NEE with shadow rays, ambient sky, cosine-weighted
//! bounces) with progressive temporal accumulation while camera and world are static.

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuPathTrace::new(p, w)?)) }
}

/// The same ray caster consuming LOD chunks: beyond the full-resolution region each
/// streamed LOD level doubles the cell size, so view distance grows geometrically.
fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-pathtrace".into(),
        description: "Compute-shader diffuse path tracer on the DDA voxel world: NEE shadow rays, cosine bounces, progressive accumulation".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "pathtrace".into(), "gi".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("bounces", "Diffuse indirect bounces (lit)", 0, 8, 1, true),
        ParamSpec::int("spp", "Paths per pixel per frame (lit)", 1, 64, 1, true),
        ParamSpec::bool("accumulate", "Average frames while camera and world are static", true, true),
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
    bounces: u32,
    spp: u32,
    accumulate: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, pool_mb: 2048, max_steps: 2048, frames_in_flight: 3, bounces: 1, spp: 1, accumulate: true }
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
    frame: u32,
    prev_count: u32,
    bounces: u32,
    spp: u32,
    _pad: [u32; 2],
}

pub struct GpuPathTrace {
    gpu: Gpu,
    world: VoxelWorld,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
    accum: wgpu::Buffer,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    /// Samples already averaged into `accum`; 0 forces a reset.
    count: u32,
    frame: u32,
    last: Option<(Camera, RenderMode)>,
}

impl GpuPathTrace {
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
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("pt.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pathtrace"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pt camera"),
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
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pathtrace"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pathtrace"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
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
        let accum = Self::accum_buffer(&gpu, p.width, p.height);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &accum);
        let ring = FrameRing::new(&gpu, p.width, p.height, opts.frames_in_flight.max(1), waker.clone());
        Ok(Self { gpu, world, pipeline, layout0, camera_buf, output, bind0, ring, width: p.width, height: p.height, depth: opts.frames_in_flight.max(1), waker,
            accum,
            bounces: opts.bounces.min(8),
            spp: opts.spp.clamp(1, 64),
            accumulate: opts.accumulate,
            count: 0,
            frame: 0,
            last: None,
        })
    }

    fn accum_buffer(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pt accum"),
            size: (width.max(1) as u64) * (height.max(1) as u64) * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, accum: &wgpu::Buffer) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pt camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: accum.as_entire_binding() },
            ],
        })
    }
}

impl Renderer for GpuPathTrace {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.count = 0;
        }
        self.world.apply(&self.gpu, updates);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let c = &req.camera;
        let (_, frac) = c.split_position();
        let key = (*c, req.mode);
        if !self.accumulate || self.last != Some(key) {
            self.count = 0;
        }
        self.last = Some(key);
        self.frame = self.frame.wrapping_add(1);
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
            frame: self.frame,
            prev_count: self.count,
            bounces: self.bounces,
            spp: self.spp,
            _pad: [0; 2],
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        // Cap so the running mean keeps adapting precision-wise; beyond this it is converged.
        self.count = (self.count + self.spp).min(1 << 20);
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pathtrace"),
                timestamp_writes: self.ring.compute_timestamps(ticket, true, true),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![
                ("dropped_chunks".into(), self.world.dropped_chunks() as f64),
                ("accumulated_spp".into(), self.count as f64),
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
            "bounces" => self.bounces = value.as_i64().unwrap_or(1).clamp(0, 8) as u32,
            "spp" => self.spp = value.as_i64().unwrap_or(1).clamp(1, 64) as u32,
            "accumulate" => self.accumulate = value.as_bool().unwrap_or(true),
            _ => anyhow::bail!("`{name}` cannot change while running"),
        }
        self.count = 0;
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.ring.poll(&self.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.output = mr_gpu::output_texture(&self.gpu, width, height);
        self.accum = Self::accum_buffer(&self.gpu, width, height);
        self.count = 0;
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.accum);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.width as u64 * self.height as u64 * 20 }
    }
}
