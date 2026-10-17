//! Meshing (`gpu-raster`): the face source of view-independent geometry. The executor's
//! drawer rasterises the geometry component's faces (IDEAS §12, §24):
//!
//! * **geometry** turns each chunk into packed face instances on the CPU (parallel,
//!   incremental; `greedy`, `naive`, `cubes`), uploaded per chunk (the schedule's transfer of
//!   the faces from the CPU to the GPU) and drawn with vertex pulling and one
//!   multi-draw-indirect call after per-chunk frustum and per-face-direction culling;
//! * **visibility** `faces`: every face goes to the draw stage;
//! * **draw** decides how the front face of each pixel is found and shaded (the executor's
//!   face drawer): `raster` (forward, with an optional depth pre-pass in lit mode), `gbuffer`
//!   or `visbuffer` (ids, then one compute shading pass);
//! * **shading** colours the hit (`shade_surface`, the same component as on the ray-walk back
//!   ends), with lit-mode shadow rays walking the composed **storage** with the
//!   **traversal** kernel.
//!
//! Meshes (mesh mode) share the depth buffer (`mr_gpu::meshes::GpuMeshes`): shaded forward
//! with `raster`, written into the ids or records with the deferred drawers.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_components::draw::drawer::{DrawFrame, DrawNodes};
use mr_components::geometry::{GpuGeometry, Knobs, Merge};
use mr_components::graph::{Checked, Role};
use mr_components::shaders::Walks;
use mr_components::shading::state as shade_state;
use mr_components::{names_on, require, Slot, components};
use mr_core::*;
use mr_gpu::meshes::{GpuMeshes, MeshPass, Occlusion};
use mr_gpu::voxels::{Shadows, shadows_param};
use mr_gpu::{GrowableBuffer, RangeAlloc, wgpu};
use rayon::prelude::*;
use serde::Deserialize;

use super::{CoreOpts, Draw, FaceSource, SourceSpec};
use crate::Core;

pub(crate) const BACKEND: &str = "gpu-raster";

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: BACKEND.into(),
        description: "GPU-raster back end: renders a composition (storage and traversal kernel for shadow rays, geometry, draw, shading); faces meshed on the CPU, vertex pulling, multi-draw-indirect, chunk + face culling".into(),
        tags: vec!["gpu".into(), "raster".into(), "hybrid".into(), "modular".into(), "wgpu".into(), "meshes".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}


fn describe(slot: Slot) -> String {
    components(slot).filter(|c| mr_components::unavailable(BACKEND, slot, c.name).is_none()).map(|c| format!("{}: {}", c.name, c.description)).collect::<Vec<_>>().join(" ")
}

