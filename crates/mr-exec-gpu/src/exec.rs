//! The generic GPU executor: a frame walks the graph's schedule pass by pass, each pass a
//! [`Node`] recording its GPU work into the frame's one command encoder. Nodes running on one
//! device and API in one program share a [`Region`]: the program's compiled pipelines, group 0
//! and the frame's state (on the GPU, fusing nodes means compiling them into one shader
//! module). What crosses between passes is an edge's resource ([`Edges`]), the executor's: a
//! program attaches it where it chooses. A pipeline over two GPUs runs an executor per GPU slot
//! ([`Split`]), the edge that crosses carried by its transfer ([`crate::transfer`]). Dispatch is per pass (a virtual call per node per
//! frame); every per-pixel cost stays in the shaders.

use glam::IVec3;
use mr_components::graph::{self, Role};
use mr_core::*;
use mr_gpu::{Gpu, Ticket, wgpu};

use crate::Core;
use crate::cross::{EdgeGate, Part};
use crate::transfer::Transfer;

/// The resources of the graph's edges that cross between passes on one GPU: allocated by the
/// executor from the graph, sized to the frame, remade on a resize. A program whose node
/// writes or reads an edge attaches its resource at the group and binding it chooses
/// (`layout_entry`, `bind_entry`, the declaration from `wgsl`; a render target as the pass's
/// attachment, `view`). An edge whose two ends one pass computes (fused nodes) has no
/// resource.
pub(crate) struct Edges {
    /// The hits edge's ends, `from` -> `to`.hits, if the shading's hits are bound.
    ends: Option<(String, String)>,
    hits: Option<Hits>,
}

/// What a hits edge carries per pixel, as its writer's program encodes it (the reader's
/// program decodes the same).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    /// `pack_hit` records in a storage buffer (8 bytes): what rays find.
    Records,
    /// Point ids in a storage buffer (4 bytes, written with atomics): the splats'.
    Ids,
    /// A render target of this format: the face drawers' ids or `pack_hit` records.
    Image(wgpu::TextureFormat),
}

impl Form {
    fn bytes(self, width: u32, height: u32) -> u64 {
        let px = match self {
            Self::Records => 8,
            Self::Ids => 4,
            Self::Image(f) => f.block_copy_size(None).unwrap_or(4) as u64,
        };
        (width as u64 * height as u64 * px).max(16)
    }
}

enum Resource {
    Buffer(wgpu::Buffer),
    Image(wgpu::TextureView, wgpu::Texture),
}

/// The hits edge, `from` -> `to`.hits: what the pass of `from` found at each pixel (or, when
/// that runs on another GPU, what the edge's transfer delivers), read by the pass of `to`
/// (or copied out by the transfer).
pub(crate) struct Hits {
    pub from: String,
    pub to: String,
    form: Form,
    /// Records per pixel: 1, or `LAYERS` once a ray caster writes layers (transparency).
    layers: u32,
    bytes: u64,
    resource: Resource,
}

impl Edges {
    /// The edges of `g` on `gpu`; `hits`: the form of the shading's hits when they cross
    /// between passes (`None`: the shading is fused into the pass that finds them).
    pub fn of(gpu: &Gpu, g: &graph::Checked, width: u32, height: u32, hits: Option<Form>) -> anyhow::Result<Self> {
        let to = g.shading();
        let ends = g.input(Role::Shading, "hits").map(|from| (from.to_string(), to.to_string()));
        let mut edges = Self { ends, hits: None };
        edges.set_hits(gpu, width, height, hits).map_err(|e| anyhow::anyhow!("`{to}`: {e}"))?;
        Ok(edges)
    }

    /// The hits edge's resource for `form` (`None`: fused, no resource): at creation, and when
    /// a live knob changes what its writer writes.
    pub fn set_hits(&mut self, gpu: &Gpu, width: u32, height: u32, form: Option<Form>) -> anyhow::Result<()> {
        self.hits = match form {
            Some(form) => {
                let (from, to) = self.ends.clone().ok_or_else(|| anyhow::anyhow!("no node's hits to shade (its `hits` input is not bound)"))?;
                Some(Hits { from, to, form, layers: 1, bytes: form.bytes(width, height), resource: Hits::resource_on(gpu, form, width, height, 1) })
            }
            None => None,
        };
        Ok(())
    }

