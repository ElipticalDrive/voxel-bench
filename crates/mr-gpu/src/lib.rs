//! Shared wgpu plumbing for GPU renderers: adapter selection (prefers the discrete GPU on
//! hybrid laptops), and a frame ring that handles asynchronous completion, read-back of
//! the output texture and GPU timestamps.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, bail};
use mr_core::{CompletedFrame, FrameSink, FrameStats, StageTime, Waker};
use mr_core::lighting::PixelFormat;

pub mod meshes;
pub mod program;
pub mod voxels;

pub use wgpu;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    pub timestamps: bool,
    /// The adapter's features (what a device on it could have been opened with).
    adapter_features: wgpu::Features,
    /// Opened with wgpu's experimental features.
    experimental: bool,
    /// The first failure: an uncaptured wgpu error (validation, OOM, ...) or data a holder
    /// could not keep ([`Gpu::fail`]). Any failure fails the renderer: one with an invalid
    /// pipeline or an incomplete world must not report a frame rate.
    failure: Failure,
    /// The persistent pipeline cache, once opened (`open_cache`).
    cache: std::sync::OnceLock<DiskCache>,
}

/// A driver pipeline cache kept in a file (Vulkan's `VkPipelineCache` through wgpu).
struct DiskCache {
    cache: wgpu::PipelineCache,
    path: std::path::PathBuf,
    /// Bytes last loaded or saved (saving skips an unchanged cache).
    saved: std::sync::Mutex<usize>,
}

pub struct GpuRequest<'a> {
    /// Enable wgpu's experimental features (required for ray queries).
    pub experimental: bool,
    /// Case-insensitive substring of the adapter name to use (e.g. "nvidia").
    pub adapter: Option<&'a str>,
    pub required_features: wgpu::Features,
    pub optional_features: wgpu::Features,
    /// Adjust the device limits given the adapter's limits.
    pub limits: &'a dyn Fn(&wgpu::Limits) -> wgpu::Limits,
}

impl Gpu {
    pub fn new(req: GpuRequest) -> anyhow::Result<Self> {
        let adapter = pick_adapter(req.adapter)?;
        let info = adapter.get_info();
        tracing::info!("using adapter {} ({:?}, {:?}, driver {})", info.name, info.device_type, info.backend, info.driver_info);
        let available = adapter.features();
        if !available.contains(req.required_features) {
            bail!("adapter lacks features {:?}", req.required_features - available);
        }
        // The pipeline cache where the adapter has one (`open_cache`).
        let features = req.required_features | ((req.optional_features | wgpu::Features::PIPELINE_CACHE) & available);
        let limits = (req.limits)(&adapter.limits());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("mr-gpu"),
            required_features: features,
            required_limits: limits,
            // SAFETY: opting into wgpu's experimental API surface (ray queries); the
            // renderers that ask for it are prepared for driver-level issues.
            experimental_features: if req.experimental { unsafe { wgpu::ExperimentalFeatures::enabled() } } else { wgpu::ExperimentalFeatures::disabled() },
            ..Default::default()
        }))?;
        let mut gpu = Self::from_parts(device, queue, info);
        gpu.adapter_features = available;
        gpu.experimental = req.experimental;
        Ok(gpu)
    }

    /// Wraps a device someone else opened (a library caller's): back ends render with it as
    /// long as it has the features they need.
    pub fn from_parts(device: wgpu::Device, queue: wgpu::Queue, info: wgpu::AdapterInfo) -> Self {
        let failure = Failure::default();
        let f = failure.clone();
        device.on_uncaptured_error(Arc::new(move |e| f.set(format!("wgpu error: {}", e.to_string().lines().take(12).collect::<Vec<_>>().join("\n")))));
        let features = device.features();
        Self { device, queue, info, timestamps: features.contains(wgpu::Features::TIMESTAMP_QUERY), adapter_features: features, experimental: false, failure, cache: Default::default() }
    }

    /// Opens the persistent pipeline cache `name` (the pipelines' graph key): a file per
    /// adapter and name under [`mr_core::cache_dir`], so the driver skips compiling pipelines
    /// it compiled in an earlier run. The first call in a process wins; no-op
    /// where the device has no pipeline cache (wgpu: Vulkan only) or `$MR_GPU_CACHE` is `0`.
    pub fn open_cache(&self, name: &str) {
        if !self.device.features().contains(wgpu::Features::PIPELINE_CACHE) || std::env::var_os("MR_GPU_CACHE").is_some_and(|v| v == "0") {
            return;
        }
        let Some(adapter) = wgpu::util::pipeline_cache_key(&self.info) else { return };
        let path = mr_core::cache_dir().join("driver").join(adapter).join(format!("{name}.bin"));
        let data = std::fs::read(&path).ok();
        let saved = data.as_ref().map_or(0, Vec::len);
        let _ = self.cache.get_or_init(|| {
            // SAFETY: the data is what `get_data` wrote for this adapter (the file is keyed by
            // it); wgpu and the driver validate the header (vendor, device, driver version, wgpu
            // version) and fall back to an empty cache on a mismatch.
            let cache = unsafe { self.device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor { label: Some(name), data: data.as_deref(), fallback: true }) };
            DiskCache { cache, path, saved: std::sync::Mutex::new(saved) }
        });
    }

    /// The pipeline cache every pipeline is created with (`None`: not opened).
    pub fn cache(&self) -> Option<&wgpu::PipelineCache> {
        self.cache.get().map(|c| &c.cache)
    }

    /// Writes the pipeline cache if it grew (atomically: a temporary file renamed over it,
    /// so workers of one graph running at once never leave a torn file).
    pub fn save_cache(&self) {
        let Some(c) = self.cache.get() else { return };
        let Some(data) = c.cache.get_data() else { return };
        let mut saved = c.saved.lock().unwrap();
        if data.len() == *saved {
            return;
        }
        let tmp = c.path.with_extension(format!("{}.tmp", std::process::id()));
        let written = c.path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&tmp, &data)).and_then(|_| std::fs::rename(&tmp, &c.path));
        match written {
            Ok(()) => *saved = data.len(),
            Err(e) => tracing::warn!("pipeline cache {}: {e}", c.path.display()),
        }
    }

    /// Why this device cannot serve `needs`, if it can't.
    pub fn lacks(&self, needs: &GpuNeeds) -> Option<String> {
        let have = self.device.features();
        let want = needs.required_features | (needs.optional_features & self.adapter_features);
        if !have.contains(want) {
            return Some(format!("device on {} lacks features {:?}", self.info.name, want - have));
        }
        (needs.experimental && !self.experimental).then(|| format!("device on {} was opened without wgpu's experimental features", self.info.name))
    }

    /// Fails if anything failed since the device was created (every frame starts and
    /// completes through this).
    pub fn check(&self) -> anyhow::Result<()> {
        match self.failure.0.lock().unwrap().as_ref() {
            Some(e) => bail!("{e}"),
            None => Ok(()),
        }
    }

    /// Sends the queue's pending writes and waits for them, so their staging memory is freed:
    /// for uploads too large to stage at once (`write_buffer` holds every write until the next
    /// submission).
    pub fn flush_writes(&self) {
        self.queue.submit([]);
        if let Err(e) = self.device.poll(wgpu::PollType::wait_indefinitely()) {
            self.fail(format!("waiting for uploads failed: {e}"));
        }
    }

    /// Records that frames from here on would be wrong (`why`: what was lost and the knob
    /// that would hold it): data that did not fit a buffer, a table out of slots, a failed
    /// read-back. The renderer fails with it at its next frame; nothing is dropped quietly.
    pub fn fail(&self, why: impl Into<String>) {
        self.failure.set(why.into());
    }

    /// A buffer of `size` bytes, checked against what this device allows: over
    /// `max_buffer_size`, or over `max_storage_buffer_binding_size` when `usage` binds it as
    /// storage, it fails the device naming the label, the size, the limit, the adapter and
    /// `set_by` (what sets the size: a knob, the resolution, the world), and returns a
    /// 16-byte placeholder (nothing renders past the failure; what the caller then does with
    /// it at most adds wgpu errors after the named one, which stays first).
    /// Every storage buffer is created here (`tests/architecture.rs`): a node cannot assume
    /// one GPU's big bindings.
    pub fn storage(&self, label: &str, size: u64, usage: wgpu::BufferUsages, set_by: impl FnOnce() -> String) -> wgpu::Buffer {
        let limits = self.device.limits();
        let (limit, name) = if usage.contains(wgpu::BufferUsages::STORAGE) && limits.max_storage_buffer_binding_size < limits.max_buffer_size {
            (limits.max_storage_buffer_binding_size, "max_storage_buffer_binding_size")
        } else {
            (limits.max_buffer_size, "max_buffer_size")
        };
        let size = size.max(16).next_multiple_of(4);
        let size = if size > limit {
            self.fail(over_limit(label, size, limit, name, &self.info.name, &set_by()));
            16
        } else {
            size
        };
        self.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
    }

    /// The failure slot, for callbacks that outlive a borrow of the device.
    pub fn failure(&self) -> Failure {
        self.failure.clone()
    }
}

