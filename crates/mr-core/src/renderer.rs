use std::sync::Arc;

use glam::IVec3;
use serde::{Deserialize, Serialize};

use crate::{BlockId, BlockRegistry, Camera, ChunkData, ChunkKey, Environment, Light, MeshUpdate, ParamSpec, ParamValue, PassMode, RenderMode};

/// Static description of a renderer implementation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RendererInfo {
    /// Unique name used in run plans, e.g. `gpu-dda`.
    pub name: String,
    pub description: String,
    /// Free-form category tags (`cpu`, `gpu`, `raster`, `compute`, `raytrace`, ...).
    pub tags: Vec<String>,
    pub modes: Vec<RenderMode>,
    /// Highest LOD level the renderer wants streamed. 0 means full-resolution chunks only.
    pub max_lod: u8,
    /// Knobs this renderer exposes (also its accepted `options` keys).
    #[serde(default)]
    pub params: Vec<ParamSpec>,
}

impl RendererInfo {
    /// Whether this renderer can render passes of mode `m` (meshes need the `meshes` tag).
    pub fn supports(&self, m: PassMode) -> bool {
        self.modes.contains(&m.lighting) && (!m.meshes || self.tags.iter().any(|t| t == "meshes"))
    }
}

/// Everything a renderer receives at construction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitParams {
    pub width: u32,
    pub height: u32,
    pub blocks: Arc<BlockRegistry>,
    /// A starting environment; the world's arrives as `SetEnvironment` (the host sends it
    /// first).
    pub environment: Environment,
    /// Chunks within this many chunks (Chebyshev, at each LOD level) of the camera are
    /// streamed. Useful for sizing structures and the max ray distance.
    pub view_radius_chunks: u32,
    /// Highest LOD level the host streams to this renderer (each level covers the same
    /// chunk radius at twice the cell size).
    #[serde(default)]
    pub max_lod: u8,
    /// Renderer-specific options from the run plan, as a JSON object string.
    pub options_json: String,
    /// The pipeline graph to run (`mr_components::graph::Graph` as JSON),
    /// empty for renderers that are not back ends or to run the flat composition in
    /// `options_json`.
    #[serde(default)]
    pub graph_json: String,
    /// The devices to run on: a configuration from the host, not an option of the renderer.
    #[serde(default)]
    pub devices: DeviceConfig,
    /// Time each stage of a frame (`FrameStats::stages`). GPU stages cost a
    /// timestamp each (a few µs per frame).
    #[serde(default)]
    pub stage_times: bool,
    /// How the worker's client gets the LOD levels (the graph's `client`
    /// node's component).
    #[serde(default)]
    pub client: crate::client::Levels,
    /// The link the worker's client receives the stream over (free: none).
    #[serde(default)]
    pub link: crate::link::LinkSpec,
}

/// Which devices a renderer runs on (no hardware opinion in the
/// renderers). Several GPUs, or a GPU and the CPU, are a configuration; a graph's nodes
/// pick their device by slot (`gpu` = slot 0, `gpu1` = slot 1, ...: a
/// pipeline runs on its drawer's or visibility node's GPU, its deferred shading may run on
/// another).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceConfig {
    /// GPU slots: a case-insensitive substring of the adapter name (e.g. "nvidia"), or
    /// empty for the default (the discrete GPU on hybrid laptops). Missing slots take the
    /// default.
    pub gpus: Vec<String>,
    /// CPU threads for CPU stages whose own `threads` knob is 0 (0 = all cores).
    pub cpu_threads: usize,
}

impl DeviceConfig {
    /// The adapter filter of GPU slot `slot` (`None` = the default choice).
    pub fn gpu(&self, slot: usize) -> Option<&str> {
        self.gpus.get(slot).map(String::as_str).filter(|s| !s.is_empty())
    }

    /// Threads for a CPU stage: its own knob if set, else the configuration's.
    pub fn cpu_threads(&self, knob: usize) -> usize {
        if knob > 0 { knob } else { self.cpu_threads }
    }
}

/// The device resources handed to a renderer when it is created. The host owns them:
/// renderers ask for a GPU from `gpus` (an `mr_gpu::GpuPool`, kept type-erased so that
/// `mr-core` depends on no GPU API) instead of opening one, so renderers can share a
/// device, and a library caller can hand in its own.
#[derive(Clone, Default)]
pub struct Devices {
    pub gpus: Option<Arc<dyn std::any::Any + Send + Sync>>,
}

