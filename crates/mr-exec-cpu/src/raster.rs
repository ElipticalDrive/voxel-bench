//! The CPU rasterisation pipeline (`cpu-raster`) as nodes of the generic executor
//! ([`crate::exec`]): a software rasteriser on the CPU, assembled from components
//! (`mr-components`): **geometry** (`greedy` or `naive` quads, meshed on the
//! CPU), **visibility** `faces` with **draw** `raster` (below), **shading** `direct` (here the reference
//! scene's f64 model, with CPU ray-traced shadows). Was the cpu-raster renderer.
//!
//! Shading stays on the reference's f64 model on purpose, not the f32 CPU shading
//! component (`direct::cpu::shade`): the same algorithm at a different precision. Measured
//! 2026-10-11, the f32 component lowered lit PSNR (terrain 63.1 -> 57.7 dB, room 99.0 ->
//! 76.6, many-lights 64.8 -> 61.0) at the same speed.
//!
//! Pipeline per frame (the passes: geometry 1, visibility 2-3, draw 4-5, shading 6):
//! 1. incremental parallel meshing by the geometry component,
//! 2. per-chunk frustum culling and per-face-direction culling,
//! 3. transform + homogeneous clipping of each quad, fan triangulation,
//! 4. binning into screen tiles grouped in horizontal bands,
//! 5. parallel edge-function rasterisation per band into a depth + visibility-ID buffer,
//! 6. deferred shading per pixel (the common pass, `shading::deferred`; the quad id is the
//!    record's encoding): the surface point is recomputed exactly by intersecting the
//!    pixel's ray with the quad's plane (f64), then shaded with the reference model (CPU
//!    ray-traced shadows in lit mode). With `placement = inline`, step 5
//!    shades every fragment that passes the depth test instead (forward shading).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use mr_reference::{Chunks, Shadows, VoxelScene};
use glam::{IVec3, Mat4, Vec3, Vec4};
use mr_core::*;
use mr_components::geometry::{Knobs, MeshFn};
use mr_components::graph::{Checked, Role};
use mr_components::{names_on, require, Slot};
use mr_mesh::{FACE_AXES, Mesh, Quad};
use rayon::prelude::*;
use serde::Deserialize;

use crate::exec::{self, Cx, Node, Pass, Report, Res};
use crate::{Pixel, Spec};

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
    let mut p = mr_reference::cpu_params();
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


/// The geometry's output: the chunks and their meshes.
struct Meshes {
    chunks: Chunks,
    meshes: HashMap<IVec3, Mesh>,
}

fn meshes(r: &Res) -> &Meshes {
    r.downcast_ref().expect("meshes")
}

impl Meshes {
    /// The meshes of `tris`'s visible chunks, with their first quad ids.
    fn visible(&self, tris: &Tris) -> Vec<(IVec3, &Mesh, u32)> {
        tris.visible.iter().map(|&(p, base)| (p, &self.meshes[&p], base)).collect()
    }
}

/// The geometry component: remeshes the changed chunks (in parallel, incrementally).
struct Geometry {
    dirty: HashSet<IVec3>,
    /// The geometry component's mesher and knobs.
    mesh: MeshFn,
    knobs: Knobs,
    nodes: Vec<String>,
}

impl Geometry {
    fn remesh(&mut self, m: &mut Meshes) -> usize {
        if self.dirty.is_empty() {
            return 0;
        }
        let dirty: Vec<IVec3> = self.dirty.drain().collect();
        let (scene, mesh, knobs) = (&m.chunks, self.mesh, self.knobs);
        let meshes: Vec<(IVec3, Option<Mesh>)> = dirty
            .par_iter()
            .map(|&p| (p, scene.chunk(p).map(|c| mesh(c, Face::ALL.map(|f| scene.chunk(p + f.normal())), 0, knobs))))
            .collect();
        for (p, mesh) in meshes {
            match mesh {
                Some(mesh) if !mesh.quads.is_empty() => {
                    m.meshes.insert(p, mesh);
                }
                _ => {
                    m.meshes.remove(&p);
                }
            }
        }
        dirty.len()
    }
}

