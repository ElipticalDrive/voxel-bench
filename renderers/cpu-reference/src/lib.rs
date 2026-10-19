//! Reference CPU ray caster.
//!
//! Defines "ground truth" for both render modes: exactly one ray through each pixel
//! centre, traversed in `f64` with a two-level (chunk, then block) DDA, and the exact
//! lighting model from `mr_core::lighting` with a shadow ray per light. Correctness and
//! clarity first; it is still multi-threaded so references don't take forever.

pub mod mesh;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use glam::{DVec3, IVec3, Vec3};
use mr_core::chunk::{CHUNK_BITS, local_of_block};
use mr_core::lighting::{Tone, point_light, srgb_to_linear, srgb_to_linear_f, texel_coord};
use mr_core::*;
use rayon::prelude::*;

/// Shadow rays traced in lit mode.
#[derive(Copy, Clone, Debug, Default, PartialEq, serde::Deserialize)]
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
}

/// Shared knobs of the CPU renderers.
pub fn cpu_params() -> Vec<ParamSpec> {
    vec![
        ParamSpec::int("threads", "Worker threads (0 = all cores)", 0, 256, 0, true),
        ParamSpec::choice("shadows", "Shadow rays in lit mode", &["none", "sun", "all"], "all", true),
    ]
}

fn params() -> Vec<ParamSpec> {
    cpu_params()
}

/// A rayon pool whose size can change at runtime.
pub struct Pool {
    pool: rayon::ThreadPool,
    pub threads: usize,
}

impl Pool {
    pub fn new(threads: usize) -> anyhow::Result<Self> {
        Ok(Self { pool: rayon::ThreadPoolBuilder::new().num_threads(threads).build()?, threads })
    }

    pub fn install<R: Send>(&self, f: impl FnOnce() -> R + Send) -> R {
        self.pool.install(f)
    }

    pub fn rayon(&self) -> &rayon::ThreadPool {
        &self.pool
    }

