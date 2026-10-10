//! GPU-resident voxel world shared by GPU renderers (bind group 1 in `voxels.wgsl`).
//!
//! * Chunk table: open-addressing hash map (chunk coords -> pool slot or uniform block),
//!   mirrored on the CPU and patched entry-by-entry.
//! * Pool: fixed-capacity array of dense 32^3 u16 chunks.
//! * Bricks: one bit per 4^3 brick per chunk (512 bits), for empty-space skipping.
//! * Lights: hashed world-space grid of 16^3 cells listing the lights reaching each cell.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use mr_core::chunk::{cell_of_index, local_of_block};
use mr_core::{BlockId, BlockRegistry, CHUNK_VOLUME, Camera, ChunkData, ChunkKey, Environment, Light, WorldUpdate};
use serde::Deserialize;

use crate::{Gpu, GrowableBuffer};

pub const VOXELS_WGSL: &str = include_str!("voxels.wgsl");
pub const LIGHTING_WGSL: &str = include_str!("lighting.wgsl");
/// Shadow rays answered by the software DDA in `voxels.wgsl`.
pub const DDA_OCCLUSION_WGSL: &str = "fn occluded(p: vec3<f32>, d: vec3<f32>, max_t: f32) -> bool { return trace(p, d, max_t).hit; }\n";

/// Full shader source: shared helpers, voxel world, an occlusion module, lighting and
/// the renderer's own code.
pub fn shader_source(occlusion: &str, own: &str) -> String {
    format!("{}\n{}\n{}\n{}\n{}", crate::COMMON_WGSL, VOXELS_WGSL, occlusion, LIGHTING_WGSL, own)
}

const EMPTY: u32 = 0xffff_ffff;
const TOMB: u32 = 0xffff_fffe;
pub const UNIFORM_BIT: u32 = 0x8000_0000;
const CHUNK_BYTES: u64 = CHUNK_VOLUME as u64 * 2;
const BRICK_WORDS: usize = 16;
const LIGHT_CELL_BITS: i32 = 4;

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Shadows {
    None,
    Sun,
    #[default]
    All,
}

impl Shadows {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Shadows::None,
            "sun" => Shadows::Sun,
            "all" => Shadows::All,
            _ => return None,
        })
    }

    /// Applies a `shadows` choice parameter.
    pub fn set(&mut self, v: &mr_core::ParamValue) -> anyhow::Result<()> {
        *self = v.as_str().and_then(Self::parse).ok_or_else(|| anyhow::anyhow!("bad shadows value"))?;
        Ok(())
    }
}

