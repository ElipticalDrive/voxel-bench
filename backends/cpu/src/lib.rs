//! The CPU executor (PLAN step 13e3): every pipeline whose image is made on the CPU, one
//! executor over one core with a lane per kind of visibility node:
//! - ray casting ([`raycast`]): one primary ray per pixel walked over a storage, monomorphised
//!   per storage (`cpu`; compiled on demand per composition, PLAN step 8);
//! - rasterisation ([`raster`]): meshed quads drawn into a visibility buffer (`cpu-raster`);
//! - columns ([`columns`]): the Voxel Space heightmap march (`cpu-voxelspace`).
//!
//! Which lane runs is the graph's: rasterisation when a drawer draws faces, else by the
//! visibility node's component. The rest is shared ([`Core`]): the thread pool every frame
//! runs in (the `threads` knob), the output image, its size and the timed stages' nodes.

use cpu_reference::Pool;
use mr_components::graph::{self, Role};
use mr_core::*;

mod columns;
mod perf;
mod raster;
mod raycast;

pub use raycast::{Generic, Spec, Traversal, storage_type, traversal};

pub fn cpu_entry() -> RendererEntry {
    RendererEntry { info: raycast::info, create: |p, _, _| create(raycast::BACKEND, p) }
}

pub fn cpu_raster_entry() -> RendererEntry {
    RendererEntry { info: raster::info, create: |p, _, _| create(raster::BACKEND, p) }
}

pub fn cpu_voxelspace_entry() -> RendererEntry {
    RendererEntry { info: columns::info, create: |p, _, _| create(columns::BACKEND, p) }
}

/// The renderer entry of a pipeline compiled for one composition (`S`, storage `R`); the
/// generated crate's worker serves only this.
pub fn entry_for<R: mr_components::storage::cpu::Repr, S: Spec>() -> RendererEntry {
    RendererEntry { info: raycast::info, create: |p, _, _| run(raycast::Raycast::<R, S>::new(&p, &graph::of_init(raycast::BACKEND, &p)?)?) }
}

/// The executor for `backend`'s graph: rasterisation when a drawer draws faces, else the lane
/// of the visibility node's component.
fn create(backend: &str, p: InitParams) -> anyhow::Result<Box<dyn Renderer>> {
    let g = graph::of_init(backend, &p)?;
    if g.find(Role::Draw).is_some() {
        return run(raster::CpuRaster::new(&p, &g)?);
    }
    let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("{backend}: no visibility node"))?;
    match g.node(vis).component.as_str() {
        "columns" => run(columns::CpuColumns::new(&p, &g)?),
        _ => raycast::create(&p, &g),
    }
}

fn run<L: Lane + 'static>((core, lane): (Core, L)) -> anyhow::Result<Box<dyn Renderer>> {
    Ok(Box::new(Executor { core, lane }))
}

/// Everything but the lane.
pub(crate) struct Core {
    /// Every frame runs in it.
    pool: Pool,
    frame: Frame,
}

/// The output image and what frames are timed by.
pub(crate) struct Frame {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    /// The nodes of the timed stages (PLAN step 11): visibility with shading, visibility,
    /// shading.
    stage_nodes: [Vec<String>; 3],
}

impl Core {
    /// `threads`: the knob's value (0 = every core the run grants).
    fn new(p: &InitParams, g: &graph::Checked, threads: usize) -> anyhow::Result<Self> {
        Ok(Self {
            pool: Pool::new(p.devices.cpu_threads(threads))?,
            frame: Frame {
                pixels: vec![0; (p.width * p.height * 4) as usize],
                width: p.width,
                height: p.height,
                stage_nodes: [g.ids(&[Role::Visibility, Role::Shading]), g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            },
        })
    }
}

/// A lane: the visibility node's implementation, its world and what it keeps.
trait Lane: Send {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()>;
    /// Renders `frame.pixels` (in `pool`).
    fn render(&mut self, frame: &mut Frame, pool: &Pool, req: &FrameRequest) -> FrameStats;
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>>;
    /// After the frame's resize.
    fn resize(&mut self, frame: &Frame);
    /// CPU bytes beyond the image.
    fn memory(&self) -> u64;
}

/// The core and one lane (static dispatch).
struct Executor<L> {
    core: Core,
    lane: L,
}

impl<L: Lane> Renderer for Executor<L> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        self.lane.update(updates)
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (core, lane) = (&mut self.core, &mut self.lane);
        let stats = core.pool.install(|| lane.render(&mut core.frame, &core.pool, req));
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&core.frame.pixels) });
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if let Some(r) = self.lane.set_param(name, value) {
            return r;
        }
        match name {
            "threads" => {
                let n = value.as_i64().unwrap_or(0) as usize;
                if n != self.core.pool.threads {
                    self.core.pool = Pool::new(n)?;
                }
                Ok(())
            }
            _ => anyhow::bail!("unknown parameter `{name}`"),
        }
    }

    fn flush(&mut self, _: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        let f = &mut self.core.frame;
        (f.width, f.height) = (width, height);
        f.pixels = vec![0; (width * height * 4) as usize];
        self.lane.resize(f);
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport { cpu_bytes: self.core.frame.pixels.len() as u64 + self.lane.memory(), gpu_bytes: 0 }
    }
}
