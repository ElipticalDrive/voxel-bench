//! The CPU columns lane (`cpu-voxelspace`; components): storage
//! `heightmap`, visibility `columns`, shading `direct`. The march is the columns component's
//! (`mr_components::visibility::columns::cpu`), rayon-parallel over screen columns. Lit
//! mode: the shading component (`direct::cpu::shade`) with the heightmap gradient as the top
//! faces' normal; ambient, sun and point lights, shadow rays walking the heightmap storage
//! (`Heightmap::trace`), as gpu-voxelspace. Shading placement: inline in the
//! march, or deferred (the march writes hit records, the common pass `shading::deferred`
//! shades them). The heightmap is patched from the change log.

use std::time::Instant;

use cpu_reference::{Chunks, Pool, Shadows, World};
use glam::{IVec3, Vec3};
use mr_components::shading::direct::cpu::{Ctx, Surface, shade};
use mr_components::storage::heightmap::Heightmap;
use mr_components::visibility::columns::cpu::{Deferred, Inline, March, Records};
use mr_components::visibility::record::Record;
use mr_core::*;
use rayon::prelude::*;

use crate::{Core, Frame, Lane, Pixel, Spec, fixed};

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

pub(crate) struct CpuColumns<S, P> {
    spec: std::marker::PhantomData<S>,
    /// The heightmap storage.
    hm: Heightmap,
    /// `placement = deferred`: the march writes records (`recs`, column-major), the common
    /// deferred pass shades them into `pixels`; inline, it shades into `cols`, transposed.
    deferred: bool,
    recs: Vec<Record>,
    chunks: Chunks,
    cols: Vec<P>,
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

impl<S: Spec, P: Pixel> CpuColumns<S, P> {
    pub(crate) fn new(p: &InitParams, g: &mr_components::graph::Checked) -> anyhow::Result<(Core<P>, Self)> {
        let opts: Options = p.options()?;
        use mr_components::{Slot, require};
        for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        let deferred = mr_components::parse_placement(&opts.placement)?;
        fixed("tone", S::TONE, g.tone())?;
        let lane = Self {
            spec: std::marker::PhantomData,
            hm: Heightmap::new(p.view_radius_chunks),
            deferred,
            recs: if deferred { vec![Record::SKY; (p.width * p.height) as usize] } else { Vec::new() },
            chunks: Chunks::default(),
            cols: if deferred { Vec::new() } else { vec![P::zeroed(); (p.width * p.height) as usize] },
        };
        Ok((Core::new(p, g, opts.threads, opts.shadows)?, lane))
    }
}

impl<S: Spec, P: Pixel> Lane for CpuColumns<S, P> {
    type Px = P;

    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.chunks.apply(updates);
        let scene = &self.chunks;
        self.hm.apply(changed, |pos| scene.chunk(pos));
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame<P>, _: &Pool, world: &World, req: &FrameRequest) -> FrameStats {
        let start = Instant::now();
        self.chunks.prepare();
        let (w, h) = (frame.width as usize, frame.height as usize);
        let cam = &req.camera;
        let cam_block = cam.split_position().0;
        let (hm, scene) = (&self.hm, world);
        let lit = req.mode == RenderMode::Lit;
        let sky: P = scene.sky();
        let m = March::new(hm, cam, w, h, hm.march_cells() as usize);
        // Shadow rays walk the heightmap.
        let shadows = S::SHADOWS.unwrap_or(scene.shadows);
        let ctx = Ctx {
            blocks: &scene.blocks,
            env: &scene.env,
            cam: cam_block,
            sun_shadows: shadows != Shadows::None,
            light_shadows: shadows == Shadows::All,
            lights_near: |world: IVec3| scene.lights_near(world),
            occluded: |p: Vec3, d: Vec3, dist: f32| hm.trace(cam_block, p, d, dist).is_some(),
            tone: S::TONE.unwrap_or(scene.tone),
        };
        let shade_s = |s: &Surface| -> P { shade(&ctx, s, req.mode) };
        let (t_march, stages) = if self.deferred {
            // The march writes column-major records (one contiguous column per task); the
            // common pass reads them across and writes rows, so no transpose remains.
            let recs = &mut self.recs;
            recs.par_chunks_mut(h).enumerate().for_each(|(gx, col)| m.column(gx, &mut Records { col }));
            let t_march = start.elapsed().as_secs_f64() * 1e3;
            let deferred = Deferred { recs, m: &m, shade: &shade_s, lit, sky };
            mr_components::shading::deferred::shade(&deferred, w as u32, &mut frame.pixels);
            let t_shade = start.elapsed().as_secs_f64() * 1e3 - t_march;
            (t_march, vec![StageTime::cpu("columns", &frame.stage_nodes[1], t_march as f32), StageTime::cpu("shade", &frame.stage_nodes[2], t_shade as f32)])
        } else {
            // Column-major output (one contiguous column per task), transposed afterwards.
            self.cols.par_chunks_mut(h).enumerate().for_each(|(gx, col)| m.column(gx, &mut Inline::new(col, &m, &shade_s, lit, sky)));
            let t_march = start.elapsed().as_secs_f64() * 1e3;
            let cols = &self.cols;
            frame.pixels.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                for (x, px) in row.iter_mut().enumerate() {
                    *px = cols[x * h + y];
                }
            });
            let t_transpose = start.elapsed().as_secs_f64() * 1e3 - t_march;
            // The transpose is the executor's (column-major to rows), no node's.
            (t_march, vec![StageTime::cpu("columns+shade", &frame.stage_nodes[0], t_march as f32), StageTime::cpu("transpose", &[], t_transpose as f32)])
        };
        let total = start.elapsed().as_secs_f64() * 1e3;
        FrameStats {
            cpu_ms: total as f32,
            gpu_ms: None,
            legend: None,
            counters: self.hm.counters().into_iter().chain([
                ("march_ms".into(), t_march),
                (if self.deferred { "shade_ms" } else { "transpose_ms" }.into(), total - t_march),
            ]).collect(),
            stages,
        }
    }

    fn set_param(&mut self, _: &str, _: &ParamValue) -> Option<anyhow::Result<()>> {
        None
    }

    fn resize(&mut self, frame: &Frame<P>) {
        let n = (frame.width * frame.height) as usize;
        if self.deferred {
            self.recs = vec![Record::SKY; n];
        } else {
            self.cols = vec![P::zeroed(); n];
        }
    }

    fn memory(&self) -> u64 {
        self.chunks.dense_bytes() + (std::mem::size_of_val(&self.cols[..]) + self.recs.len() * 8 + self.hm.hm.len() * 4) as u64
    }
}
