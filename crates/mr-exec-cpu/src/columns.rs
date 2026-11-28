//! The CPU columns pipeline (`columns-cpu`; components) as nodes of the generic
//! executor ([`crate::exec`]): storage `heightmap`, visibility `columns`, shading `direct`.
//! The march is the columns component's (`mr_components::visibility::columns::cpu`),
//! rayon-parallel over screen columns. Lit mode: the shading component (`direct::cpu::shade`)
//! with the heightmap gradient as the top faces' normal; ambient, sun and point lights,
//! shadow rays walking the heightmap storage (`Heightmap::trace`: the `heightfield` kernel the
//! graph binds to the shading's `shadow` port), as columns-gpu. Shading
//! placement: inline in the march (one fused pass), or deferred (the march writes hit
//! records, the common pass `shading::deferred` shades them). The heightmap is patched from
//! the change log.

use std::collections::HashMap;
use std::time::Instant;

use mr_reference::{Chunks, Shadows, World};
use glam::{IVec3, Vec3};
use mr_components::graph::{Checked, Role};
use mr_components::shading::direct::cpu::{Ctx, Surface, shade};
use mr_components::storage::heightmap::Heightmap;
use mr_components::visibility::columns::cpu::{Deferred, Inline, March, Records};
use mr_components::visibility::record::Record;
use mr_core::*;
use rayon::prelude::*;

use crate::exec::{self, Cx, Node, Pass, Report, Res};
use crate::{Pixel, Spec, fixed};

pub(crate) fn info() -> RendererInfo {
    RendererInfo {
        name: "cpu".into(),
        description: "Comanche-style Voxel Space on the CPU: per-column front-to-back heightmap march with a y-buffer, rayon-parallel over columns (2.5D, no overhangs)".into(),
        tags: vec!["cpu".into(), "heightmap".into(), "2.5d".into(), "software".into()],
        features: Features::of(&[Feature::Lit]).lacking(Feature::Volume, "storage `heightmap` is 2.5D: a column is solid below its top block"),
        params: vec![
            REGION.placement_param("inline"),
            ParamSpec::int("threads", "Worker threads (0 = all cores)", 0, 256, 0, true),
            ParamSpec::choice("shadows", "Shadow rays in lit mode (walking the heightmap)", &["none", "sun", "all"], "all", true).of("shading", &["direct"]),
        ],
    }
}

/// The region's own knobs (the nodes' are read from the graph).
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct Options {
    threads: usize,
}

/// The shading node's knobs.
#[derive(serde::Deserialize)]
#[serde(default)]
struct ShadingKnobs {
    placement: String,
    shadows: Shadows,
}

impl Default for ShadingKnobs {
    fn default() -> Self {
        Self { placement: "inline".into(), shadows: Shadows::All }
    }
}

pub(crate) const REGION: &mr_components::graph::Builds = &mr_components::graph::CPU_COLUMNS;


/// The storage's output: the heightmap and the chunks it is patched from.
struct Field {
    hm: Heightmap,
    chunks: Chunks,
}

fn field(r: &Res) -> &Field {
    r.downcast_ref().expect("a heightmap")
}

impl Field {
    /// The march of this frame's `w` x `h` image.
    fn march<'a>(&'a self, req: &FrameRequest, w: u32, h: u32) -> March<'a> {
        March::new(&self.hm, &req.camera, w as usize, h as usize, self.hm.march_cells() as usize)
    }
}

/// The shading component for this frame, its shadow rays walking the heightmap `shadow`
/// (the storage its shadow site's march walks; none bound: no shadow rays).
fn shader<'a, S: Spec, P: Pixel>(shadow: Option<&'a Field>, world: &'a World, req: &FrameRequest) -> impl Fn(&Surface) -> P + Sync + 'a {
    let (hm, cam) = (shadow.map(|f| &f.hm), req.camera.split_position().0);
    let shadows = if hm.is_some() { S::SHADOWS.unwrap_or(world.shadows) } else { Shadows::None };
    let ctx = Ctx {
        blocks: &world.blocks,
        env: &world.env,
        cam,
        sun_shadows: shadows != Shadows::None,
        light_shadows: shadows == Shadows::All,
        lights_near: move |at: IVec3| world.lights_near(at),
        transmit: move |p: Vec3, d: Vec3, dist: f32| if hm.is_some_and(|hm| hm.trace(cam, p, d, dist).is_some()) { 0.0 } else { 1.0 },
        tone: S::TONE.unwrap_or(world.tone),
    };
    let mode = req.mode;
    move |s: &Surface| shade(&ctx, s, mode)
}