    /// The hits edge, if it crosses between passes.
    pub fn hits(&self) -> Option<&Hits> {
        self.hits.as_ref()
    }

    /// The hits edge if it crosses between passes as `pack_hit` records in a buffer.
    pub fn records(&self) -> Option<&Hits> {
        self.hits.as_ref().filter(|h| h.form == Form::Records)
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        if let Some(h) = &mut self.hits {
            h.bytes = h.form.bytes(width, height) * h.layers as u64;
            h.resource = Hits::resource_on(gpu, h.form, width, height, h.layers);
        }
    }

    /// `layers` records per pixel in the records edge from now on (a new, larger buffer:
    /// group 0 is bound anew); nothing without one.
    pub fn set_layers(&mut self, gpu: &Gpu, width: u32, height: u32, layers: u32) {
        if let Some(h) = self.hits.as_mut().filter(|h| h.form == Form::Records && h.layers != layers) {
            h.layers = layers;
            h.bytes = h.form.bytes(width, height) * layers as u64;
            h.resource = Hits::resource_on(gpu, h.form, width, height, layers);
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.hits.as_ref().map_or(0, |h| h.bytes)
    }
}

impl Hits {
    /// The records' bytes for a frame of `width` x `height`.
    pub fn size(width: u32, height: u32) -> u64 {
        Form::Records.bytes(width, height)
    }

    fn resource_on(gpu: &Gpu, form: Form, width: u32, height: u32, layers: u32) -> Resource {
        match form {
            Form::Records | Form::Ids => {
                // Copied out and written by the transfers between GPUs and by gates.
                let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
                let label = if form == Form::Records { "hit records" } else { "hit ids" };
                Resource::Buffer(gpu.storage(label, form.bytes(width, height) * layers as u64, usage, || format!("the resolution ({width}x{height}, {layers} layer(s))")))
            }
            Form::Image(format) => {
                let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("hits"),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    // Copied out by the transfer between GPUs, read back and written by gates.
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                Resource::Image(texture.create_view(&Default::default()), texture)
            }
        }
    }

    /// The edge's buffer (the records' or the ids').
    pub fn buffer(&self) -> &wgpu::Buffer {
        match &self.resource {
            Resource::Buffer(b) => b,
            Resource::Image(..) => panic!("`{}` -> `{}`.hits is a render target, not a buffer", self.from, self.to),
        }
    }

    /// The edge's render target (the pass of `from` draws into it; the pass of `to` reads it
    /// as a texture).
    pub fn view(&self) -> &wgpu::TextureView {
        match &self.resource {
            Resource::Image(v, _) => v,
            Resource::Buffer(_) => panic!("`{}` -> `{}`.hits is a buffer, not a render target", self.from, self.to),
        }
    }

    /// The edge's render target as a texture (copied out by the transfer).
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        match &self.resource {
            Resource::Image(_, t) => Some(t),
            Resource::Buffer(_) => None,
        }
    }

    /// The edge's resource, as a gate between passes reads it back and writes it.
    pub fn part(&self) -> Part {
        match &self.resource {
            Resource::Buffer(b) => Part::Buffer(b.clone()),
            Resource::Image(_, t) => Part::Texture(t.clone()),
        }
    }

    /// Whether the edge carries `pack_hit` records (a buffer of them, or a target of 8-byte
    /// texels holding them: `gbuffer`'s), what a second GPU's shading reads.
    pub fn holds_records(&self) -> bool {
        matches!(self.form, Form::Records | Form::Image(wgpu::TextureFormat::Rg32Uint))
    }

    /// The declaration a program attaches the records with, and `record_at` (the common
    /// deferred pass's reader); the root provides `cam` (the frame's width).
    pub fn wgsl(group: u32, binding: u32) -> String {
        format!(
            "@group({group}) @binding({binding}) var<storage, read_write> records: array<vec2<u32>>;\n\
             fn record_at(px: vec2<u32>) -> vec2<u32> {{ return records[px.y * cam.width + px.x]; }}\n"
        )
    }