impl Node for Geometry {
    fn output(&mut self, _: u32, _: u32) -> Res {
        Box::new(Meshes { chunks: Chunks::default(), meshes: HashMap::new() })
    }
    fn update(&mut self, out: &mut Res, updates: &[WorldUpdate]) -> anyhow::Result<()> {
        let m = out.downcast_mut::<Meshes>().expect("meshes");
        for p in m.chunks.apply(updates) {
            self.dirty.insert(p);
            for f in Face::ALL {
                let nb = p + f.normal();
                if m.meshes.contains_key(&nb) || m.chunks.chunk(nb).is_some() {
                    self.dirty.insert(nb);
                }
            }
        }
        Ok(())
    }
    fn run(&mut self, _: &Cx, _: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let m = out.downcast_mut::<Meshes>().expect("meshes");
        m.chunks.prepare();
        let remeshed = self.remesh(m);
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu("geometry", &self.nodes, t as f32));
        report.counters.extend([("remeshed".into(), remeshed as f64), ("mesh_ms".into(), t)]);
    }
    fn set_param(&mut self, out: &mut Res, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "geometry" => match require(BACKEND, Slot::Geometry, value.as_str().unwrap_or("")) {
                Ok(g) => self.mesh = mesh_fn(&g),
                Err(e) => return Some(Err(e)),
            },
            "cull" => self.knobs.cull = value.as_bool().unwrap_or(true),
            _ => return None,
        }
        self.dirty.extend(meshes(out).meshes.keys().copied());
        Some(Ok(()))
    }
    fn memory(&self, out: &Res) -> u64 {
        let m = meshes(out);
        let quads: usize = m.meshes.values().map(|m| m.quads.len()).sum();
        m.chunks.dense_bytes() + quads as u64 * 8
    }
}

/// The visibility's output: the visible chunks with a global quad numbering for visibility
/// ids (0 = empty), and their screen-space triangles.
struct Tris {
    visible: Vec<(IVec3, u32)>,
    tris: Vec<Tri>,
}

/// Visibility `faces`: per-chunk frustum culling and per-face-direction culling, transform,
/// homogeneous clipping, fan triangulation.
struct Faces {
    nodes: Vec<String>,
}

impl Node for Faces {
    fn output(&mut self, _: u32, _: u32) -> Res {
        Box::new(Tris { visible: vec![], tris: vec![] })
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let m = meshes(ins[0]);
        let (w, h) = (cx.width, cx.height);
        let cam = cx.req.camera;
        let (cam_block, frac) = cam.split_position();
        let vp = cam.projection(w as f32 / h as f32) * cam.view_rotation();
        let planes = frustum_planes(vp);

        let mut visible: Vec<(IVec3, &Mesh, u32)> = vec![];
        let mut next_id = 1u32;
        for (pos, m) in &m.meshes {
            let min = (*pos * 32 - cam_block).as_vec3() - frac;
            if aabb_visible(&planes, min, min + 32.0) {
                visible.push((*pos, m, next_id));
                next_id += m.quads.len() as u32;
            }
        }
        visible.sort_by_key(|v| v.2);

        // Transform, clip, triangulate (parallel per chunk), into last frame's triangles'
        // memory.
        let (fw, fh) = (w as f32, h as f32);
        let out = out.downcast_mut::<Tris>().expect("triangles");
        let tris = &mut out.tris;
        tris.clear();
        tris.par_extend(visible
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
            }));
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu("transform", &self.nodes, t as f32));
        report.counters.extend([("chunks_drawn".into(), visible.len() as f64), ("triangles".into(), tris.len() as f64), ("geometry_ms".into(), t)]);
        out.visible.clear();
        out.visible.extend(visible.iter().map(|&(p, _, base)| (p, base)));
    }
}

/// Draw `raster`: bins the triangles into (band, tile) cells, rasterises each band in
/// parallel into the depth buffer and quad ids (its output); `fused` (`placement = inline`):
/// shading compiled in, each fragment passing the depth test shaded into the image (its
/// output).
struct Draw<P> {
    fused: bool,
    tile_width: u32,
    band_height: u32,
    depth: Vec<f32>,
    nodes: Vec<String>,
    px: std::marker::PhantomData<P>,
}

