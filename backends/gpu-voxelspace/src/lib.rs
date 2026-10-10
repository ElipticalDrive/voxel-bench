//! GPU-voxelspace back end (components, IDEAS §12): storage `heightmap`, visibility
//! `columns`, shading `direct` (the reference lit model with heightmap-gradient normals on
//! column tops; shadow rays walk the GPU voxel pool). Was the gpu-voxelspace renderer.
//!
//! Voxel Space (Comanche, 1992) style 2.5D renderer: a CPU-built heightmap ring (top
//! solid block height + block id per (x, z) column) marched front-to-back per screen
//! column in a compute shader, drawing spans with a per-column y-buffer.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{IVec2, IVec3};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source};
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
        params: vec![
            ParamSpec::choice("storage", "Storage component", &mr_components::names_on(BACKEND, mr_components::Slot::Storage), "heightmap", false),
            ParamSpec::choice("visibility", "Visibility component", &mr_components::names_on(BACKEND, mr_components::Slot::Visibility), "columns", false),
            ParamSpec::choice("shading", "Shading component", &mr_components::names_on(BACKEND, mr_components::Slot::Shading), "direct", false),
            ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
        ],
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    storage: String,
    visibility: String,
    shading: String,
    frames_in_flight: usize,
}

const BACKEND: &str = "gpu-voxelspace";

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, storage: "heightmap".into(), visibility: "columns".into(), shading: "direct".into(), frames_in_flight: 3 }
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
    hm_mask: u32,
    max_cells: u32,
}

pub struct GpuVoxelSpace {
    gpu: Gpu,
    world: VoxelWorld,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    hm_buf: wgpu::Buffer,
    /// CPU copy of the heightmap ring, `size * size` entries.
    hm: Vec<u32>,
    size: i32,
    /// Loaded chunk y positions per chunk column.
    columns: HashMap<IVec2, Vec<i32>>,
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
                shadows: Shadows::None,
                max_steps: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        let size = (((2 * p.view_radius_chunks + 2) * CHUNK_SIZE as u32).next_power_of_two()) as i32;
        let hm = vec![0u32; (size * size) as usize];
        let hm_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heightmap"),
            size: hm.len() as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("vs.wgsl"));
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
        let bind0 = Self::bind0(&gpu, &layout0, &camera_buf, &output, &hm_buf);
        let depth = opts.frames_in_flight.max(1);
        let ring = FrameRing::new(&gpu, p.width, p.height, depth, waker.clone());
        Ok(Self { gpu, world, pipeline, layout0, camera_buf, hm_buf, hm, size, columns: HashMap::new(), output, bind0, ring, width: p.width, height: p.height, depth, waker })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture, hm: &wgpu::Buffer) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxelspace"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: hm.as_entire_binding() },
            ],
        })
    }

    /// Rebuilds the 32x32 heightmap tile of chunk column `c` and uploads it.
    fn rebuild_tile(&mut self, c: IVec2) {
        let mut ys = self.columns.get(&c).cloned().unwrap_or_default();
        ys.sort_unstable_by(|a, b| b.cmp(a));
        let mask = self.size - 1;
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let mut entry = 0u32;
                'found: for &cy in &ys {
                    if let Some(chunk) = self.world.chunk(IVec3::new(c.x, cy, c.y)) {
                        for ly in (0..CHUNK_SIZE).rev() {
                            let b = chunk.get(IVec3::new(lx, ly, lz));
                            if b != BlockId::AIR {
                                let y = cy * CHUNK_SIZE + ly;
                                entry = ((y + 32768).clamp(1, 65535) as u32) | ((b.0 as u32) << 16);
                                break 'found;
                            }
                        }
                    }
                }
                let wx = c.x * CHUNK_SIZE + lx;
                let wz = c.y * CHUNK_SIZE + lz;
                self.hm[((wz & mask) * self.size + (wx & mask)) as usize] = entry;
            }
        }
        for lz in 0..CHUNK_SIZE {
            let wz = c.y * CHUNK_SIZE + lz;
            let start = ((wz & mask) * self.size + ((c.x * CHUNK_SIZE) & mask)) as usize;
            self.gpu.queue.write_buffer(&self.hm_buf, start as u64 * 4, bytemuck::cast_slice(&self.hm[start..start + CHUNK_SIZE as usize]));
        }
    }
}

impl Renderer for GpuVoxelSpace {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        let mut tiles = HashSet::new();
        for pos in changed {
            let c = IVec2::new(pos.x, pos.z);
            let ys = self.columns.entry(c).or_default();
            let held = self.world.chunk(pos).is_some();
            match (held, ys.iter().position(|&y| y == pos.y)) {
                (true, None) => ys.push(pos.y),
                (false, Some(i)) => {
                    ys.swap_remove(i);
                }
                _ => {}
            }
            if ys.is_empty() {
                self.columns.remove(&c);
            }
            tiles.insert(c);
        }
        for c in tiles {
            self.rebuild_tile(c);
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
            hm_mask: (self.size - 1) as u32,
            max_cells: (self.world.cfg.view_radius * CHUNK_SIZE as u32) * 2,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cam));
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
            counters: vec![("heightmap_size".into(), self.size as f64), ("chunk_columns".into(), self.columns.len() as f64)],
        };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
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
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output, &self.hm_buf);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let hm = self.hm.len() as u64 * 4;
        MemoryReport { cpu_bytes: hm, gpu_bytes: hm + self.world.memory_bytes() + self.width as u64 * self.height as u64 * 4 }
    }
}
