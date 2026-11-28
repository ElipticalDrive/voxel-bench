//! The CPU rasterisation pipeline (`raster-cpu`) as nodes of the generic executor
//! ([`crate::exec`]): a software rasteriser on the CPU, assembled from components
//! (`mr-components`): **geometry** (`greedy` or `naive` quads, meshed on the
//! CPU), **visibility** `faces` with **draw** `raster` (below), **shading** `direct` (here the reference
//! scene's f64 model), its shadow rays through the graph's CPU **storage** and **traversal**
//! kernel (the ray caster's, `crate::raycast::walked`). Was the raster-cpu renderer.
//!
//! Shading stays on the reference's f64 model on purpose, not the f32 CPU shading
//! component (`direct::cpu::shade`): the same algorithm at a different precision. Measured
//! 2026-10-11, the f32 component lowered lit PSNR (terrain 63.1 -> 57.7 dB, room 99.0 ->
//! 76.6, many-lights 64.8 -> 61.0) at the same speed.
//!
//! Pipeline per frame (the passes: geometry 1, visibility 2-3, draw 4-5, shading 6):
//! 0. the shadow rays' storage brought up to date (first, so an inline draw sees it),
//! 1. incremental parallel meshing by the geometry component,
//! 2. per-chunk frustum culling and per-face-direction culling,
//! 3. transform + homogeneous clipping of each quad, fan triangulation,
//! 4. binning into screen tiles grouped in horizontal bands,
//! 5. parallel edge-function rasterisation per band into a depth + visibility-ID buffer,
//! 6. deferred shading per pixel (the common pass, `shading::deferred`; the quad id is the
//!    record's encoding): the surface point is recomputed exactly by intersecting the
//!    pixel's ray with the quad's plane (f64), then shaded with the reference model (shadow
//!    rays walking the storage in lit mode). With `placement = inline`, step 5
//!    shades every fragment that passes the depth test instead (forward shading).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use mr_reference::{Chunks, Shadows, World};
use glam::{DVec3, IVec3, Mat4, Vec3, Vec4};
use mr_core::*;
use mr_components::geometry::{Knobs, MeshFn};
use mr_components::graph::{Checked, Role};
use mr_components::visibility::discover::{self, grid};
use mr_mesh::{FACE_AXES, Mesh, Quad};
use rayon::prelude::*;
use serde::Deserialize;

use crate::exec::{self, Cx, Node, Pass, Report, Res};
use crate::{Pixel, Spec};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu".into(),
        description: "CPU software rasteriser: the geometry component's quads, homogeneous clipping, tiled multithreaded edge-function raster into a visibility buffer, deferred exact shading (reference model, f64)".into(),
        tags: vec!["cpu".into(), "raster".into(), "software".into(), "visibility-buffer".into(), "modular".into()],
        features: Features::of(&[Feature::Lit, Feature::Volume]),
        params: REGION.lighting_knobs(params()),
    }
}

/// The region's own knobs (the nodes' are read from the graph).
#[derive(Deserialize, Default)]
#[serde(default)]
struct Options {
    threads: usize,
}

/// A geometry node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct GeometryKnobs {
    cull: bool,
}

impl Default for GeometryKnobs {
    fn default() -> Self {
        Self { cull: true }
    }
}

/// The draw node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct DrawKnobs {
    /// Tile width in pixels.
    tile_width: u32,
    /// Band height in pixels (bands are the unit of parallelism).
    band_height: u32,
}

impl Default for DrawKnobs {
    fn default() -> Self {
        Self { tile_width: 64, band_height: 16 }
    }
}

/// The shading node's knobs.
#[derive(Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    placement: String,
    shadows: Shadows,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { placement: "deferred".into(), shadows: Shadows::All }
    }
}

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::CPU_RASTER;


