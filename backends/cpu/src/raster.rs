//! The CPU raster lane (`cpu-raster`; PLAN step 13e3: was the cpu-raster back end): a
//! software rasteriser on the CPU, assembled from components
//! (`mr-components`, IDEAS §12): **geometry** (`greedy` or `naive` quads, meshed on the
//! CPU), **visibility** `faces` with **draw** `raster` (below), **shading** `direct` (here the reference
//! scene's f64 model, with CPU ray-traced shadows). Was the cpu-raster renderer.
//!
//! Shading stays on the reference's f64 model on purpose, not the f32 CPU shading
//! component (`direct::cpu::shade`): the same algorithm at a different precision. Measured
//! 2026-10-11, the f32 component lowered lit PSNR (terrain 63.1 -> 57.7 dB, room 99.0 ->
//! 76.6, many-lights 64.8 -> 61.0) at the same speed.
//!
//! Pipeline per frame:
//! 1. incremental parallel meshing by the geometry component,
//! 2. per-chunk frustum culling and per-face-direction culling,
//! 3. transform + homogeneous clipping of each quad, fan triangulation,
//! 4. binning into screen tiles grouped in horizontal bands,
//! 5. parallel edge-function rasterisation per band into a depth + visibility-ID buffer,
//! 6. deferred shading per pixel (the common pass, `shading::deferred`; the quad id is the
//!    record's encoding): the surface point is recomputed exactly by intersecting the
//!    pixel's ray with the quad's plane (f64), then shaded with the reference model (CPU
//!    ray-traced shadows in lit mode). With `placement = inline` (PLAN step 12c), step 5
//!    shades every fragment that passes the depth test instead (forward shading).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use cpu_reference::{Chunks, Pool, Shadows, VoxelScene, World};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_core::*;
use mr_components::geometry::{Knobs, MeshFn};
use mr_components::{names_on, require, Slot};
use mr_mesh::{FACE_AXES, Mesh, Quad};
use rayon::prelude::*;
use serde::Deserialize;

use crate::{Core, Frame, Lane};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu-raster".into(),
        description: "CPU-raster back end: geometry component's quads, homogeneous clipping, tiled multithreaded edge-function raster into a visibility buffer, deferred exact shading".into(),
        tags: vec!["cpu".into(), "raster".into(), "software".into(), "visibility-buffer".into(), "modular".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: mr_components::lighting_knobs(BACKEND, params()),
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    /// Tile width in pixels.
    tile_width: u32,
    /// Band height in pixels (bands are the unit of parallelism).
    band_height: u32,
    threads: usize,
    shadows: Shadows,
    geometry: String,
    visibility: String,
    draw: String,
    shading: String,
    placement: String,
    merge: String,
    cull: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { tile_width: 64, band_height: 16, threads: 0, shadows: Shadows::All, geometry: "greedy".into(), visibility: "faces".into(), draw: "raster".into(), shading: "direct".into(), placement: "deferred".into(), merge: "block".into(), cull: true }
    }
}

pub(crate) const BACKEND: &str = "cpu-raster";

fn params() -> Vec<ParamSpec> {
    let mut p = cpu_reference::cpu_params();
    p.push(ParamSpec::choice("geometry", "Geometry component (quads meshed on the CPU)", &names_on(BACKEND, Slot::Geometry), "greedy", true));
    p.push(ParamSpec::choice("visibility", "Visibility strategy", &names_on(BACKEND, Slot::Visibility), "faces", false));
    p.push(ParamSpec::choice("draw", "Draw component: `raster` here is edge functions into a visibility buffer, then deferred shading", &names_on(BACKEND, Slot::Draw), "raster", false));
    p.push(ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false));
    p.push(mr_components::placement_param(BACKEND, "deferred"));
    p.push(ParamSpec::bool("cull", "naive: skip faces hidden by solid neighbours", true, true).of("geometry", &["naive"]));
    p.push(ParamSpec::int("tile_width", "Raster tile width (pixels)", 8, 1024, 64, true).of("draw", &["raster"]));
    p.push(ParamSpec::int("band_height", "Raster band height (pixels); bands are the unit of parallelism", 1, 512, 16, true).of("draw", &["raster"]));
    p
}

