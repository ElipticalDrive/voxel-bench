//! The CPU executor (PLAN step 13e3): every pipeline whose image is made on the CPU, one
//! executor over one core with a lane per kind of visibility node:
//! - ray casting ([`raycast`]): one primary ray per pixel walked over a storage, monomorphised
//!   per storage (`cpu`);
//! - rasterisation ([`raster`]): meshed quads drawn into a visibility buffer (`cpu-raster`);
//! - columns ([`columns`]): the Voxel Space heightmap march (`cpu-voxelspace`).
//!
//! Which lane runs is the graph's: rasterisation when a drawer draws faces, else by the
//! visibility node's component. The rest is shared ([`Core`]): the thread pool every frame
//! runs in (the `threads` knob), the world (`cpu_reference::World`: blocks, environment,
//! lights, the `shadows` knob; PLAN step 13e4d), the output image, its size and the timed
//! stages' nodes. The chunks are each lane's representation (its storage).
//!
//! Every lane is also compiled on demand for one graph (PLAN steps 8 and 16a: the host's
//! `pipelines` generates a crate naming the lane, its storage type and a [`Spec`]): its
//! choices are then constants the compiler folds, and the crate is built for this CPU.

use cpu_reference::{Pool, World};
use mr_components::graph::{self, Role};
use mr_core::*;

mod columns;
mod perf;
mod raster;
mod raycast;

pub use cpu_reference::Shadows;
pub use mr_core::lighting::Tone;
pub use raycast::{Traversal, level_mask, shadow_kernel, storage_type, traversal};

/// What a pipeline compiled for one graph bakes in (PLAN steps 8 and 16a): a generated crate
/// implements this with the graph's values, so they are constants the compiler folds (no
/// per-ray `match`, one packet width, no ISA dispatch: the whole crate is compiled for this
/// CPU). [`Generic`], the precompiled build, leaves everything open (`None`: read at run
/// time). A lane reads the constants it folds; [`Spec::KNOBS`] is what the worker refuses to
/// change (at creation and live) with the reason.
pub trait Spec: Send + Sync + 'static {
    /// The knob values compiled in (name, value as JSON); another value needs another build.
    const KNOBS: mr_core::baked::Knobs = &[];
    /// Ray casting: the primary rays' kernel (the `traversal` and `exact` knobs).
    const TRAVERSAL: Option<Traversal> = None;
    /// Ray casting: rays per packet.
    const PACKET: Option<usize> = None;
    /// Ray casting: the primary kernel's level mask (the `skip*` knobs).
    const LEVELS: Option<u32> = None;
    /// Ray casting: the shadow rays' own kernel and levels (`None` inside: the primary's).
    const SHADOW: Option<Option<(Traversal, u32)>> = None;
    /// The post's tone mapper (from the graph).
    const TONE: Option<Tone> = None;
    /// The shading's shadow rays (the `shadows` knob).
    const SHADOWS: Option<Shadows> = None;
    /// Compiled with `target-cpu=native`: the hot loop needs no ISA dispatch.
    const NATIVE: bool = false;
}

/// The precompiled build: every knob live.
pub struct Generic;
impl Spec for Generic {}

/// A value the lane folds when `S` baked it, else its run-time one; creation fails if a baked
/// graph value (tone, shadow kernel) differs from the graph's.
fn fixed<T: PartialEq + std::fmt::Debug>(what: &str, baked: Option<T>, actual: T) -> anyhow::Result<T> {
    match baked {
        Some(b) if b != actual => anyhow::bail!("this pipeline was compiled for {what} {b:?} (not {actual:?}); a change needs another build"),
        _ => Ok(actual),
    }
}

pub fn cpu_entry() -> RendererEntry {
    RendererEntry { info: raycast::info, create: |p, _, _| create(raycast::BACKEND, p) }
}

pub fn cpu_raster_entry() -> RendererEntry {
    RendererEntry { info: raster::info, create: |p, _, _| create(raster::BACKEND, p) }
}

pub fn cpu_voxelspace_entry() -> RendererEntry {
    RendererEntry { info: columns::info, create: |p, _, _| create(columns::BACKEND, p) }
}