fn params() -> Vec<ParamSpec> {
    let mut p = mr_reference::cpu_params();
    p.push(REGION.placement_param("deferred"));
    p.push(ParamSpec::bool("exact", "incremental: recompute exit times from the integer cell (no float drift) instead of accumulating them", true, true).of("traversal", &["incremental"]));
    for (k, size) in [(1, 2), (2, 4), (3, 8), (4, 16)] {
        p.push(ParamSpec::bool(&format!("skip{size}"), &format!("incremental: use {size}^3 cubes (level {k}) if the storage has them"), true, true).of("traversal", &["incremental"]));
    }
    p.push(ParamSpec::bool("cull", "naive: skip faces hidden by solid neighbours; cubes: skip blocks hidden on every side", true, true).of("geometry", &["naive", "cubes"]));
    p.push(ParamSpec::int("tile_width", "Raster tile width (pixels)", 8, 1024, 64, true).of("draw", &["raster"]));
    p.push(ParamSpec::int("band_height", "Raster band height (pixels); bands are the unit of parallelism", 1, 512, 16, true).of("draw", &["raster"]));
    p.extend(mr_components::visibility::discover::params());
    // A compiled build of this region folds none of them (it shades with the reference model,
    // shadows and tone read at run time).
    p.into_iter().map(|q| q.baked(false)).collect()
}

/// Screen-space triangle: (x, y, depth) per vertex plus visibility id.
#[derive(Copy, Clone)]
struct Tri {
    v: [[f32; 3]; 3],
    id: u32,
}


/// The geometry's output: the chunks and their meshes (shared with the frame's visible set
/// downstream; a remeshed chunk gets a new one).
struct Meshes {
    chunks: Chunks,
    meshes: HashMap<IVec3, Arc<Mesh>>,
}

fn meshes(r: &Res) -> &Meshes {
    r.downcast_ref().expect("meshes")
}

