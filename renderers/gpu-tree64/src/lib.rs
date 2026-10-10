//! Sparse 64-tree ray caster.
//!
//! The world is cut into 64^3 regions (2x2x2 chunks). Each region is a three-level
//! 64-tree (64 -> 16 -> 4 -> voxels): every node has a 4x4x4 child mask, children are
//! stored contiguously and addressed by popcount, uniform subtrees collapse to a single
//! node, and leaves store only their solid voxels' block ids. Regions live in a GPU hash
//! table. Rays skip the largest empty node containing them at every step.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::chunk::local_of_block;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_gpu::voxels::{Shadows, VoxelWorld, VoxelWorldConfig, hash3, shader_source, shadows_param};
use mr_gpu::{FrameRing, Gpu, GpuRequest, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "gpu-tree64".into(),
            description: "Sparse 64-tree per 64^3 region (popcount-packed children, compressed leaves, uniform collapse); hierarchical empty-space skipping".into(),
            tags: vec!["gpu".into(), "compute".into(), "raycast".into(), "tree".into(), "wgpu".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: params(),
        },
        create: |p, w| Ok(Box::new(GpuTree64::new(p, w)?)),
    }
}

fn params() -> Vec<ParamSpec> {
    vec![
        shadows_param(),
        ParamSpec::int("max_steps", "Traversal step limit per ray", 32, 16384, 1024, true),
        ParamSpec::int("nodes_mb", "Tree node buffer limit (MiB)", 16, 4096, 512, false),
        ParamSpec::int("leaves_mb", "Leaf voxel buffer limit (MiB)", 16, 4096, 1024, false),
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
    leaves_mb: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { adapter: None, shadows: Shadows::All, frames_in_flight: 3, max_steps: 1024, nodes_mb: 512, leaves_mb: 1024 }
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

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Default, PartialEq, Eq)]
struct Node {
    mask_lo: u32,
    mask_hi: u32,
    child: u32,
    block: u32,
}

const NO_REGION: u32 = 0xffff_ffff;

enum Built {
    Empty,
    Uniform(u16),
    Node(Node, bool),
}

/// Region builder output, with relative pointers.
struct RegionTree {
    nodes: Vec<Node>,
    /// Whether `nodes[i].child` points into the leaf-id array (true) or nodes (false).
    leaf: Vec<bool>,
    ids: Vec<u16>,
    root: u32,
}

struct Builder<'a> {
    get: &'a dyn Fn(IVec3) -> BlockId,
    nodes: Vec<Node>,
    leaf: Vec<bool>,
    ids: Vec<u16>,
}

fn bit(c: IVec3) -> u32 {
    (c.x | (c.z << 2) | (c.y << 4)) as u32
}

fn set(mask: &mut u64, b: u32) {
    *mask |= 1 << b;
}

impl Builder<'_> {
    fn build(&mut self, origin: IVec3, size: i32) -> Built {
        let cs = size / 4;
        let mut children = vec![];
        let mut mask = 0u64;
        let mut uniform: Option<Option<u16>> = None; // Some(Some(b)) while all children uniform b
        if cs == 1 {
            let mut ids = vec![];
            for y in 0..4 {
                for z in 0..4 {
                    for x in 0..4 {
                        let c = IVec3::new(x, y, z);
                        let b = (self.get)(origin + c);
                        if b.is_solid() {
                            ids.push((bit(c), b.0));
                        }
                    }
                }
            }
            if ids.is_empty() {
                return Built::Empty;
            }
            if ids.len() == 64 && ids.iter().all(|i| i.1 == ids[0].1) {
                return Built::Uniform(ids[0].1);
            }
            ids.sort_by_key(|i| i.0);
            let offset = self.ids.len() as u32;
            for (b, id) in ids {
                set(&mut mask, b);
                self.ids.push(id);
            }
            return Built::Node(Node { mask_lo: mask as u32, mask_hi: (mask >> 32) as u32, child: offset, block: 0 }, true);
        }
        for b in 0..64u32 {
            let c = IVec3::new((b & 3) as i32, ((b >> 4) & 3) as i32, ((b >> 2) & 3) as i32);
            let r = self.build(origin + c * cs, cs);
            uniform = match (&r, uniform) {
                (Built::Uniform(u), None) => Some(Some(*u)),
                (Built::Uniform(u), Some(Some(v))) if *u == v => Some(Some(v)),
                _ => Some(None),
            };
            match r {
                Built::Empty => {}
                Built::Uniform(u) => {
                    set(&mut mask, b);
                    children.push((Node { block: u as u32, ..Default::default() }, false));
                }
                Built::Node(n, leaf) => {
                    set(&mut mask, b);
                    children.push((n, leaf));
                }
            }
        }
        if mask == 0 {
            return Built::Empty;
        }
        if mask == u64::MAX {
            if let Some(Some(u)) = uniform {
                return Built::Uniform(u);
            }
        }
        let base = self.nodes.len() as u32;
        for (n, leaf) in children {
            self.nodes.push(n);
            self.leaf.push(leaf);
        }
        Built::Node(Node { mask_lo: mask as u32, mask_hi: (mask >> 32) as u32, child: base, block: 0 }, false)
    }
}

