//! Host side of the GPU storages, shared by the GPU back ends: keeps a storage's data on
//! the GPU in step with the world and binds it as group 2 (`aux`, `regions`).
//!
//! * **Pool storages** read the shared voxel pool (`VoxelWorld`, top cell = a 32^3 chunk)
//!   plus `aux_words` u32 of side data per pool slot, rebuilt per changed dense chunk.
//! * **Region storages** own their data: one variable-size blob of u32 words per
//!   2^`shift` region, built on the CPU (in parallel, for dirty regions only) and placed in
//!   `aux`; `regions` is a hash table from region coordinates to the blob's word offset.
//!   They need no voxel pool.
//! * **The DAG storage** (`svo_dag`) keeps one global hash-consed node pool in `aux`
//!   (append-only on the GPU too) and chunk roots in `regions`. No voxel pool either.
//! * **The texture storage** (`clipmap`) writes changed chunks into toroidal 3D textures,
//!   bound as group 2 bindings 4-7.

use std::collections::{HashMap, HashSet};

use glam::IVec3;
use mr_core::chunk::local_of_block;
use mr_core::{BlockId, CHUNK_VOLUME, ChunkData};
use mr_gpu::voxels::{VoxelWorld, hash3};
use mr_gpu::{Gpu, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;

use super::clipmap::Clipmap;
use super::svo_dag::Dag;
use super::{GpuStorage, Kind, RegionBlob};

const CHUNK_BYTES: u64 = CHUNK_VOLUME as u64 * 2;
const EMPTY: u32 = 0xffff_ffff;
const UNIFORM_BIT: u32 = 0x8000_0000;

/// Bind group 2 of a GPU back end's pipelines: `aux` (binding 0), `regions` (binding 1),
/// `aux4`, the same buffer as `aux` seen as 16-byte words (binding 2; region blobs are
/// 16-byte aligned, so 4-word records load in one access) and `region_head` (binding 3,
/// uniform: the table's mask and longest probe).
/// Texture storages add their own bindings (`clipmap`: 4-7).
pub fn layout(gpu: &Gpu, st: &GpuStorage, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayout {
    let entry = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    let head = wgpu::BindGroupLayoutEntry {
        binding: 3,
        visibility,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    let mut entries = vec![entry(0), entry(1), entry(2), head];
    if let Kind::Clipmap = st.kind {
        entries.extend(super::clipmap::layout_entries(visibility));
    }
    gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("storage"), entries: &entries })
}

/// What a storage needs to know about the run.
pub struct Limits {
    /// Side data / own data limit (MiB); pool storages size it per pool slot.
    pub mb: u32,
    /// Streaming radius in chunks (sizes the clipmap).
    pub view_radius: u32,
    /// More than 256 block types.
    pub wide_ids: bool,
}

struct Region {
    /// Range in `aux`, in 16-byte units (none for uniform regions).
    words: Option<(u32, u32)>,
    /// What `s_top` returns: the blob's word offset, or `UNIFORM_BIT | block`.
    handle: u32,
}

pub struct StorageData {
    st: GpuStorage,
    aux: GrowableBuffer,
    regions_buf: wgpu::Buffer,
    head_buf: wgpu::Buffer,
    bind: Option<wgpu::BindGroup>,
    // Region storages.
    alloc: RangeAlloc,
    regions: HashMap<IVec3, Region>,
    dirty: HashSet<IVec3>,
    table_dirty: bool,
    rebuilt: usize,
    overflowed: bool,
    dag: Dag,
    clip: Option<Clipmap>,
}

impl StorageData {
    pub fn new(gpu: &Gpu, st: GpuStorage, limits: Limits) -> Self {
        let limit_mb = limits.mb;
        let max = match st.kind {
            Kind::Pool { aux_words, .. } => (limit_mb as u64 * 1024 * 1024 / CHUNK_BYTES).max(1) * aux_words as u64 * 4,
            Kind::Region { .. } | Kind::Dag => limit_mb as u64 * 1024 * 1024,
            Kind::Clipmap => 256,
        };
        let aux = GrowableBuffer::new(gpu, "storage aux", 1 << 16, max.max(256), wgpu::BufferUsages::STORAGE);
        let regions_buf = Self::table_buffer(gpu, 16 * 16);
        let head_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("storage region head"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        // An empty table (mask 0, nothing found) until the first upload.
        gpu.queue.write_buffer(&regions_buf, 0, bytemuck::cast_slice(&[[0u32, 0, 0, EMPTY]]));
        gpu.queue.write_buffer(&head_buf, 0, bytemuck::cast_slice(&[0u32; 4]));
        Self {
            alloc: RangeAlloc::new((aux.size() / 16) as u32),
            st,
            aux,
            regions_buf,
            head_buf,
            bind: None,
            regions: HashMap::new(),
            dirty: HashSet::new(),
            table_dirty: true,
            rebuilt: 0,
            overflowed: false,
            dag: Dag::default(),
            clip: matches!(st.kind, Kind::Clipmap).then(|| Clipmap::new(gpu, limits.view_radius, limits.wide_ids)),
        }
    }

    fn table_buffer(gpu: &Gpu, size: u64) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("storage regions"), size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    /// Whether this storage reads the shared voxel pool (`VoxelWorldConfig::upload_voxels`).
    pub fn needs_pool(&self) -> bool {
        matches!(self.st.kind, Kind::Pool { .. })
    }

    /// After `VoxelWorld::apply`: brings the side data of the `changed` chunks up to date
    /// (pool storages) or marks their regions dirty (region storages; rebuilt in
    /// `prepare`).
    pub fn update(&mut self, gpu: &Gpu, world: &VoxelWorld, changed: &[IVec3]) -> anyhow::Result<()> {
        match self.st.kind {
            Kind::Pool { aux_words: 0, .. } => {}
            Kind::Pool { aux_words, build } => {
                let unit = aux_words as u64 * 4;
                // Keep the side data indexed like the pool: every changed dense chunk
                // rewrites its slot (slots of unloaded chunks are simply no longer read).
                if self.aux.reserve(gpu, world.pool_slots() as u64 * unit)? {
                    self.bind = None;
                }
                for &pos in changed {
                    if let (Some(slot), Some(ChunkData::Dense(d))) = (world.slot_of(pos), world.chunk(pos)) {
                        gpu.queue.write_buffer(self.aux.buffer(), slot as u64 * unit, bytemuck::cast_slice(&build(d)));
                    }
                }
            }
            Kind::Region { shift, .. } => {
                for &pos in changed {
                    self.dirty.insert(pos >> (shift - 5));
                }
            }
            Kind::Dag => self.dirty.extend(changed),
            Kind::Clipmap => {
                let clip = self.clip.as_mut().unwrap();
                for &pos in changed {
                    clip.store(gpu, pos, world.chunk(pos));
                }
            }
        }
        Ok(())
    }

    /// Rebuilds dirty regions, uploads the region table and returns the bind group.
    pub fn prepare(&mut self, gpu: &Gpu, world: &VoxelWorld, layout: &wgpu::BindGroupLayout) -> &wgpu::BindGroup {
        self.rebuilt = 0;
        match self.st.kind {
            Kind::Pool { .. } | Kind::Clipmap => {}
            Kind::Region { shift, build } => self.rebuild(gpu, world, shift, build),
            Kind::Dag => self.rebuild_dag(gpu, world),
        }
        if self.table_dirty && matches!(self.st.kind, Kind::Region { .. } | Kind::Dag) {
            self.upload_table(gpu);
        }
        if self.bind.is_none() {
            let mut entries = vec![
                wgpu::BindGroupEntry { binding: 0, resource: self.aux.buffer().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.regions_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.aux.buffer().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.head_buf.as_entire_binding() },
            ];
            if let Some(clip) = &self.clip {
                entries.extend(clip.bind_entries());
            }
            self.bind = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("storage"), layout, entries: &entries }));
        }
        self.bind.as_ref().unwrap()
    }

    fn rebuild(&mut self, gpu: &Gpu, world: &VoxelWorld, shift: u32, build: fn(&dyn Fn(IVec3) -> BlockId) -> RegionBlob) {
        if self.dirty.is_empty() {
            return;
        }
        let dirty: Vec<IVec3> = self.dirty.drain().collect();
        let per = 1i32 << (shift - 5);
        let built: Vec<(IVec3, RegionBlob)> = dirty
            .par_iter()
            .map(|&r| {
                // The region's chunks, x-fastest.
                let n = per as usize;
                let chunks: Vec<Option<&ChunkData>> = (0..n * n * n).map(|i| world.chunk(r * per + IVec3::new((i % n) as i32, (i / n % n) as i32, (i / (n * n)) as i32))).collect();
                if chunks.iter().all(|c| c.is_none()) {
                    return (r, RegionBlob::Empty);
                }
                let get = |p: IVec3| {
                    let c: IVec3 = p >> 5;
                    chunks[(c.x + (c.y + c.z * per) * per) as usize].map_or(BlockId::AIR, |ch| ch.get(local_of_block(p)))
                };
                (r, build(&get))
            })
            .collect();
        self.rebuilt = built.len();
        for (r, blob) in built {
            if let Some(old) = self.regions.remove(&r) {
                if let Some((off, n)) = old.words {
                    self.alloc.free(off, n);
                }
            }
            self.table_dirty = true;
            let region = match blob {
                RegionBlob::Empty => continue,
                RegionBlob::Uniform(b) => Region { words: None, handle: UNIFORM_BIT | b as u32 },
                RegionBlob::Words(w) => {
                    // In 16-byte units: blobs start 16-byte aligned (`aux4`).
                    let units = w.len().div_ceil(4) as u32;
                    let off = match self.aux.alloc(gpu, &mut self.alloc, units, 16) {
                        Ok((off, grew)) => {
                            if grew {
                                self.bind = None;
                            }
                            off
                        }
                        Err(e) => {
                            if !self.overflowed {
                                tracing::error!("{e:#}; raise `pool_mb` (the storage limit)");
                                self.overflowed = true;
                            }
                            continue;
                        }
                    };
                    gpu.queue.write_buffer(self.aux.buffer(), off as u64 * 16, bytemuck::cast_slice(&w));
                    Region { words: Some((off, units)), handle: off * 4 }
                }
            };
            self.regions.insert(r, region);
        }
    }

    fn rebuild_dag(&mut self, gpu: &Gpu, world: &VoxelWorld) {
        let changed = self.dag.rebuild(&mut self.dirty, &|p| world.chunk(p));
        if changed.is_empty() {
            return;
        }
        self.rebuilt = changed.len();
        self.table_dirty = true;
        for c in changed {
            match self.dag.roots.get(&c) {
                // A uniform root is an inline block: LEAF == UNIFORM_BIT.
                Some(&root) => self.regions.insert(c, Region { words: None, handle: root }),
                None => self.regions.remove(&c),
            };
        }
        // Append the new nodes (all of them after a garbage collection).
        let n = self.dag.nodes.len();
        match self.aux.reserve(gpu, n.max(1) as u64 * 32) {
            Ok(true) => self.bind = None,
            Ok(false) => {}
            Err(e) => {
                if !self.overflowed {
                    tracing::error!("{e:#}; raise `pool_mb` (the storage limit)");
                    self.overflowed = true;
                }
                return;
            }
        }
        if self.dag.uploaded < n {
            gpu.queue.write_buffer(self.aux.buffer(), self.dag.uploaded as u64 * 32, bytemuck::cast_slice(&self.dag.nodes[self.dag.uploaded..]));
            self.dag.uploaded = n;
        }
    }

    /// Open-addressing table of slots `(x, y, z, handle)`, with its mask and longest probe
    /// in `head_buf`; must match `region_top.wgsl`.
    fn upload_table(&mut self, gpu: &Gpu) {
        let cap = (self.regions.len() * 2).next_power_of_two().max(16);
        let mut table = vec![[0u32, 0, 0, EMPTY]; cap];
        let mut probe = 0u32;
        for (r, reg) in &self.regions {
            let mut i = hash3(*r) as usize & (cap - 1);
            let mut n = 0;
            while table[i][3] != EMPTY {
                i = (i + 1) & (cap - 1);
                n += 1;
            }
            probe = probe.max(n);
            table[i] = [r.x as u32, r.y as u32, r.z as u32, reg.handle];
        }
        gpu.queue.write_buffer(&self.head_buf, 0, bytemuck::cast_slice(&[cap as u32 - 1, probe, 0, 0]));
        let bytes = (table.len() * 16) as u64;
        if bytes > self.regions_buf.size() {
            self.regions_buf = Self::table_buffer(gpu, bytes.next_power_of_two());
            self.bind = None;
        }
        gpu.queue.write_buffer(&self.regions_buf, 0, bytemuck::cast_slice(&table));
        self.table_dirty = false;
    }

    /// GPU bytes the storage's own data occupies (pool storages: the side-data buffer;
    /// region storages: the blobs actually allocated, plus the table).
    pub fn memory_bytes(&self) -> u64 {
        match self.st.kind {
            Kind::Pool { aux_words: 0, .. } => 0,
            Kind::Pool { .. } => self.aux.size(),
            Kind::Region { .. } => self.regions.values().filter_map(|r| r.words).map(|(_, n)| n as u64 * 16).sum::<u64>() + self.regions_buf.size(),
            Kind::Dag => self.dag.nodes.len() as u64 * 32 + self.regions_buf.size(),
            Kind::Clipmap => self.clip.as_ref().map_or(0, |c| c.memory_bytes()),
        }
    }

    /// Per-frame counters (region storages).
    pub fn counters(&self) -> Vec<(String, f64)> {
        match self.st.kind {
            Kind::Pool { .. } => vec![],
            Kind::Region { .. } => vec![("regions".into(), self.regions.len() as f64), ("regions_rebuilt".into(), self.rebuilt as f64)],
            Kind::Dag => vec![
                ("chunks".into(), self.regions.len() as f64),
                ("dag_nodes".into(), self.dag.nodes.len() as f64),
                ("dedup_ratio".into(), self.dag.dedup_ratio()),
                ("chunks_rebuilt".into(), self.rebuilt as f64),
            ],
            Kind::Clipmap => vec![("clipmap_evictions".into(), self.clip.as_ref().map_or(0.0, |c| c.dropped as f64))],
        }
    }
}