/// A visible chunk: position, mesh, first quad id.
type Visible = (IVec3, Arc<Mesh>, u32);

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
                    m.meshes.insert(p, Arc::new(mesh));
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
    fn viewed(&self) -> bool {
        false
    }
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
        let remeshed = self.remesh(m);
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu("geometry", &self.nodes, t as f32));
        report.counters.extend([("remeshed".into(), remeshed as f64), ("mesh_ms".into(), t)]);
    }
    /// The mesher replaced: every chunk is meshed again.
    fn switch(&mut self, out: &mut Res, s: &mr_components::graph::Switch) -> Option<anyhow::Result<()>> {
        let c = s.component.as_ref().filter(|_| s.role == Role::Geometry)?;
        self.mesh = mesh_fn(c);
        self.dirty.extend(meshes(out).meshes.keys().copied());
        Some(Ok(()))
    }
    fn set_param(&mut self, out: &mut Res, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        match name {
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
    fn gate(&self, chain: mr_core::gate::GateChain, late: bool, w: u32, h: u32) -> Option<Box<dyn Node>> {
        Some(exec::GatePass::<Meshes>::boxed(chain, late, w, h))
    }
}

/// The visibility's output: the visible chunks' meshes with a global quad numbering for
/// visibility ids (0 = empty), and their screen-space triangles.
#[derive(Clone)]
struct Tris {
    visible: Vec<Visible>,
    tris: Vec<Tri>,
    /// The frame the triangles are on the screen of.
    size: (u32, u32),
}

/// On the wire: the frame's size, the visible set ([`put_visible`]), then the triangles (a
/// count; each its three (x, y, depth) and its quad id, which must name a visible quad).
impl exec::Crossing for Tris {
    fn sent(&self) -> Self {
        self.clone()
    }
    fn encode(&self, out: &mut Vec<u8>) {
        let mut p = mr_core::gate::Put(out);
        p.u32(self.size.0);
        p.u32(self.size.1);
        put_visible(&mut p, &self.visible);
        p.u32(self.tris.len() as u32);
        for t in &self.tris {
            t.v.iter().flatten().for_each(|&c| p.f32(c));
            p.u32(t.id);
        }
    }
    fn decode(bytes: &[u8]) -> Result<Self, mr_core::gate::WireError> {
        let mut g = mr_core::gate::Get(bytes);
        let size = (g.u32()?, g.u32()?);
        let (visible, next) = get_visible(&mut g)?;
        let n = g.count(40)?;
        let mut tris = Vec::with_capacity(n);
        for _ in 0..n {
            let mut v = [[0f32; 3]; 3];
            for c in v.iter_mut().flatten() {
                *c = g.f32()?;
            }
            let id = g.u32()?;
            if id == 0 || id >= next {
                return Err(mr_core::gate::WireError("a triangle names no visible quad".into()));
            }
            tris.push(Tri { v, id });
        }
        g.end()?;
        Ok(Tris { visible, tris, size })
    }
    fn blank(w: u32, h: u32) -> Self {
        Tris { visible: vec![], tris: vec![], size: (w, h) }
    }
    fn fits(&self, w: u32, h: u32) -> bool {
        self.size == (w, h)
    }
}

/// On the wire: the meshes of the world (a count; each chunk's position, per-face counts
/// and quads), not the chunks the geometry remeshes from: the visibility reads only the
/// meshes. They are the same from every camera.
impl exec::Crossing for Meshes {
    const VIEWED: bool = false;
    fn sent(&self) -> Self {
        Meshes { chunks: Chunks::default(), meshes: self.meshes.clone() }
    }
    fn encode(&self, out: &mut Vec<u8>) {
        let mut p = mr_core::gate::Put(out);
        p.u32(self.meshes.len() as u32);
        for (pos, m) in &self.meshes {
            pos.to_array().into_iter().for_each(|c| p.i32(c));
            m.counts.iter().for_each(|&c| p.u32(c));
            p.u32(m.quads.len() as u32);
            m.quads.iter().flatten().for_each(|&q| p.u32(q));
        }
    }
    fn decode(bytes: &[u8]) -> Result<Self, mr_core::gate::WireError> {
        let mut g = mr_core::gate::Get(bytes);
        let n = g.count(4 * 10)?;
        let mut meshes = HashMap::with_capacity(n);
        for _ in 0..n {
            let pos = IVec3::new(g.i32()?, g.i32()?, g.i32()?);
            let mut counts = [0u32; 6];
            for c in &mut counts {
                *c = g.u32()?;
            }
            let quads = g.count(8)?;
            let quads: Vec<[u32; 2]> = (0..quads).map(|_| Ok([g.u32()?, g.u32()?])).collect::<Result<_, mr_core::gate::WireError>>()?;
            if counts.iter().map(|&c| c as u64).sum::<u64>() != quads.len() as u64 {
                return Err(mr_core::gate::WireError("a mesh's face counts do not add up to its quads".into()));
            }
            meshes.insert(pos, Arc::new(Mesh { quads, counts }));
        }
        g.end()?;
        Ok(Meshes { chunks: Chunks::default(), meshes })
    }
    fn blank(_: u32, _: u32) -> Self {
        Meshes { chunks: Chunks::default(), meshes: HashMap::new() }
    }
    fn fits(&self, _: u32, _: u32) -> bool {
        true
    }
}

/// Visibility `faces`: per-chunk frustum culling and per-face-direction culling, transform,
/// homogeneous clipping, fan triangulation. It reads the geometry's meshes, or another
/// culling's visible set (culled again for this frame's camera); a drawer reading the
/// geometry straight runs one of its own (`stage`: its name in the reports).
struct Faces {
    nodes: Vec<String>,
    stage: &'static str,
}

impl Node for Faces {
    fn output(&mut self, _: u32, _: u32) -> Res {
        Box::new(Tris { visible: vec![], tris: vec![], size: (0, 0) })
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        match ins[0].downcast_ref::<Tris>() {
            Some(t) => self.cull(cx, t.visible.iter().map(|(p, m, _)| (p, m)), out, report),
            None => self.cull(cx, meshes(ins[0]).meshes.iter(), out, report),
        }
    }
    fn gate(&self, chain: mr_core::gate::GateChain, late: bool, w: u32, h: u32) -> Option<Box<dyn Node>> {
        Some(exec::GatePass::<Tris>::boxed(chain, late, w, h))
    }
}

impl Faces {
    fn cull<'m>(&mut self, cx: &Cx, chunks: impl Iterator<Item = (&'m IVec3, &'m Arc<Mesh>)>, out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let (w, h) = (cx.width, cx.height);
        let cam = cx.req.camera;
        let (cam_block, frac) = cam.split_position();
        let vp = cam.projection(w as f32 / h as f32) * cam.view_rotation();
        let planes = frustum_planes(vp);

        let out = out.downcast_mut::<Tris>().expect("triangles");
        out.size = (w, h);
        let visible = &mut out.visible;
        visible.clear();
        let mut next_id = 1u32;
        for (pos, m) in chunks {
            let min = (*pos * 32 - cam_block).as_vec3() - frac;
            if aabb_visible(&planes, min, min + 32.0) {
                visible.push((*pos, m.clone(), next_id));
                next_id += m.quads.len() as u32;
            }
        }
        visible.sort_by_key(|v| v.2);

        // Transform, clip, triangulate (parallel per chunk), into last frame's triangles'
        // memory.
        let (fw, fh) = (w as f32, h as f32);
        let tris = &mut out.tris;
        tris.clear();
        tris.par_extend(visible
            .par_iter()
            .flat_map_iter(|(pos, m, base)| {
                let (pos, base) = (*pos, *base);
                let min = (pos * 32 - cam_block).as_vec3() - frac;
                let mut out = vec![];
                let mut poly = Vec::with_capacity(12);
                let mut scratch = Vec::with_capacity(12);
                let mut first = 0usize;
                for (f, &n) in m.counts.iter().enumerate() {
                    let axis = f / 2;
                    let facing = if f % 2 == 0 { 0.0 > min[axis] } else { 0.0 < min[axis] + 32.0 };
                    if facing {
                        for (k, &packed) in m.quads[first..first + n as usize].iter().enumerate() {
                            let qi = first + k;
                            let q = Quad::decode(packed);
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
        report.stages.push(StageTime::cpu(self.stage, &self.nodes, t as f32));
        report.counters.extend([("chunks_drawn".into(), visible.len() as f64), ("triangles".into(), tris.len() as f64), ("geometry_ms".into(), t)]);
    }
}

/// CPU discovery (visibility `quadtree` or `cluster`): the face set its rays found from this
/// frame's camera, as each chunk's unit quads ([`Meshes`], drawn as a geometry's: transformed
/// and culled where they are read). Its rays walk its own copy of the world in their storage,
/// its exposure tests the exposure port's (`grid::CpuWorld`, as discover-cpu's); the face set persists across
/// frames as there (faces not re-confirmed for `KEEP_FRAMES` passes expire).
struct Discover {
    cpu: grid::CpuWorld,
    visibility: String,
    knobs: discover::Knobs,
    state: Option<discover::State>,
    /// The drawn records as of the last pass.
    records: Vec<[u32; 2]>,
    nodes: Vec<String>,
}

/// Discovery's own choices where discover-cpu has region knobs: all-miss tiles up to this
/// size are sky, a face lives this many passes unconfirmed, the discovery FOV unwidened.
const SKY_TILE: u32 = 8;
const KEEP_FRAMES: u32 = 8;

impl Node for Discover {
    /// World-space faces: a copy kept a frame late is drawn from this frame's camera.
    fn viewed(&self) -> bool {
        false
    }
    fn output(&mut self, _: u32, _: u32) -> Res {
        Box::new(Meshes { chunks: Chunks::default(), meshes: HashMap::new() })
    }
    fn update(&mut self, _: &mut Res, updates: &[WorldUpdate]) -> anyhow::Result<()> {
        self.cpu.apply(updates);
        Ok(())
    }
    fn run(&mut self, cx: &Cx, _: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let grid = self.cpu.snapshot();
        let settings = self.knobs.settings(&self.visibility, SKY_TILE, KEEP_FRAMES, 0.0, false);
        let o = grid.discover(self.state.take().unwrap_or_default(), &cx.req.camera, cx.width, cx.height, &settings);
        o.records.apply(&mut self.records);
        // The records as each chunk's unit quads, grouped per face.
        let mut faces: HashMap<IVec3, [Vec<[u32; 2]>; 6]> = HashMap::new();
        for &r in &self.records {
            let (cell, face, block) = discover::unpack_record(r, o.records.anchor);
            let (chunk, l) = (cell.div_euclid(IVec3::splat(32)), cell.rem_euclid(IVec3::splat(32)));
            faces.entry(chunk).or_default()[face as usize].push([l.x as u32 | (l.y as u32) << 5 | (l.z as u32) << 10 | face << 25, block as u32]);
        }
        let m = out.downcast_mut::<Meshes>().expect("meshes");
        m.meshes = faces.into_iter().map(|(p, f)| (p, Arc::new(Mesh { counts: f.each_ref().map(|v| v.len() as u32), quads: f.concat() }))).collect();
        let faces_s = o.state.s.len();
        self.state = Some(o.state);
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu("discover", &self.nodes, t as f32));
        let pixels = cx.width as f64 * cx.height as f64;
        report.counters.extend([
            ("rays".into(), o.rays as f64),
            ("rays_per_pixel".into(), o.rays as f64 / pixels),
            ("faces_s".into(), faces_s as f64),
            ("faces_drawn".into(), self.records.len() as f64),
            ("discovery_ms".into(), t),
        ]);
        report.counters.extend(o.extra);
    }
    fn set_param(&mut self, _: &mut Res, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        self.knobs.set(name, value).map(|_| Ok(()))
    }
    fn memory(&self, out: &Res) -> u64 {
        let quads: usize = meshes(out).meshes.values().map(|m| m.quads.len()).sum();
        self.cpu.memory() + self.state.as_ref().map_or(0, |s| s.s.len() as u64 * 16) + (self.records.len() + quads) as u64 * 8
    }
    fn gate(&self, chain: mr_core::gate::GateChain, late: bool, w: u32, h: u32) -> Option<Box<dyn Node>> {
        Some(exec::GatePass::<Meshes>::boxed(chain, late, w, h))
    }
}

/// The drawer's hit records: the quad id per pixel (0 = sky) and the visible set the ids
/// index (the quad behind an id), as the visibility numbered it this frame.
#[derive(Clone)]
struct Ids {
    ids: Vec<u32>,
    visible: Vec<Visible>,
}

/// A visible set on the wire: a count; each chunk's position, first id, per-face counts and
/// quads.
fn put_visible(p: &mut mr_core::gate::Put, visible: &[Visible]) {
    p.u32(visible.len() as u32);
    for (pos, m, base) in visible {
        pos.to_array().into_iter().for_each(|c| p.i32(c));
        p.u32(*base);
        m.counts.iter().for_each(|&c| p.u32(c));
        p.u32(m.quads.len() as u32);
        m.quads.iter().flatten().for_each(|&q| p.u32(q));
    }
}

/// What [`put_visible`] wrote, and the id after its last quad. It must read as a frame's:
/// the first ids follow on from 1 without gaps (else the message is lost, as a decoder
/// rejects it).
fn get_visible(g: &mut mr_core::gate::Get) -> Result<(Vec<Visible>, u32), mr_core::gate::WireError> {
    let bad = |w: &str| mr_core::gate::WireError(w.to_string());
    let chunks = g.count(4 * 11)?;
    let mut visible = Vec::with_capacity(chunks);
    let mut next = 1u32;
    for _ in 0..chunks {
        let pos = IVec3::new(g.i32()?, g.i32()?, g.i32()?);
        let base = g.u32()?;
        let mut counts = [0u32; 6];
        for c in &mut counts {
            *c = g.u32()?;
        }
        let quads = g.count(8)?;
        let quads: Vec<[u32; 2]> = (0..quads).map(|_| Ok([g.u32()?, g.u32()?])).collect::<Result<_, mr_core::gate::WireError>>()?;
        if base != next || counts.iter().map(|&c| c as u64).sum::<u64>() != quads.len() as u64 {
            return Err(bad("the visible set does not number its quads"));
        }
        next = base.checked_add(quads.len() as u32).ok_or_else(|| bad("quad ids past u32"))?;
        visible.push((pos, Arc::new(Mesh { quads, counts }), base));
    }
    Ok((visible, next))
}

/// On the wire: the ids (a count, 4 bytes each), then the visible set ([`put_visible`]).
/// Every id must name a quad (else the message is lost, as a decoder rejects it).
impl exec::Crossing for Ids {
    fn sent(&self) -> Self {
        self.clone()
    }
    fn encode(&self, out: &mut Vec<u8>) {
        let mut p = mr_core::gate::Put(out);
        p.u32(self.ids.len() as u32);
        self.ids.iter().for_each(|&i| p.u32(i));
        put_visible(&mut p, &self.visible);
    }
    fn decode(bytes: &[u8]) -> Result<Self, mr_core::gate::WireError> {
        let mut g = mr_core::gate::Get(bytes);
        let n = g.count(4)?;
        let ids: Vec<u32> = (0..n).map(|_| g.u32()).collect::<Result<_, _>>()?;
        let (visible, next) = get_visible(&mut g)?;
        g.end()?;
        if ids.iter().any(|&i| i >= next) {
            return Err(mr_core::gate::WireError("an id names no visible quad".into()));
        }
        Ok(Ids { ids, visible })
    }
    fn blank(w: u32, h: u32) -> Self {
        Ids { ids: vec![0; (w * h) as usize], visible: vec![] }
    }
    fn fits(&self, w: u32, h: u32) -> bool {
        self.ids.len() == (w * h) as usize
    }
}

/// Draw `raster`: bins the triangles into (band, tile) cells, rasterises each band in
/// parallel into the depth buffer and quad ids (its output, [`Ids`]); `fused`
/// (`placement = inline`): shading compiled in, each fragment passing the depth test shaded
/// into the image (its output); the shading's shadow rays walk the storage (`ins[1]`, none:
/// no shadow rays). Reading the geometry's meshes straight (no culling node), it transforms
/// and clips them itself (`transform`: a [`Faces`] of its own and its triangles).
struct Draw<P> {
    fused: bool,
    occluded: Occluded,
    tile_width: u32,
    band_height: u32,
    depth: Vec<f32>,
    nodes: Vec<String>,
    transform: Option<(Faces, Res)>,
    /// The storage read when no shadow site is bound (`occluded` is then [`never`]).
    none: Res,
    px: std::marker::PhantomData<P>,
}

/// The walk of a shading without a shadow site: nothing occludes.
fn never(_: &Res, _: IVec3, _: Vec3, _: Vec3, _: f32) -> bool {
    false
}

impl<P: Pixel> Node for Draw<P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        self.depth = vec![0.0; (w * h) as usize];
        if self.fused { exec::image::<P>(w, h) } else { Box::new(Ids { ids: vec![0u32; (w * h) as usize], visible: vec![] }) }
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let mut own = self.transform.take();
        let t: &Tris = match own.as_mut() {
            Some((faces, tris)) if ins[0].is::<Meshes>() => {
                faces.run(cx, &ins[..1], tris, report);
                tris.downcast_ref().expect("triangles")
            }
            _ => ins[0].downcast_ref::<Tris>().expect("triangles"),
        };
        let start = Instant::now();
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
        let rows = (bh * w) as usize;
        if self.fused {
            let quads = Quads::new(&t.visible, (ins.get(1).copied().unwrap_or(&self.none), self.occluded), cx);
            let raster = Raster { tris, bins: &bins, tiles_x, tw, bh, w, shade: |id, x, y| quads.shade::<P>(id, x, y), sky: cx.world.sky() };
            let pixels = out.downcast_mut::<Vec<P>>().expect("an image");
            self.depth.par_chunks_mut(rows).zip(pixels.par_chunks_mut(rows)).enumerate().for_each(|(band, (depth, pixels))| raster.band::<true>(band, depth, &mut [], pixels));
        } else {
            let sky: P = cx.world.sky();
            let raster = Raster { tris, bins: &bins, tiles_x, tw, bh, w, shade: |_, _, _| sky, sky };
            let out = out.downcast_mut::<Ids>().expect("quad ids");
            self.depth.par_chunks_mut(rows).zip(out.ids.par_chunks_mut(rows)).enumerate().for_each(|(band, (depth, ids))| raster.band::<false>(band, depth, ids, &mut []));
            out.visible.clone_from(&t.visible);
        }
        self.transform = own;
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
        (std::mem::size_of_val(&self.depth[..]) + out.downcast_ref::<Ids>().map_or(0, |r| std::mem::size_of_val(&r.ids[..]))) as u64
    }
    fn gate(&self, chain: mr_core::gate::GateChain, late: bool, w: u32, h: u32) -> Option<Box<dyn Node>> {
        (!self.fused).then(|| exec::GatePass::<Ids>::boxed(chain, late, w, h))
    }
}

/// The common deferred pass over the quad ids; its shadow rays walk the storage (`ins[1]`).
struct Shading<P> {
    occluded: Occluded,
    /// The storage read when no shadow site is bound (`occluded` is then [`never`]).
    none: Res,
    nodes: Vec<String>,
    px: std::marker::PhantomData<P>,
}

impl<P: Pixel> Node for Shading<P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        exec::image::<P>(w, h)
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let r = ins[0].downcast_ref::<Ids>().expect("quad ids");
        let quads = Quads::new(&r.visible, (ins.get(1).copied().unwrap_or(&self.none), self.occluded), cx);
        mr_components::shading::deferred::shade(&QuadIds { ids: &r.ids, quads: &quads, sky: cx.world.sky() }, cx.width, out.downcast_mut::<Vec<P>>().expect("an image"));
        let t = start.elapsed().as_secs_f64() * 1e3;
        report.stages.push(StageTime::cpu("shade", &self.nodes, t as f32));
        report.counters.push(("shade_ms".into(), t));
    }
    fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
        *out = exec::image::<P>(w, h);
    }
}

