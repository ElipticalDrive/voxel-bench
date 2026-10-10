//! Sparse voxel DAG ray caster (Kampe et al. 2013).
//!
//! Each 32^3 chunk is built into a binary octree whose leaves are block ids. Interior
//! nodes (8 child references) are hash-consed in one global table shared by ALL chunks, so
//! identical subtrees (stone interiors, repeated structures) are stored once. A child
//! reference with the high bit set is an inline value (block id, 0 = air) - uniform
//! subtrees collapse to it. Chunk roots live in a GPU hash table. Nodes are append-only;
//! garbage from edits is reclaimed by a full rebuild once it exceeds the live size.
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::chunk::local_of_block;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, hash3, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "gpu-svo-dag".into(),
            description: "Sparse voxel DAG (Kampe et al. 2013): per-chunk 32^3 octrees hash-consed into one global DAG sharing identical subtrees; hierarchical empty-space skipping".into(),
            tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "tree".into(), "dag".into(), "wgpu".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(),
        },
        create: |p, w| Ok(Box::new(GpuSvoDag::new(p, w)?)),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("max_steps", "Traversal step limit per ray", 32, 16384, 1024, true),
        ParamSpec::int("nodes_mb", "DAG node buffer limit (MiB)", 16, 4096, 1024, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ]
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    adapter: Option<String>,
    shadows: Shadows,
    frames_in_flight: usize,
    max_steps: u32,
    nodes_mb: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, frames_in_flight: 3, max_steps: 1024, nodes_mb: 1024 }
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
    region_mask: u32,
    region_probe: u32,
    max_steps: u32,
    _p: [u32; 3],
}

const NO_REGION: u32 = 0xffff_ffff;
const LEAF: u32 = 0x8000_0000;

/// Builds one chunk's octree with chunk-local hash-consing. Children always precede their
/// parent, so merging into the global DAG in order only needs a single remap pass.
struct LocalDag {
    nodes: Vec<[u32; 8]>,
    map: HashMap<[u32; 8], u32>,
}

impl LocalDag {
    fn build(&mut self, get: &dyn Fn(IVec3) -> BlockId, origin: IVec3, size: i32) -> u32 {
        if size == 1 {
            return LEAF | get(origin).0 as u32;
        }
        let h = size / 2;
        let ch: [u32; 8] = std::array::from_fn(|i| self.build(get, origin + IVec3::new((i & 1) as i32, ((i >> 1) & 1) as i32, (i >> 2) as i32) * h, h));
        if ch[0] & LEAF != 0 && ch.iter().all(|&c| c == ch[0]) {
            return ch[0];
        }
        if let Some(&i) = self.map.get(&ch) {
            return i;
        }
        let i = self.nodes.len() as u32;
        self.nodes.push(ch);
        self.map.insert(ch, i);
        i
    }
}

fn build_chunk(chunk: &ChunkData) -> (Vec<[u32; 8]>, u32) {
    let mut d = LocalDag { nodes: vec![], map: HashMap::new() };
    let root = d.build(&|p| chunk.get(local_of_block(p)), IVec3::ZERO, 32);
    (d.nodes, root)
}

pub struct GpuSvoDag {
    gpu: Gpu,
    world: VoxelWorld,
    opts: Options,
    pipeline: wgpu::ComputePipeline,
    layout0: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    output: wgpu::Texture,
    ring: FrameRing,
    waker: Waker,
    width: u32,
    height: u32,
    node_buf: wgpu::Buffer,
    region_buf: wgpu::Buffer,
    /// Global hash-consed DAG (all chunks).
    dag: Vec<[u32; 8]>,
    dag_map: HashMap<[u32; 8], u32>,
    uploaded: usize,
    /// DAG size right after the last full rebuild (garbage-collection trigger).
    dag_base: usize,
    /// Per chunk: its tree size with only chunk-local sharing (for the dedup ratio).
    local_nodes: HashMap<IVec3, u32>,
    regions: HashMap<IVec3, u32>,
    dirty: HashSet<IVec3>,
    region_mask: u32,
    region_probe: u32,
    table_dirty: bool,
    bind0: Option<wgpu::BindGroup>,
}

