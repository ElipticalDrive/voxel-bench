//! CPU-voxelspace back end (components, IDEAS §12): storage `heightmap`, visibility
//! `columns`, shading `direct`. Was the cpu-voxelspace renderer.
//!
//! CPU port of gpu-voxelspace: Comanche (1992) style 2.5D heightmap renderer. A heightmap
//! ring (top solid block height + block id per (x, z) column) is marched front-to-back per
//! screen column (rayon-parallel over columns), drawing spans with a per-column y-buffer.
//! Lit mode: the shading component (`direct::cpu::shade`) with the heightmap gradient as
//! the top faces' normal; ambient, sun and point lights, shadow rays walking the heightmap
//! storage (`Heightmap::trace`), as gpu-voxelspace.

use std::time::Instant;

use cpu_reference::{Pool, Shadows, VoxelScene};
use glam::{IVec2, IVec3, Vec2, Vec3};
use mr_components::shading::direct::cpu::{Ctx, Surface, shade};
use mr_components::storage::heightmap::{Heightmap, entry_top};
use mr_core::lighting::encode_srgb;
use mr_core::*;
use rayon::prelude::*;

pub fn entry() -> RendererEntry {
    RendererEntry {
        info: || RendererInfo {
            name: "cpu-voxelspace".into(),
            description: "Comanche-style Voxel Space on the CPU: per-column front-to-back heightmap march with a y-buffer, rayon-parallel over columns (2.5D, no overhangs)".into(),
            tags: vec!["cpu".into(), "heightmap".into(), "2.5d".into(), "software".into()],
            modes: vec![RenderMode::Unlit, RenderMode::Lit],
            max_lod: 0,
            params: vec![
                ParamSpec::choice("storage", "Storage component", &mr_components::names_on(BACKEND, mr_components::Slot::Storage), "heightmap", false),
                ParamSpec::choice("visibility", "Visibility component", &mr_components::names_on(BACKEND, mr_components::Slot::Visibility), "columns", false),
                ParamSpec::choice("shading", "Shading component", &mr_components::names_on(BACKEND, mr_components::Slot::Shading), "direct", false),
                ParamSpec::int("threads", "Worker threads (0 = all cores)", 0, 256, 0, true),
                ParamSpec::choice("shadows", "Shadow rays in lit mode (walking the heightmap)", &["none", "sun", "all"], "all", true).of("shading", &["direct"]),
            ],
        },
        create: |p, _, _| Ok(Box::new(CpuVoxelSpace::new(p)?)),
    }
}

pub struct CpuVoxelSpace {
    /// The nodes of the timed stage (PLAN step 11).
    stage_nodes: Vec<String>,
    width: u32,
    height: u32,
    scene: VoxelScene,
    pool: Pool,
    /// The heightmap storage.
    hm: Heightmap,
    /// Cells marched per column (2 * view radius in blocks, as in gpu-voxelspace).
    max_cells: usize,
    cols: Vec<u8>,
    pixels: Vec<u8>,
}

#[derive(serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    storage: String,
    visibility: String,
    shading: String,
    threads: usize,
    shadows: Shadows,
}

impl Default for Options {
    fn default() -> Self {
        Self { storage: "heightmap".into(), visibility: "columns".into(), shading: "direct".into(), threads: 0, shadows: Shadows::All }
    }
}

const BACKEND: &str = "cpu-voxelspace";

const EMPTY: f32 = -1e6;

impl CpuVoxelSpace {
    pub fn new(p: InitParams) -> anyhow::Result<Self> {
        let opts: Options = p.options()?;
        use mr_components::{Slot, require};
        for (slot, name) in [(Slot::Storage, &opts.storage), (Slot::Visibility, &opts.visibility), (Slot::Shading, &opts.shading)] {
            require(BACKEND, slot, name)?;
        }
        let mut scene = VoxelScene::new(&p);
        scene.shadows = opts.shadows;
        let stage_nodes = mr_components::graph::of_init(BACKEND, &p)?.ids(&[mr_components::graph::Role::Visibility, mr_components::graph::Role::Shading]);
        Ok(Self {
            stage_nodes,
            width: p.width,
            height: p.height,
            scene,
            pool: Pool::new(p.devices.cpu_threads(opts.threads))?,
            hm: Heightmap::new(p.view_radius_chunks),
            max_cells: (p.view_radius_chunks * CHUNK_SIZE as u32 * 2) as usize,
            cols: vec![0; (p.width * p.height * 4) as usize],
            pixels: vec![0; (p.width * p.height * 4) as usize],
        })
    }