impl InitParams {
    /// Parses the renderer-specific options into `T`; missing options use `T::default()`.
    pub fn options<T: serde::de::DeserializeOwned + Default>(&self) -> anyhow::Result<T> {
        if self.options_json.trim().is_empty() || self.options_json.trim() == "{}" {
            return Ok(T::default());
        }
        Ok(serde_json::from_str(&self.options_json)?)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEdit {
    pub pos: IVec3,
    pub block: BlockId,
}

/// Incremental change to the renderer's view of the world: the only way world state reaches
/// a renderer (the strict contract). Its client hands them over in order,
/// before the [`FrameRequest`] they precede, but nothing is guaranteed to have arrived: the
/// stream crosses a link that may delay, drop and resend, or reorder it, so a chunk inside
/// the region may be missing for any number of frames, chunks arrive in any order, coarse
/// levels before fine ones. Renderers draw what they hold (holes where nothing arrived).
///
/// Chunk semantics: a chunk that is not loaded is air. A renderer only gets `SetBlocks`
/// for chunks it currently holds; an edit that makes an absent chunk non-empty arrives as
/// a `ChunkLoad`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorldUpdate {
    ChunkLoad { key: ChunkKey, data: ChunkData },
    ChunkUnload { key: ChunkKey },
    SetBlocks(Vec<BlockEdit>),
    LightUpsert(Vec<Light>),
    LightRemove(Vec<u32>),
    SetEnvironment(Environment),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameRequest {
    pub frame_id: u64,
    pub camera: Camera,
    pub mode: RenderMode,
    /// Draw meshes (mesh mode) this frame. Only sent to renderers with the `meshes` tag.
    #[serde(default)]
    pub meshes: bool,
    /// Whether the pixels must be delivered to the host. When false the renderer may
    /// skip any read-back entirely.
    pub readback: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FrameStats {
    /// CPU time the renderer spent on this frame (render-thread work, excluding waiting).
    pub cpu_ms: f32,
    /// GPU execution time if measured (timestamp queries).
    pub gpu_ms: Option<f32>,
    /// Arbitrary per-frame counters (rays cast, triangles drawn, ...).
    pub counters: Vec<(String, f64)>,
    /// What the colours mean while a diagnostic `view` is active (`None`: the image).
    #[serde(default)]
    pub legend: Option<crate::Legend>,
    /// The frame's time per stage: CPU stages as the renderer timed them,
    /// GPU stages from timestamps between them (filled in when the frame completes).
    #[serde(default)]
    pub stages: Vec<StageTime>,
}

/// One stage of a frame and the graph nodes it ran: callers only, since fused nodes
/// (storages, kernels) run inside their callers and are timed with them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StageTime {
    pub name: String,
    pub nodes: Vec<String>,
    // (No `skip_serializing_if`: postcard, the worker protocol, is not self-describing.)
    pub cpu_ms: Option<f32>,
    pub gpu_ms: Option<f32>,
}

impl StageTime {
    pub fn cpu(name: &str, nodes: &[String], ms: f32) -> Self {
        Self { name: name.into(), nodes: nodes.to_vec(), cpu_ms: Some(ms), gpu_ms: None }
    }
}

pub struct CompletedFrame<'a> {
    pub frame_id: u64,
    pub stats: FrameStats,
    /// RGBA8 sRGB, top row first, tightly packed (`width * 4` bytes per row). Present
    /// whenever the request asked for read-back.
    pub pixels: Option<&'a [u8]>,
}

pub trait FrameSink {
    fn complete(&mut self, frame: CompletedFrame<'_>);
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryReport {
    pub cpu_bytes: u64,
    pub gpu_bytes: u64,
}

/// Lets a renderer's background threads (e.g. a GPU fence waiter) ask the worker to call
/// [`Renderer::poll`] as soon as possible.
#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    pub fn noop() -> Self {
        Self::new(|| {})
    }

    pub fn wake(&self) {
        (self.0)()
    }
}

/// A renderer under test. Lives on a single thread inside a worker process, but may spawn
/// whatever internal threads/GPU queues it likes.
///
/// Frames may complete asynchronously: `render` may return before the frame is done, and
/// completes any number of earlier frames through the sink (in submission order).
/// Asynchronous renderers must call the [`Waker`] they were created with when frames may
/// have finished; the worker then calls `poll`. `flush` must block until every submitted
/// frame has completed.
pub trait Renderer {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()>;
    /// Mesh geometry and instances (mesh mode). Only renderers with the `meshes` tag get
    /// these; the default ignores them.
    fn update_meshes(&mut self, _update: MeshUpdate) -> anyhow::Result<()> {
        Ok(())
    }
    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    /// Changes a `live` parameter between frames. Values arrive already clamped to the
    /// parameter's declared domain.
    fn set_param(&mut self, name: &str, _value: &ParamValue) -> anyhow::Result<()> {
        anyhow::bail!("parameter `{name}` cannot change while running")
    }

    /// Complete any finished frames without blocking.
    fn poll(&mut self, _sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        Ok(())
    }
    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()>;
    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()>;
    fn memory(&self) -> MemoryReport {
        MemoryReport::default()
    }
}

/// How the worker discovers and constructs a renderer.
pub struct RendererEntry {
    pub info: fn() -> RendererInfo,
    pub create: fn(InitParams, &Devices, Waker) -> anyhow::Result<Box<dyn Renderer>>,
}
