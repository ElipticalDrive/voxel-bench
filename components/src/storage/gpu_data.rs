//! Host side of the GPU storages, shared by the GPU back ends: keeps a storage's data on
//! the GPU in step with the world and binds it as group 2 (`aux`, `regions`).
//!
//! * **Pool storages** read the shared voxel pool (`VoxelWorld`, top cell = a 32^3 chunk)
//!   plus `aux_words` u32 of side data per pool slot, rebuilt per changed dense chunk.
//! * **Region storages** own their data: one variable-size blob of u32 words per
//!   2^`shift` region, built on the CPU (in parallel, for dirty regions only) and placed in
//!   `aux`; `regions` is a hash table from region coordinates to the blob's word offset.
//!   They need no voxel pool.

use std::collections::{HashMap, HashSet};

use glam::IVec3;
use mr_core::chunk::local_of_block;
use mr_core::{BlockId, CHUNK_VOLUME, ChunkData};
use mr_gpu::voxels::{VoxelWorld, hash3};
use mr_gpu::{Gpu, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;

use super::{GpuStorage, Kind, RegionBlob};

const CHUNK_BYTES: u64 = CHUNK_VOLUME as u64 * 2;
const EMPTY: u32 = 0xffff_ffff;
const UNIFORM_BIT: u32 = 0x8000_0000;

/// Bind group 2 of a GPU back end's pipelines: `aux` (binding 0), `regions` (binding 1) and
/// `aux4`, the same buffer as `aux` seen as 16-byte words (binding 2; region blobs are
/// 16-byte aligned, so 4-word records load in one access).
pub fn layout(gpu: &Gpu, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayout {
    let entry = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("storage"), entries: &[entry(0), entry(1), entry(2)] })
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
    bind: Option<wgpu::BindGroup>,
    // Region storages.
    alloc: RangeAlloc,
    regions: HashMap<IVec3, Region>,
    dirty: HashSet<IVec3>,
    table_dirty: bool,
    rebuilt: usize,
    overflowed: bool,
}

impl StorageData {
    /// `limit_mb` caps the side data (pool storages: per pool slot up to the pool limit;
    /// region storages: all blobs).
    pub fn new(gpu: &Gpu, st: GpuStorage, limit_mb: u32) -> Self {
        let max = match st.kind {
            Kind::Pool { aux_words, .. } => (limit_mb as u64 * 1024 * 1024 / CHUNK_BYTES).max(1) * aux_words as u64 * 4,
            Kind::Region { .. } => limit_mb as u64 * 1024 * 1024,
        };
        let aux = GrowableBuffer::new(gpu, "storage aux", 1 << 16, max.max(256), wgpu::BufferUsages::STORAGE);
        let regions_buf = Self::table_buffer(gpu, 16 * 16);
        // An empty table (mask 0, nothing found) until the first upload.
        gpu.queue.write_buffer(&regions_buf, 0, bytemuck::cast_slice(&[[0u32, 0, 0, 0], [0, 0, 0, EMPTY]]));
        Self {
            alloc: RangeAlloc::new((aux.size() / 16) as u32),
            st,
            aux,
            regions_buf,
            bind: None,
            regions: HashMap::new(),
            dirty: HashSet::new(),
            table_dirty: true,
            rebuilt: 0,
            overflowed: false,
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
        }
        Ok(())
    }

    /// Rebuilds dirty regions, uploads the region table and returns the bind group.
    pub fn prepare(&mut self, gpu: &Gpu, world: &VoxelWorld, layout: &wgpu::BindGroupLayout) -> &wgpu::BindGroup {
        self.rebuilt = 0;
        if let Kind::Region { shift, build } = self.st.kind {
            self.rebuild(gpu, world, shift, build);
            if self.table_dirty {
                self.upload_table(gpu);
            }
        }
        if self.bind.is_none() {
            self.bind = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("storage"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.aux.buffer().as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.regions_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.aux.buffer().as_entire_binding() },
                ],
            }));
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

    /// Open-addressing table: entry 0 is the header (mask, max probe), entries `1 + i` the
    /// slots `(x, y, z, handle)`; must match `region_top.wgsl`.
    fn upload_table(&mut self, gpu: &Gpu) {
        let cap = (self.regions.len() * 2).next_power_of_two().max(16);
        let mut table = vec![[0u32, 0, 0, EMPTY]; cap + 1];
        let mut probe = 0u32;
        for (r, reg) in &self.regions {
            let mut i = hash3(*r) as usize & (cap - 1);
            let mut n = 0;
            while table[1 + i][3] != EMPTY {
                i = (i + 1) & (cap - 1);
                n += 1;
            }
            probe = probe.max(n);
            table[1 + i] = [r.x as u32, r.y as u32, r.z as u32, reg.handle];
        }
        table[0] = [cap as u32 - 1, probe, 0, 0];
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
        }
    }

    /// Per-frame counters (region storages).
    pub fn counters(&self) -> Vec<(String, f64)> {
        match self.st.kind {
            Kind::Pool { .. } => vec![],
            Kind::Region { .. } => vec![("regions".into(), self.regions.len() as f64), ("regions_rebuilt".into(), self.rebuilt as f64)],
        }
    }
}
