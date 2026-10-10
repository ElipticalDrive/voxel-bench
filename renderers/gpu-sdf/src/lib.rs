//! Distance-field ray marching ("sphere tracing" with Chebyshev distances).
//!
//! For every voxel of a dense chunk the CPU computes the Chebyshev distance to the nearest
//! solid voxel of that chunk (separable transform, three 1D passes). On the GPU a ray in
//! a voxel at distance `k` leaves the whole empty `(2k-1)^3` cube around it in one step,
//! clipped to the chunk; empty chunks are skipped whole. Compare with gpu-dda's fixed
//! 4^3 brick bitmasks: big open spaces cost fewer steps, cluttered ones about the same.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::chunk::cell_index;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, UNIFORM_BIT, VoxelWorld, VoxelWorldConfig, shader_source, shadows_param};
use mr_gpu::meshes::{MESH_COMPOSITE_WGSL, MeshComposite};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, wgpu};
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "gpu-sdf".into(),
            description: "Distance-field marching: per-voxel Chebyshev distance to the nearest solid lets rays leap whole empty cubes".into(),
            tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "distance-field".into(), "wgpu".into(), "meshes".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(),
        },
        create: |p, w| Ok(Box::new(GpuSdf::new(p, w)?)),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("max_steps", "Marching step limit per ray", 32, 16384, 1024, true),
        ParamSpec::int("pool_mb", "Voxel pool limit (MiB)", 64, 6144, 2048, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    shadows: Shadows,
    pool_mb: u32,
    max_steps: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, pool_mb: 2048, max_steps: 1024, frames_in_flight: 3 }
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
    max_steps: u32,
    _p0: u32,
}

const DF_BYTES: u64 = CHUNK_VOLUME as u64;

/// Chebyshev distance (in voxels, capped at 32) from every cell to the nearest solid cell
/// of the chunk; 0 for solid cells. Separable: rows, then max-min passes along z and y.
pub fn chebyshev_df(chunk: &DenseChunk) -> Vec<u8> {
    const INF: i32 = 64;
    let n = 32usize;
    let mut a = vec![INF; CHUNK_VOLUME];
    // x: 1D distance to the nearest solid in the row.
    for y in 0..32 {
        for z in 0..32 {
            let mut last = -INF;
            for x in 0..32 {
                let i = cell_index(IVec3::new(x, y, z));
                if chunk.0[i].is_solid() {
                    last = x;
                }
                a[i] = (x - last).min(INF);
            }
            let mut next = 2 * INF;
            for x in (0..32).rev() {
                let i = cell_index(IVec3::new(x, y, z));
                if chunk.0[i].is_solid() {
                    next = x;
                }
                a[i] = a[i].min(next - x);
            }
        }
    }
    // z, then y: d(c) = min over c' on the line of max(|c - c'|, d_prev(c')).
    let pass = |src: &Vec<i32>, axis: usize| -> Vec<i32> {
        let mut out = vec![INF; CHUNK_VOLUME];
        let mut line = [0i32; 32];
        for u in 0..32 {
            for v in 0..32 {
                let at = |t: i32| -> usize {
                    let p = match axis {
                        2 => IVec3::new(u, v, t),
                        _ => IVec3::new(u, t, v),
                    };
                    cell_index(p)
                };
                for t in 0..32 {
                    line[t as usize] = src[at(t)];
                }
                for t in 0..32i32 {
                    let mut best = INF;
                    for s in 0..32i32 {
                        let cand = (t - s).abs().max(line[s as usize]);
                        best = best.min(cand);
                    }
                    out[at(t)] = best;
                }
            }
        }
        out
    };
    let a = pass(&a, 2);
    let a = pass(&a, 1);
    let _ = n;
    a.into_iter().map(|d| d.min(32) as u8).collect()
}

pub struct GpuSdf {
    gpu: Gpu,
    world: VoxelWorld,
    meshes: MeshComposite,
    opts: Options,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    dist: GrowableBuffer,
    output: wgpu::Texture,
    bind0: Option<wgpu::BindGroup>,
    ring: FrameRing,
    width: u32,
    height: u32,
    waker: Waker,
    dirty: HashSet<IVec3>,
    /// Latest field generation per chunk; results of older generations are dropped.
    generation: HashMap<IVec3, u64>,
    next_gen: u64,
    results: (crossbeam_channel::Sender<FieldResult>, crossbeam_channel::Receiver<FieldResult>),
    in_progress: usize,
}

struct FieldResult {
    pos: IVec3,
    slot: u32,
    generation: u64,
    field: Vec<u8>,
}

