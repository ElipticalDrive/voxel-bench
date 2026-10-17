//! Shared wgpu plumbing for GPU renderers: adapter selection (prefers the discrete GPU on
//! hybrid laptops), and a frame ring that handles asynchronous completion, read-back of
//! the output texture and GPU timestamps.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, bail};
use mr_core::{CompletedFrame, FrameSink, FrameStats, StageTime, Waker};

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
    /// First uncaptured wgpu error (validation, OOM, ...). Any error fails the renderer:
    /// a renderer with an invalid pipeline must not report a frame rate.
    error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
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
        let features = req.required_features | (req.optional_features & available);
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
        let error = Arc::new(std::sync::Mutex::new(None));
        let e2 = error.clone();
        device.on_uncaptured_error(Arc::new(move |e| {
            tracing::error!("wgpu: {e}");
            e2.lock().unwrap().get_or_insert_with(|| e.to_string());
        }));
        let features = device.features();
        Self { device, queue, info, timestamps: features.contains(wgpu::Features::TIMESTAMP_QUERY), adapter_features: features, experimental: false, error }
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

    /// Fails if wgpu reported an error since the device was created.
    pub fn check(&self) -> anyhow::Result<()> {
        match self.error.lock().unwrap().as_ref() {
            Some(e) => bail!("wgpu error: {}", e.lines().take(12).collect::<Vec<_>>().join("\n")),
            None => Ok(()),
        }
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

/// The GPU devices of one process (IDEAS §17, requirement 2): handed to renderers through
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
    /// Stage marks (PLAN step 11): `MAX_MARKS` per slot.
    marks: Option<wgpu::QuerySet>,
    /// The GPU stages of the frame being recorded.
    recording: std::sync::Mutex<Vec<StageTime>>,
    /// The frame being recorded started its clock before its passes ([`Self::start_clock`]):
    /// passes asking to begin it do not.
    clock_started: std::sync::atomic::AtomicBool,
    timestamp_period: f32,
    width: u32,
    height: u32,
    padded_row: u32,
    pixels: Vec<u8>,
    /// Signals the waiter thread that work was submitted. `None` once shutting down.
    submitted: Option<std::sync::mpsc::Sender<()>>,
    waiter: Option<std::thread::JoinHandle<()>>,
}

impl FrameRing {
    /// `waker` is called from a background thread whenever submitted work completes.
    pub fn new(gpu: &Gpu, width: u32, height: u32, depth: usize, waker: Waker) -> Self {
        let (submitted, rx) = std::sync::mpsc::channel::<()>();
        let device = gpu.device.clone();
        let waiter = std::thread::Builder::new()
            .name("gpu-waiter".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    while rx.try_recv().is_ok() {}
                    // Fires map/work-done callbacks for everything submitted so far.
                    if let Err(e) = device.poll(wgpu::PollType::wait_indefinitely()) {
                        tracing::error!("gpu wait failed: {e}");
                    }
                    waker.wake();
                }
            })
            .expect("spawn gpu-waiter");
        let padded_row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
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
            padded_row,
            pixels: vec![0; (width * height * 4) as usize],
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
    /// submits the encoder, and tracks the frame until the GPU is done with it.
    pub fn submit(
        &mut self,
        gpu: &Gpu,
        mut encoder: wgpu::CommandEncoder,
        t: Ticket,
        output: &wgpu::Texture,
        frame_id: u64,
        readback: bool,
        stats: FrameStats,
    ) {
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
        gpu.queue.submit([encoder.finish()]);

        let remaining = Arc::new(AtomicU32::new(0));
        let track = |buf: &wgpu::Buffer| {
            remaining.fetch_add(1, Ordering::SeqCst);
            let r = remaining.clone();
            buf.map_async(wgpu::MapMode::Read, .., move |res| {
                if let Err(e) = res {
                    tracing::error!("map failed: {e}");
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
                    if ts[1] > ts[0] {
                        p.stats.gpu_ms = Some((ts[1] - ts[0]) as f32 * self.timestamp_period / 1e6);
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
                {
                    let v = slot.readback.get_mapped_range(..)?;
                    let row = (self.width * 4) as usize;
                    for y in 0..self.height as usize {
                        let src = y * self.padded_row as usize;
                        self.pixels[y * row..(y + 1) * row].copy_from_slice(&v[src..src + row]);
                    }
                }
                slot.readback.unmap();
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

/// Creates the RGBA8 output texture renderers draw into (storage + render target + copy).
pub fn output_texture(gpu: &Gpu, width: u32, height: u32) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("output"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
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
        if let Some((&p, &pl)) = self.free.range(..off).next_back() {
            if p + pl == off {
                self.free.remove(&p);
                off = p;
                n += pl;
            }
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
/// GPU), up to a maximum. Lets several renderers share the GPU without each reserving its
/// worst case up front.
pub struct GrowableBuffer {
    buf: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsages,
    max: u64,
}

impl GrowableBuffer {
    pub fn new(gpu: &Gpu, label: &'static str, initial: u64, max: u64, usage: wgpu::BufferUsages) -> Self {
        let limits = gpu.device.limits();
        let max = max.min(limits.max_buffer_size).min(limits.max_storage_buffer_binding_size as u64).max(256);
        let usage = usage | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
        let size = initial.clamp(256, max).next_multiple_of(16);
        Self { buf: gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false }), label, usage, max }
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

    /// Ensures the buffer holds at least `bytes`. Returns `Ok(true)` if it was reallocated
    /// (bind groups referencing it must be recreated), `Err` if `bytes` exceeds the maximum.
    pub fn reserve(&mut self, gpu: &Gpu, bytes: u64) -> anyhow::Result<bool> {
        if bytes <= self.buf.size() {
            return Ok(false);
        }
        if bytes > self.max {
            bail!("{} would need {} MiB, limit is {} MiB", self.label, bytes >> 20, self.max >> 20);
        }
        let size = (self.buf.size() * 2).max(bytes).min(self.max).next_multiple_of(16);
        let new = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some(self.label), size, usage: self.usage, mapped_at_creation: false });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.buf, 0, &new, 0, self.buf.size());
        // Pending queue writes to the old buffer are flushed before this submission.
        gpu.queue.submit([enc.finish()]);
        tracing::debug!("{} grown to {} MiB", self.label, size >> 20);
        self.buf = new;
        Ok(true)
    }

    /// Allocates `n` units of `unit` bytes from `alloc`, growing buffer and allocator as
    /// needed. Returns the offset and whether the buffer was reallocated.
    pub fn alloc(&mut self, gpu: &Gpu, alloc: &mut RangeAlloc, n: u32, unit: u64) -> anyhow::Result<(u32, bool)> {
        let mut grew = false;
        loop {
            if let Some(off) = alloc.alloc(n) {
                return Ok((off, grew));
            }
            let want = (alloc.capacity() as u64 + n as u64).max(alloc.capacity() as u64 * 2) * unit;
            grew |= self.reserve(gpu, want.min(self.max).max((alloc.capacity() as u64 + n as u64) * unit))?;
            let cap = (self.buf.size() / unit).min(u32::MAX as u64) as u32;
            if cap <= alloc.capacity() {
                bail!("{} is full", self.label);
            }
            alloc.grow(cap);
        }
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
}