    /// Handles the shared `threads` / `shadows` parameters; returns false if `name` is
    /// not one of them.
    pub fn set_common(&mut self, world: &mut World, name: &str, value: &ParamValue) -> anyhow::Result<bool> {
        match name {
            "threads" => {
                let n = value.as_i64().unwrap_or(0) as usize;
                if n != self.threads {
                    *self = Pool::new(n)?;
                }
                Ok(true)
            }
            "shadows" => {
                world.shadows = parse_shadows(value)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

/// The `shadows` knob's value.
pub fn parse_shadows(value: &ParamValue) -> anyhow::Result<Shadows> {
    value.as_str().and_then(Shadows::parse).ok_or_else(|| anyhow::anyhow!("bad shadows value"))
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Options {
    threads: usize,
    shadows: Shadows,
}

pub fn entry() -> RendererEntry {
    RendererEntry { info, create: |p, _, _| Ok(Box::new(CpuReference::new(p)?)) }
}

fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu-reference".into(),
        description: "Multi-threaded f64 two-level DDA ray caster; defines the reference images".into(),
        tags: vec!["cpu".into(), "raycast".into(), "reference".into(), "meshes".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: params(),
    }
}

const LIGHT_CELL_BITS: i32 = 4;

/// The world every CPU renderer shares (the CPU executor's core keeps one):
/// the block registry, the environment, the point lights (and their grid) and the shadow
/// setting. The chunks are each renderer's own representation ([`Chunks`]: dense blocks).
pub struct World {
    pub blocks: Arc<BlockRegistry>,
    pub env: Environment,
    pub shadows: Shadows,
    /// The post's tone mapper (the reference's own: `clamp`).
    pub tone: Tone,
    lights: HashMap<u32, Light>,
    light_grid: HashMap<IVec3, Vec<Light>>,
    lights_dirty: bool,
}

impl World {
    pub fn new(p: &InitParams) -> Self {
        Self { blocks: p.blocks.clone(), env: p.environment, shadows: Shadows::All, tone: Tone::Clamp, lights: HashMap::new(), light_grid: HashMap::new(), lights_dirty: false }
    }

    /// The sky's pixel (a linear colour, through the post).
    pub fn sky(&self) -> [u8; 4] {
        self.tone.encode(self.env.sky)
    }

    /// Applies the updates that are the world's (lights, environment); returns the others
    /// (chunks, edits) for the renderer's chunks.
    pub fn apply(&mut self, updates: Vec<WorldUpdate>) -> Vec<WorldUpdate> {
        let mut rest = Vec::with_capacity(updates.len());
        for u in updates {
            match u {
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
                u => rest.push(u),
            }
        }
        rest
    }

    /// Rebuilds the light grid after updates; call before shading.
    pub fn prepare(&mut self) {
        if self.lights_dirty {
            self.light_grid.clear();
            for l in self.lights.values() {
                let lo = (l.position - l.radius as f64).floor().as_ivec3() >> LIGHT_CELL_BITS;
                let hi = (l.position + l.radius as f64).floor().as_ivec3() >> LIGHT_CELL_BITS;
                for z in lo.z..=hi.z {
                    for y in lo.y..=hi.y {
                        for x in lo.x..=hi.x {
                            self.light_grid.entry(IVec3::new(x, y, z)).or_default().push(*l);
                        }
                    }
                }
            }
            self.lights_dirty = false;
        }
    }

    /// Point lights that may reach world block `world` (a coarse candidate list).
    pub fn lights_near(&self, world: IVec3) -> Option<&[Light]> {
        self.light_grid.get(&(world >> LIGHT_CELL_BITS)).map(|v| v.as_slice())
    }

    pub fn light_count(&self) -> usize {
        self.lights.len()
    }
}

/// Dense chunks: the reference's representation of the world's blocks (and the CPU
/// rasteriser's and voxelspace's), traced with a two-level DDA.
#[derive(Default)]
pub struct Chunks {
    chunks: HashMap<IVec3, ChunkData>,
    bounds: Option<(IVec3, IVec3)>,
    bounds_dirty: bool,
}

/// The world with its chunks: the reference lighting model, reusable by other CPU
/// renderers.
#[derive(Copy, Clone)]
pub struct VoxelScene<'a> {
    pub world: &'a World,
    pub chunks: &'a Chunks,
}

pub struct CpuReference {
    width: u32,
    height: u32,
    world: World,
    chunks: Chunks,
    meshes: mesh::Meshes,
    pool: Pool,
    pixels: Vec<u8>,
}

pub struct Hit {
    pub t: f64,
    pub cell: IVec3,
    pub block: BlockId,
    pub face: Face,
}

impl CpuReference {
    pub fn new(p: InitParams) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let mut world = World::new(&p);
        world.shadows = opts.shadows;
        Ok(Self { width: p.width, height: p.height, pixels: vec![0; (p.width * p.height * 4) as usize], world, chunks: Chunks::default(), meshes: Default::default(), pool: Pool::new(p.devices.cpu_threads(opts.threads))? })
    }
}

impl Chunks {
    pub fn chunk(&self, pos: IVec3) -> Option<&ChunkData> {
        self.chunks.get(&pos)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Rebuilds derived structures after updates; call before tracing.
    pub fn prepare(&mut self) {
        if self.bounds_dirty {
            self.rebuild_bounds();
        }
    }

    pub fn dense_bytes(&self) -> u64 {
        self.chunks.values().filter(|c| matches!(c, ChunkData::Dense(_))).count() as u64 * CHUNK_VOLUME as u64 * 2
    }

    /// Applies the chunk updates (the world's others are ignored: [`World::apply`]); returns
    /// the LOD-0 chunk positions whose contents changed.
    pub fn apply(&mut self, updates: Vec<WorldUpdate>) -> Vec<IVec3> {
        let mut changed = vec![];
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod == 0 => {
                    if data.is_air() {
                        self.chunks.remove(&key.pos);
                    } else {
                        self.chunks.insert(key.pos, data);
                    }
                    self.bounds_dirty = true;
                    changed.push(key.pos);
                }
                WorldUpdate::ChunkUnload { key } if key.lod == 0 => {
                    self.chunks.remove(&key.pos);
                    self.bounds_dirty = true;
                    changed.push(key.pos);
                }
                WorldUpdate::ChunkLoad { .. } | WorldUpdate::ChunkUnload { .. } => {}
                WorldUpdate::SetBlocks(edits) => {
                    for e in edits {
                        let pos = ChunkKey::of_block(e.pos).pos;
                        if let Some(c) = self.chunks.get_mut(&pos) {
                            c.set(local_of_block(e.pos), e.block);
                            changed.push(pos);
                        }
                    }
                }
                WorldUpdate::LightUpsert(_) | WorldUpdate::LightRemove(_) | WorldUpdate::SetEnvironment(_) => {}
            }
        }
        changed.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        changed.dedup();
        changed
    }

    fn rebuild_bounds(&mut self) {
        let mut it = self.chunks.keys();
        self.bounds = it.next().map(|&first| it.fold((first, first), |(lo, hi), &k| (lo.min(k), hi.max(k))));
        self.bounds_dirty = false;
    }

    /// Two-level DDA. Ignores the cell containing `o` (so rays starting inside a block,
    /// e.g. a camera clipping into terrain, see out of it).
    pub fn trace(&self, o: DVec3, d: DVec3, max_t: f64) -> Option<Hit> {
        let (lo, hi) = self.bounds?;
        // Clip against the loaded-chunk AABB.
        let bmin = (lo << CHUNK_BITS as i32).as_dvec3();
        let bmax = ((hi + 1) << CHUNK_BITS as i32).as_dvec3();
        let inv = d.recip();
        let t0 = (bmin - o) * inv;
        let t1 = (bmax - o) * inv;
        let tnear = t0.min(t1).max_element().max(0.0);
        let tfar = t0.max(t1).min_element().min(max_t);
        if tnear > tfar {
            return None;
        }

        let step = d.signum().as_ivec3();
        let start_cell = o.floor().as_ivec3();
        // Begin at the AABB entry; nudge the floor so the entry cell is inside the box.
        let p = o + d * tnear;
        let mut cell = p.floor().as_ivec3().clamp(lo << CHUNK_BITS as i32, ((hi + 1) << CHUNK_BITS as i32) - 1);
        let mut axis = if tnear > 0.0 { t0.min(t1).to_array().iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0 } else { 3 };

        let next_t = |cell: IVec3, size: i32| -> DVec3 {
            let mut t = DVec3::ZERO;
            for a in 0..3 {
                let boundary = if step[a] > 0 { (cell[a].div_euclid(size) + 1) * size } else { cell[a].div_euclid(size) * size };
                t[a] = if d[a] == 0.0 { f64::INFINITY } else { (boundary as f64 - o[a]) * inv[a] };
            }
            t
        };

        let mut t = tnear;
        while t <= tfar {
            let chunk_pos = cell >> CHUNK_BITS as i32;
            match self.chunks.get(&chunk_pos) {
                None | Some(ChunkData::Uniform(BlockId::AIR)) => {
                    // Skip the whole chunk: advance to its exit face.
                    let tc = next_t(cell, CHUNK_SIZE);
                    let a = min_axis(tc);
                    t = tc[a];
                    let c0 = chunk_pos << CHUNK_BITS as i32;
                    let p = o + d * t;
                    let mut next = p.floor().as_ivec3();
                    // Make the axis we crossed exact, keep the others inside the chunk slab.
                    for k in 0..3 {
                        if k == a {
                            next[k] = if step[k] > 0 { c0[k] + CHUNK_SIZE } else { c0[k] - 1 };
                        } else {
                            next[k] = next[k].clamp(c0[k], c0[k] + CHUNK_SIZE - 1);
                        }
                    }
                    cell = next;
                    axis = a;
                }
                Some(chunk) => {
                    let c0 = chunk_pos << CHUNK_BITS as i32;
                    loop {
                        if cell != start_cell {
                            let b = chunk.get(local_of_block(cell));
                            if b.is_solid() {
                                if axis == 3 {
                                    return None; // degenerate: started on a boundary inside solid
                                }
                                return Some(Hit { t, cell, block: b, face: Face::entered(axis, step[axis]) });
                            }
                        }
                        let tv = next_t(cell, 1);
                        let a = min_axis(tv);
                        t = tv[a];
                        cell[a] += step[a];
                        axis = a;
                        if t > tfar {
                            return None;
                        }
                        if cell[a] < c0[a] || cell[a] >= c0[a] + CHUNK_SIZE {
                            break;
                        }
                    }
                }
            }
        }
        None
    }

    pub fn occluded(&self, p: DVec3, dir: DVec3, dist: f64) -> bool {
        self.trace(p, dir, dist).is_some()
    }

    pub fn max_dist(&self) -> f64 {
        // Rays are clipped to the loaded region; no artificial distance limit, so the view
        // radius can change at runtime.
        1e7
    }
}

impl VoxelScene<'_> {
    pub fn trace(&self, o: DVec3, d: DVec3, max_t: f64) -> Option<Hit> {
        self.chunks.trace(o, d, max_t)
    }