    /// The declaration a program writes the records with and `record_store(i, r)` (record
    /// `r` of pixel `i`, row-major): a visbuffer's decode on the first of two GPUs.
    pub fn store_wgsl(group: u32, binding: u32) -> String {
        format!(
            "@group({group}) @binding({binding}) var<storage, read_write> records: array<vec2<u32>>;\n\
             fn record_store(i: u32, r: vec2<u32>) {{ records[i] = r; }}\n"
        )
    }

    /// A buffer form's layout entry at `binding`.
    pub fn layout_entry(&self, binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
        crate::storage_entry(binding, false, visibility)
    }

    pub fn bind_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        let resource = match &self.resource {
            Resource::Buffer(b) => b.as_entire_binding(),
            Resource::Image(v, _) => wgpu::BindingResource::TextureView(v),
        };
        wgpu::BindGroupEntry { binding, resource }
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
    /// A node to replace while running (an edit to the graph, which the region builds
    /// edited): `None` if it is not a node this region replaces.
    fn switch(&mut self, _core: &mut Core, _s: &graph::Switch) -> Option<anyhow::Result<()>> {
        None
    }
    /// After the core's resize.
    fn resize(&mut self, core: &Core);
    /// (CPU, GPU) bytes beyond the core's.
    fn memory(&self, core: &Core) -> (u64, u64);
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
    /// After the frame's submission (readbacks' mapping).
    fn submitted(&mut self, _core: &Core) {}
    /// The per-pixel quantities its programs can write for the post's view.
    fn quantities(&self) -> &'static [mr_core::Quantity] {
        &[]
    }
    /// While a view is chosen, after the frame's passes: its own passes writing the viewed
    /// quantity (`core.view`) where its frame's programs do not.
    fn view(&mut self, _core: &Core, _enc: &mut wgpu::CommandEncoder, _ticket: Ticket) {}
    /// The resources holding what the node of role `reader` reads at `port` (`data`) when its
    /// pass starts, written by the passes before it: what a gate on that edge reads back and
    /// writes ([`crate::cross`]); `None`: the region gives it none this frame. The executor's
    /// hits edge is the executor's own (asked only for other edges).
    fn crossing(&self, _core: &Core, _reader: Role, _port: &str, _data: graph::Data) -> Option<Vec<Part>> {
        None
    }
}

/// A pass of the frame: its node's id and implementation.
pub(crate) type Pass<R> = (String, Box<dyn Node<R>>);

/// The nodes of `g`'s passes on GPU slot `slot` in frame order, each by its node's role
/// (`node`; a role without one has no pass of its own here).
/// The shading is the second executor's when it is split off ([`crate::remote::far_slot`]),
/// on another GPU or, behind gates, on the same.
pub(crate) fn passes<R>(g: &graph::Checked, slot: usize, node: impl Fn(Role) -> Option<Box<dyn Node<R>>>) -> anyhow::Result<Vec<Pass<R>>> {
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    let split = crate::remote::far_slot(g).is_some();
    Ok(schedule
        .passes()
        .filter(|p| matches!(p.device, graph::Device::Gpu(i) if i as usize == slot))
        .filter(|p| !(split && g.node(&p.node).role == Role::Shading))
        .filter_map(|p| Some((p.node.clone(), node(g.node(&p.node).role)?)))
        .collect())
}

/// The passes of the executor the shading is split off to, on GPU slot `slot`: its node's
/// role's (`node`) for every node there (what the near executor also runs there, when both
/// are one GPU's, it runs on its own copy).
pub(crate) fn far_passes<R>(g: &graph::Checked, slot: usize, node: impl Fn(Role) -> Option<Box<dyn Node<R>>>) -> anyhow::Result<Vec<Pass<R>>> {
    let schedule = g.schedule().map_err(|e| anyhow::anyhow!(e))?;
    Ok(schedule.passes().filter(|p| matches!(p.device, graph::Device::Gpu(i) if i as usize == slot)).filter_map(|p| Some((p.node.clone(), node(g.node(&p.node).role)?))).collect())
}

