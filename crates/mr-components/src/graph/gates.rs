//! Gates on edges ([`Role::Gate`]): taken out of a graph before it is checked and kept
//! beside it, so a gated graph is built as the same graph without its gates and the gates
//! run on the edges they sit on.

use std::collections::BTreeMap;

use mr_core::gate::GateSpec;

use super::*;

/// The gates on one edge, in order from the producer.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGates {
    /// The node reading through them and its port.
    pub reader: String,
    pub port: String,
    /// What it reads through them (its input in the checked graph).
    pub source: String,
    /// What crosses them.
    pub data: Data,
    /// The gate nodes' ids and knobs.
    pub gates: Vec<(String, GateSpec)>,
    /// Whether they carry the world's side into the client: the server's stream (gates
    /// reading the world, whoever reads them: the world's changes reach a renderer only
    /// through its client) or a client generator's chunks. Other gates on changes are on
    /// the client's changes into the renderer.
    pub into_client: bool,
}

/// How reports name the edge behind the graph's output (the frame it delivers).
pub const OUTPUT_PORT: &str = "frame";

impl EdgeGates {
    /// The edge as reports name it: `reader.port`, or `frame` behind the graph's output.
    pub fn edge(&self) -> String {
        if self.behind_output() { OUTPUT_PORT.to_string() } else { format!("{}.{}", self.reader, self.port) }
    }

    /// Whether the gates are behind the graph's output (no node reads them: the frame
    /// crosses them, in the worker).
    pub fn behind_output(&self) -> bool {
        self.reader.is_empty()
    }

    /// Whether the worker runs them, for every executor: on the world's side (its changes
    /// into and out of the client, a client generator's chunks) and behind the output (the
    /// frame). The rest are inside the pipeline, the executor's.
    pub fn in_worker(&self) -> bool {
        self.behind_output() || matches!(self.data, Data::Changes | Data::Chunks)
    }

    pub fn specs(&self) -> Vec<GateSpec> {
        self.gates.iter().map(|(_, s)| *s).collect()
    }
}

/// The network gate node `n` models (its knobs read as a `GateSpec`).
pub fn gate_spec(n: &Node) -> Result<GateSpec, String> {
    let s: GateSpec = toml::Value::Table(n.knobs.clone())
        .try_into()
        .map_err(|e: toml::de::Error| format!("gate `{}`: {} (knobs: {})", n.component, e.message(), GateSpec::KNOBS.join(", ")))?;
    s.check()?;
    Ok(s)
}

/// A gate's knobs as the canonical form writes them: off their defaults, read back.
pub(super) fn gate_knobs(spec: &GateSpec) -> toml::Table {
    let mut t = toml::Table::try_from(spec).expect("a GateSpec is a table");
    let none = toml::Table::try_from(GateSpec::default()).unwrap();
    t.retain(|k, v| none.get(k) != Some(v));
    t
}