/// A device's first failure ([`Gpu::fail`]).
#[derive(Clone, Default)]
pub struct Failure(Arc<std::sync::Mutex<Option<String>>>);

impl Failure {
    pub fn set(&self, why: String) {
        let mut first = self.0.lock().unwrap();
        if first.is_none() {
            tracing::error!("{why}");
            *first = Some(why);
        }
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        self.save_cache();
    }
}

/// Compiles assembled WGSL. With `MR_DUMP_WGSL=dir` set, each source is also written to
/// `dir/<label>-<hash>.wgsl`, so two builds' shaders can be compared (zero-cost checks: a
/// refactor that must not change what runs leaves the set of files identical).
pub fn shader(dev: &wgpu::Device, label: &str, src: String) -> wgpu::ShaderModule {
    if let Some(dir) = std::env::var_os("MR_DUMP_WGSL") {
        dump_wgsl(&dir, label, &src);
    }
    dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(src.into()) })
}

/// Writes `src` to `dir/<label>-<hash>.wgsl` (`MR_DUMP_WGSL`).
pub(crate) fn dump_wgsl(dir: &std::ffi::OsStr, label: &str, src: &str) {
    // FNV-1a: stable across builds and runs (unlike std's hasher).
    let h = src.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let path = std::path::Path::new(dir).join(format!("{}-{h:016x}.wgsl", label.replace([' ', '/'], "_")));
    if let Err(e) = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&path, src)) {
        tracing::warn!("MR_DUMP_WGSL: {}: {e}", path.display());
    }
}

/// What a back end needs of a GPU device (all of them take the adapter's limits).
#[derive(Clone, Debug, Default)]
pub struct GpuNeeds {
    /// wgpu's experimental features (ray queries).
    pub experimental: bool,
    pub required_features: wgpu::Features,
    /// Used when the adapter has them.
    pub optional_features: wgpu::Features,
}

/// A worker process's devices, handed to its renderer (`mr_serve::run`): GPUs open on
/// request (CPU pipelines open none), in the slots the host's configuration names.
pub fn devices(cfg: &mr_core::DeviceConfig) -> mr_core::Devices {
    mr_core::Devices { gpus: Some(Arc::new(GpuPool::new(&cfg.gpus))) }
}

/// The GPU devices of one process: handed to renderers through
/// `mr_core::Devices`, indexed by slot (the configuration's `DeviceConfig::gpus`). A
/// request is served by an open device of that slot that has what it needs, else by a new
/// device opened with them; renderers in one process (thread isolation, a library) share
/// devices that way. A fixed pool (a library caller's devices) never opens one: a request
/// its devices cannot serve fails with the reason.
pub struct GpuPool {
    adapters: Vec<String>,
    open: std::sync::Mutex<Vec<(usize, Arc<Gpu>)>>,
    fixed: bool,
}