/// The executor for graph `g`: one pass per scheduled storage, geometry, visibility, draw and
/// shading node (the storage's first: an inline draw shades with it; the kernel is fused,
/// the shading reads the storage through it); `S` what a compiled pipeline baked in.
pub(crate) fn executor<S: Spec, P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let opts: Options = p.options()?;
    let shading: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
    let deferred = exec::deferred(g, &shading.placement)?;
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    // The shading's shadow rays' storage and kernel (none bound: no shadow rays).
    let site = g.site(Role::Shading, "shadow");
    let mut walked = site.map(|site| crate::raycast::walked::<S>(p, g, site)).transpose()?;
    let occluded = walked.as_ref().map_or(never as Occluded, |w| w.occluded);
    let (mut passes, mut index): (Vec<Pass>, HashMap<String, usize>) = (vec![], HashMap::new());
    let storage_first = |p: &&mr_components::graph::Pass| g.node(&p.node).role == Role::Storage;
    for pass in schedule.passes().filter(storage_first).chain(schedule.passes().filter(|p| !storage_first(p))) {
        let id = pass.node.as_str();
        let mut ins = vec![];
        exec::reads(g, id, &index, &mut ins);
        let node: Box<dyn Node> = match g.node(id).role {
            // The shadow rays' storage, else one only read (a face source's lookup).
            Role::Storage if site.is_some_and(|s| s.storage == id) => walked.take().expect("one walked storage").node,
            Role::Storage => crate::raycast::unwalked(p, g, id)?,
            Role::Geometry => {
                let mesh = mesh_fn(&g.node(id).component);
                let k: GeometryKnobs = g.knobs(id).map_err(anyhow::Error::msg)?;
                Box::new(Geometry { dirty: HashSet::new(), mesh, knobs: Knobs { cull: k.cull, ..Default::default() }, nodes: vec![id.to_string()] })
            }
            // Discovery: its rays walk (refloor or incremental) its own copy of the world in their storage,
            // its exposure tests read the exposure port's (one copy when the same component).
            Role::Visibility if discovers(g, id) => {
                let rays = g.node(id).inputs.get("rays").ok_or_else(|| anyhow::anyhow!("`{id}`: discovery rays are not bound"))?;
                let storage = g.node(rays).inputs.get("storage").ok_or_else(|| anyhow::anyhow!("`{rays}`: walks no storage"))?;
                let exposure = g.node(id).inputs.get("exposure").ok_or_else(|| anyhow::anyhow!("`{id}`: exposure is not bound"))?;
                Box::new(Discover {
                    cpu: grid::CpuWorld::paired(&g.node(storage).component, &g.node(exposure).component)?
                        .walking(grid::Walk::of(&g.node(rays).component, &g.node(rays).knobs)?),
                    visibility: g.node(id).component.clone(),
                    knobs: g.knobs(id).map_err(anyhow::Error::msg)?,
                    state: None,
                    records: vec![],
                    nodes: vec![id.to_string()],
                })
            }
            Role::Visibility => Box::new(Faces { nodes: vec![id.to_string()], stage: "transform" }),
            // Inline, the fused shading's inputs too: its shadow rays' storage.
            Role::Draw => {
                if !deferred {
                    exec::reads(g, g.shading(), &index, &mut ins);
                    ins.retain(|&i| Some(&i) != index.get(id));
                }
                let k: DrawKnobs = g.knobs(id).map_err(anyhow::Error::msg)?;
                let nodes = if deferred { vec![id.to_string()] } else { g.ids(&[Role::Draw, Role::Shading]) };
                // Faces straight from the geometry or discovery: transformed in the draw's pass.
                let straight = g.node(id).inputs.get("faces").is_some_and(|f| g.node(f).role == Role::Geometry || discovers(g, f));
                let transform = straight.then(|| {
                    let mut f = Faces { nodes: vec![id.to_string()], stage: "transform" };
                    let tris = f.output(p.width, p.height);
                    (f, tris)
                });
                Box::new(Draw::<P> { fused: !deferred, occluded, tile_width: k.tile_width, band_height: k.band_height, depth: vec![], nodes, transform, none: Box::new(()), px: std::marker::PhantomData })
            }
            // Fused into the draw pass.
            Role::Shading if !deferred => continue,
            Role::Shading => Box::new(Shading::<P> { occluded, none: Box::new(()), nodes: vec![id.to_string()], px: std::marker::PhantomData }),
            _ => continue,
        };
        index.insert(id.to_string(), passes.len());
        passes.push(Pass::new(node, ins));
    }
    exec::late_edges(g, p.width, p.height, &mut passes, &mut index)?;
    exec::gate_edges(g, p.width, p.height, &mut passes, &mut index)?;
    let world = crate::world::<P>(p, g, shading.shadows)?;
    Ok(Box::new(exec::Executor::<P>::new(p, opts.threads, world, passes)?.baked(S::KNOBS, S::NATIVE)))
}