/// A heightmap storage node of its own (another region's: a face source's lookup).
pub(crate) fn storage(p: &InitParams) -> Box<dyn Node> {
    Box::new(Storage { init: Some(Field { hm: Heightmap::new(p.view_radius_chunks), chunks: Chunks::default() }) })
}

/// The heightmap storage.
struct Storage {
    /// The heightmap before the first frame (taken by [`Node::output`]).
    init: Option<Field>,
}

impl Node for Storage {
    fn output(&mut self, _: u32, _: u32) -> Res {
        Box::new(self.init.take().expect("one output per storage"))
    }
    fn update(&mut self, out: &mut Res, updates: &[WorldUpdate]) -> anyhow::Result<()> {
        let f = out.downcast_mut::<Field>().expect("a heightmap");
        let changed = f.chunks.apply(updates);
        let chunks = &f.chunks;
        f.hm.apply(changed, |pos| chunks.chunk(pos));
        Ok(())
    }
    fn run(&mut self, _: &Cx, _: &[&Res], out: &mut Res, report: &mut Report) {
        let f = out.downcast_mut::<Field>().expect("a heightmap");
        f.chunks.prepare();
        report.counters.extend(f.hm.counters());
    }
    fn memory(&self, out: &Res) -> u64 {
        let f = field(out);
        f.chunks.dense_bytes() + f.hm.hm.len() as u64 * 4
    }
}

/// The march; `fused`: shading inline, its output the image (marched column-major into
/// `cols`, transposed), else the column-major hit records. `cast`: the ray caster over the
/// heightmap's march instead of the column march (one ray per pixel through
/// `Heightmap::trace`, the `heightfield` kernel; fused, its records shaded in the same pass).
struct Visibility<S, P> {
    fused: bool,
    cast: bool,
    recs: Vec<Record>,
    /// Fused: the input the shadow rays walk (its own storage `0`, another `1`; none: no
    /// shadow rays).
    shadow_in: Option<usize>,
    cols: Vec<P>,
    nodes: Vec<String>,
    spec: std::marker::PhantomData<S>,
}