fn build_region(get: &dyn Fn(IVec3) -> BlockId) -> Option<RegionTree> {
    let mut b = Builder { get, nodes: vec![], leaf: vec![], ids: vec![] };
    let root = match b.build(IVec3::ZERO, 64) {
        Built::Empty => return None,
        Built::Uniform(u) => Node { block: u as u32, ..Default::default() },
        Built::Node(n, _) => n,
    };
    b.nodes.push(root);
    b.leaf.push(false);
    Some(RegionTree { root: b.nodes.len() as u32 - 1, nodes: b.nodes, leaf: b.leaf, ids: b.ids })
}

struct Region {
    nodes: (u32, u32),
    ids: (u32, u32),
    root: u32,
}

pub struct GpuTree64 {
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
    node_buf: GrowableBuffer,
    id_buf: GrowableBuffer,
    region_buf: wgpu::Buffer,
    node_alloc: RangeAlloc,
    id_alloc: RangeAlloc,
    regions: HashMap<IVec3, Region>,
    dirty: HashSet<IVec3>,
    region_mask: u32,
    region_probe: u32,
    table_dirty: bool,
    bind0: Option<wgpu::BindGroup>,
}

impl GpuTree64 {
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
        let source = shader_source("", include_str!("tree.wgsl"));
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("tree64"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let storage = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        let layout0 = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tree64"),
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
                e(3, storage),
                e(4, storage),
            ],
        });
        let pl = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tree64"),
            bind_group_layouts: &[Some(&layout0), Some(world.layout())],
            immediate_size: 0,
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("tree64"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let node_buf = GrowableBuffer::new(&gpu, "nodes", 1 << 20, (opts.nodes_mb as u64) << 20, wgpu::BufferUsages::STORAGE);
        let id_buf = GrowableBuffer::new(&gpu, "leaf ids", 1 << 20, (opts.leaves_mb as u64) << 20, wgpu::BufferUsages::STORAGE);
        let buf = |label, size: u64, usage| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let st = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let depth = opts.frames_in_flight.max(1);
        Ok(Self {
            camera_buf: buf("camera", size_of::<CameraUniform>() as u64, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST),
            node_alloc: RangeAlloc::new((node_buf.size() / 16) as u32),
            id_alloc: RangeAlloc::new((id_buf.size() / 2) as u32),
            node_buf,
            id_buf,
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
        let dirty: Vec<IVec3> = self.dirty.drain().collect();
        let world = &self.world;
        let built: Vec<(IVec3, Option<RegionTree>)> = dirty
            .par_iter()
            .map(|&r| {
                let chunks: [Option<&ChunkData>; 8] = std::array::from_fn(|i| world.chunk(r * 2 + IVec3::new((i & 1) as i32, ((i >> 1) & 1) as i32, (i >> 2) as i32)));
                if chunks.iter().all(|c| c.is_none()) {
                    return (r, None);
                }
                let get = |p: IVec3| {
                    let c: IVec3 = p >> 5;
                    chunks[(c.x | (c.y << 1) | (c.z << 2)) as usize].map(|ch| ch.get(local_of_block(p))).unwrap_or(BlockId::AIR)
                };
                (r, build_region(&get))
            })
            .collect();
        for (r, tree) in built {
            if let Some(old) = self.regions.remove(&r) {
                self.node_alloc.free(old.nodes.0, old.nodes.1);
                self.id_alloc.free(old.ids.0, old.ids.1);
            }
            self.table_dirty = true;
            let Some(mut t) = tree else { continue };
            // Leaf ids are u16 pairs; keep allocations word-aligned.
            if t.ids.len() % 2 == 1 {
                t.ids.push(0);
            }
            let nodes = self.node_buf.alloc(&self.gpu, &mut self.node_alloc, t.nodes.len() as u32, 16);
            let ids = self.id_buf.alloc(&self.gpu, &mut self.id_alloc, t.ids.len().max(2) as u32, 2);
            let ((nb, g1), (ib, g2)) = match (nodes, ids) {
                (Ok(a), Ok(b)) => (a, b),
                (a, b) => {
                    tracing::error!("tree buffers full ({:?}); raise nodes_mb / leaves_mb", a.err().or(b.err()));
                    continue;
                }
            };
            if g1 || g2 {
                self.bind0 = None;
            }
            for (n, &leaf) in t.nodes.iter_mut().zip(&t.leaf) {
                if n.block == 0 {
                    n.child += if leaf { ib } else { nb };
                }
            }
            self.gpu.queue.write_buffer(self.node_buf.buffer(), nb as u64 * 16, bytemuck::cast_slice(&t.nodes));
            if !t.ids.is_empty() {
                self.gpu.queue.write_buffer(self.id_buf.buffer(), ib as u64 * 2, bytemuck::cast_slice(&t.ids));
            }
            self.regions.insert(r, Region { nodes: (nb, t.nodes.len() as u32), ids: (ib, t.ids.len().max(2) as u32), root: nb + t.root });
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
            table[i] = [r.x as u32, r.y as u32, r.z as u32, reg.root];
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

impl Renderer for GpuTree64 {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        for pos in self.world.apply(&self.gpu, updates) {
            self.dirty.insert(pos.div_euclid(IVec3::splat(2)));
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
                label: Some("tree64"),
                layout: &self.layout0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: self.node_buf.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.id_buf.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: self.region_buf.as_entire_binding() },
                ],
            }));
        }
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("tree64"), timestamp_writes: self.ring.compute_timestamps(ticket, true, true) });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind0.as_ref().unwrap(), &[]);
            pass.set_bind_group(1, &world_bg, &[]);
            pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        }
        let nodes: u32 = self.regions.values().map(|r| r.nodes.1).sum();
        let ids: u32 = self.regions.values().map(|r| r.ids.1).sum();
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            counters: vec![
                ("regions".into(), self.regions.len() as f64),
                ("nodes".into(), nodes as f64),
                ("leaf_voxels".into(), ids as f64),
                ("regions_rebuilt".into(), rebuilt as f64),
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
        // Report what the tree actually occupies, not the reserved buffers.
        let nodes: u64 = self.regions.values().map(|r| r.nodes.1 as u64 * 16).sum();
        let ids: u64 = self.regions.values().map(|r| r.ids.1 as u64 * 2).sum();
        MemoryReport { cpu_bytes: 0, gpu_bytes: nodes + ids + self.world.memory_bytes() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_tree_queries_match_source() {
        let get = |p: IVec3| {
            if p.y < 10 || (p.x + p.z) % 7 == 0 && p.y < 30 {
                BlockId(1 + (p.x % 3) as u16)
            } else {
                BlockId::AIR
            }
        };
        let t = build_region(&get).unwrap();
        // CPU mirror of the shader's query.
        let query = |local: IVec3| -> u16 {
            let mut n = t.nodes[t.root as usize];
            let mut shift = 6;
            loop {
                if n.block != 0 {
                    return n.block as u16;
                }
                shift -= 2;
                let c = (local >> shift) & 3;
                let b = bit(c);
                let mask = n.mask_lo as u64 | (n.mask_hi as u64) << 32;
                if mask >> b & 1 == 0 {
                    return 0;
                }
                let idx = n.child + (mask & ((1u64 << b) - 1)).count_ones();
                if shift == 0 {
                    return t.ids[idx as usize];
                }
                n = t.nodes[idx as usize];
            }
        };
        for y in 0..64 {
            for z in (0..64).step_by(3) {
                for x in 0..64 {
                    let p = IVec3::new(x, y, z);
                    assert_eq!(query(p), get(p).0, "{p}");
                }
            }
        }
    }
}
