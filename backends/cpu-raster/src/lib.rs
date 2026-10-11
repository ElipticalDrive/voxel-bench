//! CPU-raster back end: a software rasteriser on the CPU, assembled from components
//! (`mr-components`, IDEAS §12): **geometry** (`greedy` or `naive` quads, meshed on the
//! CPU), **visibility** `raster` (below), **shading** `direct` (here the reference
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
//! 6. deferred shading per pixel: the surface point is recomputed exactly by intersecting
//!    the pixel's ray with the quad's plane (f64), then shaded with the reference model
//!    (CPU ray-traced shadows in lit mode).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use cpu_reference::{Pool, Shadows, VoxelScene};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use mr_components::geometry::{Knobs, MeshFn};
use mr_components::{names_on, require, Slot};
use mr_mesh::{FACE_AXES, Mesh, Quad};
use rayon::prelude::*;
use serde::Deserialize;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "cpu-raster".into(),
            description: "CPU-raster back end: geometry component's quads, homogeneous clipping, tiled multithreaded edge-function raster into a visibility buffer, deferred exact shading".into(),
            tags: vec!["cpu".into(), "raster".into(), "software".into(), "visibility-buffer".into(), "modular".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: mr_components::lighting_knobs(BACKEND, params()),
        },
        create: |p, _| Ok(Box::new(CpuRaster::new(p)?)),
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
    shading: String,
    merge: String,
    cull: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { tile_width: 64, band_height: 16, threads: 0, shadows: Shadows::All, geometry: "greedy".into(), visibility: "raster".into(), shading: "direct".into(), merge: "block".into(), cull: true }
    }
}

const BACKEND: &str = "cpu-raster";



fn params() -> Vec<ParamSpec> {
    let mut p = cpu_reference::cpu_params();
    p.push(ParamSpec::choice("geometry", "Geometry component (quads meshed on the CPU)", &names_on(BACKEND, Slot::Geometry), "greedy", true));
    p.push(ParamSpec::choice("visibility", "Visibility component", &names_on(BACKEND, Slot::Visibility), "raster", false));
    p.push(ParamSpec::choice("shading", "Shading component", &names_on(BACKEND, Slot::Shading), "direct", false));
    p.push(ParamSpec::bool("cull", "naive: skip faces hidden by solid neighbours", true, true).of("geometry", &["naive"]));
    p.push(ParamSpec::int("tile_width", "Raster tile width (pixels)", 8, 1024, 64, true).of("visibility", &["raster"]));
    p.push(ParamSpec::int("band_height", "Raster band height (pixels); bands are the unit of parallelism", 1, 512, 16, true).of("visibility", &["raster"]));
    p
}

/// Screen-space triangle: (x, y, depth) per vertex plus visibility id.
#[derive(Copy, Clone)]
struct Tri {
    v: [[f32; 3]; 3],
    id: u32,
}

pub struct CpuRaster {
    width: u32,
    height: u32,
    opts: Options,
    scene: VoxelScene,
    pool: Pool,
    meshes: HashMap<IVec3, Mesh>,
    dirty: HashSet<IVec3>,
    /// The geometry component's mesher and knobs.
    mesh: MeshFn,
    knobs: Knobs,
    depth: Vec<f32>,
    ids: Vec<u32>,
    pixels: Vec<u8>,
}

impl CpuRaster {
    pub fn new(p: InitParams) -> anyhow::Result<Self> {
        let n = (p.width * p.height) as usize;
        let opts: Options = p.options()?;
        let mut scene = VoxelScene::new(&p);
        scene.shadows = opts.shadows;
        require(BACKEND, Slot::Visibility, &opts.visibility)?;
        require(BACKEND, Slot::Shading, &opts.shading)?;
        anyhow::ensure!(opts.merge == "block", "merge = `{}`: cpu-raster has no per-pixel block lookup (only `block`)", opts.merge);
        let mesh = mesh_fn(&require(BACKEND, Slot::Geometry, &opts.geometry)?);
        let knobs = Knobs { cull: opts.cull, ..Default::default() };
        Ok(Self {
            width: p.width,
            height: p.height,
            pool: Pool::new(opts.threads)?,
            opts,
            scene,
            meshes: HashMap::new(),
            dirty: HashSet::new(),
            mesh,
            knobs,
            depth: vec![0.0; n],
            ids: vec![0; n],
            pixels: vec![0; n * 4],
        })
    }