impl GpuPool {
    /// Devices opened on request; slot `k` on the adapter matching `adapters[k]` (empty or
    /// missing: the default choice).
    pub fn new(adapters: &[String]) -> Self {
        Self { adapters: adapters.to_vec(), open: Default::default(), fixed: false }
    }

    /// Exactly these devices, slot `k` = `gpus[k]`.
    pub fn fixed(gpus: Vec<Arc<Gpu>>) -> Self {
        Self { adapters: vec![], open: std::sync::Mutex::new(gpus.into_iter().enumerate().collect()), fixed: true }
    }

    /// A device of slot `slot` that serves `needs`.
    pub fn gpu(&self, slot: usize, needs: &GpuNeeds) -> anyhow::Result<Arc<Gpu>> {
        let mut open = self.open.lock().unwrap();
        let mut why = vec![];
        for (s, g) in open.iter().filter(|(s, _)| *s == slot) {
            match g.lacks(needs) {
                None => return Ok(g.clone()),
                Some(w) => why.push(format!("slot {s}: {w}")),
            }
        }
        if self.fixed {
            bail!("no handed-in GPU device serves this renderer{}", if why.is_empty() { format!(" (no device in slot {slot})") } else { format!(": {}", why.join("; ")) });
        }
        let adapter = self.adapters.get(slot).map(String::as_str).filter(|a| !a.is_empty());
        let gpu = Arc::new(Gpu::new(GpuRequest { experimental: needs.experimental, adapter, required_features: needs.required_features, optional_features: needs.optional_features, limits: &|a| a.clone() })?);
        open.push((slot, gpu.clone()));
        Ok(gpu)
    }
}

/// The GPU of slot `slot` from the devices handed to a renderer.
pub fn gpu(devices: &mr_core::Devices, slot: usize, needs: &GpuNeeds) -> anyhow::Result<Arc<Gpu>> {
    let pool = devices.gpus.as_ref().and_then(|p| p.downcast_ref::<GpuPool>()).context("no GPU devices were handed to this renderer")?;
    pool.gpu(slot, needs)
}

struct Slot {
    readback: wgpu::Buffer,
    ts_resolve: Option<wgpu::Buffer>,
    ts_read: Option<wgpu::Buffer>,
    /// The stage marks' timestamps (resolved, then read back).
    marks_resolve: Option<wgpu::Buffer>,
    marks_read: Option<wgpu::Buffer>,
}

struct Pending {
    frame_id: u64,
    slot: usize,
    readback: bool,
    remaining: Arc<AtomicU32>,
    stats: FrameStats,
    /// The frame's GPU stages, timed from mark `k` to mark `k + 1`.
    stages: Vec<StageTime>,
}

/// Timestamps per frame at the boundaries between GPU stages (at most `MAX_MARKS - 1`
/// stages per frame).
const MAX_MARKS: u32 = 16;

/// Index of the frame-ring slot a frame is recorded into.
#[derive(Copy, Clone)]
pub struct Ticket(usize);

/// Frames in flight on the GPU, completed in order.
pub struct FrameRing {
    slots: Vec<Slot>,
    free: Vec<usize>,
    pending: VecDeque<Pending>,
    query_set: Option<wgpu::QuerySet>,
    /// Stage marks: `MAX_MARKS` per slot.
    marks: Option<wgpu::QuerySet>,
    /// The GPU stages of the frame being recorded.
    recording: std::sync::Mutex<Vec<StageTime>>,
    /// The frame being recorded started its clock before its passes ([`Self::start_clock`]):
    /// passes asking to begin it do not.
    clock_started: std::sync::atomic::AtomicBool,
    timestamp_period: f32,
    width: u32,
    height: u32,
    /// Bytes per pixel (the output's format).
    bpp: u32,
    padded_row: u32,
    pixels: Vec<u8>,
    /// Signals the waiter thread that work was submitted. `None` once shutting down.
    submitted: Option<std::sync::mpsc::Sender<()>>,
    waiter: Option<std::thread::JoinHandle<()>>,
}