    pub fn occluded(&self, p: DVec3, dir: DVec3, dist: f64) -> bool {
        self.chunks.occluded(p, dir, dist)
    }

    pub fn max_dist(&self) -> f64 {
        self.chunks.max_dist()
    }

    pub fn shade(&self, o: DVec3, d: DVec3, hit: &Hit, mode: RenderMode) -> [u8; 4] {
        let local = (o - hit.cell.as_dvec3() + d * hit.t).as_vec3();
        self.shade_surface(o + d * hit.t, local, hit.face, hit.block, mode)
    }

    /// Shades a surface point `p` on `face` of `block` (`local` = position within the
    /// block) with the reference model.
    pub fn shade_surface(&self, p: DVec3, local: Vec3, face: Face, block: BlockId, mode: RenderMode) -> [u8; 4] {
        let uv = face.uv(local);
        let tex = self.world.blocks.texture_of(block, face);
        let texel = tex.texel(texel_coord(uv.x), texel_coord(uv.y));
        if mode == RenderMode::Unlit {
            return texel;
        }
        let albedo = Vec3::new(srgb_to_linear(texel[0]), srgb_to_linear(texel[1]), srgb_to_linear(texel[2]));
        self.world.tone.encode(albedo * self.light_at(p, face.normal().as_vec3()) * self.world.env.exposure)
    }