impl<P: Pixel> Node for Draw<P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        self.depth = vec![0.0; (w * h) as usize];
        if self.fused { exec::image::<P>(w, h) } else { Box::new(vec![0u32; (w * h) as usize]) }
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let (t, m) = (ins[0].downcast_ref::<Tris>().expect("triangles"), meshes(ins[1]));
        let (w, h) = (cx.width, cx.height);
        let (fw, fh) = (w as f32, h as f32);
        let tris = &t.tris;

        // Bin triangles into (band, tile) cells.
        let (tw, bh) = (self.tile_width.max(8), self.band_height.max(1));
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

        // Rasterise: each band owns its rows of the depth buffer and of the output.
        let visible = m.visible(t);
        let quads = Quads::new(&visible, &m.chunks, cx);
        let sky: P = cx.world.sky();
        let raster = Raster { tris, bins: &bins, tiles_x, tw, bh, w, quads: &quads, sky };
        let rows = (bh * w) as usize;
        if self.fused {
            let pixels = out.downcast_mut::<Vec<P>>().expect("an image");
            self.depth.par_chunks_mut(rows).zip(pixels.par_chunks_mut(rows)).enumerate().for_each(|(band, (depth, pixels))| raster.band::<true>(band, depth, &mut [], pixels));
        } else {
            let ids = out.downcast_mut::<Vec<u32>>().expect("quad ids");
            self.depth.par_chunks_mut(rows).zip(ids.par_chunks_mut(rows)).enumerate().for_each(|(band, (depth, ids))| raster.band::<false>(band, depth, ids, &mut []));
        }
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu(if self.fused { "raster+shade" } else { "raster" }, &self.nodes, t as f32));
        report.counters.push(("raster_ms".into(), t));
    }
    fn set_param(&mut self, _: &mut Res, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
            "tile_width" => self.tile_width = value.as_i64().unwrap_or(64) as u32,
            "band_height" => self.band_height = value.as_i64().unwrap_or(16) as u32,
            _ => return None,
        }
        Some(Ok(()))
    }
    fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
        *out = self.output(w, h);
    }
    fn memory(&self, out: &Res) -> u64 {
        (std::mem::size_of_val(&self.depth[..]) + out.downcast_ref::<Vec<u32>>().map_or(0, |ids| std::mem::size_of_val(&ids[..]))) as u64
    }
}

/// The common deferred pass over the quad ids.
struct Shading<P> {
    nodes: Vec<String>,
    px: std::marker::PhantomData<P>,
}

impl<P: Pixel> Node for Shading<P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        exec::image::<P>(w, h)
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let ids = ins[0].downcast_ref::<Vec<u32>>().expect("quad ids");
        let (t, m) = (ins[1].downcast_ref::<Tris>().expect("triangles"), meshes(ins[2]));
        let visible = m.visible(t);
        let quads = Quads::new(&visible, &m.chunks, cx);
        mr_components::shading::deferred::shade(&QuadIds { ids, quads: &quads, sky: cx.world.sky() }, cx.width, out.downcast_mut::<Vec<P>>().expect("an image"));
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu("shade", &self.nodes, t as f32));
        report.counters.push(("shade_ms".into(), t));
    }
    fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
        *out = exec::image::<P>(w, h);
    }
}