/// Whether node `id` is CPU discovery (its faces a face set, not a culled set).
fn discovers(g: &Checked, id: &str) -> bool {
    let n = g.node(id);
    n.role == Role::Visibility && matches!(n.component.as_str(), "quadtree" | "cluster")
}

/// The CPU mesher of a geometry component (quads with per-face counts: greedy, naive).
fn mesh_fn(name: &str) -> MeshFn {
    match name {
        // Cube instances drawn as their six faces.
        "cubes" => |c, n, slot, k| cube_faces(&mr_components::geometry::cubes::mesh(c, n, slot, k.cull)),
        // Boxes and points as the exposed block faces they stand for.
        _ => mr_components::geometry::drawn(name).expect("validated").mesh,
    }
}

/// The six unit faces of each cube instance (`[x | y << 5 | z << 10, block | slot << 16]`),
/// grouped per face as the quads' encoding has them.
fn cube_faces(cubes: &Mesh) -> Mesh {
    let n = cubes.quads.len();
    let mut quads = Vec::with_capacity(n * 6);
    for f in 0..6u32 {
        quads.extend(cubes.quads.iter().map(|q| [q[0] | f << 25, q[1]]));
    }
    Mesh { quads, counts: [n as u32; 6] }
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


/// The shadow rays' walk over the storage's output (`crate::raycast::Walked::occluded`).
type Occluded = fn(&Res, IVec3, Vec3, Vec3, f32) -> bool;

/// The quads a frame draws, numbered for visibility ids (0 = nothing), and how a quad's
/// pixel is shaded: the quad id is raster-cpu's encoding of the hit record, decoded here
/// (the pixel's ray against the quad's plane, in f64) and shaded with the reference model,
/// its shadow rays walking the storage (relative to the camera's block, in f32, as the walk
/// takes them).
struct Quads<'a> {
    visible: &'a [Visible],
    bases: Vec<u32>,
    world: &'a World,
    storage: &'a Res,
    occluded: Occluded,
    cam: Camera,
    cam_block: IVec3,
    w: u32,
    h: u32,
    mode: RenderMode,
}

