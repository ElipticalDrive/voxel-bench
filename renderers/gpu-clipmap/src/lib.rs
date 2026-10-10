//! GPU compute ray caster over a camera-centred clipmap: block ids live in a toroidal 3D
//! texture (addressed by absolute position modulo its size) with a 4^3-brick occupancy
//! texture and a per-chunk owner tag texture; rays DDA through them with `textureLoad`.

use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{DDA_OCCLUSION_WGSL, Shadows, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use glam::IVec3;
use std::collections::HashMap;
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, w| Ok(Box::new(GpuClipmap::new(p, w)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "gpu-clipmap".into(),
        description: "Compute-shader ray caster over a camera-centred toroidal 3D texture (clipmap) with 4^3 brick occupancy skipping".into(),
        tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "clipmap".into(), "wgpu".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("max_steps", "DDA step limit per ray", 32, 16384, 2048, true),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    /// Adapter name substring (default: prefer a discrete GPU).
    adapter: Option<String>,
    shadows: Shadows,
    max_steps: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, max_steps: 2048, frames_in_flight: 3 }
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

pub struct GpuClipmap {
    gpu: Gpu,
    world: VoxelWorld,
    clip: Clipmap,
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
}

impl GpuClipmap {
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
                upload_voxels: false,
                pool_mb: 64,
                view_radius: p.view_radius_chunks,
                shadows: opts.shadows,
                max_steps: opts.max_steps,
                visibility: wgpu::ShaderStages::COMPUTE,
            },
        )?;
        // The shared hash-table `trace` is renamed away; clipmap.wgsl defines its own.
        let source = shader_source(DDA_OCCLUSION_WGSL, include_str!("clipmap.wgsl")).replacen("fn trace(o:", "fn trace_hash(o:", 1);
        let clip = Clipmap::new(&gpu, p.view_radius_chunks, p.blocks.blocks.len() > 256);
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("clipmap"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clipmap camera"),
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
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("clipmap"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), Some(&clip.layout)],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("clipmap"),
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
        Ok(Self { gpu, world, clip, pipeline, layout0, camera_buf, output, bind0, ring, width: p.width, height: p.height, depth: opts.frames_in_flight.max(1), waker })
    }

    fn bind0(gpu: &Gpu, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer, out: &wgpu::Texture) -> wgpu::BindGroup {
        let view = out.create_view(&Default::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clipmap camera"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        })
    }
}

impl Renderer for GpuClipmap {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.world.apply(&self.gpu, updates);
        for pos in changed {
            self.clip.store(&self.gpu, pos, self.world.chunk(pos));
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
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("clipmap"),
                timestamp_writes: self.ring.compute_timestamps(ticket, true, true),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind0, &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(2, &self.clip.bind, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![("dropped_chunks".into(), self.clip.dropped as f64)],
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
        self.bind0 = Self::bind0(&self.gpu, &self.layout0, &self.camera_buf, &self.output);
        self.ring = FrameRing::new(&self.gpu, width, height, self.depth, self.waker.clone());
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.clip.memory_bytes() + self.width as u64 * self.height as u64 * 4 }
    }
}

/// Vertical chunk slots are capped (worlds are much wider than tall) to bound memory.
const MAX_Y_SLOTS: i32 = 16;

fn tag(c: IVec3) -> u32 {
    let u = c.to_array().map(|v| v as u32 & 1023);
    u[0] | (u[1] << 10) | (u[2] << 20) | 0x8000_0000
}

/// Camera-centred toroidal voxel volume. Chunk `c` lives in slot `c mod slots`; the tag
/// texture records which chunk owns each slot so stale or foreign data reads as air.
struct Clipmap {
    slots: IVec3,
    wide: bool,
    vox: wgpu::Texture,
    occ: wgpu::Texture,
    tags: wgpu::Texture,
    layout: wgpu::BindGroupLayout,
    bind: wgpu::BindGroup,
    owner: HashMap<IVec3, IVec3>,
    dropped: u64,
    scratch: Vec<u8>,
}