impl<S: Spec, P: Pixel> Node for Visibility<S, P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        if self.fused { exec::image::<P>(w, h) } else { Box::new(vec![Record::SKY; (w * h) as usize]) }
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let f = field(ins[0]);
        let (w, h) = (cx.width as usize, cx.height as usize);
        let m = f.march(cx.req, cx.width, cx.height);
        if self.cast {
            // One ray per pixel, column-major records as the march writes them.
            let cam = cx.req.camera.split_position().0;
            let cast = |recs: &mut [Record]| {
                recs.par_chunks_mut(h).enumerate().for_each(|(gx, col)| {
                    for (gy, r) in col.iter_mut().enumerate() {
                        let d = m.pixel_ray(gx as f32 + 0.5, gy as f32 + 0.5);
                        *r = match f.hm.trace(cam, m.o, d, f32::MAX) {
                            Some((t, cell, axis, block)) => Record::hit(t, block, Face::entered(axis, if d[axis] > 0.0 { 1 } else { -1 }), cell),
                            None => Record::SKY,
                        };
                    }
                });
            };
            if !self.fused {
                cast(out.downcast_mut::<Vec<Record>>().expect("hit records"));
                let t = start.elapsed().as_secs_f32() * 1e3;
                report.stages.push(StageTime::cpu("raycast", &self.nodes, t));
                report.counters.push(("cast_ms".into(), t as f64));
                return;
            }
            self.recs.resize(w * h, Record::SKY);
            cast(&mut self.recs);
            let shade_s = shader::<S, P>(self.shadow_in.map(|i| field(ins[i])), cx.world, cx.req);
            let deferred = Deferred { recs: &self.recs, m: &m, shade: &shade_s, lit: cx.req.mode == RenderMode::Lit, sky: cx.world.sky() };
            mr_components::shading::deferred::shade(&deferred, cx.width, out.downcast_mut::<Vec<P>>().expect("an image"));
            let t = start.elapsed().as_secs_f32() * 1e3;
            report.stages.push(StageTime::cpu("raycast+shade", &self.nodes, t));
            report.counters.push(("cast_ms".into(), t as f64));
            return;
        }
        if !self.fused {
            // One contiguous column per task; the deferred pass reads them across and writes
            // rows, so no transpose remains.
            let recs = out.downcast_mut::<Vec<Record>>().expect("hit records");
            recs.par_chunks_mut(h).enumerate().for_each(|(gx, col)| m.column(gx, &mut Records { col }));
            let t = start.elapsed().as_secs_f32() * 1e3;
            report.stages.push(StageTime::cpu("columns", &self.nodes, t));
            report.counters.push(("march_ms".into(), t as f64));
            return;
        }
        let (lit, sky) = (cx.req.mode == RenderMode::Lit, cx.world.sky::<P>());
        let shade_s = shader::<S, P>(self.shadow_in.map(|i| field(ins[i])), cx.world, cx.req);
        self.cols.resize(w * h, P::zeroed());
        self.cols.par_chunks_mut(h).enumerate().for_each(|(gx, col)| m.column(gx, &mut Inline::new(col, &m, &shade_s, lit, sky)));
        let t_march = start.elapsed().as_secs_f64() * 1e3;
        let (cols, pixels) = (&self.cols, out.downcast_mut::<Vec<P>>().expect("an image"));
        pixels.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, px) in row.iter_mut().enumerate() {
                *px = cols[x * h + y];
            }
        });
        let t_transpose = start.elapsed().as_secs_f64() * 1e3 - t_march;
        // The transpose is the executor's (column-major to rows), no node's.
        report.stages.extend([StageTime::cpu("columns+shade", &self.nodes, t_march as f32), StageTime::cpu("transpose", &[], t_transpose as f32)]);
        report.counters.extend([("march_ms".into(), t_march), ("transpose_ms".into(), t_transpose)]);
    }
    fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
        *out = self.output(w, h);
    }
    fn memory(&self, out: &Res) -> u64 {
        (std::mem::size_of_val(&self.cols[..]) + std::mem::size_of_val(&self.recs[..]) + out.downcast_ref::<Vec<Record>>().map_or(0, |r| std::mem::size_of_val(&r[..]))) as u64
    }
    fn gate(&self, chain: mr_core::gate::GateChain, late: bool, w: u32, h: u32) -> Option<Box<dyn Node>> {
        (!self.fused).then(|| exec::GatePass::<Vec<Record>>::boxed(chain, late, w, h))
    }
}

/// The common deferred pass over the hit records (decoded against the march's heightmap,
/// `ins[field_in]`); its shadow rays walk the heightmap the `heightfield` kernel bound to its
/// `shadow` port reads (`ins[shadow_in]`; none bound: no shadow rays).
struct Shading<S, P> {
    field_in: usize,
    shadow_in: Option<usize>,
    nodes: Vec<String>,
    spec: std::marker::PhantomData<(S, P)>,
}

impl<S: Spec, P: Pixel> Node for Shading<S, P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        exec::image::<P>(w, h)
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let (recs, f) = (ins[0].downcast_ref::<Vec<Record>>().expect("hit records"), field(ins[self.field_in]));
        let m = f.march(cx.req, cx.width, cx.height);
        let shade_s = shader::<S, P>(self.shadow_in.map(|i| field(ins[i])), cx.world, cx.req);
        let deferred = Deferred { recs, m: &m, shade: &shade_s, lit: cx.req.mode == RenderMode::Lit, sky: cx.world.sky() };
        mr_components::shading::deferred::shade(&deferred, cx.width, out.downcast_mut::<Vec<P>>().expect("an image"));
        let t = start.elapsed().as_secs_f32() * 1e3;
        report.stages.push(StageTime::cpu("shade", &self.nodes, t));
        report.counters.push(("shade_ms".into(), t as f64));
    }
    fn resize(&mut self, out: &mut Res, w: u32, h: u32) {
        *out = exec::image::<P>(w, h);
    }
}