/// Screen-space triangle: (x, y, depth) per vertex plus visibility id.
#[derive(Copy, Clone)]
struct Tri {
    v: [[f32; 3]; 3],
    id: u32,
}

pub(crate) struct CpuRaster {
    /// The nodes of the timed stages (PLAN step 11): meshing, transform, raster, shading;
    /// raster with inline shading.
    stage_nodes: [Vec<String>; 5],
    /// `placement = deferred` (the default): the rasteriser writes quad ids, the common
    /// deferred pass shades each pixel; inline, each fragment passing the depth test shades.
    deferred: bool,
    opts: Options,
    chunks: Chunks,
    meshes: HashMap<IVec3, Mesh>,
    dirty: HashSet<IVec3>,
    /// The geometry component's mesher and knobs.
    mesh: MeshFn,
    knobs: Knobs,
    depth: Vec<f32>,
    ids: Vec<u32>,
}

impl CpuRaster {
    pub(crate) fn new(p: &InitParams, g: &mr_components::graph::Checked) -> anyhow::Result<(Core, Self)> {
        let n = (p.width * p.height) as usize;
        let opts: Options = p.options()?;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        require(BACKEND, Slot::Draw, &opts.draw)?;
        require(BACKEND, Slot::Shading, &opts.shading)?;
        anyhow::ensure!(opts.merge == "block", "merge = `{}`: cpu-raster has no per-pixel block lookup (only `block`)", opts.merge);
        let mesh = mesh_fn(&require(BACKEND, Slot::Geometry, &opts.geometry)?);
        let knobs = Knobs { cull: opts.cull, ..Default::default() };
        use mr_components::graph::Role;
        let core = Core::new(p, g, opts.threads, opts.shadows)?;
        Ok((core, Self {
            stage_nodes: [g.ids(&[Role::Geometry]), g.ids(&[Role::Visibility]), g.ids(&[Role::Draw]), g.ids(&[Role::Shading]), g.ids(&[Role::Draw, Role::Shading])],
            deferred: mr_components::parse_placement(&opts.placement)?,
            opts,
            chunks: Chunks::default(),
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            mesh,
            knobs,
            depth: vec![0.0; n],
            ids: vec![0; n],
        }))
    }

    fn remesh(&mut self) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let dirty: Vec<IVec3> = self.dirty.drain().collect();
        let (scene, mesh, knobs) = (&self.chunks, self.mesh, self.knobs);
        let meshes: Vec<(IVec3, Option<Mesh>)> = dirty
            .par_iter()
            .map(|&p| (p, scene.chunk(p).map(|c| mesh(c, Face::ALL.map(|f| scene.chunk(p + f.normal())), 0, knobs))))
            .collect();
        for (p, m) in meshes {
            match m {
                Some(m) if !m.quads.is_empty() => {
                    self.meshes.insert(p, m);
                }
                _ => {
                    self.meshes.remove(&p);
                }
            }
        }
        dirty.len()
    }
}

/// The CPU mesher of a geometry component (quads with per-face counts: greedy, naive).
fn mesh_fn(name: &str) -> MeshFn {
    mr_components::geometry::gpu(name).expect("validated").mesh
}

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

/// Clips a convex polygon in homogeneous clip space against the visible frustum
/// (left, right, bottom, top, near) using Sutherland-Hodgman.
fn clip(poly: &mut Vec<Vec4>, scratch: &mut Vec<Vec4>) {
    let planes = [
        |c: Vec4| c.w + c.x,
        |c: Vec4| c.w - c.x,
        |c: Vec4| c.w + c.y,
        |c: Vec4| c.w - c.y,
        |c: Vec4| c.w - c.z,
    ];
    for dist in planes {
        if poly.is_empty() {
            return;
        }
        scratch.clear();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (da, db) = (dist(a), dist(b));
            if da >= 0.0 {
                scratch.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                scratch.push(a + (b - a) * (da / (da - db)));
            }
        }
        std::mem::swap(poly, scratch);
    }
}