fn params() -> Vec<ParamSpec> {
    let mut p = vec![
        ParamSpec::choice("storage", &format!("Storage component the lit-mode shadow rays walk (with the traversal kernel). {}", describe(Slot::Storage)), &names_on(BACKEND, Slot::Storage), "bricks", false),
        ParamSpec::choice("geometry", &format!("Geometry component. {}", describe(Slot::Geometry)), &names_on(BACKEND, Slot::Geometry), "greedy", true),
        ParamSpec::choice("visibility", &format!("Visibility strategy. {}", describe(Slot::Visibility)), &names_on(BACKEND, Slot::Visibility), "faces", false),
        ParamSpec::choice("draw", &format!("Draw component. {}", describe(Slot::Draw)), &names_on(BACKEND, Slot::Draw), "raster", true),
        ParamSpec::choice("traversal", &format!("Traversal kernel of the lit-mode shadow rays. {}", describe(Slot::Traversal)), &names_on(BACKEND, Slot::Traversal), "refloor", false),
        ParamSpec::choice("shading", &format!("Shading component. {}", describe(Slot::Shading)), &names_on(BACKEND, Slot::Shading), "direct", true),
        ParamSpec::choice("meshes", &format!("Meshes component (mesh mode). {}", describe(Slot::Meshes)), &names_on(BACKEND, Slot::Meshes), "depth", true),
        ParamSpec::choice("merge", "greedy: merge faces of the same block only (`block`), or any exposed coplanar faces with a per-pixel block lookup (`exposure`)", &["block", "exposure"], "block", true).of("geometry", &["greedy"]),
        ParamSpec::bool("cull", "naive / cubes: skip faces (blocks) hidden by solid neighbours (false = every face of every solid block)", true, true).of("geometry", &["naive", "cubes"]),
        ParamSpec::bool("depth_prepass", "raster, lit mode: depth pre-pass so lighting runs once per pixel", true, true).of("draw", &["raster"]),
        shadows_param(),
        ParamSpec::int("max_steps", "Shadow-ray step limit", 32, 16384, 2048, true),
        ParamSpec::int("mesh_budget", "Chunks remeshed per frame, nearest first (0 = all)", 0, 100000, 0, true),
        ParamSpec::int("pool_mb", "Voxel pool limit for shadow rays (MiB); for region storages (tree64) the limit of their own data", 64, 6144, 1536, false),
        ParamSpec::int("quad_buffer_mb", "Instance buffer limit (MiB; grows on demand)", 16, 2040, 1024, false),
        ParamSpec::int("frames_in_flight", "GPU frames queued", 1, 8, 3, false),
    ];
    p.extend(shade_state::params());
    p
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    geometry: String,
    visibility: String,
    draw: String,
    traversal: String,
    shading: String,
    meshes: String,
    merge: String,
    cull: bool,
    /// raster, lit mode: lay down depth first so the expensive fragment shader runs once
    /// per pixel.
    depth_prepass: bool,
    bounces: u32,
    spp: u32,
    accumulate: bool,
    shadows: Shadows,
    max_steps: u32,
    mesh_budget: usize,
    /// Voxel pool for lighting rays (unused in unlit mode but always allocated).
    pool_mb: u32,
    quad_buffer_mb: u32,
    frames_in_flight: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            storage: "bricks".into(),
            geometry: "greedy".into(),
            visibility: "faces".into(),
            draw: "raster".into(),
            traversal: "refloor".into(),
            shading: "direct".into(),
            meshes: "depth".into(),
            merge: "block".into(),
            cull: true,
            depth_prepass: true,
            bounces: 1,
            spp: 1,
            accumulate: true,
            shadows: Shadows::All,
            max_steps: 2048,
            mesh_budget: 0,
            pool_mb: 1536,
            quad_buffer_mb: 1024,
            frames_in_flight: 3,
        }
    }
}


fn parse_merge(s: &str) -> anyhow::Result<Merge> {
    match s {
        "block" => Ok(Merge::Block),
        "exposure" => Ok(Merge::Exposure),
        _ => anyhow::bail!("merge must be `block` or `exposure`, not `{s}`"),
    }
}

/// Bind groups of the storage instances: the shadow rays' at 2, the others' after the
/// deferred drawers' images (group 3).
const STORAGE_GROUPS: &[u32] = &[2, 4, 5];

/// The ray-casting sites' walks (PLAN step 9c): the shadow rays' first (the storage and
/// traversal stages), bounces and mesh shadows their own or the same; returns the walks and
/// the indices of (shadow, bounce, mesh shadow).
fn walks(g: &Checked) -> anyhow::Result<(Walks, usize, usize, usize)> {
    let mut walks = Walks::default();
    let shadow = g.site(Role::Shading, "shadow").ok_or_else(|| anyhow::anyhow!("{BACKEND}: shadow rays are not bound"))?;
    let shadow = walks.site(g, shadow, STORAGE_GROUPS, false);
    let bounce = g.site(Role::Shading, "bounce").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
    let mesh_shadow = g.site(Role::Meshes, "shadow").map_or(shadow, |s| walks.site(g, s, STORAGE_GROUPS, false));
    Ok((walks, shadow, bounce, mesh_shadow))
}

fn geometry(name: &str) -> GpuGeometry {
    mr_components::geometry::gpu(name).expect("validated")
}