/// One GPU slot's executor: its core, a region and its nodes' passes in frame order (static
/// dispatch over the region).
pub(crate) struct Executor<R> {
    core: Core,
    region: R,
    passes: Vec<Pass<R>>,
    /// The gates on edges between its passes.
    gates: Vec<EdgeGate>,
    /// The mesh instances last given: accumulated shading starts over when they move.
    instances: Vec<mr_core::MeshInstance>,
}

/// The renderer running `region`'s `passes` over `core`.
pub(crate) fn run<R: Region + 'static>(core: Core, region: R, passes: Vec<Pass<R>>) -> anyhow::Result<Box<dyn Renderer>> {
    core.gpu.save_cache();
    Ok(Box::new(Executor::new(core, region, passes, false)))
}

/// The renderer of a region's GPU, alone or (the shading on another GPU, `far`) split with
/// that GPU's executor.
pub(crate) fn run_or_split<R: Region + 'static>((core, region, far): (Core, R, Option<crate::remote::Far>), g: &graph::Checked, passes: Vec<Pass<R>>) -> anyhow::Result<Box<dyn Renderer>> {
    match far {
        Some(far) => split(g, Executor::new(core, region, passes, true), far),
        None => run(core, region, passes),
    }
}

impl<R: Region> Executor<R> {
    /// `split`: the shading's hits cross to another executor (their gates are the transfer's).
    pub fn new(core: Core, region: R, passes: Vec<Pass<R>>, split: bool) -> Self {
        let ids: Vec<String> = passes.iter().map(|(id, _)| id.clone()).collect();
        let gates = crate::cross::gates(&core, &ids, split);
        Self { core, region, passes, gates, instances: vec![] }
    }

    /// The frame's commands on this GPU, before their submission: its ticket on the ring
    /// (frames completing to `sink`), its passes and its stats.
    fn encode(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<(wgpu::CommandEncoder, Ticket, FrameStats)> {
        let (core, region) = (&mut self.core, &mut self.region);
        region.compile(core, req)?;
        let ticket = core.ring.begin(&core.gpu, sink)?;
        let start = std::time::Instant::now();
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        region.setup(core, req);
        if let Some(v) = &core.view {
            v.clear(&mut enc);
        }
        let mut gate_ms = 0.0;
        for (i, (_, pass)) in self.passes.iter_mut().enumerate() {
            for gate in self.gates.iter_mut().filter(|g| g.before == i) {
                let reader = core.graph.nodes.get(&gate.reader).map(|n| n.role).expect("the reader is a node of the graph");
                let data = core.gates.iter().find(|e| e.reader == gate.reader && e.port == gate.port).expect("the gate's edge").data;
                let parts = match core.edges.hits().filter(|_| gate.port == "hits") {
                    Some(h) => Some(vec![h.part()]),
                    None => region.crossing(core, reader, &gate.port, data),
                };
                let parts = parts.ok_or_else(|| anyhow::anyhow!("`{}`.{}: nothing crosses between the passes this frame to put through its gates", gate.reader, gate.port))?;
                gate_ms += gate.cross(core, parts, &mut enc, req)?;
            }
            pass.run(&mut Cx { core, region, enc: &mut enc, ticket, req });
            for gate in self.gates.iter().filter(|g| g.before == i) {
                gate.restore(&mut enc);
            }
        }
        // The post's view over the passes' image (its legend for the region's).
        if core.view.is_some() {
            region.view(core, &mut enc, ticket);
        }
        let format = core.output_format();
        let viewed = core.view.as_mut().map(|v| {
            v.encode(&core.gpu, &mut enc, &core.output, format, core.view_max);
            v.legend(core.view_max)
        });
        let (counters, legend, stages) = region.stats(core, req);
        let legend = viewed.or(legend);
        let mut stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend, stages, gates: vec![] };
        if !self.gates.is_empty() {
            let ids: Vec<String> = self.gates.iter().flat_map(EdgeGate::nodes).collect();
            stats.stages.push(StageTime::cpu("gate", &ids, gate_ms));
            stats.gates.extend(self.gates.iter().flat_map(EdgeGate::reports));
        }
        region.finish(core, &mut enc, ticket);
        Ok((enc, ticket, stats))
    }

