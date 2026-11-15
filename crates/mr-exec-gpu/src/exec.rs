//! The generic GPU executor: a frame walks the graph's schedule pass by pass, each pass a
//! [`Node`] recording its GPU work into the frame's one command encoder. Nodes running on one
//! device and API in one program share a [`Region`]: the program's compiled pipelines, group 0
//! and the frame's state (on the GPU, fusing nodes means compiling them into one shader
//! module). What crosses between passes is an edge's resource ([`Edges`]), the executor's: a
//! program attaches it where it chooses. Dispatch is per pass (a virtual call per node per
//! frame); every per-pixel cost stays in the shaders.

use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_core::*;
use mr_gpu::{Gpu, Ticket, wgpu};

use crate::Core;

/// The resources of the graph's edges that cross between passes on one GPU: allocated by the
/// executor from the graph, sized to the frame, remade on a resize. A program whose node
/// writes or reads an edge attaches its resource at the group and binding it chooses
/// (`layout_entry`, `bind_entry`, the declaration from `wgsl`). An edge whose two ends one
/// pass computes (fused nodes) has no resource.
pub(crate) struct Edges {
    hits: Option<Hits>,
}

/// The hit records' edge, `from` -> `to`.hits: one `pack_hit` record per pixel (8 bytes),
/// written by the pass of `from` (or, when that runs on another GPU, by the edge's transfer)
/// and read by the pass of `to` (or copied out by the transfer).
pub(crate) struct Hits {
    pub from: String,
    pub to: String,
    buffer: wgpu::Buffer,
}

impl Edges {
    /// The edges of `g` on `gpu`; `hits`: the shading's hit records cross between passes (the
    /// shading is not fused into the pass that finds the hits).
    pub fn of(gpu: &Gpu, g: &graph::Checked, width: u32, height: u32, hits: bool) -> anyhow::Result<Self> {
        let hits = if hits {
            let to = g.shading();
            let from = g.input(Role::Shading, "hits").ok_or_else(|| anyhow::anyhow!("`{to}`: no node's hits to shade (its `hits` input is not bound)"))?;
            Some(Hits { from: from.to_string(), to: to.to_string(), buffer: Hits::buffer_on(gpu, width, height) })
        } else {
            None
        };
        Ok(Self { hits })
    }

    /// The hit records' edge, if it crosses between passes.
    pub fn hits(&self) -> Option<&Hits> {
        self.hits.as_ref()
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        if let Some(h) = &mut self.hits {
            h.buffer = Hits::buffer_on(gpu, width, height);
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.hits.as_ref().map_or(0, |h| h.buffer.size())
    }
}

impl Hits {
    /// The records' bytes for a frame of `width` x `height`.
    pub fn size(width: u32, height: u32) -> u64 {
        (width as u64 * height as u64 * 8).max(16)
    }

    fn buffer_on(gpu: &Gpu, width: u32, height: u32) -> wgpu::Buffer {
        // Copied out and written by the transfers between GPUs.
        let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
        gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("hit records"), size: Self::size(width, height), usage, mapped_at_creation: false })
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// The declaration a program attaches the records with, and `record_at` (the common
    /// deferred pass's reader); the root provides `cam` (the frame's width).
    pub fn wgsl(group: u32, binding: u32) -> String {
        format!(
            "@group({group}) @binding({binding}) var<storage, read_write> records: array<vec2<u32>>;\n\
             fn record_at(px: vec2<u32>) -> vec2<u32> {{ return records[px.y * cam.width + px.x]; }}\n"
        )
    }

    pub fn layout_entry(&self, binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
        crate::storage_entry(binding, false, visibility)
    }

    pub fn bind_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry { binding, resource: self.buffer.as_entire_binding() }
    }
}

/// What every pass of a frame shares.
pub(crate) struct Cx<'a, R> {
    pub core: &'a mut Core,
    pub region: &'a mut R,
    pub enc: &'a mut wgpu::CommandEncoder,
    pub ticket: Ticket,
    pub req: &'a FrameRequest,
}

/// A node's implementation on the GPU: one pass of the schedule (fused nodes are compiled
/// into its region's program).
pub(crate) trait Node<R> {
    /// Records its work for this frame.
    fn run(&mut self, cx: &mut Cx<R>);
}