impl CpuRaster {
    fn frame(&mut self, frame: &mut Frame, world: &World, req: &FrameRequest) -> FrameStats {
        let start = Instant::now();

        self.chunks.prepare();
        let remeshed = self.remesh();
        let t_mesh = start.elapsed().as_secs_f64() * 1e3;
        let (w, h) = (frame.width, frame.height);
        let cam = req.camera;
        let (cam_block, frac) = cam.split_position();
        let vp = cam.projection(w as f32 / h as f32) * cam.view_rotation();
        let planes = frustum_planes(vp);

        // Visible chunks, with a global quad numbering for visibility ids (0 = empty).
        let mut visible: Vec<(IVec3, &Mesh, u32)> = vec![];
        let mut next_id = 1u32;
        for (pos, m) in &self.meshes {
            let min = (*pos * 32 - cam_block).as_vec3() - frac;
            if aabb_visible(&planes, min, min + 32.0) {
                visible.push((*pos, m, next_id));
                next_id += m.quads.len() as u32;
            }
        }
        visible.sort_by_key(|v| v.2);

        // Transform, clip, triangulate (parallel per chunk).
        let (fw, fh) = (w as f32, h as f32);
        let tris: Vec<Tri> = visible
            .par_iter()
            .flat_map_iter(|&(pos, m, base)| {
                let min = (pos * 32 - cam_block).as_vec3() - frac;
                let mut out = vec![];
                let mut poly = Vec::with_capacity(12);
                let mut scratch = Vec::with_capacity(12);
                let mut first = 0usize;
                for (f, &n) in m.counts.iter().enumerate() {
                    let axis = f / 2;
                    let facing = if f % 2 == 0 { 0.0 > min[axis] } else { 0.0 < min[axis] + 32.0 };
                    if facing {
                        for qi in first..first + n as usize {
                            let q = Quad::decode(m.quads[qi]);
                            poly.clear();
                            let [c0, c1, c2, c3] = q.corners();
                            for c in [c0, c1, c3, c2] {
                                poly.push(vp * (min + c).extend(1.0));
                            }
                            clip(&mut poly, &mut scratch);
                            if poly.len() < 3 {
                                continue;
                            }
                            let sv: Vec<[f32; 3]> = poly
                                .iter()
                                .map(|c| {
                                    let n = c.truncate() / c.w;
                                    [(n.x * 0.5 + 0.5) * fw, (0.5 - n.y * 0.5) * fh, n.z]
                                })
                                .collect();
                            for k in 1..sv.len() - 1 {
                                out.push(Tri { v: [sv[0], sv[k], sv[k + 1]], id: base + qi as u32 });
                            }
                        }
                    }
                    first += n as usize;
                }
                out
            })
            .collect();
        let t_geom = start.elapsed().as_secs_f64() * 1e3;

        // Bin triangles into (band, tile) cells.
        let (tw, bh) = (self.opts.tile_width.max(8), self.opts.band_height.max(1));
        let tiles_x = w.div_ceil(tw) as usize;
        let bands = h.div_ceil(bh) as usize;
        let mut bins: Vec<Vec<u32>> = vec![vec![]; tiles_x * bands];
        for (i, t) in tris.iter().enumerate() {
            let xs = [t.v[0][0], t.v[1][0], t.v[2][0]];
            let ys = [t.v[0][1], t.v[1][1], t.v[2][1]];
            let x0 = (xs.iter().copied().fold(f32::MAX, f32::min).max(0.0) as u32 / tw) as usize;
            let x1 = ((xs.iter().copied().fold(f32::MIN, f32::max).min(fw - 1.0).max(0.0)) as u32 / tw) as usize;
            let y0 = (ys.iter().copied().fold(f32::MAX, f32::min).max(0.0) as u32 / bh) as usize;
            let y1 = ((ys.iter().copied().fold(f32::MIN, f32::max).min(fh - 1.0).max(0.0)) as u32 / bh) as usize;
            for by in y0..=y1.min(bands - 1) {
                for bx in x0..=x1.min(tiles_x - 1) {
                    bins[by * tiles_x + bx].push(i as u32);
                }
            }
        }

        // Rasterise: each band owns its rows of the depth/id buffers (and of the image when
        // shading is inline).
        let quads = Quads { visible: &visible, bases: visible.iter().map(|v| v.2).collect(), scene: VoxelScene { world, chunks: &self.chunks }, cam, w, h, mode: req.mode };
        let sky = world.sky();
        let raster = Raster { tris: &tris, bins: &bins, tiles_x, tw, bh, w, quads: &quads, sky };
        let rows = (bh * w) as usize;
        let bands = self.depth.par_chunks_mut(rows).zip(self.ids.par_chunks_mut(rows)).zip(frame.pixels.par_chunks_mut(rows * 4)).enumerate();
        if self.deferred {
            bands.for_each(|(band, ((depth, ids), _))| raster.band::<false>(band, depth, ids, &mut []));
        } else {
            bands.for_each(|(band, ((depth, ids), pixels))| raster.band::<true>(band, depth, ids, pixels));
        }
        let t_raster = start.elapsed().as_secs_f64() * 1e3;

        // Deferred shading: the common pass over the quad ids.
        if self.deferred {
            mr_components::shading::deferred::shade(&QuadIds { ids: &self.ids, quads: &quads, sky }, w, &mut frame.pixels);
        }
        let total = start.elapsed().as_secs_f64() * 1e3;
        let n = &self.stage_nodes;
        let mut stages = vec![StageTime::cpu("geometry", &n[0], t_mesh as f32), StageTime::cpu("transform", &n[1], (t_geom - t_mesh) as f32)];
        if self.deferred {
            stages.extend([StageTime::cpu("raster", &n[2], (t_raster - t_geom) as f32), StageTime::cpu("shade", &n[3], (total - t_raster) as f32)]);
        } else {
            stages.push(StageTime::cpu("raster+shade", &n[4], (t_raster - t_geom) as f32));
        }
        let stats = FrameStats {
            cpu_ms: total as f32,
            gpu_ms: None,
            legend: None,
            counters: vec![
                ("chunks_drawn".into(), visible.len() as f64),
                ("triangles".into(), tris.len() as f64),
                ("remeshed".into(), remeshed as f64),
                ("mesh_ms".into(), t_mesh),
                ("geometry_ms".into(), t_geom - t_mesh),
                ("raster_ms".into(), t_raster - t_geom),
                ("shade_ms".into(), total - t_raster),
            ],
            stages,
        };
        stats
    }
}

