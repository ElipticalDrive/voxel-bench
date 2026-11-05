//! The generic GPU executor: a frame walks the graph's schedule pass by pass, each pass a
//! [`Node`] recording its GPU work into the frame's one command encoder. Nodes running on one
//! device and API in one program share a [`Region`]: the program's compiled pipelines, group 0
//! and the frame's state (on the GPU, fusing nodes means compiling them into one shader
//! module). Dispatch is per pass (a virtual call per node per frame); every per-pixel cost
//! stays in the shaders.

use mr_core::*;
use mr_gpu::{Ticket, wgpu};

use crate::{Core, Lane, State};

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

/// The state a region's nodes share, and what of the frame is its own beyond its nodes'
/// passes: its start and submission, and what its nodes' passes share. Its world changes,
/// knobs, resizes, memory and completed frames are a lane's.
pub(crate) trait Region: State {
    /// Before the frame: recompiles what its knobs changed.
    fn compile(&mut self, core: &mut Core) -> anyhow::Result<()>;
    /// The frame's state before its passes (camera, shading state, world).
    fn setup(&mut self, core: &mut Core, req: &FrameRequest);
    /// The frame's counters, legend and CPU stage times.
    fn stats(&self, core: &Core, req: &FrameRequest) -> (Vec<(String, f64)>, Option<Legend>, Vec<StageTime>);
    /// Before the frame's submission (readbacks).
    fn finish(&mut self, enc: &mut wgpu::CommandEncoder);
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

/// A region and its nodes' passes in frame order, run as the executor's lane.
pub(crate) struct Nodes<R> {
    region: R,
    passes: Vec<Box<dyn Node<R>>>,
}

impl<R> Nodes<R> {
    pub fn new(region: R, passes: Vec<Box<dyn Node<R>>>) -> Self {
        Self { region, passes }
    }
}

impl<R: Region> State for Nodes<R> {
    fn updating(&mut self, updates: &[WorldUpdate]) {
        self.region.updating(updates);
    }

    fn updated(&mut self, core: &Core, changed: &[glam::IVec3]) -> anyhow::Result<()> {
        self.region.updated(core, changed)
    }

    fn update_meshes(&mut self, update: MeshUpdate) {
        self.region.update_meshes(update);
    }

    fn set_param(&mut self, core: &mut Core, name: &str, value: &ParamValue) -> Option<anyhow::Result<()>> {
        self.region.set_param(core, name, value)
    }

    fn resize(&mut self, core: &Core) {
        self.region.resize(core);
    }

    fn memory(&self, core: &Core) -> (u64, u64) {
        self.region.memory(core)
    }

    fn poll(&mut self, core: &mut Core, sink: &mut dyn FrameSink, wait: bool) -> anyhow::Result<()> {
        self.region.poll(core, sink, wait)
    }
}

impl<R: Region> Lane for Nodes<R> {
    fn render(&mut self, core: &mut Core, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.region.compile(core)?;
        let ticket = self.region.begin(core, sink)?;
        let start = std::time::Instant::now();
        let mut enc = core.gpu.device.create_command_encoder(&Default::default());
        self.region.setup(core, req);
        let mut cx = Cx { core, region: &mut self.region, enc: &mut enc, ticket, req };
        for pass in &mut self.passes {
            pass.run(&mut cx);
        }
        let (counters, legend, stages) = self.region.stats(core, req);
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters, legend, stages };
        self.region.finish(&mut enc);
        self.region.submit(core, enc, ticket, req, stats, sink)
    }
}
