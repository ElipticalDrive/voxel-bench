//! The valid graphs `accept` refuses: graphs meeting the validity rules ([`Graph::check`])
//! that no executor builds, grouped by why. Every valid graph is meant to execute (fused
//! where a region fuses it, else as unfused passes), so each refusal here is a gap, and the
//! test keeps their count from growing.

use std::collections::{BTreeMap, HashSet};

use super::*;

/// Why a valid graph is refused, in the three kinds of gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Gap {
    /// No region has the graph's shape (its image's shading over hits from a node no region
    /// starts from, or the region is the other device kind's).
    Shape,
    /// A region has the shape, but not this node: a role, component, device kind, count or
    /// binding it does not declare (the component runs only inside other regions).
    Node,
    /// An edge the region has no resource for: a timing it does not run, a device boundary
    /// it does not split at, an API it does not run the node with.
    Edge,
}

impl Gap {
    pub const ALL: [Gap; 3] = [Gap::Shape, Gap::Node, Gap::Edge];

    pub fn name(self) -> &'static str {
        match self {
            Gap::Shape => "no region of that shape",
            Gap::Node => "node outside the region's declarations",
            Gap::Edge => "edge without a resource form",
        }
    }
}

/// A finer reason, read off `accept`'s message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Why {
    /// No region has this shape.
    NoRegion,
    /// The region is the other executor's (its visibility on the CPU, the drawer on the GPU ...).
    OtherExecutor,
    /// The region builds no node of this role.
    Role,
    /// The region builds the role on the other device kind.
    Kind,
    /// The region builds other components of the role.
    Component,
    /// The region builds another number of nodes of the role.
    Count,
    /// The region reads other components (or nothing) at a port.
    Port,
    /// The region reads two ports as one node.
    Same,
    /// The region runs the edge at another timing.
    Timing,
    /// The region runs on one GPU or splits only at the hit records.
    Devices,
    /// The region runs the node with another API.
    Api,
    /// Anything else (the message says).
    Other,
}

impl Why {
    pub fn gap(self) -> Gap {
        match self {
            Why::NoRegion | Why::OtherExecutor => Gap::Shape,
            Why::Role | Why::Kind | Why::Component | Why::Count | Why::Port | Why::Same | Why::Other => Gap::Node,
            Why::Timing | Why::Devices | Why::Api => Gap::Edge,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Why::NoRegion => "no region has the shape",
            Why::OtherExecutor => "the shape is the other executor's region",
            Why::Role => "role not built by the region",
            Why::Kind => "role built on the other device kind",
            Why::Component => "component not built by the region",
            Why::Count => "number of nodes of a role",
            Why::Port => "what a port reads",
            Why::Same => "two ports read as one node",
            Why::Timing => "edge timing",
            Why::Devices => "device boundary",
            Why::Api => "api",
            Why::Other => "other",
        }
    }

    /// The reason `accept`'s message `m` names.
    pub fn of(m: &str) -> Why {
        if m.contains("no region builds this shape") || m.contains("no region of the") {
            Why::NoRegion
        } else if m.contains("builds no pipeline of this shape") {
            Why::OtherExecutor
        } else if m.contains("runs this edge") || m.contains("hits prev:") || m.contains("read the world's changes a frame late") {
            Why::Timing
        } else if m.contains("shades the hit records") || m.contains("GPU") && (m.contains("split") || m.contains("second GPU") || m.contains("one GPU")) {
            Why::Devices
        } else if m.contains(" node (only ") {
            Why::Role
        } else if m.contains("reads") && m.contains("as one node") {
            Why::Same
        } else if m.contains("nothing of its own") || m.contains("reads") && (m.contains(" here, not ") || m.contains("nothing there")) {
            Why::Port
        } else if m.contains(" with ") && m.contains(", not ") && (m.contains("runs its") || m.contains("one api")) {
            Why::Api
        } else if m.contains("builds") && m.contains(" on ") && m.contains(", not on ") {
            Why::Kind
        } else if m.contains("(here: ") && m.contains("builds") {
            Why::Count
        } else if m.contains("builds") && m.contains(", not `") {
            Why::Component
        } else {
            Why::Other
        }
    }
}

/// A valid graph `accept` refuses, with its message.
#[derive(Clone, Debug)]
pub struct Refused {
    pub graph: Graph,
    pub why: Why,
    pub message: String,
}

/// The graphs one edit from `g` the executors' tests do not reach by [`edits`]: each
/// non-world node moved to the other device kind (and a stage node to a second GPU), each
/// bound input timed `prev`.
fn moves(g: &Graph) -> Vec<Graph> {
    let mut v = Vec::new();
    for (id, n) in &g.nodes {
        if matches!(n.role, Role::World | Role::Generator | Role::Client | Role::Gate) {
            continue;
        }
        for d in [Device::Cpu, Device::Gpu(0), Device::Gpu(1)] {
            if n.device == Some(d) {
                continue;
            }
            let mut h = g.clone();
            h.nodes.get_mut(id).unwrap().device = Some(d);
            v.push(h);
        }
        for p in n.inputs.keys() {
            if n.when(p) == When::Prev {
                continue;
            }
            let mut h = g.clone();
            h.nodes.get_mut(id).unwrap().timing.insert(p.clone(), When::Prev);
            v.push(h);
        }
    }
    v
}