/// The quads a frame draws, numbered for visibility ids (0 = nothing), and how a quad's
/// pixel is shaded: the quad id is cpu-raster's encoding of the hit record, decoded here
/// (the pixel's ray against the quad's plane, in f64) and shaded with the reference model.
struct Quads<'a> {
    visible: &'a [(IVec3, &'a Mesh, u32)],
    bases: Vec<u32>,
    scene: VoxelScene<'a>,
    cam: Camera,
    w: u32,
    h: u32,
    mode: RenderMode,
}

impl Quads<'_> {
    #[inline]
    fn shade(&self, id: u32, x: u32, y: u32) -> [u8; 4] {
        let k = self.bases.partition_point(|&b| b <= id) - 1;
        let (pos, m, base) = self.visible[k];
        let q = Quad::decode(m.quads[(id - base) as usize]);
        let (a, ua, va) = FACE_AXES[q.face as usize];
        let positive = q.face as usize % 2 == 0;
        let plane = (pos * 32 + q.pos)[a] as f64 + positive as i32 as f64;
        let d = self.cam.ray_dir(x, y, self.w, self.h).as_dvec3();
        let o = self.cam.position;
        let hit = if d[a].abs() > 1e-12 { o + d * ((plane - o[a]) / d[a]) } else { o };
        let mut local = Vec3::ZERO;
        local[a] = positive as i32 as f32;
        local[ua] = (hit[ua] - hit[ua].floor()) as f32;
        local[va] = (hit[va] - hit[va].floor()) as f32;
        self.scene.shade_surface(hit, local, q.face, q.block, self.mode)
    }
}

/// What every band of a frame's rasterisation reads.
struct Raster<'a> {
    tris: &'a [Tri],
    bins: &'a [Vec<u32>],
    tiles_x: usize,
    tw: u32,
    bh: u32,
    w: u32,
    quads: &'a Quads<'a>,
    sky: [u8; 4],
}