    /// Submits the frame as this GPU's image and completes finished frames.
    fn submit(&mut self, enc: wgpu::CommandEncoder, ticket: Ticket, req: &FrameRequest, stats: FrameStats, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let core = &mut self.core;
        core.ring.submit(&core.gpu, enc, ticket, &core.output, req.frame_id, req.readback, stats);
        self.region.submitted(core);
        core.ring.poll(&core.gpu, sink, false)
    }

    /// Knob `name` set where it is one of the region's or the core's (`None`: of neither).
    fn offer(&mut self, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        // The post's view: the core's, then the region's (which recompiles its writer).
        match name {
            "view" => {
                let qs = self.region.quantities();
                if let Err(e) = self.core.set_view(value.as_str().unwrap_or(""), qs) {
                    return Some(Err(e));
                }
                // A region whose own passes write its quantities has nothing to recompile.
                return Some(self.region.set_param(&mut self.core, name, value).unwrap_or(Ok(())));
            }
            "view_max" => {
                self.core.view_max = value.as_i64().unwrap_or(256).clamp(1, 16384) as u32;
                return Some(Ok(()));
            }
            _ => {}
        }
        if let Some(r) = self.region.set_param(&mut self.core, name, value) {
            // A region rebuilds its pipelines lazily, at its next frame; they reach the file
            // with the next change or at exit.
            self.core.gpu.save_cache();
            return Some(r);
        }
        self.core.set_param(name, value)
    }

    /// Node `s` replaced, where it is one of the region's (it rebuilds its pipelines lazily,
    /// like a knob's).
    fn replace(&mut self, s: &graph::Switch) -> Option<anyhow::Result<()>> {
        let r = self.region.switch(&mut self.core, s);
        self.core.gpu.save_cache();
        r
    }
}

/// Every node of `switches` replaced by the first of `replace` that has it.
fn switched(switches: &[graph::Switch], mut replace: impl FnMut(&graph::Switch) -> Option<anyhow::Result<()>>) -> anyhow::Result<()> {
    for s in switches {
        replace(s).unwrap_or_else(|| anyhow::bail!("{} node `{}` cannot change while running", s.role.name(), s.node))?;
    }
    Ok(())
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
        let moved = match &update {
            MeshUpdate::Meshes(_) => true,
            MeshUpdate::Instances(i) => *i != self.instances && {
                self.instances.clone_from(i);
                true
            },
        };
        if moved {
            self.core.accum.invalidate();
        }
        self.region.update_meshes(update);
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (enc, ticket, stats) = self.encode(req, sink)?;
        self.submit(enc, ticket, req, stats, sink)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, false)
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        self.offer(name, value).unwrap_or_else(|| anyhow::bail!("`{name}` cannot change while running"))
    }

    fn edit(&mut self, edits: &str) -> anyhow::Result<()> {
        let (g, switches) = graph::live(&self.core.graph, edits)?;
        switched(&switches, |s| self.replace(s))?;
        self.core.graph = g.graph;
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.core.ring.poll(&self.core.gpu, sink, true)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        mr_gpu::check_resolution(&self.core.gpu, width, height)?;
        self.core.resize(width, height);
        self.region.resize(&self.core);
        self.gates.iter_mut().for_each(EdgeGate::resize);
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let (cpu, gpu) = self.region.memory(&self.core);
        let gates: u64 = self.gates.iter().map(EdgeGate::memory_bytes).sum();
        MemoryReport { cpu_bytes: cpu, gpu_bytes: self.core.memory_bytes() + gpu + gates, gpu_device_bytes: self.core.gpu.peak_allocated_bytes() }
    }
}