/// The `shadows` knob shared by GPU renderers.
pub fn shadows_param() -> mr_core::ParamSpec {
    mr_core::ParamSpec::choice("shadows", "Shadow rays in lit mode", &["none", "sun", "all"], "all", true)
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Params {
    cam_block: [i32; 3],
    max_steps: u32,
    bounds_min: [i32; 3],
    table_mask: u32,
    bounds_max: [i32; 3],
    max_probe: u32,
    sun_dir: [f32; 3],
    has_sun: u32,
    sun_color: [f32; 3],
    max_dist: f32,
    ambient: [f32; 3],
    exposure: f32,
    light_mask: u32,
    light_max_probe: u32,
    shadows: u32,
    max_lod: u32,
    lod_radius: u32,
    _lod_pad: [u32; 3],
}

// Keep the WGSL struct (`VoxelParams`) and this one in sync.
const _: () = assert!(size_of::<Params>() == 128);

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GpuLight {
    block: [i32; 3],
    radius: f32,
    frac: [f32; 3],
    intensity: f32,
    color: [f32; 3],
    _pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LightCell {
    key: [i32; 3],
    start: u32,
    count: u32,
    _pad: [u32; 3],
}

pub fn hash3(c: IVec3) -> u32 {
    let h = (c.x as u32).wrapping_mul(0x8da6b343) ^ (c.y as u32).wrapping_mul(0xd8163841) ^ (c.z as u32).wrapping_mul(0xcb1ab31f);
    h ^ (h >> 16)
}

/// CPU mirror of the GPU chunk hash table.
struct Table {
    entries: Vec<[u32; 4]>,
    live: usize,
    tombs: usize,
    max_probe: u32,
    dirty: Vec<u32>,
    full_upload: bool,
}

impl Table {
    fn new(cap: usize) -> Self {
        Self { entries: vec![[0, 0, 0, EMPTY]; cap], live: 0, tombs: 0, max_probe: 0, dirty: vec![], full_upload: true }
    }

    fn mask(&self) -> usize {
        self.entries.len() - 1
    }

    fn find(&self, c: IVec3) -> Option<usize> {
        let mut i = hash3(c) as usize & self.mask();
        for _ in 0..=self.max_probe {
            let e = self.entries[i];
            if e[3] == EMPTY {
                return None;
            }
            if e[3] != TOMB && [e[0] as i32, e[1] as i32, e[2] as i32] == c.to_array() {
                return Some(i);
            }
            i = (i + 1) & self.mask();
        }
        None
    }

    fn get(&self, c: IVec3) -> Option<u32> {
        self.find(c).map(|i| self.entries[i][3])
    }

    fn insert(&mut self, c: IVec3, v: u32) {
        if let Some(i) = self.find(c) {
            self.entries[i][3] = v;
            self.dirty.push(i as u32);
            return;
        }
        if (self.live + self.tombs + 1) * 2 > self.entries.len() {
            let cap = if (self.live + 1) * 4 > self.entries.len() { self.entries.len() * 2 } else { self.entries.len() };
            self.rehash(cap);
        }
        let mut i = hash3(c) as usize & self.mask();
        let mut probe = 0;
        while self.entries[i][3] != EMPTY && self.entries[i][3] != TOMB {
            i = (i + 1) & self.mask();
            probe += 1;
        }
        if self.entries[i][3] == TOMB {
            self.tombs -= 1;
        }
        self.entries[i] = [c.x as u32, c.y as u32, c.z as u32, v];
        self.live += 1;
        self.max_probe = self.max_probe.max(probe);
        self.dirty.push(i as u32);
    }

    fn remove(&mut self, c: IVec3) {
        if let Some(i) = self.find(c) {
            self.entries[i][3] = TOMB;
            self.live -= 1;
            self.tombs += 1;
            self.dirty.push(i as u32);
        }
    }

    fn rehash(&mut self, cap: usize) {
        let old = std::mem::take(&mut self.entries);
        *self = Self::new(cap);
        for e in old {
            if e[3] != EMPTY && e[3] != TOMB {
                self.insert(IVec3::new(e[0] as i32, e[1] as i32, e[2] as i32), e[3]);
            }
        }
        self.dirty.clear();
        self.full_upload = true;
    }
}

struct GrowBuf {
    buf: wgpu::Buffer,
    label: &'static str,
}

impl GrowBuf {
    fn new(gpu: &Gpu, label: &'static str, size: u64) -> Self {
        Self { buf: storage(gpu, label, size.max(256)), label }
    }

    /// Uploads `data`, reallocating if needed. Returns true if the buffer was replaced.
    fn write(&mut self, gpu: &Gpu, data: &[u8]) -> bool {
        let mut grew = false;
        if data.len() as u64 > self.buf.size() {
            self.buf = storage(gpu, self.label, (data.len() as u64).next_power_of_two());
            grew = true;
        }
        if !data.is_empty() {
            gpu.queue.write_buffer(&self.buf, 0, data);
        }
        grew
    }
}

fn storage(gpu: &Gpu, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.next_multiple_of(16),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Chunk-table key of a chunk at a LOD: the LOD is folded into y (`y * 8 + lod`).
/// Mirrored by `chunk_key` in `voxels.wgsl`.
pub fn table_key(key: ChunkKey) -> IVec3 {
    IVec3::new(key.pos.x, key.pos.y * 8 + key.lod as i32, key.pos.z)
}

pub struct VoxelWorldConfig {
    /// Highest LOD level stored; coarser chunk loads are ignored.
    pub max_lod: u8,
    /// Upload voxel data (pool, bricks, chunk table) for GPU traversal. When false only
    /// textures, lights and the CPU-side chunk mirror are maintained.
    pub upload_voxels: bool,
    pub pool_mb: u32,
    pub view_radius: u32,
    pub shadows: Shadows,
    pub max_steps: u32,
    pub visibility: wgpu::ShaderStages,
}

pub struct VoxelWorld {
    layout: wgpu::BindGroupLayout,
    bind_group: Option<wgpu::BindGroup>,
    params: wgpu::Buffer,
    table: Table,
    table_buf: wgpu::Buffer,
    pool: GrowableBuffer,
    bricks: GrowableBuffer,
    free_slots: Vec<u32>,
    /// Slots currently allocated (grows on demand up to `max_slots`).
    pool_slots: u32,
    max_slots: u32,
    chunks: HashMap<ChunkKey, ChunkData>,
    slots: HashMap<ChunkKey, u32>,
    /// Loaded region in blocks (min, max exclusive), over all LODs.
    bounds: Option<(IVec3, IVec3)>,
    bounds_dirty: bool,
    face_tex: wgpu::Buffer,
    texels: wgpu::Buffer,
    lights: HashMap<u32, Light>,
    lights_dirty: bool,
    light_buf: GrowBuf,
    cell_buf: GrowBuf,
    idx_buf: GrowBuf,
    light_mask: u32,
    light_max_probe: u32,
    pub env: Environment,
    pub cfg: VoxelWorldConfig,
    dropped_chunks: u64,
}

impl VoxelWorld {
    pub fn new(gpu: &Gpu, blocks: &BlockRegistry, env: Environment, cfg: VoxelWorldConfig) -> anyhow::Result<Self> {
        let limits = gpu.device.limits();
        let max_bytes = (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
        let max_slots = if cfg.upload_voxels { ((cfg.pool_mb as u64 * 1024 * 1024).min(max_bytes) / CHUNK_BYTES).max(1) as u32 } else { 1 };
        let pool_slots = max_slots.min(64);
        if cfg.upload_voxels {
            tracing::info!("voxel pool: grows up to {max_slots} chunks ({} MiB)", max_slots as u64 * CHUNK_BYTES >> 20);
        }
        let side = 2 * cfg.view_radius as usize + 1;
        let table = Table::new((side * side * side * 2 * (cfg.max_lod as usize + 1)).next_power_of_two().max(1024));

        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: cfg.visibility, ty, count: None };
        let ro = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        let mut entries = vec![entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None })];
        entries.extend((1..=8).map(|b| entry(b, ro)));
        let layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("voxel world"), entries: &entries });

        let mk = |label, data: &[u8]| {
            let b = storage(gpu, label, data.len() as u64);
            gpu.queue.write_buffer(&b, 0, data);
            b
        };
        let face_tex = mk("face table", bytemuck::cast_slice(&blocks.face_table()));
        let texels = mk("texels", bytemuck::cast_slice(&blocks.packed_texels()));
        Ok(Self {
            params: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("voxel params"),
                size: size_of::<Params>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            table_buf: storage(gpu, "chunk table", (table.entries.len() * 16) as u64),
            table,
            pool: GrowableBuffer::new(gpu, "voxel pool", pool_slots as u64 * CHUNK_BYTES, max_slots as u64 * CHUNK_BYTES, wgpu::BufferUsages::STORAGE),
            bricks: GrowableBuffer::new(gpu, "bricks", pool_slots as u64 * BRICK_WORDS as u64 * 4, max_slots as u64 * BRICK_WORDS as u64 * 4, wgpu::BufferUsages::STORAGE),
            free_slots: (0..pool_slots).rev().collect(),
            pool_slots,
            max_slots,
            chunks: HashMap::new(),
            slots: HashMap::new(),
            bounds: None,
            bounds_dirty: false,
            face_tex,
            texels,
            lights: HashMap::new(),
            lights_dirty: true,
            light_buf: GrowBuf::new(gpu, "lights", 256),
            cell_buf: GrowBuf::new(gpu, "light cells", 256),
            idx_buf: GrowBuf::new(gpu, "light indices", 256),
            light_mask: 0,
            light_max_probe: 0,
            layout,
            bind_group: None,
            env,
            cfg,
            dropped_chunks: 0,
        })
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// LOD-0 chunk data currently held, for renderers that also need CPU access.
    pub fn chunk(&self, pos: IVec3) -> Option<&ChunkData> {
        self.chunks.get(&ChunkKey::lod0(pos))
    }

    pub fn chunk_positions(&self) -> impl Iterator<Item = &IVec3> {
        self.chunks.keys().filter(|k| k.lod == 0).map(|k| &k.pos)
    }

    pub fn block(&self, p: IVec3) -> BlockId {
        self.chunks.get(&ChunkKey::of_block(p)).map(|c| c.get(local_of_block(p))).unwrap_or(BlockId::AIR)
    }

    /// Chunks held per LOD level.
    pub fn chunks_per_lod(&self) -> Vec<usize> {
        let mut v = vec![0; self.cfg.max_lod as usize + 1];
        for k in self.chunks.keys() {
            v[k.lod as usize] += 1;
        }
        v
    }

    pub fn memory_bytes(&self) -> u64 {
        (if self.cfg.upload_voxels { self.pool.size() + self.bricks.size() } else { 0 }) + self.table_buf.size() + self.light_buf.buf.size() + self.cell_buf.buf.size() + self.idx_buf.buf.size()
    }

    /// Applies world updates. Returns the LOD-0 chunk positions whose contents changed
    /// (loaded, unloaded or edited), for renderers keeping derived data (meshes).
    pub fn apply(&mut self, gpu: &Gpu, updates: Vec<WorldUpdate>) -> Vec<IVec3> {
        let mut changed = vec![];
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod <= self.cfg.max_lod => {
                    self.set_chunk(gpu, key, data);
                    if key.lod == 0 {
                        changed.push(key.pos);
                    }
                }
                WorldUpdate::ChunkUnload { key } if key.lod <= self.cfg.max_lod => {
                    self.set_chunk(gpu, key, ChunkData::Uniform(BlockId::AIR));
                    if key.lod == 0 {
                        changed.push(key.pos);
                    }
                }
                WorldUpdate::ChunkLoad { .. } | WorldUpdate::ChunkUnload { .. } => {}
                WorldUpdate::SetBlocks(edits) => {
                    let mut touched: HashMap<IVec3, ChunkData> = HashMap::new();
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        let Some(c) = touched.get_mut(&pos) else {
                            if let Some(c) = self.chunks.get(&ChunkKey::lod0(pos)) {
                                let mut c = c.clone();
                                c.set(local_of_block(e.pos), e.block);
                                touched.insert(pos, c);
                            }
                            continue;
                        };
                        c.set(local_of_block(e.pos), e.block);
                    }
                    for (pos, c) in touched {
                        self.set_chunk(gpu, ChunkKey::lod0(pos), c.compact());
                        changed.push(pos);
                    }
                }
                WorldUpdate::LightUpsert(ls) => {
                    for l in ls {
                        self.lights.insert(l.id, l);
                    }
                    self.lights_dirty = true;
                }
                WorldUpdate::LightRemove(ids) => {
                    for id in ids {
                        self.lights.remove(&id);
                    }
                    self.lights_dirty = true;
                }
                WorldUpdate::SetEnvironment(e) => self.env = e,
            }
        }
        changed
    }

    fn set_chunk(&mut self, gpu: &Gpu, pos: ChunkKey, data: ChunkData) {
        let tk = table_key(pos);
        if let Some(slot) = self.slots.remove(&pos) {
            self.free_slots.push(slot);
        }
        self.bounds_dirty = true;
        if !self.cfg.upload_voxels {
            if data.is_air() {
                self.chunks.remove(&pos);
            } else {
                self.chunks.insert(pos, data);
            }
            return;
        }
        match &data {
            ChunkData::Uniform(BlockId::AIR) => {
                self.table.remove(tk);
                self.chunks.remove(&pos);
                return;
            }
            ChunkData::Uniform(b) => self.table.insert(tk, UNIFORM_BIT | b.0 as u32),
            ChunkData::Dense(d) => {
                if self.free_slots.is_empty() && self.pool_slots < self.max_slots {
                    let new = (self.pool_slots * 2).min(self.max_slots);
                    let grown = self.pool.reserve(gpu, new as u64 * CHUNK_BYTES).and_then(|a| Ok(a | self.bricks.reserve(gpu, new as u64 * BRICK_WORDS as u64 * 4)?));
                    match grown {
                        Ok(_) => {
                            self.free_slots.extend((self.pool_slots..new).rev());
                            self.pool_slots = new;
                            self.bind_group = None;
                        }
                        Err(e) => tracing::error!("growing voxel pool: {e:#}"),
                    }
                }
                let Some(slot) = self.free_slots.pop() else {
                    if self.dropped_chunks == 0 {
                        tracing::error!("voxel pool full ({} chunks); raise `pool_mb`", self.pool_slots);
                    }
                    self.dropped_chunks += 1;
                    self.table.remove(tk);
                    self.chunks.remove(&pos);
                    return;
                };
                gpu.queue.write_buffer(self.pool.buffer(), slot as u64 * CHUNK_BYTES, bytemuck::cast_slice(d.as_u16()));
                let mut bits = [0u32; BRICK_WORDS];
                for (i, b) in d.0.iter().enumerate() {
                    if b.is_solid() {
                        let l: IVec3 = cell_of_index(i) >> 2;
                        let bi = (l.x | (l.z << 3) | (l.y << 6)) as usize;
                        bits[bi >> 5] |= 1 << (bi & 31);
                    }
                }
                gpu.queue.write_buffer(self.bricks.buffer(), slot as u64 * BRICK_WORDS as u64 * 4, bytemuck::cast_slice(&bits));
                self.slots.insert(pos, slot);
                self.table.insert(tk, slot);
            }
        }
        self.chunks.insert(pos, data);
    }

    fn rebuild_lights(&mut self, gpu: &Gpu) -> bool {
        let mut cells: HashMap<IVec3, Vec<u32>> = HashMap::new();
        let mut gl = Vec::with_capacity(self.lights.len());
        for (i, l) in self.lights.values().enumerate() {
            let block = l.position.floor();
            gl.push(GpuLight {
                block: block.as_ivec3().to_array(),
                radius: l.radius,
                frac: (l.position - block).as_vec3().to_array(),
                intensity: l.intensity,
                color: l.color.to_array(),
                _pad: 0.0,
            });
            let lo = (l.position - l.radius as f64).floor().as_ivec3() >> LIGHT_CELL_BITS;
            let hi = (l.position + l.radius as f64).floor().as_ivec3() >> LIGHT_CELL_BITS;
            for z in lo.z..=hi.z {
                for y in lo.y..=hi.y {
                    for x in lo.x..=hi.x {
                        cells.entry(IVec3::new(x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        let cap = (cells.len() * 2).next_power_of_two().max(16);
        let mut table = vec![LightCell::zeroed(); cap];
        let mut idx = vec![];
        let mut max_probe = 0;
        for (c, ls) in cells {
            let mut i = hash3(c) as usize & (cap - 1);
            let mut probe = 0;
            while table[i].count != 0 {
                i = (i + 1) & (cap - 1);
                probe += 1;
            }
            max_probe = max_probe.max(probe);
            table[i] = LightCell { key: c.to_array(), start: idx.len() as u32, count: ls.len() as u32, _pad: [0; 3] };
            idx.extend(ls);
        }
        if gl.is_empty() {
            gl.push(GpuLight::zeroed());
        }
        if idx.is_empty() {
            idx.push(0);
        }
        self.light_mask = cap as u32 - 1;
        self.light_max_probe = max_probe;
        self.lights_dirty = false;
        let a = self.light_buf.write(gpu, bytemuck::cast_slice(&gl));
        let b = self.cell_buf.write(gpu, bytemuck::cast_slice(&table));
        let c = self.idx_buf.write(gpu, bytemuck::cast_slice(&idx));
        a || b || c
    }

    /// Uploads pending changes and per-frame parameters; returns the bind group to use
    /// as group 1.
    pub fn prepare(&mut self, gpu: &Gpu, cam: &Camera) -> &wgpu::BindGroup {
        let mut rebind = self.bind_group.is_none();
        if self.lights_dirty {
            rebind |= self.rebuild_lights(gpu);
        }
        if self.table.full_upload {
            let size = (self.table.entries.len() * 16) as u64;
            if self.table_buf.size() != size {
                self.table_buf = storage(gpu, "chunk table", size);
                rebind = true;
            }
            gpu.queue.write_buffer(&self.table_buf, 0, bytemuck::cast_slice(&self.table.entries));
            self.table.full_upload = false;
            self.table.dirty.clear();
        } else {
            let mut dirty = std::mem::take(&mut self.table.dirty);
            dirty.sort_unstable();
            dirty.dedup();
            for i in dirty {
                gpu.queue.write_buffer(&self.table_buf, i as u64 * 16, bytemuck::cast_slice(&self.table.entries[i as usize]));
            }
        }
        if self.bounds_dirty {
            self.bounds = self.chunks.keys().fold(None, |b: Option<(IVec3, IVec3)>, k| {
                let (lo, hi) = (k.origin(), k.origin() + k.extent());
                Some(b.map_or((lo, hi), |(a, z)| (a.min(lo), z.max(hi))))
            });
            self.bounds_dirty = false;
        }
        let (cam_block, _) = cam.split_position();
        let (bmin, bmax) = match self.bounds {
            Some((lo, hi)) => (lo - cam_block, hi - cam_block),
            None => (IVec3::ONE, IVec3::ZERO),
        };
        let sun = self.env.sun;
        let params = Params {
            cam_block: cam_block.to_array(),
            max_steps: self.cfg.max_steps,
            bounds_min: bmin.to_array(),
            table_mask: self.table.mask() as u32,
            bounds_max: bmax.to_array(),
            max_probe: self.table.max_probe,
            sun_dir: sun.map(|s| (-s.direction).to_array()).unwrap_or_default(),
            has_sun: sun.is_some() as u32,
            sun_color: sun.map(|s| s.color.to_array()).unwrap_or_default(),
            // Rays are clipped to the loaded region, so no artificial limit is needed and the
            // view radius can change at runtime.
            max_dist: 1e7,
            ambient: self.env.ambient.to_array(),
            exposure: self.env.exposure,
            light_mask: self.light_mask,
            light_max_probe: self.light_max_probe,
            shadows: match self.cfg.shadows {
                Shadows::None => 0,
                Shadows::Sun => 1,
                Shadows::All => 2,
            },
            max_lod: self.cfg.max_lod as u32,
            lod_radius: self.cfg.view_radius,
            _lod_pad: [0; 3],
        };
        gpu.queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        if rebind {
            let bufs = [&self.params, &self.table_buf, self.pool.buffer(), self.bricks.buffer(), &self.face_tex, &self.texels, &self.light_buf.buf, &self.cell_buf.buf, &self.idx_buf.buf];
            let entries: Vec<_> = bufs.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32, resource: b.as_entire_binding() }).collect();
            self.bind_group = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("voxel world"), layout: &self.layout, entries: &entries }));
        }
        self.bind_group.as_ref().unwrap()
    }

    pub fn table_lookup(&self, pos: IVec3) -> Option<u32> {
        self.table.get(table_key(ChunkKey::lod0(pos)))
    }

    pub fn dropped_chunks(&self) -> u64 {
        self.dropped_chunks
    }
}
