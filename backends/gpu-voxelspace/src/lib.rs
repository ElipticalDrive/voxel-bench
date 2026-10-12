//! GPU-voxelspace back end (components, IDEAS §12): storage `heightmap`, visibility
//! `columns`, shading `direct` or `pathtrace` (heightmap-gradient normals on column tops;
//! shadow and bounce rays walk the heightmap, `hm_trace`). Was the gpu-voxelspace renderer.
//!
//! Voxel Space (Comanche, 1992) style 2.5D renderer: a CPU-built heightmap ring (top
//! solid block height + block id per (x, z) column) marched front-to-back per screen
//! column in a compute shader, drawing spans with a per-column y-buffer.

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_components::shading::state::{self as shade_state, ShadeState};
use mr_components::storage::heightmap::{self, Heightmap};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuVoxelSpace::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-voxelspace".into(),
        description: "Comanche-style Voxel Space: per-column front-to-back heightmap march with a y-buffer (2.5D, no overhangs)".into(),
        tags: vec!["gpu".into(), "compute".into(), "heightmap".into(), "2.5d".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, vec![
            ParamSpec::choice("storage", "Storage component", &mr_components::names_on(BACKEND, mr_components::Slot::Storage), "heightmap", false),
            ParamSpec::choice("visibility", "Visibility component", &mr_components::names_on(BACKEND, mr_components::Slot::Visibility), "columns", false),
            ParamSpec::choice("shading", "Shading component", &mr_components::names_on(BACKEND, mr_components::Slot::Shading), "direct", false),
            shadows_param(),
            ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
        ]
        .into_iter()
        .chain(shade_state::params())
        .collect()),
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    storage: String,
    visibility: String,
    shading: String,
    shadows: Shadows,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    frames_in_flight: usize,
}

const BACKEND: &str = "gpu-voxelspace";

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, storage: "heightmap".into(), visibility: "columns".into(), shading: "direct".into(), shadows: Shadows::All, bounces: 1, spp: 1, accumulate: true, frames_in_flight: 3 }
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
    _p0: u32,
    max_cells: u32,
}

pub struct GpuVoxelSpace {
    gpu: Gpu,
    world: VoxelWorld,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    /// The heightmap storage: built on the CPU, uploaded to `hm_buf`; `hm_info` = mask,
    /// columns a walk may cross.
    hm: Heightmap,
    hm_buf: wgpu::Buffer,
    hm_info: wgpu::Buffer,
    /// Shading state (pathtrace's accumulation).
    accum: ShadeState,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuVoxelSpace {
    pub fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        use mr_components::{Slot, require};
        for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        let gpu = Gpu::new(GpuRequest {
            experimental: false,
            adapter: opts.adapter.as_deref(),
            required_features: wgpu::Features::empty(),
            optional_features: wgpu::Features::TIMESTAMP_QUERY,
            limits: &|a| a.clone(),
        })?;
        // Used for CPU chunk storage, the texture atlas and lighting parameters only.
        let world = VoxelWorld::new(
            &gpu,
            &p.blocks,
            p.environment,
            VoxelWorldConfig {
                max_lod: 0,
                upload_voxels: false,
                pool_mb: 64,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let hm = Heightmap::new(p.view_radius_chunks);
        let hm_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heightmap"),
            size: hm.hm.len() as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hm_info = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("heightmap info"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        gpu.queue.write_buffer(&hm_info, 0, bytemuck::cast_slice(&hm.info()));
        let shade = mr_components::shading::gpu_wgsl(&opts.shading).expect("validated");
        // Shadow and bounce rays walk the heightmap (its `ctrace`).
        let source = shader_source(mr_components::visibility::OCCLUDED_WGSL, &format!("{}\n{}\n{}\n{shade}", heightmap::gpu_wgsl(0, 2), ShadeState::wgsl(0, 4), include_str!("vs.wgsl")));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("voxelspace"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxelspace"),
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
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                ShadeState::layout_entries(4, wgpu::ShaderStages::COMPUTE)[0],
                ShadeState::layout_entries(4, wgpu::ShaderStages::COMPUTE)[1],
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxelspace"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("voxelspace"),
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
        let accum = ShadeState::new(&gpu, p.width, p.height, &opts.shading, opts.bounces, opts.spp, opts.accumulate);
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &hm_buf, &hm_info, &accum);
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone());
        Ok(Self { gpu, world, pipeline, layout0, camera_buf, hm, hm_buf, hm_info, accum, output, bind0, ring, width: p.width, height: p.height, depth, waker })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, hm: &wgpu::Buffer, hm_info: &wgpu::Buffer, accum: &ShadeState) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxelspace"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: hm.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: hm_info.as_entire_binding() },
                accum.bind_entries(4)[0].clone(),
                accum.bind_entries(4)[1].clone(),
            ],
        })
    }
}

impl Renderer for GpuVoxelSpace {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.accum.invalidate();
        }
        let changed = self.world.apply(&self.gpu, updates);
        let world = &self.world;
        for c in self.hm.apply(changed, |pos| world.chunk(pos)) {
            for (start, len) in self.hm.dirty_rows(c) {
                self.gpu.queue.write_buffer(&self.hm_buf, start as u64 * 4, bytemuck::cast_slice(&self.hm.hm[start..start + len]));
            }
        }
        self.gpu.queue.write_buffer(&self.hm_info, 0, bytemuck::cast_slice(&self.hm.info()));
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
            _p0: 0,
            max_cells: (self.world.cfg.view_radius * CHUNK_SIZE as u32) * 2,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
        self.accum.begin(&self.gpu, c, req.mode);
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("voxelspace"),
                timestamp_writes: self.ring.compute_timestamps(ticket, true, true),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(64), 1, 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: [("heightmap_size".into(), self.hm.size as f64), ("chunk_columns".into(), self.hm.columns() as f64)].into_iter().chain(self.accum.counter(req.mode)).collect(),
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            _ if self.accum.set_param(name, value).is_some() => {}
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
        self.accum.resize(&self.gpu, width, height);
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.hm_buf, &self.hm_info, &self.accum);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let hm = self.hm.hm.len() as u64 * 4;
        MemoryReport { cpu_bytes: hm, gpu_bytes: hm + self.world.memory_bytes() + self.accum.memory_bytes() + self.width as u64 * self.height as u64 * 4 }
    }
}
