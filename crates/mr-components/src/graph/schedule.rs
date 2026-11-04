//! The schedule: how an executor runs a checked graph.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::Api;

use super::*;

/// How an executor runs a checked graph: computed once when the pipeline is
/// created, so it costs nothing per frame. A *pass* is a node that does work of its own
/// (it has an input that is not fused data, or no inputs: the world); nodes whose inputs
/// and output are all fused data (traversal kernels) are only compiled into the passes that
/// call them, with the storages they read (a storage is both: its update from the change
/// log is a pass, its data is compiled into its readers). Whether two adjacent passes on
/// one device and API run as one (shading `placement = inline` in ray casting) is the
/// executor's fusion, decided when the pipeline is compiled, not part of the schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    /// In frame order: producers before consumers over this frame's edges.
    pub steps: Vec<Step>,
    /// Edges between devices (never fused data): the executor's transfer adapters.
    pub transfers: Vec<Edge>,
    /// Values kept across frames: `prev` edges, and `iterate` edges' first pass.
    pub kept: Vec<Edge>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Pass(Pass),
    /// An iteration (a cycle closed by an `iterate` edge, `passes` its count): the body
    /// starts at the edge's consumer, which runs `passes` times, the rest `passes - 1`.
    Loop { passes: u8, body: Vec<Pass> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    pub node: String,
    pub device: Device,
    pub api: Option<Api>,
    /// Fused nodes compiled into it (the kernels and storages it reads, transitively).
    pub fused: Vec<String>,
    /// What it keeps across frames ([`state`]).
    pub state: Option<&'static str>,
}

/// An edge `from` → `to`.`port`, with its data, timing and devices.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub port: String,
    pub data: Data,
    pub when: When,
    pub from_device: Device,
    pub to_device: Device,
}

impl Schedule {
    /// Whether devices overlap frames: a `prev` edge between devices lets the consumer's
    /// device run frame N while the producer's computes frame N + 1.
    pub fn pipelined(&self) -> bool {
        self.transfers.iter().any(|e| e.when == When::Prev)
    }

    /// Every pass, loops flattened, in frame order.
    pub fn passes(&self) -> impl Iterator<Item = &Pass> {
        self.steps.iter().flat_map(|s| match s {
            Step::Pass(p) => std::slice::from_ref(p),
            Step::Loop { body, .. } => body.as_slice(),
        })
    }
}

impl fmt::Display for Schedule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let pass = |f: &mut fmt::Formatter, p: &Pass, indent: &str| {
            let api = p.api.map_or(String::new(), |a| format!(":{}", a.name()));
            let fused = if p.fused.is_empty() { String::new() } else { format!(" [{}]", p.fused.join(", ")) };
            let state = p.state.map_or(String::new(), |s| format!("  (keeps {s})"));
            writeln!(f, "{indent}{} @{}{api}{fused}{state}", p.node, p.device)
        };
        for s in &self.steps {
            match s {
                Step::Pass(p) => pass(f, p, "  ")?,
                Step::Loop { passes, body } => {
                    writeln!(f, "  loop, {passes} passes:")?;
                    for p in body {
                        pass(f, p, "    ")?;
                    }
                }
            }
        }
        let edge = |e: &Edge| format!("{} -> {}.{} ({}, {})", e.from, e.to, e.port, e.data.name(), e.when.label());
        for e in &self.transfers {
            writeln!(f, "  transfer {}: {} -> {}", edge(e), e.from_device, e.to_device)?;
        }
        for e in &self.kept {
            writeln!(f, "  kept across frames: {}", edge(e))?;
        }
        if self.pipelined() {
            writeln!(f, "  pipelined: devices overlap one frame")?;
        }
        Ok(())
    }
}