impl Raster<'_> {
    /// Rasterises band `band` into its rows of the depth and id buffers; with `INLINE`
    /// (`placement = inline`) every fragment that passes the depth test is shaded into
    /// `pixels` instead (forward shading: overdrawn fragments are shaded too).
    fn band<const INLINE: bool>(&self, band: usize, depth: &mut [f32], ids: &mut [u32], pixels: &mut [u8]) {
        let (w, tw, bh) = (self.w, self.tw, self.bh);
        depth.fill(0.0);
        if INLINE {
            for px in pixels.chunks_exact_mut(4) {
                px.copy_from_slice(&self.sky);
            }
        } else {
            ids.fill(0);
        }
        let y_base = band as u32 * bh;
        let band_rows = depth.len() / w as usize;
        for bx in 0..self.tiles_x {
            let tx0 = bx as u32 * tw;
            let tx1 = (tx0 + tw).min(w);
            for &ti in &self.bins[band * self.tiles_x + bx] {
                let t = &self.tris[ti as usize];
                let [a, b, c] = t.v;
                let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                if area.abs() < 1e-12 {
                    continue;
                }
                let inv_area = 1.0 / area;
                let minx = a[0].min(b[0]).min(c[0]).floor().max(tx0 as f32) as u32;
                let maxx = (a[0].max(b[0]).max(c[0]).ceil() as u32).min(tx1);
                let miny = (a[1].min(b[1]).min(c[1]).floor().max(y_base as f32) as u32).max(y_base);
                let maxy = (a[1].max(b[1]).max(c[1]).ceil() as u32).min(y_base + band_rows as u32);
                for y in miny..maxy {
                    let py = y as f32 + 0.5;
                    let row = (y - y_base) as usize * w as usize;
                    for x in minx..maxx {
                        let px = x as f32 + 0.5;
                        let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) * inv_area;
                        let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) * inv_area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                        let i = row + x as usize;
                        if z > depth[i] {
                            depth[i] = z;
                            if INLINE {
                                pixels[i * 4..i * 4 + 4].copy_from_slice(&self.quads.shade(t.id, x, y));
                            } else {
                                ids[i] = t.id;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// `placement = deferred`: the frame's quad ids, shaded by the common deferred pass.
struct QuadIds<'a> {
    ids: &'a [u32],
    quads: &'a Quads<'a>,
    sky: [u8; 4],
}

impl mr_components::shading::deferred::Records for QuadIds<'_> {
    type Record = u32;
    #[inline]
    fn record_at(&self, x: u32, y: u32) -> u32 {
        self.ids[(y * self.quads.w + x) as usize]
    }
    #[inline]
    fn hit(id: u32) -> bool {
        id != 0
    }
    #[inline]
    fn shade(&self, id: u32, x: u32, y: u32) -> [u8; 4] {
        self.quads.shade(id, x, y)
    }
    #[inline]
    fn miss(&self, _: u32, _: u32) -> [u8; 4] {
        self.sky
    }
}

impl Lane for CpuRaster {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        for p in self.chunks.apply(updates) {
            self.dirty.insert(p);
            for f in Face::ALL {
                let nb = p + f.normal();
                if self.meshes.contains_key(&nb) || self.chunks.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame, _: &Pool, world: &World, req: &FrameRequest) -> FrameStats {
        self.frame(frame, world, req)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "tile_width" => self.opts.tile_width = value.as_i64().unwrap_or(64) as u32,
            "band_height" => self.opts.band_height = value.as_i64().unwrap_or(16) as u32,
            "geometry" => match require(BACKEND, Slot::Geometry, value.as_str().unwrap_or("")) {
                Ok(g) => {
                    self.mesh = mesh_fn(&g);
                    self.dirty.extend(self.meshes.keys().copied());
                }
                Err(e) => return Some(Err(e)),
            },
            "cull" => {
                self.knobs.cull = value.as_bool().unwrap_or(true);
                self.dirty.extend(self.meshes.keys().copied());
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    fn resize(&mut self, frame: &Frame) {
        let n = (frame.width * frame.height) as usize;
        self.depth = vec![0.0; n];
        self.ids = vec![0; n];
    }

    fn memory(&self) -> u64 {
        let quads: usize = self.meshes.values().map(|m| m.quads.len()).sum();
        self.chunks.dense_bytes() + quads as u64 * 8 + (self.depth.len() + self.ids.len()) as u64 * 4
    }
}