impl FrameRing {
    /// `waker` is called from a background thread whenever submitted work completes;
    /// `format` is the output's.
    pub fn new(gpu: &Gpu, width: u32, height: u32, format: PixelFormat, depth: usize, waker: Waker) -> Self {
        let bpp = format.bytes();
        let (submitted, rx) = std::sync::mpsc::channel::<()>();
        let device = gpu.device.clone();
        let failure = gpu.failure();
        let waiter = std::thread::Builder::new()
            .name("gpu-waiter".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    while rx.try_recv().is_ok() {}
                    // Fires map/work-done callbacks for everything submitted so far.
                    if let Err(e) = device.poll(wgpu::PollType::wait_indefinitely()) {
                        failure.set(format!("gpu wait failed: {e}"));
                    }
                    waker.wake();
                }
            })
            .expect("spawn gpu-waiter");
        let padded_row = (width * bpp).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let query_set = gpu.timestamps.then(|| {
            gpu.device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("frame timestamps"), ty: wgpu::QueryType::Timestamp, count: depth as u32 * 2 })
        });
        let marks = gpu.timestamps.then(|| {
            gpu.device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("stage marks"), ty: wgpu::QueryType::Timestamp, count: depth as u32 * MAX_MARKS })
        });
        let buffer = |label, usage| gpu.timestamps.then(|| gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: MAX_MARKS as u64 * 8, usage, mapped_at_creation: false }));
        let slots = (0..depth)
            .map(|_| Slot {
                marks_resolve: buffer("marks resolve", wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
                marks_read: buffer("marks read", wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
                readback: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("readback"),
                    size: padded_row as u64 * height as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                ts_resolve: gpu.timestamps.then(|| {
                    gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("ts resolve"),
                        size: 16,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    })
                }),
                ts_read: gpu.timestamps.then(|| {
                    gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("ts read"),
                        size: 16,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    })
                }),
            })
            .collect();
        Self {
            slots,
            free: (0..depth).rev().collect(),
            pending: VecDeque::new(),
            query_set,
            marks,
            recording: Default::default(),
            clock_started: Default::default(),
            timestamp_period: gpu.queue.get_timestamp_period(),
            width,
            height,
            bpp,
            padded_row,
            pixels: vec![0; (width * height * bpp) as usize],
            submitted: Some(submitted),
            waiter: Some(waiter),
        }
    }

    /// Reserves a slot for a new frame, waiting for (and completing) old frames if all
    /// slots are busy.
    pub fn begin(&mut self, gpu: &Gpu, sink: &mut dyn FrameSink) -> anyhow::Result<Ticket> {
        gpu.check()?;
        self.poll(gpu, sink, false)?;
        while self.free.is_empty() {
            gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
            self.poll(gpu, sink, false)?;
        }
        self.clock_started.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(Ticket(self.free.pop().unwrap()))
    }

    /// Starts frame `t`'s clock now, on `enc` (before work outside passes: acceleration
    /// structure builds): a timestamp in the encoder where the device writes them there, else
    /// an empty pass. The frame's passes asking to begin it then do not.
    pub fn start_clock(&self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, t: Ticket) {
        let Some((q, begin, _)) = self.timestamps(t) else { return };
        if gpu.device.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
            enc.write_timestamp(q, begin);
        } else {
            enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("frame begin"), timestamp_writes: self.compute_timestamps(t, true, false) });
        }
        self.clock_started.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Whether a pass asking to begin the frame's clock does (not after `start_clock`).
    fn begins(&self, begin: bool) -> bool {
        begin && !self.clock_started.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// `(query_set, begin_index, end_index)` for this frame's timestamps, if supported.
    /// Write `begin` in the first pass of the frame and `end` in the last.
    pub fn timestamps(&self, t: Ticket) -> Option<(&wgpu::QuerySet, u32, u32)> {
        self.query_set.as_ref().map(|q| (q, t.0 as u32 * 2, t.0 as u32 * 2 + 1))
    }

    pub fn compute_timestamps(&self, t: Ticket, begin: bool, end: bool) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        let begin = self.begins(begin);
        // A pass writing neither is no timestamp pass (wgpu rejects one without indices).
        self.timestamps(t).filter(|_| begin || end).map(|(q, b, e)| wgpu::ComputePassTimestampWrites {
            query_set: q,
            beginning_of_pass_write_index: begin.then_some(b),
            end_of_pass_write_index: end.then_some(e),
        })
    }

    pub fn render_timestamps(&self, t: Ticket, begin: bool, end: bool) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let begin = self.begins(begin);
        // A pass writing neither is no timestamp pass (wgpu rejects one without indices).
        self.timestamps(t).filter(|_| begin || end).map(|(q, b, e)| wgpu::RenderPassTimestampWrites {
            query_set: q,
            beginning_of_pass_write_index: begin.then_some(b),
            end_of_pass_write_index: end.then_some(e),
        })
    }

    /// Times stages (`stage` marks them) or not (`stage` does nothing).
    pub fn with_stages(mut self, on: bool) -> Self {
        if !on {
            self.marks = None;
        }
        self
    }

    pub fn stages(&self) -> bool {
        self.marks.is_some()
    }

    /// Starts GPU stage `name`, running graph nodes `nodes`, on `enc`: a timestamp
    /// between the commands before (the previous stage's) and after. The last stage ends
    /// when the frame is submitted. Commands before the first stage are not attributed.
    /// Costs an empty pass per stage; nothing without timestamp support.
    pub fn stage(&self, enc: &mut wgpu::CommandEncoder, t: Ticket, name: &str, nodes: &[String]) {
        let Some(q) = &self.marks else { return };
        let mut rec = self.recording.lock().unwrap();
        if rec.len() as u32 + 1 >= MAX_MARKS {
            debug_assert!(false, "more than {} GPU stages in a frame", MAX_MARKS - 1);
            return;
        }
        Self::mark(enc, q, t.0 as u32 * MAX_MARKS + rec.len() as u32);
        rec.push(StageTime { name: name.into(), nodes: nodes.to_vec(), cpu_ms: None, gpu_ms: None });
    }

    fn mark(enc: &mut wgpu::CommandEncoder, q: &wgpu::QuerySet, index: u32) {
        enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("stage mark"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites { query_set: q, beginning_of_pass_write_index: Some(index), end_of_pass_write_index: None }),
        });
    }

    /// Records read-back of `output` (an RGBA8 texture of the ring's size) if requested,
    /// submits the encoder, and tracks the frame until the GPU is done with it. Returns the
    /// submission (to wait for it alone).
    pub fn submit(
        &mut self,
        gpu: &Gpu,
        mut encoder: wgpu::CommandEncoder,
        t: Ticket,
        output: &wgpu::Texture,
        frame_id: u64,
        readback: bool,
        stats: FrameStats,
    ) -> wgpu::SubmissionIndex {
        let slot = &self.slots[t.0];
        if readback {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: output, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo {
                    buffer: &slot.readback,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.padded_row), rows_per_image: None },
                },
                wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            );
        }
        let stages = std::mem::take(&mut *self.recording.lock().unwrap());
        if let (false, Some(q), Some(res), Some(read)) = (stages.is_empty(), &self.marks, &slot.marks_resolve, &slot.marks_read) {
            let b = t.0 as u32 * MAX_MARKS;
            let n = stages.len() as u32 + 1;
            Self::mark(&mut encoder, q, b + n - 1);
            encoder.resolve_query_set(q, b..b + n, res, 0);
            encoder.copy_buffer_to_buffer(res, 0, read, 0, n as u64 * 8);
        }
        if let (Some(q), Some(res), Some(read)) = (&self.query_set, &slot.ts_resolve, &slot.ts_read) {
            let b = t.0 as u32 * 2;
            encoder.resolve_query_set(q, b..b + 2, res, 0);
            encoder.copy_buffer_to_buffer(res, 0, read, 0, 16);
        }
        let index = gpu.queue.submit([encoder.finish()]);

        let remaining = Arc::new(AtomicU32::new(0));
        let track = |buf: &wgpu::Buffer| {
            remaining.fetch_add(1, Ordering::SeqCst);
            let r = remaining.clone();
            let failure = gpu.failure();
            buf.map_async(wgpu::MapMode::Read, .., move |res| {
                if let Err(e) = res {
                    failure.set(format!("frame read-back failed: {e}"));
                }
                r.fetch_sub(1, Ordering::SeqCst);
            });
        };
        if readback {
            track(&slot.readback);
        }
        if let Some(read) = &slot.ts_read {
            track(read);
        }
        if let (false, Some(read)) = (stages.is_empty(), &slot.marks_read) {
            track(read);
        }
        if remaining.load(Ordering::SeqCst) == 0 {
            remaining.fetch_add(1, Ordering::SeqCst);
            let r = remaining.clone();
            gpu.queue.on_submitted_work_done(move || {
                r.fetch_sub(1, Ordering::SeqCst);
            });
        }
        self.pending.push_back(Pending { frame_id, slot: t.0, readback, remaining, stats, stages });
        if let Some(s) = &self.submitted {
            let _ = s.send(());
        }
        index
    }

    /// Completes finished frames (in order). With `wait`, blocks until all are done.
    pub fn poll(&mut self, gpu: &Gpu, sink: &mut dyn FrameSink, wait: bool) -> anyhow::Result<()> {
        gpu.check()?;
        if self.pending.is_empty() {
            return Ok(());
        }
        gpu.device.poll(if wait { wgpu::PollType::wait_indefinitely() } else { wgpu::PollType::Poll })?;
        while let Some(p) = self.pending.front() {
            if p.remaining.load(Ordering::SeqCst) != 0 {
                if wait {
                    gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
                    continue;
                }
                break;
            }
            let mut p = self.pending.pop_front().unwrap();
            let slot = &self.slots[p.slot];
            if let Some(read) = &slot.ts_read {
                {
                    let v = read.get_mapped_range(..)?;
                    let ts: [u64; 2] = [u64::from_le_bytes(v[0..8].try_into()?), u64::from_le_bytes(v[8..16].try_into()?)];
                    // Added to what the frame brought (a pipeline over two GPUs: the other's).
                    if ts[1] > ts[0] {
                        p.stats.gpu_ms = Some(p.stats.gpu_ms.unwrap_or(0.0) + (ts[1] - ts[0]) as f32 * self.timestamp_period / 1e6);
                    }
                }
                read.unmap();
            }
            if let (false, Some(read)) = (p.stages.is_empty(), &slot.marks_read) {
                {
                    let v = read.get_mapped_range(..)?;
                    let ts = |k: usize| u64::from_le_bytes(v[k * 8..k * 8 + 8].try_into().unwrap());
                    for (k, mut s) in std::mem::take(&mut p.stages).into_iter().enumerate() {
                        let (a, b) = (ts(k), ts(k + 1));
                        s.gpu_ms = Some(if b > a { (b - a) as f32 * self.timestamp_period / 1e6 } else { 0.0 });
                        p.stats.stages.push(s);
                    }
                }
                read.unmap();
            }
            if p.readback {
                let t0 = std::time::Instant::now();
                {
                    let v = slot.readback.get_mapped_range(..)?;
                    let row = (self.width * self.bpp) as usize;
                    for y in 0..self.height as usize {
                        let src = y * self.padded_row as usize;
                        self.pixels[y * row..(y + 1) * row].copy_from_slice(&v[src..src + row]);
                    }
                }
                slot.readback.unmap();
                p.stats.counters.push(("readback_copy_ms".into(), t0.elapsed().as_secs_f64() * 1e3));
            }
            self.free.push(p.slot);
            sink.complete(CompletedFrame { frame_id: p.frame_id, stats: p.stats, pixels: p.readback.then_some(&self.pixels[..]) });
        }
        Ok(())
    }

    pub fn in_flight(&self) -> usize {
        self.pending.len()
    }
}