impl<'a> Quads<'a> {
    fn new(visible: &'a [Visible], (storage, occluded): (&'a Res, Occluded), cx: &Cx<'a>) -> Self {
        let bases = visible.iter().map(|v| v.2).collect();
        let cam = cx.req.camera;
        Quads { visible, bases, world: cx.world, storage, occluded, cam, cam_block: cam.split_position().0, w: cx.width, h: cx.height, mode: cx.req.mode }
    }

    #[inline]
    fn shade<P: Pixel>(&self, id: u32, x: u32, y: u32) -> P {
        let k = self.bases.partition_point(|&b| b <= id) - 1;
        let (pos, m, base) = &self.visible[k];
        let (pos, base) = (*pos, *base);
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
        let base: DVec3 = self.cam_block.as_dvec3();
        self.world.shade_surface(hit, local, q.face, q.block, self.mode, |p, d, dist| (self.occluded)(self.storage, self.cam_block, (p - base).as_vec3(), d.as_vec3(), dist as f32))
    }
}

/// What every band of a frame's rasterisation reads.
struct Raster<'a, P, F> {
    tris: &'a [Tri],
    bins: &'a [Vec<u32>],
    tiles_x: usize,
    tw: u32,
    bh: u32,
    w: u32,
    /// Shades quad `id` at a pixel (with `INLINE`; the quad table's).
    shade: F,
    sky: P,
}

impl<P: Pixel, F: Fn(u32, u32, u32) -> P + Sync> Raster<'_, P, F> {
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
                                pixels[i] = (self.shade)(t.id, x, y);
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

