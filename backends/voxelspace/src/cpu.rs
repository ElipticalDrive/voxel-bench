//! `cpu-voxelspace`, the voxelspace executor's CPU lane (components, IDEAS §12): storage
//! `heightmap`, visibility `columns`, shading `direct`. The march is the columns component's
//! (`mr_components::visibility::columns::cpu`), rayon-parallel over screen columns. Lit
//! mode: the shading component (`direct::cpu::shade`) with the heightmap gradient as the top
//! faces' normal; ambient, sun and point lights, shadow rays walking the heightmap storage
//! (`Heightmap::trace`), as gpu-voxelspace. Shading placement (PLAN step 12c): inline in the
//! march, or deferred (the march writes hit records, the common pass `shading::deferred`
//! shades them).

use std::time::Instant;

use cpu_reference::{Pool, Shadows, VoxelScene};
use glam::{IVec2, IVec3, Vec3};
use mr_components::shading::direct::cpu::{Ctx, Surface, shade};
use mr_components::storage::heightmap::Heightmap;
use mr_components::visibility::columns::cpu::{Deferred, Inline, March, Records};
use mr_components::visibility::record::Record;
use mr_core::lighting::encode_srgb;
use mr_core::*;
use rayon::prelude::*;

use crate::{Core, Lane};

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

pub(crate) struct CpuColumns {
    /// `placement = deferred`: the march writes records (`recs`, column-major), the common
    /// deferred pass shades them into `pixels`; inline, it shades into `cols`, transposed.
    deferred: bool,
    recs: Vec<Record>,
    scene: VoxelScene,
    pool: Pool,
    cols: Vec<u8>,
    pixels: Vec<u8>,
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

impl CpuColumns {
    fn render_inner(&mut self, core: &Core, req: &FrameRequest) -> FrameStats {
        let start = Instant::now();
        self.scene.prepare();
        let (w, h) = (core.width as usize, core.height as usize);
        let cam = &req.camera;
        let cam_block = cam.split_position().0;
        let (hm, scene) = (&core.hm, &self.scene);
        let lit = req.mode == RenderMode::Lit;
        let sky = encode_srgb(scene.env.sky);
        let m = March::new(hm, cam, w, h, core.max_cells as usize);
        // Shadow rays walk the heightmap.
        let ctx = Ctx {
            blocks: &scene.blocks,
            env: &scene.env,
            cam: cam_block,
            sun_shadows: scene.shadows != Shadows::None,
            light_shadows: scene.shadows == Shadows::All,
            lights_near: |world: IVec3| scene.lights_near(world),
            occluded: |p: Vec3, d: Vec3, dist: f32| hm.trace(cam_block, p, d, dist).is_some(),
        };
        let shade_s = |s: &Surface| shade(&ctx, s, req.mode);
        let (t_march, stages) = if self.deferred {
            // The march writes column-major records (one contiguous column per task); the
            // common pass reads them across and writes rows, so no transpose remains.
            let recs = &mut self.recs;
            recs.par_chunks_mut(h).enumerate().for_each(|(gx, col)| m.column(gx, &mut Records { col }));
            let t_march = start.elapsed().as_secs_f64() * 1e3;
            let deferred = Deferred { recs, m: &m, shade: &shade_s, lit, sky };
            mr_components::shading::deferred::shade(&deferred, w as u32, &mut self.pixels);
            let t_shade = start.elapsed().as_secs_f64() * 1e3 - t_march;
            (t_march, vec![StageTime::cpu("columns", &core.stage_nodes[1], t_march as f32), StageTime::cpu("shade", &core.stage_nodes[2], t_shade as f32)])
        } else {
            // Column-major output (one contiguous column per task), transposed afterwards.
            self.cols.par_chunks_mut(h * 4).enumerate().for_each(|(gx, col)| m.column(gx, &mut Inline::new(col, &m, &shade_s, lit, sky)));
            let t_march = start.elapsed().as_secs_f64() * 1e3;
            let cols = &self.cols;
            self.pixels.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                for x in 0..w {
                    let s = (x * h + y) * 4;
                    row[x * 4..x * 4 + 4].copy_from_slice(&cols[s..s + 4]);
                }
            });
            let t_transpose = start.elapsed().as_secs_f64() * 1e3 - t_march;
            // The transpose is the executor's (column-major to rows), no node's.
            (t_march, vec![StageTime::cpu("columns+shade", &core.stage_nodes[0], t_march as f32), StageTime::cpu("transpose", &[], t_transpose as f32)])
        };
        let total = start.elapsed().as_secs_f64() * 1e3;
        FrameStats {
            cpu_ms: total as f32,
            gpu_ms: None,
            legend: None,
            counters: core.counters().into_iter().chain([
                ("march_ms".into(), t_march),
                (if self.deferred { "shade_ms" } else { "transpose_ms" }.into(), total - t_march),
            ]).collect(),
            stages,
        }
    }
}

impl Lane for CpuColumns {
    fn new(p: &InitParams, _: &Devices, _: Waker, _: &Core) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        use mr_components::{Slot, require};
        for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        let mut scene = VoxelScene::new(p);
        scene.shadows = opts.shadows;
        let deferred = mr_components::parse_placement(&opts.placement)?;
        Ok(Self {
            deferred,
            recs: if deferred { vec![Record::SKY; (p.width * p.height) as usize] } else { Vec::new() },
            scene,
            pool: Pool::new(p.devices.cpu_threads(opts.threads))?,
            cols: if deferred { Vec::new() } else { vec![0; (p.width * p.height * 4) as usize] },
            pixels: vec![0; (p.width * p.height * 4) as usize],
        })
    }

    fn apply(&mut self, updates: Vec<WorldUpdate>) -> Vec<IVec3> {
        self.scene.apply(updates)
    }

    fn chunk(&self, pos: IVec3) -> Option<&ChunkData> {
        self.scene.chunk(pos)
    }

    fn heights(&mut self, _: &Heightmap, _: &[IVec2]) {}

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match name {
            "threads" => {
                let n = value.as_i64().unwrap_or(0) as usize;
                if n != self.pool.threads {
                    self.pool = Pool::new(n)?;
                }
            }
            "shadows" => self.scene.shadows = value.as_str().and_then(Shadows::parse).ok_or_else(|| anyhow::anyhow!("bad shadows value"))?,
            _ => anyhow::bail!("unknown parameter `{name}`"),
        }
        Ok(())
    }

    fn render(&mut self, core: &Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let pool = std::mem::replace(&mut self.pool, Pool::new(1)?);
        let stats = pool.install(|| self.render_inner(core, req));
        self.pool = pool;
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&self.pixels) });
        Ok(())
    }

    fn flush(&mut self, _: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }

    fn resize(&mut self, core: &Core) -> anyhow::Result<()> {
        let (width, height) = (core.width, core.height);
        let n = (width * height * 4) as usize;
        if self.deferred {
            self.recs = vec![Record::SKY; (width * height) as usize];
        } else {
            self.cols = vec![0; n];
        }
        self.pixels = vec![0; n];
        Ok(())
    }

    fn memory(&self, _: &Core) -> (u64, u64) {
        (self.scene.dense_bytes() + (self.pixels.len() + self.cols.len() + self.recs.len() * 8) as u64, 0)
    }
}