impl Drop for FrameRing {
    /// Joins the waiter thread so no thread is still inside the driver at process exit.
    fn drop(&mut self) {
        self.submitted = None;
        if let Some(t) = self.waiter.take() {
            let _ = t.join();
        }
    }
}

/// The texture format of output pixels in `f`.
pub fn output_format(f: PixelFormat) -> wgpu::TextureFormat {
    match f {
        PixelFormat::Rgba8 => wgpu::TextureFormat::Rgba8Unorm,
        PixelFormat::Rgba16F => wgpu::TextureFormat::Rgba16Float,
    }
}

/// Creates the output texture renderers draw into (storage + render target + copy).
pub fn output_texture(gpu: &Gpu, width: u32, height: u32, format: PixelFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("output"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: output_format(format),
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// WGSL helpers mirroring `mr_core` conventions; prepend to shader sources.
pub const COMMON_WGSL: &str = include_str!("common.wgsl");

/// First-fit range allocator (offsets in arbitrary units, e.g. quads).
pub struct RangeAlloc {
    free: BTreeMap<u32, u32>,
    cap: u32,
}

impl RangeAlloc {
    pub fn new(cap: u32) -> Self {
        Self { free: BTreeMap::from([(0, cap)]), cap }
    }

    pub fn capacity(&self) -> u32 {
        self.cap
    }

    pub fn alloc(&mut self, n: u32) -> Option<u32> {
        let (&off, &len) = self.free.iter().find(|(_, l)| **l >= n)?;
        self.free.remove(&off);
        if len > n {
            self.free.insert(off + n, len - n);
        }
        Some(off)
    }

    pub fn free(&mut self, mut off: u32, mut n: u32) {
        if n == 0 {
            return;
        }
        if let Some((&p, &pl)) = self.free.range(..off).next_back()
            && p + pl == off {
            self.free.remove(&p);
            off = p;
            n += pl;
        }
        if let Some(&nl) = self.free.get(&(off + n)) {
            self.free.remove(&(off + n));
            n += nl;
        }
        self.free.insert(off, n);
    }

    /// Extends the managed range to `cap` units.
    pub fn grow(&mut self, cap: u32) {
        if cap > self.cap {
            let old = self.cap;
            self.cap = cap;
            self.free(old, cap - old);
        }
    }
}

/// A storage buffer that starts small and doubles on demand (contents are copied on the
/// GPU), up to a maximum (at most what the device binds). Lets several renderers share the
/// GPU without each reserving its worst case up front. A request beyond the maximum fails
/// the device ([`Gpu::fail`]): the caller may skip the data, the renderer stops at its next
/// frame.
/// The `limit` of a [`GrowableBuffer`] with no maximum of its own.
pub const DEVICE_BOUND: &str = "what the device binds";

pub struct GrowableBuffer {
    buf: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsages,
    max: u64,
}

impl GrowableBuffer {
    pub fn new(gpu: &Gpu, label: &'static str, initial: u64, max: u64, usage: wgpu::BufferUsages) -> Self {
        let limits = gpu.device.limits();
        // A multiple of 16 (sizes are rounded up to one), so no size exceeds what the device binds.
        let max = (max.min(limits.max_buffer_size).min(limits.max_storage_buffer_binding_size) & !15).max(256);
        let usage = usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
        let size = initial.clamp(256, max).next_multiple_of(16);
        Self { buf: gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false }), label, usage, max }
    }

    /// A buffer of `size` bytes from the start, never grown (a [`PagedBuffer`]'s later pages:
    /// no copy, so no old and new buffer at once); `None` if the device is out of memory (it
    /// has failed).
    /// `doing` names the buffer's place for the message.
    fn whole(gpu: &Gpu, label: &'static str, size: u64, usage: wgpu::BufferUsages, doing: impl FnOnce(u64) -> String) -> Option<Self> {
        let mut b = Self::new(gpu, label, 256, size, usage);
        let size = b.max;
        b.buf = create_named(gpu, label, size, b.usage, || doing(size))?;
        Some(b)
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buf
    }

    pub fn size(&self) -> u64 {
        self.buf.size()
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    /// Ensures the buffer holds at least `bytes`. Returns `Some(true)` if it was reallocated
    /// (bind groups referencing it must be recreated), `None` if `bytes` exceeds the maximum
    /// (the device has failed, naming `limit` as what sets the maximum: a knob, [`DEVICE_BOUND`]).
    pub fn reserve(&mut self, gpu: &Gpu, bytes: u64, limit: &str) -> Option<bool> {
        let r = self.grow_to(gpu, bytes);
        if r.is_none() {
            gpu.fail(format!("{}: more than {} MiB needed; the limit is {limit}; frames would miss data", self.label, self.max >> 20));
        }
        r
    }

    /// [`Self::reserve`] without failing the device: `None` if `bytes` exceeds the maximum.
    fn grow_to(&mut self, gpu: &Gpu, bytes: u64) -> Option<bool> {
        if bytes <= self.buf.size() {
            return Some(false);
        }
        if bytes > self.max {
            return None;
        }
        let size = (self.buf.size() * 2).max(bytes).min(self.max).next_multiple_of(16);
        let old = self.buf.size();
        let new = create_named(gpu, self.label, size, self.usage, || format!("growing it from {} to {} MiB", old >> 20, size >> 20))?;
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.buf, 0, &new, 0, self.buf.size());
        // Pending queue writes to the old buffer are flushed before this submission.
        gpu.queue.submit([enc.finish()]);
        tracing::debug!("{} grown to {} MiB", self.label, size >> 20);
        std::mem::replace(&mut self.buf, new).destroy();
        // The old buffer is freed once the copy is done and the device polled: wait for it,
        // or a run of doublings (one large remesh) holds every old size at once.
        if let Err(e) = gpu.device.poll(wgpu::PollType::wait_indefinitely()) {
            gpu.fail(format!("{}: waiting for its copy failed: {e}", self.label));
        }
        Some(true)
    }

    /// Allocates `n` units of `unit` bytes from `alloc`, growing buffer and allocator as
    /// needed. Returns the offset and whether the buffer was reallocated; `None` as
    /// [`Self::reserve`].
    pub fn alloc(&mut self, gpu: &Gpu, alloc: &mut RangeAlloc, n: u32, unit: u64, limit: &str) -> Option<(u32, bool)> {
        let r = self.alloc_within(gpu, alloc, n, unit);
        if r.is_none() {
            gpu.fail(format!("{}: more than {} MiB needed; the limit is {limit}; frames would miss data", self.label, self.max >> 20));
        }
        r
    }

    /// [`Self::alloc`] without failing the device: `None` if the range does not fit within the maximum.
    fn alloc_within(&mut self, gpu: &Gpu, alloc: &mut RangeAlloc, n: u32, unit: u64) -> Option<(u32, bool)> {
        let mut grew = false;
        loop {
            if let Some(off) = alloc.alloc(n) {
                return Some((off, grew));
            }
            // The whole buffer is the allocator's by now and no hole fits: past the maximum.
            let cap = alloc.capacity() as u64;
            if (cap + 1) * unit > self.max {
                return None;
            }
            let need = ((cap + n.max(1) as u64).max(cap * 2) * unit).min(self.max);
            grew |= self.grow_to(gpu, need)?;
            alloc.grow((self.buf.size() / unit).min(u32::MAX as u64) as u32);
        }
    }
}