    fn render_inner(&mut self, req: &FrameRequest) -> FrameStats {
        let start = Instant::now();
        self.scene.prepare();
        let (w, h) = (self.width as usize, self.height as usize);
        let cam = &req.camera;
        let (cam_block, o) = cam.split_position();
        let (fwd, right, up) = (cam.forward(), cam.right(), cam.up());
        let thf = (cam.fov_y * 0.5).tan();
        let aspect = w as f32 / h as f32;
        let (hm, scene) = (&self.hm, &self.scene);
        let lit = req.mode == RenderMode::Lit;
        let sky = encode_srgb(scene.env.sky);
        let max_cells = self.max_cells;
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
        let hm_at = |c: IVec2| -> u32 { hm.at(c.x + cam_block.x, c.y + cam_block.z) };
        let top_of = |e: u32| -> f32 { entry_top(e).map_or(EMPTY, |y| (y + 1 - cam_block.y) as f32) };
        let row_of = |p: Vec3| -> f32 {
            let depth = p.dot(fwd).max(1e-4);
            let sy = p.dot(up) / (depth * thf);
            (1.0 - sy) * 0.5 * h as f32
        };
        let pixel_ray = |x: f32, row: f32| -> Vec3 {
            let sx = (2.0 * x / w as f32 - 1.0) * thf * aspect;
            let sy = (1.0 - 2.0 * row / h as f32) * thf;
            (fwd + sx * right + sy * up).normalize()
        };
        // Column-major output (one contiguous column per task), transposed afterwards.
        self.cols.par_chunks_mut(h * 4).enumerate().for_each(|(gx, col)| {
            let x = gx as f32 + 0.5;
            let dc = pixel_ray(x, h as f32 * 0.5);
            let mut hd = Vec2::new(dc.x, dc.z);
            if hd.length() < 1e-5 {
                hd = Vec2::new(up.x, up.z) * -fwd.y.signum();
            }
            if hd.length() < 1e-5 {
                hd = Vec2::X;
            }
            let hd = hd.normalize();
            let mut ybuf = h as i32;
            let mut cell = IVec2::new(o.x.floor() as i32, o.z.floor() as i32);
            let stp = IVec2::new(if hd.x > 0.0 { 1 } else { -1 }, if hd.y > 0.0 { 1 } else { -1 });
            let inv = Vec2::ONE / hd.abs().max(Vec2::splat(1e-9));
            let oxz = Vec2::new(o.x, o.z);
            let edge = Vec2::new(
                if hd.x > 0.0 { (cell.x + 1) as f32 } else { cell.x as f32 },
                if hd.y > 0.0 { (cell.y + 1) as f32 } else { cell.y as f32 },
            );
            let mut tmax = ((edge - oxz) * Vec2::new(hd.x.signum(), hd.y.signum()) * inv).max(Vec2::ZERO);
            let mut t0 = 0.0f32;
            let mut axis = 0usize;
            let mut first = true;
            let mut i = 0;
            while i < max_cells && ybuf > 0 {
                i += 1;
                let t1 = tmax.x.min(tmax.y);
                let e = hm_at(cell);
                let hh = top_of(e);
                if hh > -1e5 {
                    let p0 = Vec3::new(o.x + hd.x * t0, hh, o.z + hd.y * t0);
                    let p1 = Vec3::new(o.x + hd.x * t1, hh, o.z + hd.y * t1);
                    let r = row_of(p0).min(row_of(p1));
                    let rtop = (r.floor() as i32).max(0);
                    let block = BlockId((e >> 16) as _);
                    // Heightmap-gradient normal for top faces (lit mode only).
                    let top_n = if lit {
                        let g = |c: IVec2| { let v = top_of(hm_at(c)); if v > -1e5 { v } else { 0.0 } };
                        let gx = (g(cell - IVec2::X) - g(cell + IVec2::X)).clamp(-2.0, 2.0);
                        let gz = (g(cell - IVec2::Y) - g(cell + IVec2::Y)).clamp(-2.0, 2.0);
                        Vec3::new(gx * 0.5, 2.0, gz * 0.5).normalize()
                    } else {
                        Vec3::Y
                    };
                    for row in rtop..ybuf {
                        let d = pixel_ray(x, row as f32 + 0.5);
                        let mut face = Face::PosY;
                        let mut t = 0.0;
                        let mut side = false;
                        if !first {
                            let a = if axis == 1 { 2 } else { 0 };
                            let wall = if stp[axis] < 0 { cell[axis] + 1 } else { cell[axis] } as f32;
                            if d[a].abs() > 1e-6 {
                                let tw = (wall - o[a]) / d[a];
                                if tw > 0.0 && o.y + d.y * tw <= hh {
                                    side = true;
                                    t = tw;
                                    face = Face::from_index(a * 2 + (d[a] > 0.0) as usize);
                                }
                            }
                        }
                        if !side {
                            t = if d.y < -1e-6 { (hh - o.y) / d.y } else { t0 / Vec2::new(d.x, d.z).length().max(1e-4) };
                        }
                        let p = o + d * t;
                        let local = (p - Vec3::new(cell.x as f32, hh - 1.0, cell.y as f32)).clamp(Vec3::ZERO, Vec3::splat(0.999));
                        let n = if side { face.normal().as_vec3() } else { top_n };
                        let px = shade(&ctx, &Surface { p, n, local, block: block.0, face }, req.mode);
                        let k = row as usize * 4;
                        col[k..k + 4].copy_from_slice(&px);
                    }
                    ybuf = ybuf.min(rtop);
                }
                first = false;
                t0 = t1;
                if tmax.x < tmax.y {
                    cell.x += stp.x;
                    tmax.x += inv.x;
                    axis = 0;
                } else {
                    cell.y += stp.y;
                    tmax.y += inv.y;
                    axis = 1;
                }
            }
            for row in 0..ybuf.max(0) as usize {
                col[row * 4..row * 4 + 4].copy_from_slice(&sky);
            }
        });
        let t_march = start.elapsed().as_secs_f64() * 1e3;
        let cols = &self.cols;
        self.pixels.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                let s = (x * h + y) * 4;
                row[x * 4..x * 4 + 4].copy_from_slice(&cols[s..s + 4]);
            }
        });
        let total = start.elapsed().as_secs_f64() * 1e3;
        FrameStats {
            cpu_ms: total as f32,
            gpu_ms: None,
            legend: None,
            counters: vec![
                ("heightmap_size".into(), hm.size as f64),
                ("chunk_columns".into(), hm.columns() as f64),
                ("march_ms".into(), t_march),
                ("transpose_ms".into(), total - t_march),
            ],
            // The transpose is the executor's (column-major to rows), no node's.
            stages: vec![StageTime::cpu("columns+shade", &self.stage_nodes, t_march as f32), StageTime::cpu("transpose", &[], (total - t_march) as f32)],
        }
    }
}

impl Renderer for CpuVoxelSpace {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let changed = self.scene.apply(updates);
        let scene = &self.scene;
        self.hm.apply(changed, |pos| scene.chunk(pos));
        Ok(())
    }

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
        let n = (width * height * 4) as usize;
        self.width = width;
        self.height = height;
        self.cols = vec![0; n];
        self.pixels = vec![0; n];
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: self.scene.dense_bytes() + self.hm.hm.len() as u64 * 4 + self.pixels.len() as u64 * 2, gpu_bytes: 0 }
    }
}