impl GpuSdf {
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
        let source = shader_source("", &format!("{MESH_COMPOSITE_WGSL}\n{}", include_str!("sdf.wgsl")));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("sdf"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sdf"),
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
                e(2, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sdf"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout()), None, Some(meshes.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sdf"),
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
        let dist = GrowableBuffer::new(&gpu, "distance field", 64 * DF_BYTES, opts.pool_mb as u64 * 1024 * 1024 / 2, wgpu::BufferUsages::STORAGE);
        let depth = opts.frames_in_flight.max(1);
        Ok(Self {
            meshes,
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            gpu,
            world,
            opts,
            pipeline,
            layout0,
            camera_buf,
            dist,
            bind0: None,
            width: p.width,
            height: p.height,
            waker,
            dirty: HashSet::new(),
            generation: HashMap::new(),
            next_gen: 0,
            results: crossbeam_channel::unbounded(),
            in_progress: 0,
        })
    }

    fn upload(&mut self, slot: u32, field: &[u8]) {
        match self.dist.reserve(&self.gpu, (slot as u64 + 1) * DF_BYTES) {
            Ok(true) => self.bind0 = None,
            Ok(false) => {}
            Err(e) => {
                tracing::error!("{e:#}");
                return;
            }
        }
        self.gpu.queue.write_buffer(self.dist.buffer(), slot as u64 * DF_BYTES, field);
    }

    /// Marks changed chunks "unknown" (rendered with plain stepping) and computes their
    /// fields in the background; finished fields are uploaded if still current.
    /// Returns (fields started, fields uploaded).
    fn update_fields(&mut self) -> (usize, usize) {
        let unknown = vec![255u8; CHUNK_VOLUME];
        let mut started = 0;
        for pos in std::mem::take(&mut self.dirty) {
            let Some(slot) = self.world.table_lookup(pos).filter(|s| s & UNIFORM_BIT == 0) else {
                self.generation.remove(&pos);
                continue;
            };
            let Some(ChunkData::Dense(d)) = self.world.chunk(pos) else { continue };
            let d: Arc<DenseChunk> = d.clone();
            self.next_gen += 1;
            let generation = self.next_gen;
            self.generation.insert(pos, generation);
            self.upload(slot, &unknown);
            let tx = self.results.0.clone();
            rayon::spawn(move || {
                let _ = tx.send(FieldResult { pos, slot, generation, field: chebyshev_df(&d) });
            });
            self.in_progress += 1;
            started += 1;
        }
        let mut uploaded = 0;
        while let Ok(r) = self.results.1.try_recv() {
            self.in_progress -= 1;
            let current = self.generation.get(&r.pos) == Some(&r.generation) && self.world.table_lookup(r.pos) == Some(r.slot);
            if current {
                self.upload(r.slot, &r.field);
                uploaded += 1;
            }
        }
        (started, uploaded)
    }
}

impl Renderer for GpuSdf {
    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.meshes.apply(update);
        Ok(())
    }

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        self.dirty.extend(self.world.apply(&self.gpu, updates));
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let (started, uploaded) = self.update_fields();
        let c = &req.camera;
        let (_, frac) = c.split_position();
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
            max_steps: self.opts.max_steps,
            _p0: 0,
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if self.bind0.is_none() {
            let view = self.output.create_view(&Default::default());
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sdf"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.dist.buffer().as_entire_binding() },
                ],
            }));
        }
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        // With meshes, the mesh pass carries the frame's begin timestamp.
        let meshes_on = self.meshes.active(req.meshes);
        let ts = if meshes_on { self.ring.render_timestamps(ticket, true, false) } else { None };
        self.meshes.encode(&self.gpu, &mut enc, c, self.width as f32 / self.height as f32, req.mode, req.meshes, &world_bg, ts);
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sdf"), timestamp_writes: self.ring.compute_timestamps(ticket, !meshes_on, true) });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.set_bind_group(3, self.meshes.bind_group(), &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters: vec![("fields_started".into(), started as f64), ("fields_uploaded".into(), uploaded as f64), ("fields_pending".into(), self.in_progress as f64)] };
        self.ring.submit(&self.gpu, enc, ticket, &self.output, req.frame_id, req.readback, stats);
        self.ring.poll(&self.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "shadows" => self.world.cfg.shadows.set(value)?,
            "max_steps" => self.opts.max_steps = value.as_i64().unwrap_or(1024) as u32,
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
        self.meshes.resize(&self.gpu, width, height);
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        self.bind0 = None;
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: 0, gpu_bytes: self.world.memory_bytes() + self.dist.size() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_field_matches_brute_force() {
        let mut c = DenseChunk::filled(BlockId::AIR);
        for p in [IVec3::new(3, 4, 5), IVec3::new(20, 30, 1), IVec3::new(31, 0, 31), IVec3::new(10, 10, 10)] {
            c.set(p, BlockId(1));
        }
        let solids: Vec<IVec3> = (0..CHUNK_VOLUME).map(mr_core::chunk::cell_of_index).filter(|p| c.get(*p).is_solid()).collect();
        let df = chebyshev_df(&c);
        for i in (0..CHUNK_VOLUME).step_by(37) {
            let p = mr_core::chunk::cell_of_index(i);
            let want = solids.iter().map(|s| (*s - p).abs().max_element()).min().unwrap().min(32);
            assert_eq!(df[i] as i32, want, "{p}");
        }
    }
}