/// The executor for graph `g`: one pass per scheduled geometry, visibility, draw and shading
/// node; `S` what a compiled pipeline baked in.
pub(crate) fn executor<S: Spec, P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let opts: Options = p.options()?;
    require(BACKEND, Slot::Visibility, &opts.visibility)?;
    require(BACKEND, Slot::Draw, &opts.draw)?;
    require(BACKEND, Slot::Shading, &opts.shading)?;
    anyhow::ensure!(opts.merge == "block", "merge = `{}`: cpu-raster has no per-pixel block lookup (only `block`)", opts.merge);
    let mesh = mesh_fn(&require(BACKEND, Slot::Geometry, &opts.geometry)?);
    let deferred = mr_components::parse_placement(&opts.placement)?;
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    let (mut passes, mut index): (Vec<Pass>, HashMap<String, usize>) = (vec![], HashMap::new());
    let (mut geometry, mut faces) = (None, None);
    for pass in schedule.passes() {
        let id = pass.node.as_str();
        let mut ins = vec![];
        let node: Box<dyn Node> = match g.node(id).role {
            Role::Geometry => {
                geometry = Some(passes.len());
                Box::new(Geometry { dirty: HashSet::new(), mesh, knobs: Knobs { cull: opts.cull, ..Default::default() }, nodes: vec![id.to_string()] })
            }
            Role::Visibility => {
                faces = Some(passes.len());
                Box::new(Faces { nodes: vec![id.to_string()] })
            }
            // Drawing and shading recompute the surface from the quad (its mesh), which the
            // graph leaves implicit (only the triangles or ids are inputs): read the
            // geometry's pass, and the visibility's for the shading, too.
            Role::Draw => {
                exec::reads(g, id, &index, &mut ins);
                ins.extend(geometry);
                let nodes = if deferred { vec![id.to_string()] } else { g.ids(&[Role::Draw, Role::Shading]) };
                Box::new(Draw::<P> { fused: !deferred, tile_width: opts.tile_width, band_height: opts.band_height, depth: vec![], nodes, px: std::marker::PhantomData })
            }
            // Fused into the draw pass.
            Role::Shading if !deferred => continue,
            Role::Shading => {
                exec::reads(g, id, &index, &mut ins);
                ins.extend(faces);
                ins.extend(geometry);
                Box::new(Shading::<P> { nodes: vec![id.to_string()], px: std::marker::PhantomData })
            }
            _ => continue,
        };
        if ins.is_empty() {
            exec::reads(g, id, &index, &mut ins);
        }
        index.insert(id.to_string(), passes.len());
        passes.push(Pass { node, ins });
    }
    let world = crate::world::<P>(p, g, opts.shadows)?;
    Ok(Box::new(exec::Executor::<P>::new(p, opts.threads, world, passes)?.baked(S::KNOBS, S::NATIVE)))
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

impl<'a> Quads<'a> {
    fn new(visible: &'a [(IVec3, &'a Mesh, u32)], chunks: &'a Chunks, cx: &Cx<'a>) -> Self {
        let bases = visible.iter().map(|v| v.2).collect();
        Quads { visible, bases, scene: VoxelScene { world: cx.world, chunks }, cam: cx.req.camera, w: cx.width, h: cx.height, mode: cx.req.mode }
    }

    #[inline]
    fn shade<P: Pixel>(&self, id: u32, x: u32, y: u32) -> P {
        let k = self.bases.partition_point(|&b| b <= id) - 1;
        let (pos, m, base) = self.visible[k];
        let q = Quad::decode(m.quads[(id - base) as usize]);
        let (a, ua, va) = FACE_AXES[q.face as usize];
        let positive = (q.face as usize).is_multiple_of(2);
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
struct Raster<'a, P> {
    tris: &'a [Tri],
    bins: &'a [Vec<u32>],
    tiles_x: usize,
    tw: u32,
    bh: u32,
    w: u32,
    quads: &'a Quads<'a>,
    sky: P,
}

impl<P: Pixel> Raster<'_, P> {
    /// Rasterises band `band` into its rows of the depth and id buffers; with `INLINE`
    /// (`placement = inline`) every fragment that passes the depth test is shaded into
    /// `pixels` instead (forward shading: overdrawn fragments are shaded too).
    fn band<const INLINE: bool>(&self, band: usize, depth: &mut [f32], ids: &mut [u32], pixels: &mut [P]) {
        let (w, tw, bh) = (self.w, self.tw, self.bh);
        depth.fill(0.0);
        if INLINE {
            pixels.fill(self.sky);
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
                                pixels[i] = self.quads.shade(t.id, x, y);
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
struct QuadIds<'a, P> {
    ids: &'a [u32],
    quads: &'a Quads<'a>,
    sky: P,
}

impl<P: Pixel> mr_components::shading::deferred::Records for QuadIds<'_, P> {
    type Record = u32;
    type Px = P;
    #[inline]
    fn record_at(&self, x: u32, y: u32) -> u32 {
        self.ids[(y * self.quads.w + x) as usize]
    }
    #[inline]
    fn hit(id: u32) -> bool {
        id != 0
    }
    #[inline]
    fn shade(&self, id: u32, x: u32, y: u32) -> P {
        self.quads.shade(id, x, y)
    }
    #[inline]
    fn miss(&self, _: u32, _: u32) -> P {
        self.sky
    }
}