/// A pipeline over two GPUs: an executor per GPU slot, each running its device's passes of
/// the schedule, and the transfer of the hits edge that crosses between them. A frame: the
/// first executor's passes, submitted with the records sent ([`Transfer::send`]); the records
/// the second's shading reads delivered to its edge ([`Transfer::arrive`]); the second
/// executor's passes with the camera those records were cast with, its image the frame's.
/// The frame's stats are both GPUs' (the second's ring adds its GPU time to the first's).
struct Split<R, F> {
    near: Executor<R>,
    transfer: Transfer,
    far: Executor<F>,
}

/// The renderer running `near` and `far`, the executors of graph `g`'s two GPU slots.
pub(crate) fn split<R: Region + 'static, F: Region + 'static>(g: &graph::Checked, near: Executor<R>, far: Executor<F>) -> anyhow::Result<Box<dyn Renderer>> {
    near.core.gpu.save_cache();
    let transfer = Transfer::of(g, &near.core, &far.core)?;
    Ok(Box::new(Split { near, transfer, far }))
}

impl<R: Region, F: Region> Renderer for Split<R, F> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        self.far.update(updates.clone())?;
        self.near.update(updates)
    }

    fn update_meshes(&mut self, update: MeshUpdate) -> anyhow::Result<()> {
        self.near.update_meshes(update)
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        let (near, far, transfer) = (&mut self.near, &mut self.far, &mut self.transfer);
        // The first GPU's frames are the first half of one: their stats go with the records.
        let (enc, ticket, stats) = near.encode(req, transfer.sink())?;
        transfer.send(&mut near.core, enc, ticket, req, stats);
        near.region.submitted(&near.core);
        let (mut stats, camera) = transfer.arrive(&mut near.core, &far.core, req)?;
        let req = FrameRequest { camera, ..req.clone() };
        let (enc, ticket, shaded) = far.encode(&req, sink)?;
        let (stage, wait_ms, cpu_ms) = transfer.times();
        stats.stages.push(stage);
        stats.counters.extend(shaded.counters);
        stats.stages.extend(shaded.stages);
        stats.counters.push(("transfer_wait_ms".into(), wait_ms as f64));
        stats.cpu_ms += shaded.cpu_ms + cpu_ms;
        far.submit(enc, ticket, &req, stats, sink)
    }

    fn poll(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.far.poll(sink)
    }

    /// A knob reaches both executors (the cores' knobs: shadows, the shading state's; the
    /// shading); it must be one of either's.
    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        match (self.far.offer(name, value), self.near.offer(name, value)) {
            (Some(Err(e)), _) => Err(e),
            (_, Some(r)) | (Some(r), None) => r,
            (None, None) => anyhow::bail!("`{name}` cannot change while running"),
        }
    }

    /// A replaced node reaches both executors, like a knob.
    fn edit(&mut self, edits: &str) -> anyhow::Result<()> {
        let (g, switches) = graph::live(&self.near.core.graph, edits)?;
        switched(&switches, |s| match (self.far.replace(s), self.near.replace(s)) {
            (Some(Err(e)), _) => Some(Err(e)),
            (_, Some(r)) | (Some(r), None) => Some(r),
            (None, None) => None,
        })?;
        self.far.core.graph = g.graph.clone();
        self.near.core.graph = g.graph;
        Ok(())
    }

    fn flush(&mut self, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.near.flush(self.transfer.sink())?;
        self.far.flush(sink)
    }

    fn resize(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.near.resize(width, height)?;
        self.transfer.resize(&self.near.core)?;
        self.far.resize(width, height)
    }

    fn memory(&self) -> MemoryReport {
        let (near, far) = (self.near.memory(), self.far.memory());
        MemoryReport { cpu_bytes: near.cpu_bytes + far.cpu_bytes, gpu_bytes: near.gpu_bytes + far.gpu_bytes + self.transfer.memory_bytes(), gpu_device_bytes: if std::sync::Arc::ptr_eq(&self.near.core.gpu, &self.far.core.gpu) { near.gpu_device_bytes } else { near.gpu_device_bytes + far.gpu_device_bytes } }
    }
}
