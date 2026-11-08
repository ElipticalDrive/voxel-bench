//! The CPU executor: every pipeline whose image is made on the CPU, each run as passes of the
//! generic node executor ([`exec`]):
//! - ray casting ([`raycast`]): one primary ray per pixel walked over a storage, monomorphised
//!   per storage (`raycast-cpu`);
//! - rasterisation ([`raster`]): meshed quads drawn into a visibility buffer (`raster-cpu`);
//! - columns ([`columns`]): the Voxel Space heightmap march (`columns-cpu`).
//!
//! Which nodes are built is the graph's: rasterisation when a drawer draws faces, else by the
//! visibility node's component. Each pipeline declares the nodes it builds
//! (`graph::CPU_RAYCAST`, `CPU_RASTER`, `CPU_COLUMNS`); `Graph::accept` refuses any other graph
//! with the reason. The executor owns the thread pool every frame runs in (the
//! `threads` knob), the world (`mr_reference::World`: blocks, environment, lights, the
//! `shadows` knob) and the image size; the chunks are each pipeline's representation (its
//! storage or geometry node's output).
//!
//! Every pipeline is also compiled on demand for one graph (the host's
//! `pipelines` generates a crate naming the entry, its storage type, its pixel and a [`Spec`]):
//! its choices are then constants the compiler folds, and the crate is built for this CPU.
//!
//! Every pipeline is monomorphised over the output's [`Pixel`]: RGBA8 (`[u8; 4]`), or RGBA16F
//! ([`Half4`]) for the `hdr` post. The precompiled build holds both and picks by the post.

use mr_reference::World;
use mr_components::graph::{self, Role};
use mr_core::*;

mod columns;
mod exec;
mod perf;
mod raster;
mod raycast;

pub use mr_reference::Shadows;
pub use mr_core::lighting::{Half4, Pixel, Tone};
use mr_core::lighting::PixelFormat;
pub use raycast::{Traversal, level_mask, shadow_kernel, storage_type, traversal};

/// What a pipeline compiled for one graph bakes in: a generated crate
/// implements this with the graph's values, so they are constants the compiler folds (no
/// per-ray `match`, one packet width, no ISA dispatch: the whole crate is compiled for this
/// CPU). [`Generic`], the precompiled build, leaves everything open (`None`: read at run
/// time). A pipeline reads the constants it folds; [`Spec::KNOBS`] is what the worker refuses to
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

/// A value the pipeline folds when `S` baked it, else its run-time one; creation fails if a baked
/// graph value (tone, shadow kernel) differs from the graph's.
fn fixed<T: PartialEq + std::fmt::Debug>(what: &str, baked: Option<T>, actual: T) -> anyhow::Result<T> {
    match baked {
        Some(b) if b != actual => anyhow::bail!("this pipeline was compiled for {what} {b:?} (not {actual:?}); a change needs another build"),
        _ => Ok(actual),
    }
}

/// The CPU executor: the pipeline its graph picks.
pub fn entry() -> RendererEntry {
    RendererEntry { name: "cpu", info: |p| Ok(info(graph::of_init("cpu", p)?.preset.expect("of_init: a preset"))), create: |p, _, _| create(p) }
}

/// The knobs of every pipeline, by preset.
pub fn infos() -> Vec<RendererInfo> {
    vec![raster::info(), raycast::info(), columns::info()]
}

/// The knobs of pipeline `preset`.
pub fn info(preset: &str) -> RendererInfo {
    match preset {
        raycast::PRESET => raycast::info(),
        raster::PRESET => raster::info(),
        columns::PRESET => columns::info(),
        other => panic!("`{other}` is not run by the CPU executor"),
    }
}

/// Ray casting compiled for one graph (`S`, storage `R`, pixel `P`); the generated crate's
/// worker serves only this.
pub fn raycast_entry<R: mr_components::storage::cpu::Repr, S: Spec, P: Pixel>() -> RendererEntry {
    RendererEntry {
        name: "cpu",
        info: |_| Ok(raycast::info()),
        create: |p, _, _| {
            mr_core::baked::check(S::KNOBS, &p, &raycast::info().params)?;
            raycast::executor::<R, S, P>(&p, &graph::of_init("cpu", &p)?)
        },
    }
}