/// The failure of a buffer over a device limit ([`Gpu::storage`]).
fn over_limit(label: &str, size: u64, limit: u64, name: &str, adapter: &str, set_by: &str) -> String {
    let mib = |b: u64| b as f64 / (1u64 << 20) as f64;
    format!("{label}: {:.1} MiB is over what {adapter} allows ({:.1} MiB, `{name}`); its size is set by {set_by}", mib(size), mib(limit))
}

/// A buffer whose creation, when the device is out of memory, fails the device naming it
/// and `doing` (rare creations only: it waits for the error scope).
fn create_named(gpu: &Gpu, label: &str, size: u64, usage: wgpu::BufferUsages, doing: impl FnOnce() -> String) -> Option<wgpu::Buffer> {
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
    if let Some(e) = pollster::block_on(scope.pop()) {
        gpu.fail(format!("{label}: the device is out of memory {}: {e}", doing()));
        return None;
    }
    Some(buf)
}

/// Data that may outgrow one binding: [`GrowableBuffer`]s ("pages") of at most `page_max`
/// bytes each (at most what the device binds), together at most `max` bytes, each with its
/// own [`RangeAlloc`]. A range lives in one page; users bind a page at a time (a draw batch
/// per page). A page is added when no earlier one holds a range, so data that fits one
/// binding costs what a [`GrowableBuffer`] does. The first page grows (doubling, copied);
/// later ones are created whole at a quarter of `page_max` (or the range, if larger): no copy
/// while the data is already large, at most that quarter unused.
pub struct PagedBuffer {
    pages: Vec<(GrowableBuffer, RangeAlloc)>,
    label: &'static str,
    usage: wgpu::BufferUsages,
    page_max: u64,
    max: u64,
}

