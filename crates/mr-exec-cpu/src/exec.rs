//! The generic CPU executor: one executor for any graph, walking the graph's schedule
//! (`graph::Schedule`) pass by pass. It names no component: each pass is a [`Node`], an
//! implementation built for the graph by the lane modules, and every edge's value is a
//! resource the executor owns, one per producing pass (its output; any number of consumers
//! read it). Dispatch is per pass (a few virtual calls and downcasts per frame); all
//! per-ray and per-pixel work happens inside a node, monomorphised as before.
//!
//! Being migrated to: ray casting and columns run here; rasterisation's lane
//! ([`crate::Lane`]) is the old path, kept until its nodes match it in pixels and speed.

use std::any::Any;

use std::collections::HashMap;

use mr_components::graph::Checked;
use mr_reference::{Pool, World};
use mr_core::*;

use crate::Pixel;

/// An edge's value: a node's output, owned by the executor.
pub(crate) type Res = Box<dyn Any + Send + Sync>;

/// What every pass of a frame shares.
pub(crate) struct Cx<'a> {
    pub pool: &'a Pool,
    pub world: &'a World,
    pub req: &'a FrameRequest,
    pub width: u32,
    pub height: u32,
}

/// What a pass reports for the frame (merged into its [`FrameStats`]).
#[derive(Default)]
pub(crate) struct Report {
    pub stages: Vec<StageTime>,
    pub counters: Vec<(String, f64)>,
    pub legend: Option<Legend>,
}

/// A node's implementation on the CPU: one pass of the schedule (fused nodes are compiled
/// into it).
pub(crate) trait Node: Send {
    /// Its output resource, for a `width` x `height` image (once, at creation).
    fn output(&mut self, width: u32, height: u32) -> Res;
    /// Applies the world changes it receives (the world applied its own) to its output.
    fn update(&mut self, _out: &mut Res, _updates: &[WorldUpdate]) -> anyhow::Result<()> {
        Ok(())
    }
    /// Does its work for this frame: reads `ins` (its producers' outputs, in the order it
    /// was built with), writes `out`.
    fn run(&mut self, cx: &Cx, ins: &[&Res], out: &mut Res, report: &mut Report);
    /// Its knobs; `None` if `name` is not one.
    fn set_param(&mut self, _out: &mut Res, _name: &str, _value: &ParamValue) -> Option<anyhow::Result<()>> {
        None
    }
    /// After a resize: resizes `out` if its size follows the image's.
    fn resize(&mut self, _out: &mut Res, _width: u32, _height: u32) {}
    /// CPU bytes it keeps beyond its output, and in it.
    fn memory(&self, _out: &Res) -> u64 {
        0
    }
}

/// One pass: its node and the passes whose outputs it reads.
pub(crate) struct Pass {
    pub node: Box<dyn Node>,
    pub ins: Vec<usize>,
}

/// The executor: the passes in frame order; the last one's output is the image (`Vec<P>`).
pub(crate) struct Executor<P> {
    pool: Pool,
    world: World,
    width: u32,
    height: u32,
    passes: Vec<Pass>,
    outs: Vec<Res>,
    /// The knob values a compiled pipeline baked in ([`crate::Spec::KNOBS`]).
    baked: mr_core::baked::Knobs,
    /// Compiled for this machine ([`crate::Spec::NATIVE`]).
    native: bool,
    _px: std::marker::PhantomData<P>,
}

impl<P: Pixel> Executor<P> {
    pub fn new(p: &InitParams, threads: usize, world: World, mut passes: Vec<Pass>) -> anyhow::Result<Self> {
        anyhow::ensure!(passes.iter().enumerate().all(|(i, s)| s.ins.iter().all(|&j| j < i)), "a pass reads a later pass's output");
        let outs = passes.iter_mut().map(|s| s.node.output(p.width, p.height)).collect();
        Ok(Self { pool: Pool::new(p.devices.cpu_threads(threads))?, world, width: p.width, height: p.height, passes, outs, baked: &[], native: false, _px: std::marker::PhantomData })
    }