    /// Shades a mesh surface (mesh mode): `color` is the interpolated sRGB vertex colour,
    /// `n` the normal facing the viewer. Unlit returns the colour; lit uses the voxel
    /// lighting model with the voxel world casting shadows.
    pub fn shade_mesh(&self, p: DVec3, n: Vec3, color: Vec3, mode: RenderMode) -> [u8; 4] {
        if mode == RenderMode::Unlit {
            let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            return [q(color.x), q(color.y), q(color.z), 255];
        }
        let albedo = Vec3::new(srgb_to_linear_f(color.x), srgb_to_linear_f(color.y), srgb_to_linear_f(color.z));
        self.world.tone.encode(albedo * self.light_at(p, n) * self.world.env.exposure)
    }

    /// Incoming light (linear) at surface point `p` with normal `n`: ambient, sun and
    /// point lights, each with its shadow ray through the voxel world.
    pub fn light_at(&self, p: DVec3, n: Vec3) -> Vec3 {
        let p = p + n.as_dvec3() * 1e-4;
        let w = self.world;
        let mut light = w.env.ambient;
        let max_dist = self.max_dist();
        if let Some(sun) = w.env.sun {
            let ndl = n.dot(-sun.direction);
            if ndl > 0.0 && (w.shadows == Shadows::None || !self.occluded(p, -sun.direction.as_dvec3(), max_dist)) {
                light += sun.color * ndl;
            }
        }
        if let Some(ls) = w.lights_near(p.floor().as_ivec3()) {
            for l in ls {
                let to = l.position - p;
                let c = point_light(n, to.as_vec3(), l);
                if c != Vec3::ZERO {
                    let dist = to.length();
                    if w.shadows != Shadows::All || !self.occluded(p, to / dist, dist) {
                        light += c;
                    }
                }
            }
        }
        light
    }
}

fn min_axis(t: DVec3) -> usize {
    if t.x <= t.y && t.x <= t.z {
        0
    } else if t.y <= t.z {
        1
    } else {
        2
    }
}

impl Renderer for CpuReference {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let rest = self.world.apply(updates);
        self.chunks.apply(rest);
        Ok(())
    }

    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.meshes.apply(update);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let start = Instant::now();
        self.world.prepare();
        self.chunks.prepare();
        self.meshes.prepare();
        let meshes = req.meshes.then_some(&self.meshes);
        let (w, h) = (self.width, self.height);
        let cam = req.camera;
        let o = cam.position;
        let this = VoxelScene { world: &self.world, chunks: &self.chunks };
        let max_dist = this.max_dist();
        let sky = self.world.sky();
        let mut pixels = std::mem::take(&mut self.pixels);
        self.pool.install(|| pixels.par_chunks_mut((w * 4) as usize).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let d = cam.ray_dir(x, y as u32, w, h).as_dvec3();
                let hit = this.trace(o, d, max_dist);
                // Mesh mode: the nearest mesh surface in front of the voxel hit wins.
                let mesh_hit = meshes.and_then(|m| m.trace(o, d, hit.as_ref().map_or(max_dist, |h| h.t)));
                let px = match (mesh_hit, hit) {
                    (Some(m), _) => this.shade_mesh(o + d * m.t, m.normal, m.color, req.mode),
                    (None, Some(hit)) => this.shade(o, d, &hit, req.mode),
                    (None, None) => sky,
                };
                row[x as usize * 4..x as usize * 4 + 4].copy_from_slice(&px);
            }
        }));
        self.pixels = pixels;
        let stats = FrameStats {
            cpu_ms: start.elapsed().as_secs_f32() * 1e3,
            gpu_ms: None,
            legend: None,
            counters: vec![
                ("chunks".into(), self.chunks.chunk_count() as f64),
                ("lights".into(), self.world.light_count() as f64),
                ("mesh_triangles".into(), if req.meshes { self.meshes.triangle_count() as f64 } else { 0.0 }),
            ],
            stages: Vec::new(),
        };
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&self.pixels) });
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if !self.pool.set_common(&mut self.world, name, value)? {
            anyhow::bail!("unknown parameter `{name}`");
        }
        Ok(())
    }

    fn flush(&mut self, _: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.width = width;
        self.height = height;
        self.pixels = vec![0; (width * height * 4) as usize];
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: self.chunks.dense_bytes() + self.pixels.len() as u64, gpu_bytes: 0 }
    }
}