    fn remesh(&mut self) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let dirty: Vec<IVec3> = self.dirty.drain().collect();
        let (scene, mesh, knobs) = (&self.scene, self.mesh, self.knobs);
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
    fn render_inner(&mut self, req: &FrameRequest) -> FrameStats {
        let start = Instant::now();

        self.scene.prepare();
        let remeshed = self.remesh();
        let t_mesh = start.elapsed().as_secs_f64() * 1e3;
        let (w, h) = (self.width, self.height);
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

        // Rasterise: each band owns its rows of the depth/id buffers.
        let rows = (bh * w) as usize;
        self.depth.par_chunks_mut(rows).zip(self.ids.par_chunks_mut(rows)).enumerate().for_each(|(band, (depth, ids))| {
            depth.fill(0.0);
            ids.fill(0);
            let y_base = band as u32 * bh;
            let band_rows = depth.len() / w as usize;
            for bx in 0..tiles_x {
                let tx0 = bx as u32 * tw;
                let tx1 = (tx0 + tw).min(w);
                for &ti in &bins[band * tiles_x + bx] {
                    let t = &tris[ti as usize];
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
                                ids[i] = t.id;
                            }
                        }
                    }
                }
            }
        });
        let t_raster = start.elapsed().as_secs_f64() * 1e3;

        // Deferred shading.
        let sky = encode_srgb(self.scene.env.sky);
        let bases: Vec<u32> = visible.iter().map(|v| v.2).collect();
        let scene = &self.scene;
        let ids = &self.ids;
        self.pixels.par_chunks_mut((w * 4) as usize).enumerate().for_each(|(y, row)| {
            for x in 0..w as usize {
                let id = ids[y * w as usize + x];
                let px = if id == 0 {
                    sky
                } else {
                    let k = bases.partition_point(|&b| b <= id) - 1;
                    let (pos, m, base) = visible[k];
                    let q = Quad::decode(m.quads[(id - base) as usize]);
                    let (a, ua, va) = FACE_AXES[q.face as usize];
                    let positive = q.face as usize % 2 == 0;
                    let plane = (pos * 32 + q.pos)[a] as f64 + positive as i32 as f64;
                    let d = cam.ray_dir(x as u32, y as u32, w, h).as_dvec3();
                    let o = cam.position;
                    let hit = if d[a].abs() > 1e-12 { o + d * ((plane - o[a]) / d[a]) } else { o };
                    let mut local = Vec3::ZERO;
                    local[a] = positive as i32 as f32;
                    local[ua] = (hit[ua] - hit[ua].floor()) as f32;
                    local[va] = (hit[va] - hit[va].floor()) as f32;
                    scene.shade_surface(hit, local, q.face, q.block, req.mode)
                };
                row[x * 4..x * 4 + 4].copy_from_slice(&px);
            }
        });
        let total = start.elapsed().as_secs_f64() * 1e3;
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
        };
        stats
    }
}

impl Renderer for CpuRaster {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        for p in self.scene.apply(updates) {
            self.dirty.insert(p);
            for f in Face::ALL {
                let nb = p + f.normal();
                if self.meshes.contains_key(&nb) || self.scene.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if self.pool.set_common(&mut self.scene, name, value)? {
            return Ok(());
        }
        match name {
            "tile_width" => self.opts.tile_width = value.as_i64().unwrap_or(64) as u32,
            "band_height" => self.opts.band_height = value.as_i64().unwrap_or(16) as u32,
            "geometry" => {
                self.mesh = mesh_fn(&require(BACKEND, Slot::Geometry, value.as_str().unwrap_or(""))?);
                self.dirty.extend(self.meshes.keys().copied());
            }
            "cull" => {
                self.knobs.cull = value.as_bool().unwrap_or(true);
                self.dirty.extend(self.meshes.keys().copied());
            }
            _ => anyhow::bail!("unknown parameter `{name}`"),
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let pool = std::mem::replace(&mut self.pool, Pool::new(1)?);
        let stats = pool.install(|| self.render_inner(req));
        self.pool = pool;
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&self.pixels) });
        Ok(())
    }

    fn flush(&mut self, _: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        let n = (width * height) as usize;
        self.width = width;
        self.height = height;
        self.depth = vec![0.0; n];
        self.ids = vec![0; n];
        self.pixels = vec![0; n * 4];
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let quads: usize = self.meshes.values().map(|m| m.quads.len()).sum();
        MemoryReport { cpu_bytes: self.scene.dense_bytes() + quads as u64 * 8 + self.pixels.len() as u64 * 3, gpu_bytes: 0 }
    }
}