impl PagedBuffer {
    pub fn new(gpu: &Gpu, label: &'static str, initial: u64, page_max: u64, max: u64, usage: wgpu::BufferUsages) -> Self {
        let first = GrowableBuffer::new(gpu, label, initial, page_max.min(max), usage);
        let page_max = first.max();
        let alloc = RangeAlloc::new(0);
        Self { pages: vec![(first, alloc)], label, usage, page_max, max }
    }

    pub fn pages(&self) -> usize {
        self.pages.len()
    }

    pub fn buffer(&self, page: usize) -> &wgpu::Buffer {
        self.pages[page].0.buffer()
    }

    /// The bytes held, over all pages.
    pub fn size(&self) -> u64 {
        self.pages.iter().map(|(b, _)| b.size()).sum()
    }

    /// Allocates `n` units of `unit` bytes in the first page that holds them, else in a new
    /// page. Returns the page, the offset in it (in units) and whether that page's buffer is
    /// new or reallocated (bind groups referencing it must be recreated); `None` if `max` is
    /// reached or `n` units exceed a page (the device has failed, naming `limit`).
    pub fn alloc(&mut self, gpu: &Gpu, n: u32, unit: u64, limit: &str) -> Option<(usize, u32, bool)> {
        for (i, (buf, alloc)) in self.pages.iter_mut().enumerate() {
            if let Some((off, grew)) = buf.alloc_within(gpu, alloc, n, unit) {
                return Some((i, off, grew));
            }
        }
        let held: u64 = self.pages.iter().map(|(b, _)| b.max()).sum();
        let left = self.max.saturating_sub(held) & !15;
        let bytes = n as u64 * unit;
        if left < 256 || bytes > left.min(self.page_max) {
            let why = if bytes > self.page_max { format!("{} bytes in one range, more than a page holds ({} MiB)", bytes, self.page_max >> 20) } else { format!("more than {} MiB needed", self.max >> 20) };
            gpu.fail(format!("{}: {why}; the limit is {limit}; frames would miss data", self.label));
            return None;
        }
        let size = (self.page_max / 4).max(bytes).min(left).min(self.page_max);
        let mut page = (GrowableBuffer::whole(gpu, self.label, size, self.usage, |size| format!("allocating page {} ({} MiB; {} MiB held)", self.pages.len() + 1, size >> 20, self.size() >> 20))?, RangeAlloc::new(0));
        let (off, _) = page.0.alloc_within(gpu, &mut page.1, n, unit)?;
        self.pages.push(page);
        Some((self.pages.len() - 1, off, true))
    }

    pub fn free(&mut self, page: usize, off: u32, n: u32) {
        self.pages[page].1.free(off, n);
    }
}

/// The adapter renderers use: the one whose name contains `filter` (case-insensitive), else
/// the discrete GPU, else the best other one. Vulkan unless `WGPU_BACKEND` says otherwise.
pub fn pick_adapter(filter: Option<&str>) -> anyhow::Result<wgpu::Adapter> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    if std::env::var_os("WGPU_BACKEND").is_none() {
        desc.backends = wgpu::Backends::VULKAN;
    }
    // Debug object labels go through VK_EXT_debug_utils, whose loader bookkeeping is not
    // safe with several Vulkan instances on threads of one process (thread isolation):
    // it segfaults intermittently. Opt in explicitly when debugging a renderer.
    desc.flags = if std::env::var_os("MR_GPU_DEBUG").is_some() { wgpu::InstanceFlags::debugging() } else { wgpu::InstanceFlags::empty() };
    // On DX12 wgpu's indirect-call pass also rewrites indirect draws into the arguments its
    // command signatures read (first_instance as a root constant); without it they draw
    // garbage. Elsewhere it only validates, at a pass per indirect batch.
    if desc.backends.contains(wgpu::Backends::DX12) {
        desc.flags |= wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL;
    }
    let instance = wgpu::Instance::new(desc);
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    if adapters.is_empty() {
        bail!("no GPU adapters found");
    }
    let names: Vec<String> = adapters.iter().map(|a| format!("{} ({:?})", a.get_info().name, a.get_info().device_type)).collect();
    let adapter = match filter {
        Some(f) => {
            let f = f.to_lowercase();
            adapters.into_iter().find(|a| a.get_info().name.to_lowercase().contains(&f)).with_context(|| format!("no adapter matching `{f}` among {names:?}"))?
        }
        None => {
            use wgpu::DeviceType::*;
            let rank = |a: &wgpu::Adapter| match a.get_info().device_type {
                DiscreteGpu => 0,
                IntegratedGpu => 1,
                VirtualGpu => 2,
                Other => 3,
                Cpu => 4,
            };
            adapters.into_iter().min_by_key(rank).unwrap()
        }
    };
    Ok(adapter)
}