impl Graph {
    /// The graph without its gates, every reader of a gate reading what the gate reads (through
    /// any chain of gates), and the gates by edge; with the reasons when a gate is not well
    /// formed. The edges' data are filled in by [`Graph::check`].
    pub(super) fn ungated(&self) -> Result<(Graph, Vec<EdgeGates>), String> {
        let is_gate = |id: &str| self.nodes.get(id).is_some_and(|n| n.role == Role::Gate);
        if !self.nodes.values().any(|n| n.role == Role::Gate) {
            return Ok((self.clone(), Vec::new()));
        }
        let mut why = Vec::new();
        let mut specs: BTreeMap<&str, GateSpec> = BTreeMap::new();
        for (id, n) in self.nodes.iter().filter(|(_, n)| n.role == Role::Gate) {
            if !known(n.role, &n.component) {
                why.push(format!("`{id}`: unknown gate component `{}` (have: {})", n.component, host_components(Role::Gate).join(", ")));
                continue;
            }
            match gate_spec(n) {
                Ok(s) => {
                    specs.insert(id, s);
                }
                Err(e) => why.push(format!("`{id}`: {e}")),
            }
            if n.device.is_some_and(|d| d != Device::Cpu) {
                why.push(format!("`{id}`: a gate serializes on the cpu (what crosses it from a GPU is read back), not on {}", n.device.unwrap()));
            }
            if n.api.is_some() || !n.timing.is_empty() {
                why.push(format!("`{id}`: a gate has no API and no timing (time it with its knobs)"));
            }
            match n.inputs.iter().collect::<Vec<_>>().as_slice() {
                [(p, src)] if p.as_str() == "in" && self.nodes.contains_key(*src) => {}
                [(p, src)] if p.as_str() == "in" => why.push(format!("`{id}`.in: no node `{src}`")),
                _ => why.push(format!("`{id}`: a gate reads one node, on its input `in`")),
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        let mut g = self.clone();
        g.nodes.retain(|_, n| n.role != Role::Gate);
        let mut edges = Vec::new();
        let mut read: BTreeMap<&str, bool> = specs.keys().map(|k| (*k, false)).collect();
        for (id, n) in &self.nodes {
            for (p, src) in &n.inputs {
                if n.role == Role::Gate {
                    if is_gate(src) {
                        read.insert(src, true);
                    }
                    continue;
                }
                if !is_gate(src) {
                    continue;
                }
                let mut chain = Vec::new();
                let mut s = src.as_str();
                while is_gate(s) {
                    if chain.iter().any(|(c, _)| c == s) {
                        return Err(format!("the gates `{}` read each other in a cycle", chain.iter().map(|(c, _): &(String, GateSpec)| c.as_str()).collect::<Vec<_>>().join("`, `")));
                    }
                    read.insert(s, true);
                    chain.push((s.to_string(), specs[s]));
                    s = &self.nodes[s].inputs["in"];
                }
                chain.reverse();
                g.nodes.get_mut(id).unwrap().inputs.insert(p.clone(), s.to_string());
                edges.push(EdgeGates { reader: id.clone(), port: p.clone(), source: s.to_string(), data: Data::Image, gates: chain, into_client: false });
            }
        }
        // A gate no node reads is behind the graph's output: the frame crosses it (checked to
        // be the output's image in `check`).
        let unread: Vec<&str> = read.iter().filter(|(_, r)| !**r).map(|(k, _)| *k).collect();
        if unread.len() > 1 {
            return Err(format!("gates that no node reads: {} (one chain of gates may follow the graph's output)", unread.join(", ")));
        }
        if let Some(last) = unread.first() {
            let mut chain: Vec<(String, GateSpec)> = Vec::new();
            let mut s = *last;
            while is_gate(s) {
                if chain.iter().any(|(c, _)| c == s) {
                    return Err(format!("the gates `{}` read each other in a cycle", chain.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>().join("`, `")));
                }
                chain.push((s.to_string(), specs[s]));
                s = &self.nodes[s].inputs["in"];
            }
            chain.reverse();
            edges.push(EdgeGates { reader: String::new(), port: OUTPUT_PORT.into(), source: s.to_string(), data: Data::Image, gates: chain, into_client: false });
        }
        Ok((g, edges))
    }
}

impl Checked {
    /// The gates on `reader`'s input `port`, if any.
    pub fn gates_on(&self, reader: &str, port: &str) -> Option<&EdgeGates> {
        self.gates.iter().find(|e| e.reader == reader && e.port == port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(src: &str, knobs: &str) -> Node {
        let mut n = Node::new(Role::Gate, "net");
        n.inputs.insert("in".into(), src.into());
        n.knobs = knobs.parse().unwrap();
        n
    }

    /// Gates on the world, hit-record and frame edges, two on one of them: the checked graph
    /// is the graph without them (same schedule and region), the gates kept per edge with
    /// what crosses; keys differ by the gates, and a graph without gates keeps its key.
    #[test]
    fn gates_on_edges() {
        let base = shipped("raycast-cpu").unwrap();
        let plain = base.check().unwrap();
        let mut g = base.clone();
        g.nodes.insert("g.world".into(), gate("world", "latency_ms = 30\nloss = 0.01\nresend = true"));
        g.nodes.insert("g.hits1".into(), gate("visibility", "loss = 0.1"));
        g.nodes.insert("g.hits2".into(), gate("g.hits1", "bandwidth_mbps = 100"));
        g.nodes.get_mut("storage").unwrap().inputs.insert("changes".into(), "g.world".into());
        g.nodes.get_mut("shading").unwrap().inputs.insert("hits".into(), "g.hits2".into());
        g.nodes.insert("g.frame".into(), gate("shading", "latency_ms = 5"));
        g.nodes.insert("post".into(), {
            let mut p = Node::new(Role::Post, "clamp");
            p.inputs.insert("image".into(), "g.frame".into());
            p
        });
        let ck = g.check().unwrap();
        assert_eq!(ck.graph.nodes.len(), plain.graph.nodes.len());
        assert_eq!(ck.region.map(|r| r.id), plain.region.map(|r| r.id));
        assert_eq!(ck.schedule().unwrap().to_string(), plain.schedule().unwrap().to_string());
        let on = |r: &str, p: &str| ck.gates_on(r, p).unwrap();
        assert_eq!((on("storage", "changes").data, on("storage", "changes").source.as_str()), (Data::Changes, "client"));
        assert_eq!(on("storage", "changes").gates[0].1.latency_ms, 30.0);
        let hits = on("shading", "hits");
        assert_eq!((hits.data, hits.source.as_str()), (Data::Hits, "visibility"));
        assert_eq!(hits.gates.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["g.hits1", "g.hits2"]);
        assert_eq!(on("post", "image").data, Data::Image);
        assert_eq!(ck.gates.len(), 3);
        // Behind the output: the frame, by any chain of gates no node reads.
        let mut out = g.clone();
        out.nodes.insert("g.out1".into(), gate("post", "loss = 0.2"));
        out.nodes.insert("g.out2".into(), gate("g.out1", ""));
        let ck = out.check().unwrap();
        let frame = ck.gates.iter().find(|e| e.behind_output()).unwrap();
        assert_eq!((frame.edge().as_str(), frame.source.as_str(), frame.data), ("frame", "post", Data::Image));
        assert_eq!(frame.gates.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["g.out1", "g.out2"]);
        assert!(out.canonical().unwrap().contains("|gate output.frame (loss=0.2) ()"), "{}", out.canonical().unwrap());
        assert_ne!(out.key().unwrap(), g.key().unwrap());
        assert_ne!(g.key().unwrap(), base.key().unwrap());
        assert_eq!(base.key().unwrap(), plain.graph.key().unwrap());
        assert!(g.canonical().unwrap().contains("|gate "), "{}", g.canonical().unwrap());
        let label = g.label().unwrap();
        assert!(label.contains("gate direct.hits (loss=0.1) (bandwidth_mbps=100.0)"), "{label}");
        // `latency_ms = 30` and `30.0` are one network.
        let mut h = g.clone();
        h.nodes.get_mut("g.world").unwrap().knobs.insert("latency_ms".into(), toml::Value::Float(30.0));
        assert_eq!(h.key().unwrap(), g.key().unwrap());
    }

    /// What a gate refuses, with the reason.
    #[test]
    fn refused() {
        let base = shipped("raycast-cpu").unwrap();
        let with = |id: &str, n: Node, reader: &str, port: &str| {
            let mut g = base.clone();
            g.nodes.insert(id.into(), n);
            g.nodes.get_mut(reader).unwrap().inputs.insert(port.into(), id.into());
            g.check().unwrap_err()
        };
        assert!(with("g", gate("storage", ""), "kernel", "storage").contains("compiled into their caller"));
        assert!(with("g", gate("visibility", "loss = 2.0"), "shading", "hits").contains("loss"));
        assert!(with("g", gate("visibility", "speed = 1"), "shading", "hits").contains("speed"));
        assert!(with("g", gate("nowhere", ""), "shading", "hits").contains("no node `nowhere`"));
        let mut gpu = gate("visibility", "");
        gpu.device = Some(Device::Gpu(0));
        assert!(with("g", gpu, "shading", "hits").contains("serializes on the cpu"));
        // A gate no node reads is behind the output: it must read the output.
        let mut g = base.clone();
        g.nodes.insert("g".into(), gate("visibility", ""));
        assert!(g.check().unwrap_err().contains("reads `visibility`, not the output"));
        let mut g = base.clone();
        g.nodes.insert("g".into(), gate("visibility", ""));
        g.nodes.insert("h".into(), gate("shading", ""));
        assert!(g.check().unwrap_err().contains("one chain of gates may follow"));
        let mut g = base.clone();
        g.nodes.insert("a".into(), gate("b", ""));
        g.nodes.insert("b".into(), gate("a", ""));
        g.nodes.get_mut("shading").unwrap().inputs.insert("hits".into(), "a".into());
        assert!(g.check().unwrap_err().contains("cycle"));
        let mut g = base.clone();
        g.nodes.insert("g".into(), gate("visibility", ""));
        let sh = g.nodes.get_mut("shading").unwrap();
        sh.inputs.insert("hits".into(), "g".into());
        sh.timing.insert("hits".into(), When::Prev);
        // A previous-frame edge: refused where the executor does not hand its value over
        // itself (here the CPU ray caster), naming the gate.
        let why = g.check().and_then(|c| c.built()).unwrap_err();
        assert!(why.contains("`g` on `shading.hits`") && why.contains("timing `prev`"), "{why}");
        // GPU discovery's hole detection: last frame's sky mask and the drawer's faces in
        // its loop cross gates (what crossed is read from the reader's own copy next
        // frame), except with the row merge, whose runs hole detection cannot draw.
        let holes = shipped("discover-gpu").unwrap().with_edits(&[
            Change::add("holes", Role::Holes, "coverage"),
            Change::bind("holes", "covered", "draw"),
            Change::bind("holes", "sky", "draw"),
            Change::Timing("holes".into(), "sky".into(), When::Prev),
            Change::bind("holes", "changes", "world"),
            Change::bind("visibility", "where", "holes"),
            Change::Timing("draw".into(), "faces".into(), When::Iterate(2)),
        ]);
        let holes = holes.unwrap();
        let mut sky = holes.clone();
        sky.nodes.insert("g".into(), gate("draw", ""));
        sky.nodes.get_mut("holes").unwrap().inputs.insert("sky".into(), "g".into());
        sky.check().and_then(|c| c.built()).unwrap();
        let mut faces = holes.clone();
        faces.nodes.insert("g".into(), gate("visibility", ""));
        faces.nodes.get_mut("draw").unwrap().inputs.insert("faces".into(), "g".into());
        faces.check().and_then(|c| c.built()).unwrap();
        faces.nodes.get_mut("visibility").unwrap().knobs.insert("merge".into(), "rows".into());
        let why = faces.check().and_then(|c| c.built()).unwrap_err();
        assert!(why.contains("`g` on `draw.faces`") && why.contains("merge"), "{why}");
    }
}