/// The face source's WGSL: the geometry component's vertex placement and the chunks' faces.
fn source_wgsl(geometry: &str) -> String {
    format!("{}\n{}", self::geometry(geometry).wgsl, include_str!("chunks.wgsl"))
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

const MAX_SLOTS: usize = 1 << 16;
const QUAD_BYTES: u64 = 8;

struct ChunkMesh {
    slot: u16,
    offset: u32,
    counts: [u32; 6],
}

impl ChunkMesh {
    fn len(&self) -> u32 {
        self.counts.iter().sum()
    }
}

pub(crate) struct Meshing {
    opts: Options,
    /// Geometry the faces were meshed with; `stale`: a compile-time knob (geometry, shading,
    /// meshes) changed, the drawer is rebuilt at the next frame (one rebuild for several).
    built: String,
    stale: bool,
    /// The storage of the mesh-shadow site's walk.
    mesh_storage: usize,
    /// Mesh mode: drawn into the same depth buffer.
    mesh: GpuMeshes,
    quad_buf: GrowableBuffer,
    chunk_buf: wgpu::Buffer,
    indirect: wgpu::Buffer,
    bind0: Option<wgpu::BindGroup>,
    /// The nodes of the timed stages (PLAN step 11): geometry and culling on the CPU (the
    /// drawer's are the core's).
    geometry_nodes: Vec<String>,
    cull_nodes: Vec<String>,
    meshes: HashMap<IVec3, ChunkMesh>,
    dirty: HashSet<IVec3>,
    alloc: RangeAlloc,
    free_slots: Vec<u16>,
    overflowed: bool,
}

impl Meshing {
    pub(crate) fn new(p: &InitParams, devices: &Devices, waker: Waker, g: &Checked) -> anyhow::Result<(Core, Draw, Self)> {
        let opts: Options = p.options()?;
        require(BACKEND, Slot::Meshes, &opts.meshes)?;
        parse_merge(&opts.merge)?;
        require(BACKEND, Slot::Storage, &opts.storage)?;
        require(BACKEND, Slot::Traversal, &opts.traversal)?;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        let geometry = require(BACKEND, Slot::Geometry, &opts.geometry)?;
        let draw = require(BACKEND, Slot::Draw, &opts.draw)?;
        let shading = require(BACKEND, Slot::Shading, &opts.shading)?;
        // The faces cross from the CPU (meshing) to the GPU (the drawer): the schedule's
        // transfer; its adapter is the per-chunk upload.
        let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
        if !schedule.transfers.iter().any(|e| e.port == "faces" && g.node(&e.to).role == Role::Visibility) {
            anyhow::bail!("{BACKEND}: the faces do not cross from the CPU to the GPU");
        }
        let (walks, shadow, bounce, mesh_shadow) = walks(g)?;
        let mesh_storage = walks.kernels[mesh_shadow].storage.ok_or_else(|| anyhow::anyhow!("{BACKEND}: mesh shadows walk a storage (not ray queries)"))?;
        let (occ, rays) = walks.shadow_program(mesh_shadow);
        tracing::info!("gpu-raster: storage {}, geometry {geometry}, draw {draw}, traversal {}, shading {shading}, {} walks", walks.storages[0].component, walks.kernels[0].component, walks.kernels.len());
        let o = CoreOpts { shading: &shading, draw: &draw, bounces: opts.bounces, spp: opts.spp, accumulate: opts.accumulate, shadows: opts.shadows, max_steps: opts.max_steps, pool_mb: opts.pool_mb, frames_in_flight: opts.frames_in_flight };
        let meshes = opts.meshes != "none";
        let (core, drawing) = super::new(
            p,
            devices,
            waker,
            g,
            &o,
            opts.depth_prepass,
            SourceSpec {
                wgsl: source_wgsl(&geometry),
                walks,
                lead: None,
                sites: [shadow, bounce],
                varyings: false,
                strip: self::geometry(&geometry).strip,
                meshes: Some(mesh_shadow),
                draw_meshes: meshes,
                features: wgpu::Features::INDIRECT_FIRST_INSTANCE,
                depth_read: false,
                nodes: DrawNodes { draw: g.ids(&[Role::Visibility, Role::Draw]), shade: g.ids(&[Role::Shading]), extra: g.ids(&[Role::Meshes]) },
            },
        )?;
        let gpu = &core.gpu;
        let max_bytes = (gpu.device.limits().max_storage_buffer_binding_size as u64).min(gpu.device.limits().max_buffer_size);
        // Capped below 2 GiB: GrowableBuffer rounds sizes up to powers of two, which would
        // overshoot a 2^31-4 storage-binding limit.
        let quad_capacity = ((opts.quad_buffer_mb.min(2040) as u64 * 1024 * 1024).min(max_bytes) / QUAD_BYTES) as u32;
        let storage = |label, size: u64, extra| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
                mapped_at_creation: false,
            })
        };
        let quad_buf = GrowableBuffer::new(gpu, "quads", 1 << 20, quad_capacity as u64 * QUAD_BYTES, wgpu::BufferUsages::STORAGE);
        let chunk_buf = storage("chunk positions", MAX_SLOTS as u64 * 16, wgpu::BufferUsages::empty());
        let indirect = storage("indirect", 6 * 16 * 4096, wgpu::BufferUsages::INDIRECT);
        // Mesh shadows of forward drawers: their site's walk (by default the shadow rays').
        let mesh = GpuMeshes::new(gpu, &core.world, Some(Occlusion { program: &occ, rays: &rays, layout: core.storages.layout(mesh_storage) }));
        let s = Self {
            built: geometry,
            stale: false,
            mesh_storage,
            mesh,
            quad_buf,
            chunk_buf,
            indirect,
            bind0: None,
            geometry_nodes: g.ids(&[Role::Geometry]),
            cull_nodes: g.ids(&[Role::Draw]),
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            alloc: RangeAlloc::new(((1 << 20) / QUAD_BYTES) as u32),
            free_slots: (0..MAX_SLOTS as u32).rev().map(|s| s as u16).collect(),
            overflowed: false,
            opts,
        };
        Ok((core, drawing, s))
    }

    fn knobs(&self) -> Knobs {
        Knobs { merge: parse_merge(&self.opts.merge).unwrap_or_default(), cull: self.opts.cull }
    }

    fn remesh_all(&mut self) {
        self.dirty.extend(self.meshes.keys().copied());
    }

    fn drop_mesh(&mut self, pos: IVec3) {
        if let Some(m) = self.meshes.remove(&pos) {
            self.alloc.free(m.offset, m.len());
            self.free_slots.push(m.slot);
        }
    }

    /// Remeshes dirty chunks in parallel, nearest to `near` first, at most `mesh_budget` per
    /// call, and uploads the results (the faces' transfer).
    fn remesh(&mut self, core: &Core, near: IVec3) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let mut dirty: Vec<IVec3> = self.dirty.drain().collect();
        if self.opts.mesh_budget > 0 && dirty.len() > self.opts.mesh_budget {
            dirty.sort_unstable_by_key(|p| (*p - near).length_squared());
            for p in dirty.split_off(self.opts.mesh_budget) {
                self.dirty.insert(p);
            }
        }
        let (gpu, world) = (&core.gpu, &core.world);
        let mut work = vec![];
        for pos in dirty {
            if world.chunk(pos).is_none() {
                self.drop_mesh(pos);
                continue;
            }
            let slot = match self.meshes.get(&pos) {
                Some(m) => m.slot,
                None => match self.free_slots.pop() {
                    Some(s) => {
                        self.meshes.insert(pos, ChunkMesh { slot: s, offset: 0, counts: [0; 6] });
                        gpu.queue.write_buffer(&self.chunk_buf, s as u64 * 16, bytemuck::cast_slice(&[pos.x, pos.y, pos.z, 0]));
                        s
                    }
                    None => continue,
                },
            };
            work.push((pos, slot));
        }
        let (mesh_fn, knobs) = (geometry(&self.built).mesh, self.knobs());
        let results: Vec<(IVec3, mr_components::geometry::Mesh)> = work
            .par_iter()
            .map(|&(pos, slot)| {
                let center = world.chunk(pos).unwrap();
                let neighbors = Face::ALL.map(|f| world.chunk(pos + f.normal()));
                (pos, mesh_fn(center, neighbors, slot, knobs))
            })
            .collect();
        let n = results.len();
        for (pos, mesh) in results {
            let m = self.meshes.get_mut(&pos).unwrap();
            let old = (m.offset, m.len());
            m.counts = [0; 6];
            self.alloc.free(old.0, old.1);
            let len = mesh.quads.len() as u32;
            if len == 0 {
                continue;
            }
            let off = match self.quad_buf.alloc(gpu, &mut self.alloc, len, QUAD_BYTES) {
                Ok((off, grew)) => {
                    if grew {
                        self.bind0 = None;
                    }
                    off
                }
                Err(e) => {
                    if !self.overflowed {
                        tracing::error!("{e:#}; raise `quad_buffer_mb`");
                        self.overflowed = true;
                    }
                    continue;
                }
            };
            let m = self.meshes.get_mut(&pos).unwrap();
            m.offset = off;
            m.counts = mesh.counts;
            gpu.queue.write_buffer(self.quad_buf.buffer(), off as u64 * QUAD_BYTES, bytemuck::cast_slice(&mesh.quads));
        }
        n
    }
}