/// The valid graphs one edit from the shipped files (rebinding, unbinding, a new node, a
/// component swapped: [`edits`]; a node moved to another device or an edge timed `prev`:
/// [`moves`]) that `accept` refuses, each once (by canonical form), in a stable order.
pub fn refused() -> Vec<Refused> {
    let mut seen = HashSet::new();
    let mut v = Vec::new();
    for (name, _) in SHIPPED {
        let g = shipped(name).expect("a shipped file");
        let all = edits(&g).into_iter().chain(moves(&g));
        for h in all.map(Graph::pruned) {
            let Ok(ck) = h.check() else { continue };
            let Some(key) = ck.graph.canonical().ok() else { continue };
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Err(message) = h.accept() {
                v.push((key, Refused { graph: h, why: Why::of(&message), message }));
            }
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.into_iter().map(|(_, r)| r).collect()
}

/// How many of `refused` fall under each reason.
pub fn refused_counts(refused: &[Refused]) -> BTreeMap<Why, usize> {
    let mut m = BTreeMap::new();
    for r in refused {
        *m.entry(r.why).or_default() += 1;
    }
    m
}

/// The table of [`refused`] (CLI `multirenderer audit --refused`): per gap and reason the
/// count and one example message.
pub fn refused_markdown() -> String {
    let r = refused();
    let mut s = format!(
        "{} valid graphs one edit from the shipped files are refused by `accept`.\n\n| gap | reason | graphs | example |\n|---|---|---|---|\n",
        r.len()
    );
    for (why, n) in refused_counts(&r) {
        let ex = r.iter().find(|x| x.why == why).map_or("", |x| x.message.as_str());
        let ex: String = ex.chars().take(160).collect();
        s += &format!("| {} | {} | {n} | {} |\n", why.gap().name(), why.name(), ex.replace('|', "\\|"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the table and keeps every reason's count from growing (lower a bound when a
    /// gap closes; G45's target is none).
    #[test]
    fn valid_graphs_refused_only_go_down() {
        let r = refused();
        let counts = refused_counts(&r);
        println!("{}", refused_markdown());
        if std::env::var_os("MR_REFUSED_ALL").is_some() {
            for x in &r {
                println!("{:?}\t{}\t{}", x.why, x.graph.label().unwrap_or_default(), x.message);
                // `MR_REFUSED_ALL=graphs`: each graph's canonical form under its line.
                if std::env::var("MR_REFUSED_ALL").as_deref() == Ok("graphs") {
                    println!("{}", x.graph.canonical().unwrap_or_default());
                }
            }
        }
        let most: &[(Why, usize)] = &[
            (Why::NoRegion, 1),
            // Graphs with no region before (2026-11-27: the GPU drawer reading the geometry's
            // faces straight now has the meshing region's shape) are refused now by what the
            // region does not build: a few reasons below rose while the total fell (129 -> 95).
            // discover-cpu with its drawer moved to the CPU, and CPU graphs whose drawer moved
            // to a GPU: the other executor's shape (cross-device graphs).
            (Why::OtherExecutor, 9),
            // Meshes (`composite`) in the ray-query caster (2026-11-28): 5 -> 3; meshes in
            // GPU discovery's drawer (2026-11-28): 3 -> 1.
            (Why::Role, 1),
            (Why::Kind, 11),
            // Boxes and points drawn by the GPU rasteriser as their exposed faces (2026-11-27):
            // component 17 -> 11, port 11 -> 7. The ray-query caster with `depth` meshes
            // (2026-11-28) is refused by its component as the walk's is (+1); the GPU ray
            // caster over a heightmap's march (the columns region's, 2026-11-28: -1): 11.
            // `composite` and `depth` one another's alias where a region draws the other
            // (user, 2026-11-28): 11 -> 8.
            (Why::Component, 8),
            // CPU discovery's exposure tests in another storage than its rays walk (2026-11-27,
            // on the GPU drawer): 23 -> 6.
            (Why::Count, 6),
            (Why::Port, 7),
            // The world's changes timed per copy of the world on the GPU executor (2026-11-27:
            // the geometry its own copy, the CPU storages theirs): 12 -> 6.
            (Why::Timing, 6),
            // One graph refused by its timing before (2026-11-27) now meets it and is
            // refused by its devices.
            (Why::Devices, 6),
            (Why::Api, 0),
            (Why::Other, 0),
        ];
        for (why, n) in &counts {
            let bound = most.iter().find(|(w, _)| w == why).map_or(usize::MAX, |(_, b)| *b);
            assert!(*n <= bound, "{}: {n} valid graphs refused, at most {bound}", why.name());
        }
    }
}
