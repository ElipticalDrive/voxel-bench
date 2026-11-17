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
use mr_core::lighting::{Pixel, Tone, point_light, srgb_to_linear, srgb_to_linear_f, texel_coord};
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
    /// Render the LOD levels its client holds, as the pipelines do (the finest level whose
    /// region contains a cell). Off: LOD 0 only, the full-resolution reference.
    lod: bool,
    /// Lit frames by diffuse path tracing with this many indirect bounces (the `pathtrace`
    /// shading's lighting model, converged: `paths` per pixel) instead of direct lighting.
    bounces: Option<u32>,
    /// Paths per pixel when path tracing.
    paths: Option<u32>,
}

/// Paths per pixel of a path-traced reference unless `paths` says otherwise: its noise is
/// 24 dB below one path's.
const PATHS: u32 = 256;

/// The path-traced lighting model of a reference (`bounces` set).
#[derive(Copy, Clone)]
struct Paths {
    bounces: u32,
    paths: u32,
}

/// Random numbers of one pixel's paths (PCG; the same frame gives the same image).
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        let mut r = Self(seed.wrapping_mul(0x9E37_79B9).wrapping_add(1));
        r.next();
        r
    }

    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        let w = ((self.0 >> ((self.0 >> 28) + 4)) ^ self.0).wrapping_mul(277_803_737);
        (w >> 22) ^ w
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 8) as f64 / 16_777_216.0
    }

    /// A direction around `n`, cosine-weighted.
    pub fn cosine(&mut self, n: DVec3) -> DVec3 {
        let (u, phi) = (self.unit(), std::f64::consts::TAU * self.unit());
        let t = n.cross(if n.x.abs() > 0.5 { DVec3::Y } else { DVec3::X }).normalize();
        let b = n.cross(t);
        (t * (u.sqrt() * phi.cos()) + b * (u.sqrt() * phi.sin()) + n * (1.0 - u).sqrt()).normalize()
    }
}

/// The LOD levels the reference renders when asked to (`lod = true`).
const MAX_LOD: u8 = 15;

pub fn entry() -> RendererEntry {
    RendererEntry {
        name: "cpu-reference",
        info: |p| {
            let mut i = info();
            match p.options::<Options>()?.lod {
                true => i.features = i.features.with_lod(MAX_LOD),
                false => i.features.lack(Feature::Lod, "the reference holds LOD levels only with `lod = true`"),
            }
            Ok(i)
        },
        create: |p, _, _| Ok(Box::new(CpuReference::new(p)?)),
    }
}

pub fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu-reference".into(),
        description: "Multi-threaded f64 two-level DDA ray caster; defines the reference images".into(),
        tags: vec!["cpu".into(), "raycast".into(), "reference".into()],
        features: Features::of(&[Feature::Lit, Feature::Meshes, Feature::MeshRays]),
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
    pub fn sky<P: Pixel>(&self) -> P {
        P::encode(self.tone, self.env.sky)
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

    /// Shades a surface point `p` on `face` of `block` (`local` = position within the
    /// block) with the reference model, shadow rays tested by `occluded` (from a point along a
    /// unit direction within a distance: whether something is hit).
    pub fn shade_surface<P: Pixel>(&self, p: DVec3, local: Vec3, face: Face, block: BlockId, mode: RenderMode, occluded: impl Fn(DVec3, DVec3, f64) -> bool) -> P {
        let uv = face.uv(local);
        let tex = self.blocks.texture_of(block, face);
        let texel = tex.texel(texel_coord(uv.x), texel_coord(uv.y));
        if mode == RenderMode::Unlit {
            return P::from_rgba8(texel);
        }
        let albedo = Vec3::new(srgb_to_linear(texel[0]), srgb_to_linear(texel[1]), srgb_to_linear(texel[2]));
        P::encode(self.tone, albedo * self.light_at(p, face.normal().as_vec3(), occluded) * self.env.exposure)
    }

    /// Incoming light (linear) at surface point `p` with normal `n`: ambient, sun and
    /// point lights, each with its shadow ray tested by `occluded`.
    pub fn light_at(&self, p: DVec3, n: Vec3, occluded: impl Fn(DVec3, DVec3, f64) -> bool) -> Vec3 {
        let p = p + n.as_dvec3() * 1e-4;
        let mut light = self.env.ambient;
        if let Some(sun) = self.env.sun {
            let ndl = n.dot(-sun.direction);
            if ndl > 0.0 && (self.shadows == Shadows::None || !occluded(p, -sun.direction.as_dvec3(), MAX_DIST)) {
                light += sun.color * ndl;
            }
        }
        if let Some(ls) = self.lights_near(p.floor().as_ivec3()) {
            for l in ls {
                let to = l.position - p;
                let c = point_light(n, to.as_vec3(), l);
                if c != Vec3::ZERO {
                    let dist = to.length();
                    if self.shadows != Shadows::All || !occluded(p, to / dist, dist) {
                        light += c;
                    }
                }
            }
        }
        light
    }
}