/// What a region keeps across frames: its world changes, knobs, resizes, memory and completed
/// frames.
pub(crate) trait State {
    /// World changes before the GPU world applies them.
    fn updating(&mut self, _updates: &[WorldUpdate]) {}
    /// Chunks the GPU world (and the storages) changed.
    fn updated(&mut self, core: &Core, changed: &[IVec3]) -> anyhow::Result<()>;
    fn update_meshes(&mut self, _update: MeshUpdate) {}
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>>;
    /// After the core's resize.
    fn resize(&mut self, core: &Core);
    /// (CPU, GPU) bytes beyond the core's.
    fn memory(&self, core: &Core) -> (u64, u64);
    /// Completes finished frames (`wait`: all of them).
    fn poll(&mut self, core: &mut Core, sink: &mut dyn FrameSink, wait: bool) -> anyhow::Result<()> {
        core.ring.poll(&core.gpu, sink, wait)
    }
}

/// The state a region's nodes share, and what of the frame is its own beyond its nodes'
/// passes: its start and submission, and what its nodes' passes share.
pub(crate) trait Region: State {
    /// Before the frame: recompiles what its knobs changed (and refuses `req` if it cannot
    /// draw it).
    fn compile(&mut self, core: &mut Core, req: &FrameRequest) -> anyhow::Result<()>;
    /// The frame's state before its passes (camera, shading state, world).
    fn setup(&mut self, core: &mut Core, req: &FrameRequest);
    /// The frame's counters, legend and CPU stage times.
    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>);
    /// Before the frame's submission, after its nodes' passes (readbacks, diagnostics).
    fn finish(&mut self, core: &Core, enc: &mut wgpu::CommandEncoder, ticket: Ticket);
    /// The frame's ticket on its ring (frames completing to `sink`).
    fn begin(&mut self, core: &mut Core, sink: &mut dyn FrameSink) -> anyhow::Result<Ticket> {
        core.ring.begin(&core.gpu, sink)
    }
    /// Submits the frame's commands `enc` and completes finished frames.
    fn submit(&mut self, core: &mut Core, enc: wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, stats: FrameStats, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        core.ring.poll(&core.gpu, sink, false)
    }
}

/// The nodes of `g`'s passes on the GPU in frame order, each by its node's role (`node`; a
/// role without one has no pass of its own here).
pub(crate) fn passes<R>(g: &graph::Checked, node: impl Fn(Role) -> Option<Box<dyn Node<R>>>) -> anyhow::Result<Vec<Box<dyn Node<R>>>> {
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    Ok(schedule.passes().filter(|p| matches!(p.device, graph::Device::Gpu(_))).filter_map(|p| node(g.node(&p.node).role)).collect())
}

/// The core, a region and its nodes' passes in frame order (static dispatch over the region).
struct Executor<R> {
    core: Core,
    region: R,
    passes: Vec<Box<dyn Node<R>>>,
}

/// The renderer running `region`'s `passes` over `core`.
pub(crate) fn run<R: Region + 'static>(core: Core, region: R, passes: Vec<Box<dyn Node<R>>>) -> anyhow::Result<Box<dyn Renderer>> {
    core.gpu.save_cache();
    Ok(Box::new(Executor { core, region, passes }))
}

impl<R: Region> Renderer for Executor<R> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        if !updates.is_empty() {
            self.core.accum.invalidate();
        }
        self.region.updating(&updates);
        let c = &mut self.core;
        let changed = c.world.apply(&c.gpu, updates);
        c.storages.update(&c.gpu, &c.world, &changed)?;
        if let Some(a) = &mut c.accel {
            a.updated(&c.world, &changed);
        }
        self.region.updated(&self.core, &changed)
    }

    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.region.update_meshes(update);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (core, region) = (&mut self.core, &mut self.region);
        region.compile(core, req)?;
        let ticket = region.begin(core, sink)?;
        let start = std::time::Instant::now();
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        region.setup(core, req);
        let mut cx = Cx { core, region, enc: &mut enc, ticket, req };
        for pass in &mut self.passes {
            pass.run(&mut cx);
        }
        let (counters, legend, stages) = region.stats(core, req);
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend, stages };
        region.finish(core, &mut enc, ticket);
        region.submit(core, enc, ticket, req, stats, sink)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.region.poll(&mut self.core, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        if let Some(r) = self.region.set_param(&mut self.core, name, value) {
            // A region rebuilds its pipelines lazily, at its next frame; they reach the file
            // with the next change or at exit.
            self.core.gpu.save_cache();
            return r;
        }
        self.core.set_param(name, value).unwrap_or_else(|| anyhow::bail!("`{name}` cannot change while running"))
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.region.poll(&mut self.core, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.core.resize(width, height);
        self.region.resize(&self.core);
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let (cpu, gpu) = self.region.memory(&self.core);
        MemoryReport { cpu_bytes: cpu, gpu_bytes: self.core.memory_bytes() + gpu }
    }
}