impl GpuSvoDag {
    fn new(p: InitParams, waker: Waker) -> anyhow::Result<Self> {
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
                max_lod: 0, upload_voxels: false, pool_mb: 0, view_radius: p.view_radius_chunks, shadows: opts.shadows, max_steps: 0, visibility: wgpu::ShaderStages::COMPUTE },
        )?;
        let source = shader_source("", include_str!("dag.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("svo-dag"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let storage = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("svo-dag"),
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
                e(2, storage),
                e(4, storage),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("svo-dag"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("svo-dag"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let buf = |label, size: u64, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let st = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let depth = opts.frames_in_flight.max(1);
        Ok(Self {
            camera_buf: buf("camera", size_of::<CameraUniform>() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST),
            node_buf: buf("dag nodes", 1 << 20, st),
            dag: vec![],
            dag_map: HashMap::new(),
            uploaded: 0,
            dag_base: 0,
            local_nodes: HashMap::new(),
            region_buf: buf("regions", 16 * 1024, st),
            output: mr_gpu::output_texture(&gpu, p.width, p.height),
            ring: FrameRing::new(&gpu, p.width, p.height, depth, waker.clone()),
            gpu,
            world,
            opts,
            pipeline,
            layout0,
            waker,
            width: p.width,
            height: p.height,
            regions: HashMap::new(),
            dirty: HashSet::new(),
            region_mask: 0,
            region_probe: 0,
            table_dirty: true,
            bind0: None,
        })
    }

    fn rebuild(&mut self) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        // Nodes are append-only; once edits have doubled the DAG, rebuild it from scratch.
        if self.dag_base > 0 && self.dag.len() > 2 * self.dag_base + (1 << 16) {
            self.dag.clear();
            self.dag_map.clear();
            self.uploaded = 0;
            self.dag_base = 0;
            self.dirty.extend(self.regions.keys().copied());
        }
        let dirty: Vec<IVec3> = self.dirty.drain().collect();
        let world = &self.world;
        let built: Vec<(IVec3, Option<(Vec<[u32; 8]>, u32)>)> = dirty.par_iter().map(|&c| (c, world.chunk(c).map(build_chunk))).collect();
        for (c, tree) in built {
            self.regions.remove(&c);
            self.local_nodes.remove(&c);
            self.table_dirty = true;
            let Some((nodes, root)) = tree else { continue };
            let remap = |r: u32, map: &[u32]| if r & LEAF != 0 { r } else { map[r as usize] };
            let mut map = Vec::with_capacity(nodes.len());
            for n in &nodes {
                let n = n.map(|r| remap(r, &map));
                let i = *self.dag_map.entry(n).or_insert_with(|| {
                    self.dag.push(n);
                    self.dag.len() as u32 - 1
                });
                map.push(i);
            }
            let root = remap(root, &map);
            if root == LEAF {
                continue; // all air
            }
            self.local_nodes.insert(c, nodes.len() as u32);
            self.regions.insert(c, root);
        }
        if self.dag_base == 0 {
            self.dag_base = self.dag.len();
        }
        let bytes = (self.dag.len().max(1) * 32) as u64;
        if bytes > self.node_buf.size() {
            if bytes > (self.opts.nodes_mb as u64) << 20 {
                tracing::error!("DAG exceeds nodes_mb ({} MiB)", bytes >> 20);
            }
            self.node_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dag nodes"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.uploaded = 0;
            self.bind0 = None;
        }
        if self.uploaded < self.dag.len() {
            self.gpu.queue.write_buffer(&self.node_buf, self.uploaded as u64 * 32, bytemuck::cast_slice(&self.dag[self.uploaded..]));
            self.uploaded = self.dag.len();
        }
        dirty.len()
    }

    fn upload_table(&mut self) {
        let cap = (self.regions.len() * 2).next_power_of_two().max(16);
        let mut table = vec![[0u32, 0, 0, NO_REGION]; cap];
        let mut probe = 0;
        for (r, reg) in &self.regions {
            let mut i = hash3(*r) as usize & (cap - 1);
            let mut n = 0;
            while table[i][3] != NO_REGION {
                i = (i + 1) & (cap - 1);
                n += 1;
            }
            probe = probe.max(n);
            table[i] = [r.x as u32, r.y as u32, r.z as u32, *reg];
        }
        let bytes = (cap * 16) as u64;
        if bytes > self.region_buf.size() {
            self.region_buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("regions"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind0 = None;
        }
        self.gpu.queue.write_buffer(&self.region_buf, 0, bytemuck::cast_slice(&table));
        self.region_mask = cap as u32 - 1;
        self.region_probe = probe;
        self.table_dirty = false;
    }
}

impl Renderer for GpuSvoDag {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        for pos in self.world.apply(&self.gpu, updates) {
            self.dirty.insert(pos);
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let ticket = self.ring.begin(&self.gpu, sink)?;
        let start = Instant::now();
        let rebuilt = self.rebuild();
        if self.table_dirty {
            self.upload_table();
        }
        let build_ms = start.elapsed().as_secs_f64() * 1e3;
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
            region_mask: self.region_mask,
            region_probe: self.region_probe,
            max_steps: self.opts.max_steps,
            _p: [0; 3],
        };
        self.gpu.queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cu));
        let world_bg = self.world.prepare(&self.gpu, c).clone();
        if self.bind0.is_none() {
            let view = self.output.create_view(&Default::default());
            self.bind0 = Some(self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("svo-dag"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.node_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.region_buf.as_entire_binding() },
                ],
            }));
        }
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("svo-dag"), timestamp_writes: self.ring.compute_timestamps(ticket, true, true) });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let local: u64 = self.local_nodes.values().map(|&n| n as u64).sum();
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![
                ("chunks".into(), self.regions.len() as f64),
                ("dag_nodes".into(), self.dag.len() as f64),
                ("tree_nodes".into(), local as f64),
                ("dedup_ratio".into(), local as f64 / self.dag.len().max(1) as f64),
                ("chunks_rebuilt".into(), rebuilt as f64),
                ("build_ms".into(), build_ms),
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
            "max_steps" => self.opts.max_steps = value.as_i64().unwrap_or(1024) as u32,
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
        self.ring = FrameRing::new(&self.gpu, width, height, self.opts.frames_in_flight.max(1), self.waker.clone());
        self.bind0 = None;
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        // Report what the DAG actually occupies, not the reserved buffer.
        let nodes = self.dag.len() as u64 * 32;
        MemoryReport { cpu_bytes: nodes * 2, gpu_bytes: nodes + self.world.memory_bytes() }
    }
}