/// The features of the adapter [`pick_adapter`] chooses for `filter` (`None`: no adapter),
/// looked up once per filter per process: what a graph's device offers, for what its
/// pipeline supports (`mr_components::graph::Checked::narrow`).
pub fn adapter_features(filter: Option<&str>) -> Option<wgpu::Features> {
    static SEEN: std::sync::Mutex<Vec<(Option<String>, Option<wgpu::Features>)>> = std::sync::Mutex::new(Vec::new());
    let key = filter.map(str::to_string);
    let mut seen = SEEN.lock().unwrap();
    if let Some((_, f)) = seen.iter().find(|(k, _)| *k == key) {
        return *f;
    }
    let f = pick_adapter(filter).ok().map(|a| a.features());
    seen.push((key, f));
    f
}

/// Name, driver and backend of the adapter [`pick_adapter`] chooses (for records of which
/// machine a measurement came from).
pub fn adapter_info(filter: Option<&str>) -> anyhow::Result<wgpu::AdapterInfo> {
    Ok(pick_adapter(filter)?.get_info())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pool shares a device between equal needs, opens another for needs it lacks, and
    /// a fixed pool (a library caller's devices) refuses those with the reason. Needs a GPU.
    #[test]
    #[ignore]
    fn pool_shares_and_refuses() {
        let pool = GpuPool::new(&[]);
        let plain = GpuNeeds { optional_features: wgpu::Features::TIMESTAMP_QUERY, ..Default::default() };
        let a = pool.gpu(0, &plain).unwrap();
        let b = pool.gpu(0, &plain).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "equal needs share the device");
        let more = GpuNeeds { required_features: wgpu::Features::INDIRECT_FIRST_INSTANCE, ..plain.clone() };
        if a.lacks(&more).is_some() {
            assert!(!Arc::ptr_eq(&a, &pool.gpu(0, &more).unwrap()), "needs the device lacks get another device");
        }
        let fixed = GpuPool::fixed(vec![a.clone()]);
        assert!(Arc::ptr_eq(&a, &fixed.gpu(0, &GpuNeeds::default()).unwrap()));
        let rt = GpuNeeds { experimental: true, required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY, ..Default::default() };
        let err = fixed.gpu(0, &rt).err().expect("a fixed pool cannot open a device").to_string();
        assert!(err.contains("no handed-in GPU device serves"), "{err}");
        assert!(fixed.gpu(1, &plain).is_err(), "no device in slot 1");
    }

    /// A storage buffer over the device's binding limit fails the device naming the label,
    /// the limit and what sets the size (a placeholder comes back); one within it does not.
    /// Needs a GPU.
    #[test]
    #[ignore]
    fn storage_over_the_limit_fails_by_name() {
        let gpu = GpuPool::new(&[]).gpu(0, &GpuNeeds::default()).unwrap();
        let limit = gpu.device.limits().max_storage_buffer_binding_size;
        let ok = gpu.storage("small", 1000, wgpu::BufferUsages::STORAGE, || unreachable!());
        assert_eq!(ok.size(), 1000);
        assert!(gpu.check().is_ok());
        let big = gpu.storage("test buffer", limit + 16, wgpu::BufferUsages::STORAGE, || "the test's knob".into());
        assert_eq!(big.size(), 16);
        let e = gpu.check().unwrap_err().to_string();
        for want in ["test buffer", "max_storage_buffer_binding_size", "the test's knob"] {
            assert!(e.contains(want), "{e} lacks {want}");
        }
    }

    /// A growable buffer grows to its maximum, holds what fits, and fails the device (never
    /// only the caller) on the first request beyond it. Needs a GPU.
    #[test]
    #[ignore]
    fn growable_buffer_fails_the_device() {
        let gpu = GpuPool::new(&[]).gpu(0, &GpuNeeds::default()).unwrap();
        // 1000 bytes: not a multiple of the unit nor of 16; 992 are usable, 62 units of 16.
        let mut buf = GrowableBuffer::new(&gpu, "test data", 64, 1000, wgpu::BufferUsages::STORAGE);
        let mut alloc = RangeAlloc::new(0);
        let mut got = 0;
        while got < 62 {
            let (off, _) = buf.alloc(&gpu, &mut alloc, 2, 16, "`test_mb`").expect("fits");
            assert_eq!(off, got);
            got += 2;
        }
        assert!(buf.size() <= 1000 && gpu.check().is_ok());
        assert!(buf.alloc(&gpu, &mut alloc, 2, 16, "`test_mb`").is_none());
        let err = gpu.check().unwrap_err().to_string();
        assert!(err.contains("test data") && err.contains("`test_mb`"), "{err}");
        // The first failure stays.
        gpu.fail("later");
        assert_eq!(gpu.check().unwrap_err().to_string(), err);
    }

    /// A [`PagedBuffer`] fills its pages, adds pages up to its maximum and then fails the
    /// device. Needs a GPU.
    #[test]
    #[ignore]
    fn paged_buffer_adds_pages() {
        let gpu = GpuPool::new(&[]).gpu(0, &GpuNeeds::default()).unwrap();
        // Pages of 256 bytes (16 units of 16), four of them at most.
        let mut buf = PagedBuffer::new(&gpu, "test pages", 64, 256, 1024, wgpu::BufferUsages::STORAGE);
        let mut got = vec![];
        for _ in 0..12 {
            let (page, off, _) = buf.alloc(&gpu, 5, 16, "`test_mb`").expect("fits");
            got.push((page, off));
        }
        // Three ranges of 5 per page (15 of 16 units), no range across pages.
        assert_eq!(got.iter().filter(|g| g.0 == 0).map(|g| g.1).collect::<Vec<_>>(), [0, 5, 10]);
        assert_eq!((buf.pages(), got[11]), (4, (3, 10)));
        assert!(gpu.check().is_ok());
        // A freed range in an earlier page is used before failing.
        buf.free(1, 5, 5);
        assert_eq!(buf.alloc(&gpu, 4, 16, "`test_mb`").map(|r| (r.0, r.1)), Some((1, 5)));
        assert!(buf.alloc(&gpu, 5, 16, "`test_mb`").is_none());
        let err = gpu.check().unwrap_err().to_string();
        assert!(err.contains("test pages") && err.contains("`test_mb`"), "{err}");
    }
}