    /// A compiled pipeline's: refuses its `baked` knobs, says when it is `native`.
    pub fn baked(self, baked: mr_core::baked::Knobs, native: bool) -> Self {
        Self { baked, native, ..self }
    }

    fn image(&self) -> &[P] {
        self.outs.last().and_then(|o| o.downcast_ref::<Vec<P>>()).map_or(&[], |v| v.as_slice())
    }
}

impl<P: Pixel> Renderer for Executor<P> {
    fn update(&mut self, updates: Vec<WorldUpdate>) -> anyhow::Result<()> {
        let rest = self.world.apply(updates);
        for (s, out) in self.passes.iter_mut().zip(&mut self.outs) {
            s.node.update(out, &rest)?;
        }
        Ok(())
    }

    fn render(&mut self, req: &FrameRequest, sink: &mut dyn FrameSink) -> anyhow::Result<()> {
        self.world.prepare();
        let start = std::time::Instant::now();
        let mut report = Report::default();
        let (passes, outs) = (&mut self.passes, &mut self.outs);
        let cx = Cx { pool: &self.pool, world: &self.world, req, width: self.width, height: self.height };
        self.pool.install(|| {
            for (i, pass) in passes.iter_mut().enumerate() {
                // Inputs come before `i` (checked at creation): split there.
                let (done, rest) = outs.split_at_mut(i);
                let ins: Vec<&Res> = pass.ins.iter().map(|&j| &done[j]).collect();
                pass.node.run(&cx, &ins, &mut rest[0], &mut report);
            }
        });
        if self.native {
            // A pipeline compiled for this machine (observations tell it from the generic build).
            report.counters.push(("compiled_native".into(), 1.0));
        }
        let stats = FrameStats { cpu_ms: start.elapsed().as_secs_f32() * 1e3, gpu_ms: None, counters: report.counters, legend: report.legend, stages: report.stages };
        sink.complete(CompletedFrame { frame_id: req.frame_id, stats, pixels: Some(bytemuck::cast_slice(self.image())) });
        Ok(())
    }

    fn set_param(&mut self, name: &str, value: &ParamValue) -> anyhow::Result<()> {
        mr_core::baked::refuse(self.baked, name, &value.to_json())?;
        for (s, out) in self.passes.iter_mut().zip(&mut self.outs) {
            if let Some(r) = s.node.set_param(out, name, value) {
                return r;
            }
        }
        match name {
            "shadows" => {
                self.world.shadows = mr_reference::parse_shadows(value)?;
                Ok(())
            }
            "threads" => {
                let n = value.as_i64().unwrap_or(0) as usize;
                if n != self.pool.threads {
                    self.pool = Pool::new(n)?;
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
        (self.width, self.height) = (width, height);
        for (s, out) in self.passes.iter_mut().zip(&mut self.outs) {
            s.node.resize(out, width, height);
        }
        Ok(())
    }

    fn memory(&self) -> MemoryReport {
        let cpu_bytes = self.passes.iter().zip(&self.outs).map(|(s, o)| s.node.memory(o)).sum::<u64>() + std::mem::size_of_val(self.image()) as u64;
        MemoryReport { cpu_bytes, gpu_bytes: 0 }
    }
}

/// The image resource of a `width` x `height` frame.
pub(crate) fn image<P: Pixel>(width: u32, height: u32) -> Res {
    Box::new(vec![P::zeroed(); (width * height) as usize])
}

/// The passes node `id`'s inputs reach, in port order: a pass directly, through fused
/// nodes (kernels) the passes they read.
pub(crate) fn reads(g: &Checked, id: &str, index: &HashMap<String, usize>, out: &mut Vec<usize>) {
    for from in g.node(id).inputs.values() {
        match index.get(from) {
            Some(&i) if !out.contains(&i) => out.push(i),
            Some(_) => {}
            None if g.graph.nodes.contains_key(from) => reads(g, from, index, out),
            None => {}
        }
    }
}