/// How far a ray goes: rays are clipped to the loaded region, no artificial limit (so the
/// view radius can change at run time).
const MAX_DIST: f64 = 1e7;

/// Dense chunks: the reference's representation of the world's blocks (and the CPU
/// rasteriser's and voxelspace's), traced with a two-level DDA.
#[derive(Default)]
pub struct Chunks {
    chunks: HashMap<IVec3, ChunkData>,
    bounds: Option<(IVec3, IVec3)>,
    bounds_dirty: bool,
    /// The coarse levels, when it renders them ([`Chunks::with_lod`]).
    lod: Option<Lod>,
}

/// The coarse levels of a streamed region: at each level a cube of `radius` chunks around the
/// camera's chunk of that level; a cell is shown at the finest level whose cube contains it.
struct Lod {
    radius: i32,
    max: u8,
    /// The camera's block (set every frame).
    cam: IVec3,
    coarse: HashMap<ChunkKey, ChunkData>,
}

impl Lod {
    fn level(&self, cell: IVec3) -> u8 {
        (0..self.max)
            .find(|&m| {
                let s = CHUNK_BITS as i32 + m as i32;
                ((cell >> s) - (self.cam >> s)).abs().max_element() <= self.radius
            })
            .unwrap_or(self.max)
    }
}

/// The world with its chunks and meshes: the reference lighting model, reusable by other CPU
/// renderers. Meshes (when shown) are surfaces, occluders of shadow rays and bounce targets.
#[derive(Copy, Clone)]
pub struct VoxelScene<'a> {
    pub world: &'a World,
    pub chunks: &'a Chunks,
    pub meshes: Option<&'a mesh::Meshes>,
}

/// A surface a path's ray met: distance, normal facing the ray, linear albedo.
struct Surface {
    t: f64,
    n: DVec3,
    albedo: Vec3,
}

pub struct CpuReference {
    width: u32,
    height: u32,
    world: World,
    chunks: Chunks,
    meshes: mesh::Meshes,
    pool: Pool,
    pixels: Vec<u8>,
    /// Lit frames are path traced.
    paths: Option<Paths>,
}

pub struct Hit {
    pub t: f64,
    pub cell: IVec3,
    pub block: BlockId,
    pub face: Face,
    /// The LOD level of the cell hit (its side is `2^lod` blocks, `cell` its lowest corner).
    pub lod: u8,
}

impl CpuReference {
    pub fn new(p: InitParams) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        let mut world = World::new(&p);
        world.shadows = opts.shadows;
        let chunks = if opts.lod && p.max_lod > 0 { Chunks::with_lod(p.view_radius_chunks, p.max_lod.min(MAX_LOD)) } else { Chunks::default() };
        let paths = opts.bounces.map(|bounces| Paths { bounces, paths: opts.paths.unwrap_or(PATHS).max(1) });
        Ok(Self { width: p.width, height: p.height, pixels: vec![0; (p.width * p.height * 4) as usize], world, chunks, meshes: Default::default(), pool: Pool::new(p.devices.cpu_threads(opts.threads))?, paths })
    }
}

impl Chunks {
    /// Chunks that hold and trace the levels up to `max_lod` of a region of `radius` chunks
    /// per level; [`Chunks::look_from`] places the region before tracing.
    pub fn with_lod(radius: u32, max_lod: u8) -> Self {
        Self { lod: Some(Lod { radius: radius as i32, max: max_lod, cam: IVec3::ZERO, coarse: HashMap::new() }), ..Default::default() }
    }