impl Clipmap {
    fn new(gpu: &Gpu, view_radius: u32, wide: bool) -> Self {
        let max = (gpu.device.limits().max_texture_dimension_3d / 32) as i32;
        let side = (2 * view_radius as i32 + 1).min(max).max(1);
        let slots = IVec3::new(side, side.min(MAX_Y_SLOTS), side);
        let tex = |label, n: i32, format| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: (slots.x * n) as u32, height: (slots.y * n) as u32, depth_or_array_layers: (slots.z * n) as u32 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let vox = tex("clipmap voxels", 32, if wide { wgpu::TextureFormat::R16Uint } else { wgpu::TextureFormat::R8Uint });
        let occ = tex("clipmap bricks", 8, wgpu::TextureFormat::R8Uint);
        let tags = tex("clipmap tags", 1, wgpu::TextureFormat::R32Uint);
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clipmap params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&[slots.x, slots.y, slots.z, 0]));
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false },
            count: None,
        };
        let layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clipmap"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                tex_entry(1),
                tex_entry(2),
                tex_entry(3),
            ],
        });
        let views = [&vox, &occ, &tags].map(|t| t.create_view(&Default::default()));
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clipmap"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[0]) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[1]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&views[2]) },
            ],
        });
        // Fresh textures are zero-initialised: every slot reads as empty.
        Self { slots, wide, vox, occ, tags, layout, bind, owner: HashMap::new(), dropped: 0, scratch: vec![] }
    }

    fn memory_bytes(&self) -> u64 {
        let cells = (self.slots.x * self.slots.y * self.slots.z) as u64;
        cells * (32 * 32 * 32 * if self.wide { 2 } else { 1 } + 8 * 8 * 8 + 4) + 16
    }

    fn write(gpu: &Gpu, tex: &wgpu::Texture, origin: IVec3, n: u32, bpp: u32, data: &[u8]) {
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: origin.x as u32, y: origin.y as u32, z: origin.z as u32 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * bpp), rows_per_image: Some(n) },
            wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
        );
    }

    /// Stores (or, for `None`, clears) LOD-0 chunk `pos` in its toroidal slot.
    fn store(&mut self, gpu: &Gpu, pos: IVec3, data: Option<&ChunkData>) {
        let slot = pos.rem_euclid(self.slots);
        let Some(data) = data else {
            if self.owner.get(&slot) == Some(&pos) {
                self.owner.remove(&slot);
                Self::write(gpu, &self.tags, slot, 1, 4, &0u32.to_le_bytes());
            }
            return;
        };
        if let Some(prev) = self.owner.insert(slot, pos)
            && prev != pos
        {
            self.dropped += 1;
        }
        let bpp = if self.wide { 2 } else { 1 };
        let buf = &mut self.scratch;
        buf.clear();
        buf.resize(32 * 32 * 32 * bpp, 0);
        let mut occ = [0u8; 512];
        // Texture order: x fastest, then y (rows), then z (layers).
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    let b = match data {
                        ChunkData::Uniform(b) => b.0,
                        ChunkData::Dense(d) => d.get(IVec3::new(x, y, z)).0,
                    };
                    if b != 0 {
                        let i = (x | (y << 5) | (z << 10)) as usize;
                        if self.wide {
                            buf[i * 2..i * 2 + 2].copy_from_slice(&b.to_le_bytes());
                        } else {
                            buf[i] = b as u8;
                        }
                        occ[((x >> 2) | ((y >> 2) << 3) | ((z >> 2) << 6)) as usize] = 1;
                    }
                }
            }
        }
        Self::write(gpu, &self.vox, slot * 32, 32, bpp as u32, buf);
        Self::write(gpu, &self.occ, slot * 8, 8, 1, &occ);
        Self::write(gpu, &self.tags, slot, 1, 4, &tag(pos).to_le_bytes());
    }
}