/// Frustum planes (left, right, bottom, top, near) from a reverse-Z infinite projection.
fn frustum_planes(m: Mat4) -> [Vec4; 5] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2]
}

fn aabb_visible(planes: &[Vec4; 5], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = Vec3::new(if p.x >= 0.0 { max.x } else { min.x }, if p.y >= 0.0 { max.y } else { min.y }, if p.z >= 0.0 { max.z } else { min.z });
        p.truncate().dot(v) + p.w >= 0.0
    })
}

impl FaceSource for Meshing {
    fn updating(&mut self, _: &[WorldUpdate]) {}

    fn updated(&mut self, core: &Core, changed: &[IVec3]) {
        for &pos in changed {
            self.dirty.insert(pos);
            for f in Face::ALL {
                let nb = pos + f.normal();
                if self.meshes.contains_key(&nb) || core.world.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.mesh.apply(update);
    }

    fn render(&mut self, core: &mut Core, drawing: &mut Draw, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        // The meshes stage draws meshes (`depth`), or not (`none`).
        let want_meshes = req.meshes && self.opts.meshes != "none";
        if self.stale {
            let o = &self.opts;
            if drawing.rebuild(core, source_wgsl(&o.geometry), geometry(&o.geometry).strip, &o.shading, o.meshes != "none")? {
                self.bind0 = None;
            }
            if o.geometry != self.built {
                self.built = o.geometry.clone();
                self.remesh_all();
            }
            self.stale = false;
        }
        drawing.check_state(req.mode, None)?;
        let ticket = core.ring.begin(&core.gpu, sink)?;
        let start = Instant::now();
        let remeshed = self.remesh(core, req.camera.split_position().0 >> 5);
        let mesh_ms = start.elapsed().as_secs_f64() * 1e3;
        let cull_start = Instant::now();
        let c = &req.camera;
        let (cam_block, frac) = c.split_position();
        let view_proj = c.projection(core.width as f32 / core.height as f32) * c.view_rotation();
        drawing.write_frame(core, c, req.mode, 0, IVec3::ZERO);
        core.accum.begin(&core.gpu, c, req.mode);
        let world_bg = core.world.prepare(&core.gpu, c).clone();
        if want_meshes {
            // The images group is built each frame, so a new vertex buffer needs nothing more.
            self.mesh.prepare(&core.gpu, c, view_proj, req.mode);
        }

        // Cull chunks and face directions; build indirect draws.
        let planes = frustum_planes(view_proj);
        let geo = geometry(&self.built);
        let mut draws: Vec<DrawArgs> = Vec::with_capacity(self.meshes.len() * 3);
        let (mut drawn, mut total) = (0u64, 0u64);
        for (pos, m) in &self.meshes {
            total += m.len() as u64;
            if m.len() == 0 {
                continue;
            }
            let min = (*pos * 32 - cam_block).as_vec3() - frac;
            let max = min + 32.0;
            if !aabb_visible(&planes, min, max) {
                continue;
            }
            if !geo.per_face {
                // The hardware back-face culls the hidden half of each instance.
                draws.push(DrawArgs { vertex_count: geo.verts, instance_count: m.len(), first_vertex: 0, first_instance: m.offset });
                drawn += m.len() as u64;
                continue;
            }
            let mut off = m.offset;
            for (f, &n) in m.counts.iter().enumerate() {
                let axis = f / 2;
                // Faces pointing +axis are only visible from beyond the chunk's min plane.
                let visible = if f % 2 == 0 { 0.0 > min[axis] } else { 0.0 < max[axis] };
                if n > 0 && visible {
                    draws.push(DrawArgs { vertex_count: geo.verts, instance_count: n, first_vertex: 0, first_instance: off });
                    drawn += n as u64;
                }
                off += n;
            }
        }
        let bytes = (draws.len() * 16) as u64;
        if bytes > self.indirect.size() {
            self.indirect = core.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("indirect"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDIRECT,
                mapped_at_creation: false,
            });
        }
        if !draws.is_empty() {
            core.gpu.queue.write_buffer(&self.indirect, 0, bytemuck::cast_slice(&draws));
        }
        let cull_ms = cull_start.elapsed().as_secs_f32() * 1e3;

        if self.bind0.is_none() {
            self.bind0 = Some(drawing.bind(core, self.quad_buf.buffer(), &self.chunk_buf));
        }
        let bound = core.storages.prepare(&core.gpu, &core.world);
        self.mesh.set_occlusion_bind_group(bound.get(self.mesh_storage));
        let (bind2, more) = bound.split();
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        if core.build_accel(&mut enc, ticket, req) {
            self.bind0 = Some(drawing.bind(core, self.quad_buf.buffer(), &self.chunk_buf));
        }
        let bind0 = self.bind0.clone().unwrap();
        let binds: Vec<&wgpu::BindGroup> = [Some(&bind0), Some(&world_bg), bind2].into_iter().flatten().collect();
        let sky = super::sky_rgba(core);
        let clear = wgpu::Color { r: sky[0] as f64, g: sky[1] as f64, b: sky[2] as f64, a: 1.0 };
        let out_view = core.output.create_view(&Default::default());
        let n = draws.len() as u32;
        let frame = DrawFrame {
            out: &out_view,
            depth: &drawing.depth,
            clear,
            prepass: drawing.depth_prepass && req.mode == RenderMode::Lit,
            keep_depth: false,
            binds: &binds,
            more: &more,
            mesh_verts: Some(self.mesh.vertices()),
            ring: &core.ring,
            ticket,
            ts: (true, true),
            nodes: &drawing.draw_nodes,
        };
        let draw = |pass: &mut wgpu::RenderPass<'_>| {
            if n > 0 {
                pass.multi_draw_indirect(&self.indirect, 0, n);
            }
        };
        // Meshes: in the faces' forward passes; into their ids or records when deferred.
        let meshes = want_meshes && (!drawing.drawer.mode().deferred() || self.mesh.triangles() > 0);
        let mesh_draw = |pass: &mut wgpu::RenderPass<'_>, mp: MeshPass| self.mesh.draw(pass, mp, Some(&world_bg));
        drawing.drawer.encode(&core.gpu, &mut enc, &frame, &draw, meshes.then_some(&mesh_draw as &dyn Fn(&mut wgpu::RenderPass<'_>, MeshPass)), None);
        let (accel, accel_stages) = core.accel_stats();
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: vec![
                ("instances".into(), drawn as f64),
                ("instances_total".into(), total as f64),
                ("draws".into(), n as f64),
                ("remeshed".into(), remeshed as f64),
                ("mesh_ms".into(), mesh_ms),
                ("mesh_triangles".into(), if want_meshes { self.mesh.triangles() as f64 } else { 0.0 }),
            ]
            .into_iter()
            .chain(core.storages.counters())
            .chain(accel)
            .chain(core.accum.counter(req.mode))
            .collect(),
            stages: [StageTime::cpu("geometry", &self.geometry_nodes, mesh_ms as f32), StageTime::cpu("cull", &self.cull_nodes, cull_ms)].into_iter().chain(accel_stages).collect(),
        };
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        core.ring.poll(&core.gpu, sink, false)
    }

    fn set_param(&mut self, _: &mut Core, _: &mut Draw, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        let s = value.as_str().unwrap_or("").to_string();
        let r = |slot| require(BACKEND, slot, &s);
        let stale = &mut self.stale;
        let mut compiled = |slot, field: &mut String| {
            r(slot).map(|v| {
                *stale |= *field != v;
                *field = v;
            })
        };
        Some(match name {
            "mesh_budget" => {
                self.opts.mesh_budget = value.as_i64().unwrap_or(0) as usize;
                Ok(())
            }
            "geometry" => compiled(Slot::Geometry, &mut self.opts.geometry),
            "shading" => compiled(Slot::Shading, &mut self.opts.shading),
            "meshes" => compiled(Slot::Meshes, &mut self.opts.meshes),
            "visibility" => r(Slot::Visibility).map(drop),
            "merge" => parse_merge(&s).map(|_| {
                if s != self.opts.merge {
                    self.opts.merge = s.clone();
                    self.remesh_all();
                }
            }),
            "cull" => {
                let c = value.as_bool().unwrap_or(true);
                if c != self.opts.cull {
                    self.opts.cull = c;
                    self.remesh_all();
                }
                Ok(())
            }
            _ => return None,
        })
    }

    fn resize(&mut self, _: &Core, _: &Draw) {
        self.bind0 = None;
    }

    fn memory(&self, _: &Core) -> (u64, u64) {
        (0, self.quad_buf.size() + self.chunk_buf.size() + self.indirect.size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every composition's shader composes from its modules and validates (no GPU needed).
    #[test]
    fn programs_compose() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let mut n = 0;
        for st in names_on(BACKEND, Slot::Storage) {
            for k in names_on(BACKEND, Slot::Traversal) {
                for g in names_on(BACKEND, Slot::Geometry) {
                    for shading in names_on(BACKEND, Slot::Shading) {
                        let flat = mr_components::Composition { storage: st.into(), geometry: g.into(), traversal: k.into(), shading: shading.into(), ..mr_components::Composition::new(BACKEND) };
                        let mut graphs = vec![mr_components::graph::Graph::from_composition(&flat).unwrap()];
                        // Bounces and mesh shadows through walks of their own (PLAN step 9c).
                        if shading == "pathtrace" && g == "greedy" {
                            let mut gr = graphs[0].clone();
                            let node = |role, c: &str, ins: &[(&str, &str)]| {
                                let mut n = mr_components::graph::Node::new(role, c);
                                n.inputs.extend(ins.iter().map(|&(p, s)| (p.to_string(), s.to_string())));
                                n
                            };
                            gr.nodes.insert("b.storage".into(), node(Role::Storage, "distance", &[]));
                            gr.nodes.insert("b.kernel".into(), node(Role::Traversal, "sphere", &[("storage", "b.storage")]));
                            gr.nodes.insert("m.kernel".into(), node(Role::Traversal, "incremental", &[("storage", "storage")]));
                            gr.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "b.kernel".into());
                            gr.nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), "m.kernel".into());
                            graphs.push(gr);
                        }
                        for gr in graphs {
                            let ck = gr.check().unwrap();
                            gr.validate().unwrap_or_else(|e| panic!("{gr:?}: {e}"));
                            let what = format!("{st} {k} {g} {shading}");
                            let (walks, shadow, bounce, mesh_shadow) = walks(&ck).unwrap();
                            // The drawer (with mesh records for deferred drawers, and without).
                            let (program, deps, meshed) = crate::faces::programs(&walks, None, [shadow, bounce], Some(mesh_shadow));
                            let draw_src = crate::faces::draw_src(&source_wgsl(g), shading, false);
                            crate::faces::tests::drawers(&program, &deps, meshed.as_ref(), &draw_src, shading, &what, BACKEND);
                            // Forward mesh shading through the mesh-shadow site.
                            let (mut p, rays) = walks.shadow_program(mesh_shadow);
                            p.define("SHADE");
                            let m = p.naga(mr_gpu::meshes::MESH_WGSL, &[&rays, mr_gpu::program::LIGHTING, mr_gpu::program::VOXELS, mr_gpu::program::COMMON]).unwrap_or_else(|e| panic!("meshes {what}: {e}"));
                            Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&m).unwrap_or_else(|e| panic!("meshes {what}: {e:?}"));
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n > 30, "{n}");
    }
}