    /// The camera's position: the centre of the LOD regions (nothing without LOD).
    pub fn look_from(&mut self, o: DVec3) {
        if let Some(l) = &mut self.lod {
            l.cam = o.floor().as_ivec3();
        }
    }

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
    pub fn apply(&mut self, updates: &[WorldUpdate]) -> Vec<IVec3> {
        let mut changed = vec![];
        for u in updates {
            match u {
                WorldUpdate::ChunkLoad { key, data } if key.lod == 0 => {
                    if data.is_air() {
                        self.chunks.remove(&key.pos);
                    } else {
                        self.chunks.insert(key.pos, data.clone());
                    }
                    self.bounds_dirty = true;
                    changed.push(key.pos);
                }
                WorldUpdate::ChunkUnload { key } if key.lod == 0 => {
                    self.chunks.remove(&key.pos);
                    self.bounds_dirty = true;
                    changed.push(key.pos);
                }
                WorldUpdate::ChunkLoad { key, data } => {
                    if let Some(l) = self.lod.as_mut().filter(|l| key.lod <= l.max) {
                        if data.is_air() {
                            l.coarse.remove(key);
                        } else {
                            l.coarse.insert(*key, data.clone());
                        }
                    }
                }
                WorldUpdate::ChunkUnload { key } => {
                    if let Some(l) = &mut self.lod {
                        l.coarse.remove(key);
                    }
                }
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
        if let Some(l) = &self.lod {
            return self.trace_lod(l, o, d, max_t);
        }
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
                                return Some(Hit { t, cell, block: b, face: Face::entered(axis, step[axis]), lod: 0 });
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

    /// The walk through LOD levels: every cell at the finest level whose region contains it,
    /// exactly (no step crosses into another level's cells unseen). Ignores the cell containing
    /// `o`, as [`Chunks::trace`] does.
    fn trace_lod(&self, l: &Lod, o: DVec3, d: DVec3, max_t: f64) -> Option<Hit> {
        // The outermost level's region bounds everything.
        let top = CHUNK_BITS as i32 + l.max as i32;
        let c = l.cam >> top;
        let (lo, hi) = ((c - l.radius) << top, ((c + l.radius + 1) << top) - 1);
        let inv = d.recip();
        let t0 = (lo.as_dvec3() - o) * inv;
        let t1 = ((hi + 1).as_dvec3() - o) * inv;
        let tnear = t0.min(t1).max_element().max(0.0);
        let tfar = t0.max(t1).min_element().min(max_t);
        if tnear > tfar {
            return None;
        }
        let step = d.signum().as_ivec3();
        let start = o.floor().as_ivec3();
        let mut cell = (o + d * tnear).floor().as_ivec3().clamp(lo, hi);
        let mut axis = if tnear > 0.0 { t0.min(t1).to_array().iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0 } else { 3 };
        let mut t = tnear;
        while t <= tfar {
            let m = l.level(cell);
            let key = cell >> (CHUNK_BITS as i32 + m as i32);
            let chunk = if m == 0 { self.chunks.get(&key) } else { l.coarse.get(&ChunkKey { pos: key, lod: m }) };
            // The box to leave next: the cell, or for an empty chunk the largest box around the
            // cell that is all of this level (the chunk at level 0; else the chunk's octant, a
            // chunk of the level below that its region left out whole).
            let size = match chunk {
                None | Some(ChunkData::Uniform(BlockId::AIR)) => CHUNK_SIZE << m.saturating_sub(1),
                Some(chunk) => {
                    if cell >> m != start >> m {
                        let b = chunk.get(local_of_block(cell >> m));
                        if b.is_solid() {
                            if axis == 3 {
                                return None; // degenerate: started on a boundary inside solid
                            }
                            return Some(Hit { t, cell: (cell >> m) << m, block: b, face: Face::entered(axis, step[axis]), lod: m });
                        }
                    }
                    1 << m
                }
            };
            let b0 = cell & !(size - 1);
            let mut tv = DVec3::INFINITY;
            for a in 0..3 {
                if d[a] != 0.0 {
                    tv[a] = ((if step[a] > 0 { b0[a] + size } else { b0[a] }) as f64 - o[a]) * inv[a];
                }
            }
            let a = min_axis(tv);
            t = tv[a];
            // The axis crossed is exact; the others stay inside the box's slab.
            let mut next = (o + d * t).floor().as_ivec3();
            for k in 0..3 {
                next[k] = if k == a { if step[k] > 0 { b0[k] + size } else { b0[k] - 1 } } else { next[k].clamp(b0[k], b0[k] + size - 1) };
            }
            cell = next;
            axis = a;
        }
        None
    }

    pub fn occluded(&self, p: DVec3, dir: DVec3, dist: f64) -> bool {
        self.trace(p, dir, dist).is_some()
    }

    pub fn max_dist(&self) -> f64 {
        MAX_DIST
    }
}

impl VoxelScene<'_> {
    pub fn trace(&self, o: DVec3, d: DVec3, max_t: f64) -> Option<Hit> {
        self.chunks.trace(o, d, max_t)
    }

    /// Whether a voxel or a mesh is hit from `p` along `dir` within `dist`.
    pub fn occluded(&self, p: DVec3, dir: DVec3, dist: f64) -> bool {
        self.chunks.occluded(p, dir, dist) || self.meshes.is_some_and(|m| m.occluded(p, dir, dist))
    }

    pub fn max_dist(&self) -> f64 {
        self.chunks.max_dist()
    }

    /// The nearest surface, voxel or mesh.
    fn surface(&self, o: DVec3, d: DVec3, max_t: f64) -> Option<Surface> {
        let hit = self.trace(o, d, max_t);
        if let Some(m) = self.meshes.and_then(|m| m.trace(o, d, hit.as_ref().map_or(max_t, |h| h.t))) {
            return Some(Surface { t: m.t, n: m.normal.as_dvec3(), albedo: m.albedo() });
        }
        hit.map(|h| Surface { t: h.t, n: h.face.normal().as_dvec3(), albedo: self.albedo(o, d, &h) })
    }

    pub fn shade(&self, o: DVec3, d: DVec3, hit: &Hit, mode: RenderMode) -> [u8; 4] {
        self.shade_surface(o + d * hit.t, Self::local(o, d, hit), hit.face, hit.block, mode)
    }

    /// The hit point within its block.
    fn local(o: DVec3, d: DVec3, hit: &Hit) -> Vec3 {
        let p = o + d * hit.t;
        // A coarse cell shows its block's texture once per block, as the pipelines do.
        if hit.lod == 0 { (o - hit.cell.as_dvec3() + d * hit.t).as_vec3() } else { (p - p.floor()).as_vec3() }
    }

    /// The linear albedo where `hit` was hit.
    fn albedo(&self, o: DVec3, d: DVec3, hit: &Hit) -> Vec3 {
        let uv = hit.face.uv(Self::local(o, d, hit));
        let texel = self.world.blocks.texture_of(hit.block, hit.face).texel(texel_coord(uv.x), texel_coord(uv.y));
        Vec3::new(srgb_to_linear(texel[0]), srgb_to_linear(texel[1]), srgb_to_linear(texel[2]))
    }

    /// Incoming light (linear) at surface point `p` with normal `n` by diffuse path tracing,
    /// the mean of `paths` paths of up to `bounces` bounces: the `pathtrace` shading's model.
    /// Every vertex takes the sun and the point lights with their shadow rays
    /// ([`Self::light_at`]); the ambient term stands in for the sky, so only a ray that
    /// escapes and the last vertex take it. Bounces land on voxels and meshes. `bounces = 0` is
    /// [`Self::light_at`].
    pub fn light_by_paths(&self, p: DVec3, n: Vec3, bounces: u32, paths: u32, rng: &mut Rng) -> Vec3 {
        let ambient = self.world.env.ambient;
        let direct = self.light_at(p, n);
        if bounces == 0 {
            return direct;
        }
        let max_dist = self.max_dist();
        let mut sum = Vec3::ZERO;
        for _ in 0..paths {
            let (mut p, mut n, mut through) = (p, n.as_dvec3(), Vec3::ONE);
            for depth in 1..=bounces {
                let o = p + n * 1e-4;
                let d = rng.cosine(n);
                let Some(hit) = self.surface(o, d, max_dist) else {
                    sum += through * ambient;
                    break;
                };
                (p, n) = (o + d * hit.t, hit.n);
                through *= hit.albedo;
                let light = self.light_at(p, n.as_vec3());
                sum += through * if depth < bounces { light - ambient } else { light };
            }
        }
        direct - ambient + sum / paths as f32
    }

    /// Shades a surface point `p` on `face` of `block` (`local` = position within the
    /// block) with the reference model.
    pub fn shade_surface<P: Pixel>(&self, p: DVec3, local: Vec3, face: Face, block: BlockId, mode: RenderMode) -> P {
        self.world.shade_surface(p, local, face, block, mode, |p, d, dist| self.occluded(p, d, dist))
    }

    /// Shades a mesh surface (mesh mode): `color` is the interpolated sRGB vertex colour,
    /// `n` the normal facing the viewer. Unlit returns the colour; lit uses the voxel
    /// lighting model, voxels and meshes casting shadows.
    pub fn shade_mesh(&self, p: DVec3, n: Vec3, color: Vec3, mode: RenderMode) -> [u8; 4] {
        if mode == RenderMode::Unlit {
            let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            return [q(color.x), q(color.y), q(color.z), 255];
        }
        let albedo = Vec3::new(srgb_to_linear_f(color.x), srgb_to_linear_f(color.y), srgb_to_linear_f(color.z));
        self.world.tone.encode(albedo * self.light_at(p, n) * self.world.env.exposure)
    }

    /// Incoming light (linear) at surface point `p` with normal `n`: ambient, sun and
    /// point lights, each with its shadow ray through the voxels and meshes.
    pub fn light_at(&self, p: DVec3, n: Vec3) -> Vec3 {
        self.world.light_at(p, n, |p, d, dist| self.occluded(p, d, dist))
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
        self.chunks.apply(&rest);
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
        self.chunks.look_from(req.camera.position);
        self.meshes.prepare();
        let (w, h) = (self.width, self.height);
        let cam = req.camera;
        let o = cam.position;
        let this = VoxelScene { world: &self.world, chunks: &self.chunks, meshes: req.meshes.then_some(&self.meshes) };
        let max_dist = this.max_dist();
        let sky = self.world.sky();
        // Path-traced lit frames: the light at the primary point from its paths.
        let paths = self.paths.filter(|_| req.mode == RenderMode::Lit);
        let (tone, exposure) = (self.world.tone, self.world.env.exposure);
        let lit = |albedo: Vec3, p: DVec3, n: Vec3, k: Paths, rng: &mut Rng| -> [u8; 4] { tone.encode(albedo * this.light_by_paths(p, n, k.bounces, k.paths, rng) * exposure) };
        let mut pixels = std::mem::take(&mut self.pixels);
        self.pool.install(|| pixels.par_chunks_mut((w * 4) as usize).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let d = cam.ray_dir(x, y as u32, w, h).as_dvec3();
                let hit = this.trace(o, d, max_dist);
                // Mesh mode: the nearest mesh surface in front of the voxel hit wins.
                let mesh_hit = this.meshes.and_then(|m| m.trace(o, d, hit.as_ref().map_or(max_dist, |h| h.t)));
                let mut rng = Rng::new(y as u32 * w + x);
                let px = match (mesh_hit, hit, paths) {
                    (Some(m), _, Some(k)) => lit(m.albedo(), o + d * m.t, m.normal, k, &mut rng),
                    (Some(m), _, None) => this.shade_mesh(o + d * m.t, m.normal, m.color, req.mode),
                    (None, Some(hit), Some(k)) => lit(this.albedo(o, d, &hit), o + d * hit.t, hit.face.normal().as_vec3(), k, &mut rng),
                    (None, Some(hit), None) => this.shade(o, d, &hit, req.mode),
                    (None, None, _) => sky,
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

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: BlockId = BlockId(1);

    fn load(c: &mut Chunks, pos: IVec3, lod: u8, cells: &[IVec3]) {
        let mut data = ChunkData::Uniform(BlockId::AIR);
        for &cell in cells {
            data.set(cell, STONE);
        }
        c.apply(&[WorldUpdate::ChunkLoad { key: ChunkKey { pos, lod }, data }]);
    }

    /// A cell is seen at the finest level whose region contains it: a coarse chunk's cells
    /// inside the finer region are never seen, those outside are, at their size.
    #[test]
    fn lod_walk_takes_the_finest_level_present() {
        // Radius 0, two levels: level 0 is the camera's chunk (blocks 0..32), level 1 the
        // chunk of 64 blocks around it.
        let mut c = Chunks::with_lod(0, 1);
        c.look_from(DVec3::new(1.5, 1.5, 1.5));
        // Level 1: a cell inside level 0's region (blocks 10..12) and one outside (40..42).
        load(&mut c, IVec3::ZERO, 1, &[IVec3::new(5, 0, 0), IVec3::new(20, 0, 0)]);
        let (o, d) = (DVec3::new(1.5, 1.5, 1.5), DVec3::X);
        let h = c.trace(o, d, 1e7).expect("the coarse cell beyond the fine region");
        assert_eq!((h.cell, h.lod, h.t), (IVec3::new(40, 0, 0), 1, 38.5));
        // A level-0 block in front wins; the level-0 chunk being loaded hides nothing else.
        load(&mut c, IVec3::ZERO, 0, &[IVec3::new(20, 1, 1)]);
        let h = c.trace(o, d, 1e7).unwrap();
        assert_eq!((h.cell, h.lod, h.t), (IVec3::new(20, 1, 1), 0, 18.5));
        // Backwards: nothing, and the walk ends at the outer region's edge.
        assert!(c.trace(o, -d, 1e7).is_none());
        // A ray from outside the fine region entering it skips the coarse cell inside it.
        let h = c.trace(DVec3::new(50.5, 1.5, 1.5), -d, 1e7).unwrap();
        assert_eq!((h.cell, h.lod), (IVec3::new(40, 0, 0), 1));
        let h = c.trace(DVec3::new(39.5, 1.5, 1.5), -d, 1e7).unwrap();
        assert_eq!((h.cell, h.lod), (IVec3::new(20, 1, 1), 0));
        // Without the level-0 block the ray crosses the fine region to its far side unseen.
        load(&mut c, IVec3::ZERO, 0, &[]);
        assert!(c.trace(DVec3::new(39.5, 1.5, 1.5), -d, 1e7).is_none());
    }

    /// The path-traced light: without bounces the direct light; in the open the same (every
    /// path escapes into the ambient the direct model adds); under a ceiling with no sun what
    /// the ambient gives however the paths go when the ceiling is white (no energy lost or
    /// made), less when it is grey.
    #[test]
    fn paths_agree_with_direct_light_where_they_must() {
        let texture = |v: u8| Texture { name: String::new(), texels: vec![[v, v, v, 255]; TEX_SIZE * TEX_SIZE] };
        let block = |t: u16| BlockType { name: String::new(), faces: [t; 6] };
        let blocks = Arc::new(BlockRegistry { textures: vec![texture(255), texture(128)], blocks: vec![block(0), block(0), block(1)] });
        let mut world = World { blocks, env: Environment::default(), shadows: Shadows::All, tone: Tone::Clamp, lights: HashMap::new(), light_grid: HashMap::new(), lights_dirty: false };
        let ambient = world.env.ambient;
        let (at, up) = (DVec3::new(8.5, 1.0, 8.5), Vec3::Y);
        let chunks = chunk_of(&[(IVec3::new(8, 0, 8), STONE)]);
        {
            let s = VoxelScene { world: &world, chunks: &chunks, meshes: None };
            let direct = s.light_at(at, up);
            assert!(direct.max_element() > ambient.max_element(), "the sun reaches it");
            assert_eq!(s.light_by_paths(at, up, 0, 16, &mut Rng::new(1)), direct);
            assert!((s.light_by_paths(at, up, 3, 16, &mut Rng::new(1)) - direct).abs().max_element() < 1e-6);
        }
        world.env.sun = None;
        for (ceiling, grey) in [(STONE, false), (BlockId(2), true)] {
            let mut cells = vec![(IVec3::new(8, 0, 8), STONE)];
            cells.extend((0..32).flat_map(|x| (0..32).map(move |z| (IVec3::new(x, 4, z), ceiling))));
            let chunks = chunk_of(&cells);
            let s = VoxelScene { world: &world, chunks: &chunks, meshes: None };
            let light = s.light_by_paths(at, up, 2, 1024, &mut Rng::new(1));
            if grey {
                assert!(light.max_element() < ambient.max_element() * 0.9 && light.min_element() > ambient.min_element() * 0.2, "{light}");
            } else {
                assert!((light - ambient).abs().max_element() < 1e-5, "{light}");
            }
            // The same seed, the same light.
            assert_eq!(light, s.light_by_paths(at, up, 2, 1024, &mut Rng::new(1)));
        }
    }

    /// Meshes cast shadows and take bounces: a mesh square between a floor point and the sun
    /// leaves it the ambient; under a mesh ceiling with no sun, a white one gives back what the
    /// ambient gives (as a voxel ceiling does), a grey one less.
    #[test]
    fn meshes_occlude_and_take_bounces() {
        let texture = |v: u8| Texture { name: String::new(), texels: vec![[v, v, v, 255]; TEX_SIZE * TEX_SIZE] };
        let block = |t: u16| BlockType { name: String::new(), faces: [t; 6] };
        let blocks = Arc::new(BlockRegistry { textures: vec![texture(255)], blocks: vec![block(0), block(0)] });
        let mut world = World { blocks, env: Environment::default(), shadows: Shadows::All, tone: Tone::Clamp, lights: HashMap::new(), light_grid: HashMap::new(), lights_dirty: false };
        let ambient = world.env.ambient;
        let (at, up) = (DVec3::new(8.5, 1.0, 8.5), Vec3::Y);
        let chunks = chunk_of(&[(IVec3::new(8, 0, 8), STONE)]);
        // A horizontal square of side 2h at height y above `at`, its centre moved by `by`.
        let square = |y: f32, h: f32, grey: u8, by: DVec3| {
            let m = MeshData { id: 0, positions: vec![[-h, y, -h], [h, y, -h], [h, y, h], [-h, y, h]], colors: vec![[grey, grey, grey, 255]; 4], indices: vec![0, 1, 2, 0, 2, 3] };
            let mut s = mesh::Meshes::default();
            s.apply(MeshUpdate::Meshes(vec![Arc::new(m)]));
            s.apply(MeshUpdate::Instances(vec![MeshInstance { id: 0, mesh: 0, position: DVec3::new(8.5, 0.0, 8.5) + by, rotation: glam::DQuat::IDENTITY, scale: 1.0 }]));
            s.prepare();
            s
        };
        let sun = world.env.sun.expect("the default has a sun").direction.as_dvec3();
        // A square centred on the sun's ray from the point, 3 blocks up it.
        let shade = square(0.0, 3.0, 255, at - sun * 3.0 - DVec3::new(8.5, 0.0, 8.5));
        let open = VoxelScene { world: &world, chunks: &chunks, meshes: None }.light_at(at, up);
        let shaded = VoxelScene { world: &world, chunks: &chunks, meshes: Some(&shade) }.light_at(at, up);
        assert!(open.max_element() > ambient.max_element(), "the sun reaches it");
        assert!((shaded - ambient).abs().max_element() < 1e-6, "the mesh shades it: {shaded}");
        world.env.sun = None;
        for (grey, absorbs) in [(255, false), (128, true)] {
            let ceiling = square(4.0, 1e4, grey, DVec3::ZERO);
            let s = VoxelScene { world: &world, chunks: &chunks, meshes: Some(&ceiling) };
            let light = s.light_by_paths(at, up, 2, 1024, &mut Rng::new(1));
            if absorbs {
                assert!(light.max_element() < ambient.max_element() * 0.9 && light.min_element() > ambient.min_element() * 0.2, "{light}");
            } else {
                assert!((light - ambient).abs().max_element() < 1e-5, "{light}");
            }
        }
    }

    /// Chunks of one LOD-0 chunk at the origin holding these cells.
    fn chunk_of(cells: &[(IVec3, BlockId)]) -> Chunks {
        let mut data = ChunkData::Uniform(BlockId::AIR);
        for &(cell, b) in cells {
            data.set(cell, b);
        }
        let mut c = Chunks::default();
        c.apply(&[WorldUpdate::ChunkLoad { key: ChunkKey { pos: IVec3::ZERO, lod: 0 }, data }]);
        c.prepare();
        c
    }
}