/// The ray-casting lane compiled for one graph (`S`, storage `R`); the generated crate's
/// worker serves only this.
pub fn raycast_entry<R: mr_components::storage::cpu::Repr, S: Spec>() -> RendererEntry {
    RendererEntry { info: raycast::info, create: |p, _, _| compiled::<S, _>(&p, raycast::info, raycast::Raycast::<R, S>::new(&p, &graph::of_init(raycast::BACKEND, &p)?)) }
}

/// The rasterisation lane compiled for one graph.
pub fn raster_entry<S: Spec>() -> RendererEntry {
    RendererEntry { info: raster::info, create: |p, _, _| compiled::<S, _>(&p, raster::info, raster::CpuRaster::new(&p, &graph::of_init(raster::BACKEND, &p)?)) }
}

/// The columns lane compiled for one graph.
pub fn columns_entry<S: Spec>() -> RendererEntry {
    RendererEntry { info: columns::info, create: |p, _, _| compiled::<S, _>(&p, columns::info, columns::CpuColumns::<S>::new(&p, &graph::of_init(columns::BACKEND, &p)?)) }
}

fn compiled<S: Spec, L: Lane + 'static>(p: &InitParams, info: fn() -> RendererInfo, lane: anyhow::Result<(Core, L)>) -> anyhow::Result<Box<dyn Renderer>> {
    mr_core::baked::check(S::KNOBS, p, info)?;
    let (core, lane) = lane?;
    Ok(Box::new(Executor { core, lane, baked: S::KNOBS, native: S::NATIVE }))
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
        "columns" => run(columns::CpuColumns::<Generic>::new(&p, &g)?),
        _ => raycast::create(&p, &g),
    }
}

fn run<L: Lane + 'static>((core, lane): (Core, L)) -> anyhow::Result<Box<dyn Renderer>> {
    Ok(Box::new(Executor { core, lane, baked: &[], native: false }))
}

/// Everything but the lane.
pub(crate) struct Core {
    /// Every frame runs in it.
    pool: Pool,
    world: World,
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
    /// `threads`: the knob's value (0 = every core the run grants); `shadows` the world's.
    fn new(p: &InitParams, g: &graph::Checked, threads: usize, shadows: Shadows) -> anyhow::Result<Self> {
        let mut world = World::new(p);
        world.shadows = shadows;
        world.tone = g.tone();
        Ok(Self {
            pool: Pool::new(p.devices.cpu_threads(threads))?,
            world,
            frame: Frame {
                pixels: vec![0; (p.width * p.height * 4) as usize],
                width: p.width,
                height: p.height,
                stage_nodes: [g.ids(&[Role::Visibility, Role::Shading]), g.ids(&[Role::Visibility]), g.ids(&[Role::Shading])],
            },
        })
    }
}

/// A lane: the visibility node's implementation, its chunks and what it keeps.
trait Lane: Send {
    /// The chunk updates (the world applied its own).
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()>;
    /// Renders `frame.pixels` (in `pool`) of `world`.
    fn render(&mut self, frame: &mut Frame, pool: &Pool, world: &World, req: &FrameRequest) -> FrameStats;
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
    /// The knob values a compiled pipeline baked in ([`Spec::KNOBS`]).
    baked: mr_core::baked::Knobs,
    /// Compiled for this machine ([`Spec::NATIVE`]).
    native: bool,
}

impl<L: Lane> Renderer for Executor<L> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let rest = self.core.world.apply(updates);
        self.lane.update(rest)
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (core, lane) = (&mut self.core, &mut self.lane);
        core.world.prepare();
        let mut stats = core.pool.install(|| lane.render(&mut core.frame, &core.pool, &core.world, req));
        if self.native {
            // A pipeline compiled for this machine (observations tell it from the generic build).
            stats.counters.push(("compiled_native".into(), 1.0));
        }
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(&core.frame.pixels) });
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        mr_core::baked::refuse(self.baked, name, &value.to_json())?;
        if let Some(r) = self.lane.set_param(name, value) {
            return r;
        }
        match name {
            "shadows" => {
                self.core.world.shadows = cpu_reference::parse_shadows(value)?;
                Ok(())
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph value a compiled pipeline baked in cannot differ from the graph's.
    #[test]
    fn baked_graph_values_are_fixed() {
        assert!(fixed("tone", Some(Tone::Aces), Tone::Clamp).is_err());
        assert_eq!(fixed("tone", None, Tone::Clamp).unwrap(), Tone::Clamp);
    }
}