/// The executor for graph `g`: one pass per scheduled storage, visibility and shading node
/// (the kernel is fused: the shading reads the storage through it); `S` what a compiled
/// pipeline baked in.
pub(crate) fn executor<S: Spec, P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    let opts: Options = p.options()?;
    let shading: ShadingKnobs = g.knobs(g.shading()).map_err(anyhow::Error::msg)?;
    let deferred = exec::deferred(g, &shading.placement)?;
    fixed("tone", S::TONE, g.tone())?;
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    // The march's heightmap and the one the shading's shadow rays walk (none bound: no
    // shadow rays); one pass per storage node, the same one when both are.
    let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("columns: no visibility node"))?;
    // The ray caster reads the heightmap through its primary rays' march.
    let cast = g.node(vis).component == "raycast";
    let read = |id: &str, port: &str| g.node(id).inputs.get(port).cloned();
    let marched = if cast { read(vis, "primary").and_then(|k| read(&k, "storage")) } else { read(vis, "storage") };
    let marched = marched.ok_or_else(|| anyhow::anyhow!("columns: `{vis}` marches no storage"))?;
    let shadowed = g.site(Role::Shading, "shadow").map(|s| s.storage.to_string());
    // Storages first: an inline march shades with the shadow rays' heightmap.
    let storage_first = |p: &&mr_components::graph::Pass| g.node(&p.node).role == Role::Storage;
    let (mut passes, mut index): (Vec<Pass>, HashMap<String, usize>) = (vec![], HashMap::new());
    for pass in schedule.passes().filter(storage_first).chain(schedule.passes().filter(|p| !storage_first(p))) {
        let id = pass.node.as_str();
        let mut ins = vec![];
        exec::reads(g, id, &index, &mut ins);
        // Where among `ins` storage node `s` is (added if not read yet).
        let at = |ins: &mut Vec<usize>, s: &str| {
            let i = index[s];
            ins.iter().position(|&j| j == i).unwrap_or_else(|| {
                ins.push(i);
                ins.len() - 1
            })
        };
        let node: Box<dyn Node> = match g.node(id).role {
            Role::Storage => Box::new(Storage { init: Some(Field { hm: Heightmap::new(p.view_radius_chunks), chunks: Chunks::default() }) }),
            Role::Visibility => {
                let nodes = if deferred { vec![id.to_string()] } else { g.ids(&[Role::Visibility, Role::Shading]) };
                // Inline, the fused shading's shadow rays' heightmap too.
                let shadow_in = shadowed.as_deref().filter(|_| !deferred).map(|s| at(&mut ins, s));
                // The march's heightmap first (the ray caster reads it through its kernel).
                let field_in = at(&mut ins, &marched);
                if field_in != 0 {
                    ins.swap(0, field_in);
                }
                let shadow_in = shadow_in.map(|i| if i == 0 { field_in } else if i == field_in { 0 } else { i });
                Box::new(Visibility::<S, P> { fused: !deferred, cast, recs: vec![], shadow_in, cols: vec![], nodes, spec: std::marker::PhantomData })
            }
            // Fused into the visibility pass.
            Role::Shading if !deferred => continue,
            // Reads the records, the march's heightmap they are decoded against and, through
            // its shadow rays' kernel (a heightmap's own march), that heightmap.
            Role::Shading => {
                let field_in = at(&mut ins, &marched);
                let shadow_in = shadowed.as_deref().map(|s| at(&mut ins, s));
                Box::new(Shading::<S, P> { field_in, shadow_in, nodes: vec![id.to_string()], spec: std::marker::PhantomData })
            }
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