/// The rasterisation pipeline compiled for one graph.
pub fn raster_entry<S: Spec, P: Pixel>() -> RendererEntry {
    RendererEntry {
        name: "cpu",
        info: |_| Ok(raster::info()),
        create: |p, _, _| {
            mr_core::baked::check(S::KNOBS, &p, &raster::info().params)?;
            raster::executor::<S, P>(&p, &graph::of_init("cpu", &p)?)
        },
    }
}

/// The columns pipeline compiled for one graph.
pub fn columns_entry<S: Spec, P: Pixel>() -> RendererEntry {
    RendererEntry {
        name: "cpu",
        info: |_| Ok(columns::info()),
        create: |p, _, _| {
            mr_core::baked::check(S::KNOBS, &p, &columns::info().params)?;
            columns::executor::<S, P>(&p, &graph::of_init("cpu", &p)?)
        },
    }
}

/// The executor for `p`'s graph, writing the post's pixel format.
fn create(p: InitParams) -> anyhow::Result<Box<dyn Renderer>> {
    let g = graph::of_init("cpu", &p)?;
    match g.tone().format() {
        PixelFormat::Rgba8 => create_as::<[u8; 4]>(&p, &g),
        PixelFormat::Rgba16F => create_as::<Half4>(&p, &g),
    }
}

/// Rasterisation when a drawer draws faces, else by the visibility node's component.
fn create_as<P: Pixel>(p: &InitParams, g: &graph::Checked) -> anyhow::Result<Box<dyn Renderer>> {
    if g.find(Role::Draw).is_some() {
        return raster::executor::<Generic, P>(p, g);
    }
    let vis = g.find(Role::Visibility).ok_or_else(|| anyhow::anyhow!("no visibility node"))?;
    match g.node(vis).component.as_str() {
        "columns" => columns::executor::<Generic, P>(p, g),
        _ => raycast::create::<P>(p, g),
    }
}

/// The world for graph `g` with the `shadows` knob, its pixels `P` the post's.
fn world<P: Pixel>(p: &InitParams, g: &graph::Checked, shadows: Shadows) -> anyhow::Result<World> {
    let format = g.tone().format();
    anyhow::ensure!(P::FORMAT == format, "this pipeline writes {:?} pixels, the post `{}` {format:?}", P::FORMAT, g.tone().name());
    let mut world = World::new(p);
    world.shadows = shadows;
    world.tone = g.tone();
    Ok(world)
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

    /// The valid graphs one or two edits from the pipelines' own are either refused by what
    /// their executor declares, with the reason, or built (with the preset options the graph
    /// spells, as the host sends them); `accept` takes exactly the built ones.
    #[test]
    fn executors_build_what_they_declare() {
        let (mut built, mut refused) = (0, 0);
        for ck in graph::near(&[raycast::PRESET, raster::PRESET, columns::PRESET]) {
            let e = ck.preset.unwrap();
            let key = ck.graph.canonical().unwrap();
            if let Err(why) = ck.built() {
                assert!(ck.graph.accept().is_err(), "`{e}` refuses a graph `accept` takes ({why}): {key}");
                refused += 1;
                continue;
            }
            ck.graph.accept().unwrap_or_else(|why| panic!("`{e}` builds a graph `accept` refuses ({why}): {key}"));
            let opts = ck.graph.preset_options().unwrap();
            let p = InitParams {
                width: 32,
                height: 16,
                blocks: Default::default(),
                environment: Default::default(),
                view_radius_chunks: 1,
                max_lod: 0,
                options_json: serde_json::to_string(&opts).unwrap(),
                graph_json: serde_json::to_string(&ck.graph).unwrap(),
                devices: DeviceConfig { gpus: vec![], cpu_threads: 1 },
                stage_times: false,
                client: Default::default(),
                link: Default::default(),
            };
            let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| create(p).map(drop)));
            match made {
                Ok(Ok(())) => {}
                Ok(Err(why)) => panic!("`{e}` fails building {key}: {why}"),
                Err(_) => panic!("`{e}` panics building {key}"),
            }
            built += 1;
        }
        assert!(built > 0 && refused > 0, "{built} built, {refused} refused");
    }
}
