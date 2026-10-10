//! gpu-frag-dda: the gpu-dda three-level DDA ray caster, dispatched as a fragment shader
//! on a full-screen triangle so compute vs raster dispatch can be compared directly.

mod hybrid;

pub use hybrid::entry_hybrid;

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuFragDda::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-frag-dda".into(),
        description: "gpu-dda's ray caster in a fragment shader over a full-screen triangle (render pipeline instead of compute)".into(),
        tags: vec!["gpu".into(), "fragment".into(), "raycast".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
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
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, pool_mb: 2048, max_steps: 2048, frames_in_flight: 3 }
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
    _pad: [u32; 2],
}

pub struct GpuFragDda {
    gpu: Gpu,
    world: VoxelWorld,
    pipeline: wgpu::RenderPipeline,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuFragDda {
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
                visibility: wgpu::ShaderStages::FRAGMENT,
            },
        )?;
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("frag_dda.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("dda"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dda camera"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dda"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("frag dda"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let camera_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = mr_gpu::output_texture(&gpu, p.width, p.height);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf);
        let ring = FrameRing::new(&gpu, p.width, p.height, opts.frames_in_flight.max(1), waker.clone());
        Ok(Self { gpu, world, pipeline, camera_buf, output, bind0, ring, width: p.width, height: p.height, depth: opts.frames_in_flight.max(1), waker })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frag dda camera"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() }],
        })
    }
}

impl Renderer for GpuFragDda {
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
            _pad: [0; 2],
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let out_view = self.output.create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frag dda"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: self.ring.render_timestamps(ticket, true, true),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.draw(0..3, 0..1);
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
        
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.width as u64 * self.height as u64 * 4 }
    }
}