impl Checked {
    /// The executor's schedule of the graph ([`Schedule`]).
    pub fn schedule(&self) -> Result<Schedule, String> {
        let g = &self.graph;
        let out_of = |id: &str| output(g.nodes[id].role, &g.nodes[id].component);
        // Work of its own: a non-fused output, or an update from a non-fused input. The post
        // is compiled into the output of the pass making its image.
        let is_pass = |id: &str| {
            let n = &g.nodes[id];
            n.role != Role::Post && (!out_of(id).fused() || n.inputs.is_empty() || n.inputs.values().any(|s| !out_of(s).fused()))
        };
        let pass = |id: &str| {
            let n = &g.nodes[id];
            let mut fused = Vec::new();
            let mut stack: Vec<&str> = n.inputs.values().map(String::as_str).filter(|s| out_of(s).fused()).collect();
            while let Some(x) = stack.pop() {
                if !fused.iter().any(|f| f == x) {
                    fused.push(x.to_string());
                    stack.extend(g.nodes[x].inputs.values().map(String::as_str).filter(|s| out_of(s).fused()));
                }
            }
            fused.extend(g.nodes.iter().filter(|(_, r)| r.role == Role::Post && r.inputs.values().any(|s| s == id)).map(|(k, _)| k.clone()));
            fused.sort();
            Pass { node: id.to_string(), device: self.devices[id], api: self.apis.get(id).copied(), fused, state: state(n.role, &n.component) }
        };
        // Loop bodies: the nodes on this frame's paths from an iterate edge's consumer to its producer.
        let mut loop_of: HashMap<&str, usize> = HashMap::new();
        let mut loops: Vec<(u8, &str)> = Vec::new();
        for (id, n) in &g.nodes {
            for (p, src) in &n.inputs {
                let When::Iterate(k) = n.when(p) else { continue };
                let li = loops.len();
                loops.push((k, id.as_str()));
                for x in g.nodes.keys().filter(|x| g.depends(x, id) && g.depends(src, x)) {
                    if let Some(other) = loop_of.insert(x.as_str(), li) {
                        return Err(format!("`{x}` is in two iterations (through `{}` and `{id}`): nested or overlapping iterations are not supported yet", loops[other].1));
                    }
                }
            }
        }
        // Frame order with each loop contracted to one step (group n + loop index; node i of
        // the frame order is group i): producers first.
        let order = g.frame_order(&self.output)?;
        let n = order.len();
        let at: HashMap<&str, usize> = order.iter().enumerate().map(|(i, &x)| (x, i)).collect();
        let group = |i: usize| loop_of.get(order[i]).map_or(i, |&li| n + li);
        let members = |gr: usize| -> Vec<usize> { (0..n).filter(|&i| group(i) == gr).collect() };
        let inputs = |gr: usize| -> Vec<usize> { members(gr).into_iter().flat_map(|m| g.now_inputs(order[m]).map(|(_, s)| group(at[s])).collect::<Vec<_>>()).filter(|&x| x != gr).collect() };
        fn visit(gr: usize, done: &mut HashSet<usize>, groups: &mut Vec<usize>, inputs: &dyn Fn(usize) -> Vec<usize>) {
            if done.insert(gr) {
                for i in inputs(gr) {
                    visit(i, done, groups, inputs);
                }
                groups.push(gr);
            }
        }
        let (mut done, mut groups) = (HashSet::new(), Vec::new());
        for i in 0..n {
            visit(group(i), &mut done, &mut groups, &inputs);
        }
        let mut steps = Vec::new();
        for gr in groups {
            if gr < n {
                if is_pass(order[gr]) {
                    steps.push(Step::Pass(pass(order[gr])));
                }
            } else {
                // In frame order the consumer comes first (every body node depends on it).
                steps.push(Step::Loop { passes: loops[gr - n].0, body: members(gr).into_iter().filter(|&i| is_pass(order[i])).map(|i| pass(order[i])).collect() });
            }
        }
        let (mut transfers, mut kept) = (Vec::new(), Vec::new());
        for (id, n) in &g.nodes {
            for (p, src) in &n.inputs {
                let e = Edge { from: src.clone(), to: id.clone(), port: p.clone(), data: out_of(src), when: n.when(p), from_device: self.devices[src.as_str()], to_device: self.devices[id] };
                if e.when != When::Now {
                    kept.push(e.clone());
                }
                if !e.data.fused() && e.from_device != e.to_device {
                    transfers.push(e);
                }
            }
        }
        Ok(Schedule { steps, transfers, kept })
    }
}
