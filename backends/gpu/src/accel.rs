//! Ray queries' acceleration structures (PLAN step 13e4b): one BLAS per chunk from the
//! geometry node the `rayquery` kernels trace (`greedy` / `naive` quads as triangles, or
//! `boxes`, one AABB per occupied 4^3 brick with a software DDA inside), and a TLAS of the
//! chunks translated relative to the camera's block, rebuilt every frame (precision never
//! degrades far from the origin). The core keeps them when any ray-casting site of the graph
//! is a ray query (the primary rays of `gpu-rt`, or another lane's shadow and bounce rays),
//! bound in group 0 from [`ACCEL_BINDING`]; the trace (`g_rt_trace`, from the geometry
//! component) sits in the root, as naga cannot write ray queries back into a module.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use glam::IVec3;
use mr_components::geometry::{Knobs, Merge, RtKind};
use mr_core::chunk::cell_of_index;
use mr_core::*;
use mr_gpu::voxels::VoxelWorld;
use mr_gpu::{Gpu, GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use wgpu::util::DeviceExt;

/// Group 0's bindings of the acceleration structures: the TLAS, the geometry hits are decoded
/// from, the chunk info per instance slot (between the hit records and the lane's).
pub(crate) const ACCEL_BINDING: u32 = 5;

/// How the acceleration structures are built (the geometry node's knobs and the build's).
pub(crate) struct AccelSpec {
    /// The geometry component.
    pub geometry: String,
    /// naive: skip faces hidden by solid neighbours.
    pub cull: bool,
    /// Prefer fast BVH builds over fast traversal (a build flag, not code).
    pub fast_build: bool,
    /// Chunk BLAS rebuilds per frame, nearest first (0 = all).
    pub build_budget: usize,
}

const MAX_SLOTS: u32 = 1 << 16;
const NO_BRICK: u32 = 0xffff_ffff;

struct ChunkAs {
    slot: u32,
    blas: wgpu::Blas,
    /// Range in the shared geometry buffer (u32 units).
    range: (u32, u32),
    _inputs: Vec<wgpu::Buffer>,
}

/// CPU-side geometry for one chunk's BLAS.
enum Built {
    Triangles { vertices: Vec<[f32; 3]>, indices: Vec<u32>, quads: Vec<[u32; 2]> },
    Aabbs { boxes: Vec<[f32; 6]>, bricks: Vec<u32> },
}

fn tlas(gpu: &Gpu, cap: u32) -> wgpu::Tlas {
    gpu.device.create_tlas(&wgpu::CreateTlasDescriptor {
        label: Some("chunks"),
        max_instances: cap,
        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
    })
}

impl AccelSpec {
    /// For the ray queries of `walks`, if it has any: their geometry node with its knobs
    /// (`cull`, `fast_build`, `build_budget`; `merge` only `block`: ray queries have no
    /// per-pixel block lookup).
    pub fn of(g: &mr_components::graph::Checked, walks: &mr_components::shaders::Walks) -> anyhow::Result<Option<Self>> {
        let Some(geo) = &walks.geometry else { return Ok(None) };
        let knobs = &g.node(&geo.node).knobs;
        let merge = knobs.get("merge").and_then(|v| v.as_str()).unwrap_or("block");
        anyhow::ensure!(merge == "block", "`{}`: merge = `{merge}`: ray queries have no per-pixel block lookup (only `block`)", geo.node);
        Ok(Some(Self {
            geometry: geo.component.clone(),
            cull: knobs.get("cull").and_then(|v| v.as_bool()).unwrap_or(true),
            fast_build: knobs.get("fast_build").and_then(|v| v.as_bool()).unwrap_or(false),
            build_budget: knobs.get("build_budget").and_then(|v| v.as_integer()).unwrap_or(0).max(0) as usize,
        }))
    }
}

/// The acceleration structures and the geometry their hits are decoded from.
pub(crate) struct Accel {
    gpu: Arc<Gpu>,
    pub rt: RtKind,
    geometry: String,
    knobs: Knobs,
    pub fast_build: bool,
    pub build_budget: usize,
    tlas: wgpu::Tlas,
    tlas_cap: u32,
    geo_buf: GrowableBuffer,
    geo_alloc: RangeAlloc,
    chunk_info: wgpu::Buffer,
    chunks: HashMap<IVec3, ChunkAs>,
    free_slots: Vec<u32>,
    dirty: HashSet<IVec3>,
    /// The bindings changed (the TLAS or the geometry buffer grew).
    rebind: bool,
    /// This frame's TLAS instances, BLAS rebuilds and their CPU time.
    instances: u32,
    rebuilt: usize,
    pub build_ms: f64,
}

impl Accel {
    pub fn new(gpu: &Arc<Gpu>, s: &AccelSpec) -> anyhow::Result<Self> {
        let (_, rt) = mr_components::geometry::rt(&s.geometry).ok_or_else(|| anyhow::anyhow!("geometry `{}` builds no acceleration structure (have: greedy, naive, boxes)", s.geometry))?;
        let geo_buf = GrowableBuffer::new(gpu, "geometry", 1 << 20, 1 << 30, wgpu::BufferUsages::STORAGE);
        let geo_words = (geo_buf.size() / 4) as u32;
        let chunk_info = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("chunk info"), size: MAX_SLOTS as u64 * 16, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let tlas_cap = 4096;
        Ok(Self {
            tlas: tlas(gpu, tlas_cap),
            tlas_cap,
            geo_buf,
            rt,
            geometry: s.geometry.clone(),
            knobs: Knobs { merge: Merge::Block, cull: s.cull },
            fast_build: s.fast_build,
            build_budget: s.build_budget,
            geo_alloc: RangeAlloc::new(geo_words),
            chunk_info,
            chunks: HashMap::new(),
            free_slots: (0..MAX_SLOTS).rev().collect(),
            dirty: HashSet::new(),
            rebind: false,
            instances: 0,
            rebuilt: 0,
            build_ms: 0.0,
            gpu: gpu.clone(),
        })
    }

    /// The root's code: the bindings and the geometry's trace `g_rt_trace(o, d, max_t,
    /// shadow)`; `flags` is the ray flags of rays that are not shadow rays (a WGSL
    /// expression: the primary rays' triangle culling, or `0u`).
    pub fn wgsl(&self, flags: &str) -> String {
        wgsl(&self.geometry, flags)
    }

    /// Only the boxes' DDA reads voxels.
    pub fn reads_voxels(&self) -> bool {
        matches!(self.rt, RtKind::Boxes)
    }

    pub fn layout_entries(&self, stages: wgpu::ShaderStages) -> Vec<wgpu::BindGroupLayoutEntry> {
        let e = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: stages, ty, count: None };
        let ro = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None };
        vec![e(ACCEL_BINDING, wgpu::BindingType::AccelerationStructure { vertex_return: false }), e(ACCEL_BINDING + 1, ro), e(ACCEL_BINDING + 2, ro)]
    }

    pub fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![
            wgpu::BindGroupEntry { binding: ACCEL_BINDING, resource: wgpu::BindingResource::AccelerationStructure(&self.tlas) },
            wgpu::BindGroupEntry { binding: ACCEL_BINDING + 1, resource: self.geo_buf.buffer().as_entire_binding() },
            wgpu::BindGroupEntry { binding: ACCEL_BINDING + 2, resource: self.chunk_info.as_entire_binding() },
        ]
    }

    /// Chunks the GPU world changed (triangles: their neighbours too, whose face culling
    /// depends on them).
    pub fn updated(&mut self, world: &VoxelWorld, changed: &[IVec3]) {
        for &pos in changed {
            self.dirty.insert(pos);
            if let RtKind::Triangles(_) = self.rt {
                for f in Face::ALL {
                    let nb = pos + f.normal();
                    if self.chunks.contains_key(&nb) || world.chunk(nb).is_some() {
                        self.dirty.insert(nb);
                    }
                }
            }
        }
    }

    /// Builds this frame's structures into `enc`: dirty chunks' BLASes, then the TLAS with
    /// the instances relative to `cam_block`. Returns whether the bindings changed.
    pub fn build(&mut self, world: &VoxelWorld, enc: &mut wgpu::CommandEncoder, cam_block: IVec3) -> bool {
        let start = Instant::now();
        self.rebuilt = self.rebuild(world, enc, cam_block >> 5);
        self.build_ms = start.elapsed().as_secs_f64() * 1e3;
        let n = self.chunks.len() as u32;
        if n > self.tlas_cap {
            self.tlas_cap = n.next_power_of_two();
            self.tlas = tlas(&self.gpu, self.tlas_cap);
            self.rebind = true;
        }
        for (k, (pos, ch)) in self.chunks.iter().enumerate() {
            let t = (*pos * 32 - cam_block).as_vec3();
            let transform = [1.0, 0.0, 0.0, t.x, 0.0, 1.0, 0.0, t.y, 0.0, 0.0, 1.0, t.z];
            self.tlas[k] = Some(wgpu::TlasInstance::new(&ch.blas, transform, ch.slot, 0xff));
        }
        let used = self.tlas.get().iter().filter(|i| i.is_some()).count() as u32;
        for k in n..used.max(n) {
            self.tlas[k as usize] = None;
        }
        enc.build_acceleration_structures(std::iter::empty(), std::iter::once(&self.tlas));
        self.instances = n;
        std::mem::take(&mut self.rebind)
    }

    pub fn counters(&self) -> Vec<(String, f64)> {
        vec![("instances".into(), self.instances as f64), ("blas_rebuilt".into(), self.rebuilt as f64), ("build_ms".into(), self.build_ms)]
    }

    /// Its knobs; `None` if `name` is not one.
    pub fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "fast_build" => self.fast_build = value.as_bool().unwrap_or(false),
            "build_budget" => self.build_budget = value.as_i64().unwrap_or(0) as usize,
            _ => return None,
        }
        Some(Ok(()))
    }

    pub fn memory_bytes(&self) -> u64 {
        // BLAS sizes are driver-internal; BLAS input buffers are a lower bound.
        let inputs: u64 = self.chunks.values().flat_map(|c| c._inputs.iter()).map(|b| b.size()).sum();
        self.geo_buf.size() + self.chunk_info.size() + inputs
    }

    fn drop_chunk(&mut self, pos: IVec3) {
        if let Some(c) = self.chunks.remove(&pos) {
            self.geo_alloc.free(c.range.0, c.range.1);
            self.free_slots.push(c.slot);
        }
    }

    fn build_geometry(&self, world: &VoxelWorld, pos: IVec3) -> Option<Built> {
        match self.rt {
            RtKind::Triangles(mesh) => {
                let center = world.chunk(pos)?;
                let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
                let mesh = mesh(center, neighbors, 0, self.knobs);
                if mesh.quads.is_empty() {
                    return None;
                }
                let mut vertices = Vec::with_capacity(mesh.quads.len() * 4);
                let mut indices = Vec::with_capacity(mesh.quads.len() * 6);
                for q in &mesh.quads {
                    let base = vertices.len() as u32;
                    vertices.extend(mr_mesh::Quad::decode(*q).corners().map(|c| c.to_array()));
                    indices.extend([base, base + 1, base + 2, base + 2, base + 1, base + 3]);
                }
                Some(Built::Triangles { vertices, indices, quads: mesh.quads })
            }
            RtKind::Boxes => match world.chunk(pos)? {
                ChunkData::Uniform(b) if b.is_air() => None,
                ChunkData::Uniform(_) => Some(Built::Aabbs { boxes: vec![[0.0, 0.0, 0.0, 32.0, 32.0, 32.0]], bricks: vec![NO_BRICK] }),
                ChunkData::Dense(d) => {
                    let mut occupied = [false; 512];
                    for (i, b) in d.0.iter().enumerate() {
                        if b.is_solid() {
                            let l: IVec3 = cell_of_index(i) >> 2;
                            occupied[(l.x | (l.z << 3) | (l.y << 6)) as usize] = true;
                        }
                    }
                    let mut boxes = vec![];
                    let mut bricks = vec![];
                    for (bi, _) in occupied.iter().enumerate().filter(|(_, o)| **o) {
                        let b = bi as i32;
                        let lo = IVec3::new(b & 7, (b >> 6) & 7, (b >> 3) & 7).as_vec3() * 4.0;
                        boxes.push([lo.x, lo.y, lo.z, lo.x + 4.0, lo.y + 4.0, lo.z + 4.0]);
                        bricks.push(bi as u32);
                    }
                    (!boxes.is_empty()).then_some(Built::Aabbs { boxes, bricks })
                }
            },
        }
    }

    /// Rebuilds BLASes of dirty chunks into `enc`. Returns how many were rebuilt.
    fn rebuild(&mut self, world: &VoxelWorld, enc: &mut wgpu::CommandEncoder, near: IVec3) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.build_budget > 0 && dirty.len() > self.build_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.build_budget) {
                self.dirty.insert(p);
            }
        }
        let built: Vec<(IVec3, Option<Built>)> = dirty.par_iter().map(|&p| (p, self.build_geometry(world, p))).collect();
        let flags = if self.fast_build { wgpu::AccelerationStructureFlags::PREFER_FAST_BUILD } else { wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE };
        // Everything referenced by the build entries must outlive the build call.
        let mut staged: Vec<(IVec3, u32, wgpu::Blas, wgpu::BlasGeometrySizeDescriptors, Vec<wgpu::Buffer>, (u32, u32))> = vec![];
        for (pos, b) in built {
            let old_slot = self.chunks.get(&pos).map(|c| c.slot);
            if let Some(c) = self.chunks.remove(&pos) {
                self.geo_alloc.free(c.range.0, c.range.1);
                if b.is_none() {
                    self.free_slots.push(c.slot);
                }
            }
            let Some(b) = b else { continue };
            let Some(slot) = old_slot.or_else(|| self.free_slots.pop()) else { continue };
            let usage = wgpu::BufferUsages::BLAS_INPUT;
            let (sizes, inputs, words, info) = match &b {
                Built::Triangles { vertices, indices, quads } => {
                    let vb = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(vertices), usage });
                    let ib = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(indices), usage });
                    let sizes = wgpu::BlasGeometrySizeDescriptors::Triangles {
                        descriptors: vec![wgpu::BlasTriangleGeometrySizeDescriptor {
                            vertex_format: wgpu::VertexFormat::Float32x3,
                            vertex_count: vertices.len() as u32,
                            index_format: Some(wgpu::IndexFormat::Uint32),
                            index_count: Some(indices.len() as u32),
                            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
                        }],
                    };
                    (sizes, vec![vb, ib], bytemuck::cast_slice::<[u32; 2], u32>(quads).to_vec(), None)
                }
                Built::Aabbs { boxes, bricks } => {
                    let ab = self.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(boxes), usage });
                    let sizes = wgpu::BlasGeometrySizeDescriptors::AABBs {
                        descriptors: vec![wgpu::BlasAABBGeometrySizeDescriptor {
                            primitive_count: boxes.len() as u32,
                            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
                        }],
                    };
                    (sizes, vec![ab], bricks.clone(), Some(world.table_lookup(pos).unwrap_or(0xffff_ffff)))
                }
            };
            let off = match self.geo_buf.alloc(&self.gpu, &mut self.geo_alloc, words.len() as u32, 4) {
                Ok((off, grew)) => {
                    if grew {
                        self.rebind = true;
                    }
                    off
                }
                Err(e) => {
                    tracing::error!("{e:#}; chunk {pos} skipped");
                    self.free_slots.push(slot);
                    continue;
                }
            };
            self.gpu.queue.write_buffer(self.geo_buf.buffer(), off as u64 * 4, bytemuck::cast_slice(&words));
            // Triangles: first quad; boxes: chunk-table value and first brick.
            let info = match info {
                None => [off / 2, 0, 0, 0],
                Some(value) => [value, off, 0, 0],
            };
            self.gpu.queue.write_buffer(&self.chunk_info, slot as u64 * 16, bytemuck::cast_slice(&info));
            let blas = self.gpu.device.create_blas(&wgpu::CreateBlasDescriptor { label: None, flags, update_mode: wgpu::AccelerationStructureUpdateMode::Build }, sizes.clone());
            staged.push((pos, slot, blas, sizes, inputs, (off, words.len() as u32)));
        }
        let n = staged.len();
        {
            let tri_geo: Vec<Vec<wgpu::BlasTriangleGeometry>> = staged
                .iter()
                .map(|(_, _, _, sizes, inputs, _)| match sizes {
                    wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors } => vec![wgpu::BlasTriangleGeometry {
                        size: &descriptors[0],
                        vertex_buffer: &inputs[0],
                        first_vertex: 0,
                        vertex_stride: 12,
                        index_buffer: Some(&inputs[1]),
                        first_index: Some(0),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    }],
                    _ => vec![],
                })
                .collect();
            let entries: Vec<wgpu::BlasBuildEntry> = staged
                .iter()
                .zip(tri_geo)
                .map(|((_, _, blas, sizes, inputs, _), tri)| wgpu::BlasBuildEntry {
                    blas,
                    geometry: match sizes {
                        wgpu::BlasGeometrySizeDescriptors::Triangles { .. } => wgpu::BlasGeometries::TriangleGeometries(tri),
                        wgpu::BlasGeometrySizeDescriptors::AABBs { descriptors } => wgpu::BlasGeometries::AabbGeometries(vec![wgpu::BlasAabbGeometry {
                            size: &descriptors[0],
                            stride: 24,
                            aabb_buffer: &inputs[0],
                            primitive_offset: 0,
                        }]),
                    },
                })
                .collect();
            enc.build_acceleration_structures(entries.iter(), std::iter::empty());
        }
        for (pos, slot, blas, _, inputs, range) in staged {
            self.chunks.insert(pos, ChunkAs { slot, blas, range, _inputs: inputs });
        }
        n
    }
}

impl Drop for Accel {
    fn drop(&mut self) {
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let positions: Vec<IVec3> = self.chunks.keys().copied().collect();
        for p in positions {
            self.drop_chunk(p);
        }
    }
}

/// The root code of ray queries over `geometry`'s acceleration structures (see
/// [`Accel::wgsl`]).
pub(crate) fn wgsl(geometry: &str, flags: &str) -> String {
    let (geo, _) = mr_components::geometry::rt(geometry).expect("checked");
    format!(
        "@group(0) @binding({b0}) var acc: acceleration_structure;\n\
         @group(0) @binding({b1}) var<storage, read> geo: array<u32>;              // quads or brick lists\n\
         @group(0) @binding({b2}) var<storage, read> chunk_info: array<vec4<u32>>; // per instance slot\n\
         fn rt_flags() -> u32 {{ return {flags}; }}\n{geo}",
        b0 = ACCEL_BINDING,
        b1 = ACCEL_BINDING + 1,
        b2 = ACCEL_BINDING + 2,
    )
}
