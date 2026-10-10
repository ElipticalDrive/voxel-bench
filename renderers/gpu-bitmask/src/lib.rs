//! GPU compute ray caster over per-chunk occupancy bitmasks: every dense 32^3 chunk also
//! stores 32x32 u32 rows (one bit per voxel along x); empty row runs are crossed with
//! one `firstTrailingBit`/`firstLeadingBit` instead of per-voxel steps.

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::meshes::{MESH_COMPOSITE_WGSL, MeshComposite};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use glam::IVec3;
use serde::Deserialize;

const ROW_BYTES: u64 = 1024 * 4;
const UNIFORM_BIT: u32 = mr_gpu::voxels::UNIFORM_BIT;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuBitmask::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-bitmask".into(),
        description: "Compute ray caster with per-chunk x-row occupancy bitmasks: bit-scan jumps over empty row runs, ids fetched only on hits".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "bitmask".into(), "wgpu".into(), "meshes".into()],
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

pub struct GpuBitmask {
    gpu: Gpu,
    world: VoxelWorld,
    meshes: MeshComposite,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    layout2: wgpu::BindGroupLayout,
    /// 1024 u32 rows (4 KiB) per voxel-pool slot, indexed like the pool.
    rows_buf: wgpu::Buffer,
    rows_slots: u32,
    bind2: wgpu::BindGroup,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    bind0: wgpu::BindGroup,
    ring: FrameRing,
    width: u32,
    height: u32,
    depth: usize,
    waker: Waker,
}

impl GpuBitmask {
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
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
            },
        )?;
        // Mesh mode: meshes rasterised first (shaded with voxel shadows), rays capped at their depth.
        let meshes = MeshComposite::new(&gpu, &world, p.width, p.height);
        let source = shader_source("", &format!("{MESH_COMPOSITE_WGSL}\n{}", include_str!("bitmask.wgsl")));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("bitmask"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bitmask camera"),
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
            ],
        });
        let layout2 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bitmask rows"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bitmask"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(&layout2), Some(meshes.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bitmask"),
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
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output);
        let ring = FrameRing::new(&gpu, p.width, p.height, opts.frames_in_flight.max(1), waker.clone());
        let (rows_buf, bind2) = Self::rows(&gpu, &layout2, 64);
        Ok(Self {
            meshes, gpu, world, pipeline, layout0, layout2, rows_buf, rows_slots: 64, bind2, camera_buf, output, bind0, ring, width: p.width, height: p.height, depth: opts.frames_in_flight.max(1), waker })
    }

    fn rows(gpu: &Gpu, layout: &wgpu::BindGroupLayout, slots: u32) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bitmask rows"),
            size: slots as u64 * ROW_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bitmask rows"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }],
        });
        (buf, bg)
    }

    /// Writes the row bitmask of the dense chunk at `pos` into its pool slot, if any.
    fn write_rows(&self, pos: IVec3) {
        let (Some(ChunkData::Dense(d)), Some(slot)) = (self.world.chunk(pos), self.world.table_lookup(pos)) else { return };
        if slot & UNIFORM_BIT != 0 || slot >= self.rows_slots {
            return;
        }
        // Pool index is x | z << 5 | y << 10, so row (y * 32 + z) is 32 consecutive ids.
        let mut rows = [0u32; 1024];
        for (row, ids) in rows.iter_mut().zip(d.0.chunks_exact(32)) {
            *row = ids.iter().enumerate().fold(0, |m, (x, b)| m | ((b.0 != 0) as u32) << x);
        }
        self.gpu.queue.write_buffer(&self.rows_buf, slot as u64 * ROW_BYTES, bytemuck::cast_slice(&rows));
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bitmask camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        })
    }
}

impl Renderer for GpuBitmask {
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.meshes.apply(update);
        Ok(())
    }

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        // Only changed chunks can hold newly allocated slots. If one lies beyond the row
        // buffer, grow it and rewrite every chunk's rows; otherwise rewrite the changed ones.
        let max_slot = changed.iter().filter_map(|p| self.world.table_lookup(*p)).filter(|s| s & UNIFORM_BIT == 0).max();
        if let Some(m) = max_slot
            && m >= self.rows_slots
        {
            let slots = (m + 1).next_power_of_two();
            (self.rows_buf, self.bind2) = Self::rows(&self.gpu, &self.layout2, slots);
            self.rows_slots = slots;
            let all: Vec<IVec3> = self.world.chunk_positions().copied().collect();
            for p in all {
                self.write_rows(p);
            }
        } else {
            for p in changed {
                self.write_rows(p);
            }
        }
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
        // With meshes, the mesh pass carries the frame's begin timestamp.
        let meshes_on = self.meshes.active(req.meshes);
        let ts = if meshes_on { self.ring.render_timestamps(ticket, true, false) } else { None };
        self.meshes.encode(&self.gpu, &mut enc, c, self.width as f32 / self.height as f32, req.mode, req.meshes, &world_bg, ts);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("bitmask"),
                timestamp_writes: self.ring.compute_timestamps(ticket, !meshes_on, true),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            pass.set_bind_group(2, &self.bind2, &[]);
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
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.rows_slots as u64 * ROW_BYTES + self.width as u64 * self.height as u64 * 4 }
    }
}
