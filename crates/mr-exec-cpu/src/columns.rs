//! The CPU columns pipeline (`cpu-voxelspace`; components) as nodes of the generic
//! executor ([`crate::exec`]): storage `heightmap`, visibility `columns`, shading `direct`.
//! The march is the columns component's (`mr_components::visibility::columns::cpu`),
//! rayon-parallel over screen columns. Lit mode: the shading component (`direct::cpu::shade`)
//! with the heightmap gradient as the top faces' normal; ambient, sun and point lights,
//! shadow rays walking the heightmap storage (`Heightmap::trace`), as gpu-voxelspace. Shading
//! placement: inline in the march (one fused pass), or deferred (the march writes hit
//! records, the common pass `shading::deferred` shades them). The heightmap is patched from
//! the change log.

use std::collections::HashMap;
use std::time::Instant;

use mr_reference::{Chunks, Shadows, World};
use glam::{IVec3, Vec3};
use mr_components::Kind;
use mr_components::graph::{Build, Builds, Checked, Role};
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
        name: "cpu-voxelspace".into(),
        description: "Comanche-style Voxel Space on the CPU: per-column front-to-back heightmap march with a y-buffer, rayon-parallel over columns (2.5D, no overhangs)".into(),
        tags: vec!["cpu".into(), "heightmap".into(), "2.5d".into(), "software".into()],
        modes: vec![RenderMode::Unlit, RenderMode::Lit],
        max_lod: 0,
        params: vec![
            ParamSpec::choice("storage", "Storage component", &mr_components::names_on(BACKEND, mr_components::Slot::Storage), "heightmap", false),
            ParamSpec::choice("visibility", "Visibility component", &mr_components::names_on(BACKEND, mr_components::Slot::Visibility), "columns", false),
            ParamSpec::choice("shading", "Shading component", &mr_components::names_on(BACKEND, mr_components::Slot::Shading), "direct", false),
            mr_components::placement_param(BACKEND, "inline"),
            ParamSpec::int("threads", "Worker threads (0 = all cores)", 0, 256, 0, true),
            ParamSpec::choice("shadows", "Shadow rays in lit mode (walking the heightmap)", &["none", "sun", "all"], "all", true).of("shading", &["direct"]),
        ],
    }
}

#[derive(serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    visibility: String,
    shading: String,
    placement: String,
    threads: usize,
    shadows: Shadows,
}

impl Default for Options {
    fn default() -> Self {
        Self { storage: "heightmap".into(), visibility: "columns".into(), shading: "direct".into(), placement: "inline".into(), threads: 0, shadows: Shadows::All }
    }
}

pub(crate) const BACKEND: &str = "cpu-voxelspace";

/// The nodes it builds: the heightmap, the column march and the shading.
pub(crate) const BUILDS: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["heightmap"], Kind::Cpu),
        Build::one(Role::Visibility, &["columns"], Kind::Cpu),
        Build::one(Role::Shading, &["direct"], Kind::Cpu),
    ],
    same: &[],
};

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

    /// The shading component for this frame, its shadow rays walking the heightmap.
    fn shader<'a, S: Spec, P: Pixel>(&'a self, world: &'a World, req: &FrameRequest) -> impl Fn(&Surface) -> P + Sync + 'a {
        let (hm, cam) = (&self.hm, req.camera.split_position().0);
        let shadows = S::SHADOWS.unwrap_or(world.shadows);
        let ctx = Ctx {
            blocks: &world.blocks,
            env: &world.env,
            cam,
            sun_shadows: shadows != Shadows::None,
            light_shadows: shadows == Shadows::All,
            lights_near: move |at: IVec3| world.lights_near(at),
            occluded: move |p: Vec3, d: Vec3, dist: f32| hm.trace(cam, p, d, dist).is_some(),
            tone: S::TONE.unwrap_or(world.tone),
        };
        let mode = req.mode;
        move |s: &Surface| shade(&ctx, s, mode)
    }
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
/// `cols`, transposed), else the column-major hit records.
struct Visibility<S, P> {
    fused: bool,
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
        let shade_s = f.shader::<S, P>(cx.world, cx.req);
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
        (std::mem::size_of_val(&self.cols[..]) + out.downcast_ref::<Vec<Record>>().map_or(0, |r| std::mem::size_of_val(&r[..]))) as u64
    }
}

/// The common deferred pass over the hit records; shadow rays walk the heightmap.
struct Shading<S, P> {
    nodes: Vec<String>,
    spec: std::marker::PhantomData<(S, P)>,
}

impl<S: Spec, P: Pixel> Node for Shading<S, P> {
    fn output(&mut self, w: u32, h: u32) -> Res {
        exec::image::<P>(w, h)
    }
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report) {
        let start = Instant::now();
        let (recs, f) = (ins[0].downcast_ref::<Vec<Record>>().expect("hit records"), field(ins[1]));
        let m = f.march(cx.req, cx.width, cx.height);
        let shade_s = f.shader::<S, P>(cx.world, cx.req);
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

/// The executor for graph `g`: one pass per scheduled storage, visibility and shading node;
/// `S` what a compiled pipeline baked in.
pub(crate) fn executor<S: Spec, P: Pixel>(p: &InitParams, g: &Checked) -> anyhow::Result<Box<dyn Renderer>> {
    g.built_by(BACKEND, BUILDS).map_err(anyhow::Error::msg)?;
    let opts: Options = p.options()?;
    use mr_components::{Slot, require};
    for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
        require(BACKEND, slot, name)?;
    }
    let deferred = mr_components::parse_placement(&opts.placement)?;
    fixed("tone", S::TONE, g.tone())?;
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    let mut init = Some(Field { hm: Heightmap::new(p.view_radius_chunks), chunks: Chunks::default() });
    let (mut passes, mut index): (Vec<Pass>, HashMap<String, usize>) = (vec![], HashMap::new());
    let mut storage = None;
    for pass in schedule.passes() {
        let id = pass.node.as_str();
        let mut ins = vec![];
        let node: Box<dyn Node> = match g.node(id).role {
            Role::Storage => {
                storage = Some(passes.len());
                Box::new(Storage { init: init.take() })
            }
            Role::Visibility => {
                let nodes = if deferred { vec![id.to_string()] } else { g.ids(&[Role::Visibility, Role::Shading]) };
                Box::new(Visibility::<S, P> { fused: !deferred, cols: vec![], nodes, spec: std::marker::PhantomData })
            }
            // Fused into the visibility pass.
            Role::Shading if !deferred => continue,
            Role::Shading => {
                // Its shadow rays walk the heightmap, which the graph leaves implicit (no
                // shadow input): read the storage's pass too.
                exec::reads(g, id, &index, &mut ins);
                ins.extend(storage);
                Box::new(Shading::<S, P> { nodes: vec![id.to_string()], spec: std::marker::PhantomData })
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
