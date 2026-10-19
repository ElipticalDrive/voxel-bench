//! The pipeline editor's edits: each is a change to the renderer's graph
//! that is completed (inputs a new component needs are filled from a preset that has it,
//! nodes no longer reaching the image dropped) and tried as the renderer would load it. A
//! choice whose graph cannot run carries the reason instead.

use std::collections::{BTreeMap, HashMap};

use mr_components::graph::{Data, Device, Graph, Node, Role, When, inputs, output, role_components};
use mr_components::{Api, BACKENDS, Composition};
use mr_core::RendererInfo;

use crate::plan::RendererSpec;

/// The roles of the world's side: one node each, kept whatever reads them.
pub fn world_side(role: Role) -> bool {
    matches!(role, Role::World | Role::Generator | Role::Link | Role::Client)
}

/// What an input port is bound to after an edit.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Node(String),
    New(Role, &'static str),
    Unbound,
}

/// One change to the graph.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Component(String, &'static str),
    Bind(String, String, Target),
    Device(String, Option<Device>),
    Api(String, Option<Api>),
    Timing(String, String, When),
    Knob(String, String, Option<toml::Value>),
}

/// The renderer `base` running graph `g`, as a plan would load it (its options pruned to
/// those its executor knows), or why it cannot run.
pub fn load(base: &RendererSpec, g: &Graph, infos: &[RendererInfo]) -> Result<RendererSpec, String> {
    let mut s = base.clone();
    s.graph = Some(g.clone());
    s.compose = None;
    s.sites.clear();
    s.name.clear();
    // The options selecting components are the composition's, derived again.
    s.options.retain(|k, _| !mr_components::Slot::ALL.iter().any(|sl| sl.name() == k));
    s.resolve().map_err(|e| format!("{e:#}").trim_start_matches("renderer graph: ").to_string())?;
    if let Some(info) = infos.iter().find(|i| i.name == s.name) {
        let before = s.options.len();
        s.options.retain(|k, _| info.params.iter().any(|p| p.name == k));
        if s.options.len() != before {
            s.resolve().map_err(|e| format!("{e:#}"))?;
        }
    }
    Ok(s)
}

/// The graph renderer `spec` runs, with the world's side, the client and the post filled in.
pub fn graph_of(spec: &RendererSpec) -> Option<Graph> {
    spec.pipeline()?.check().ok().map(|c| c.graph)
}

/// Every graph for the preset back ends' default pipelines, with `role` set to `component`
/// where that composition is valid: the shapes a new node's inputs are copied from.
fn templates(role: Role, component: &str, executor: &str) -> Vec<Graph> {
    let mut v: Vec<(bool, Graph)> = Vec::new();
    for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
        let mut c = Composition::new(b.name);
        c.fill_defaults();
        if let Some(slot) = role.slot() {
            if !b.has(slot) {
                continue;
            }
            c.set(slot, component);
            if c.validate().is_err() {
                continue;
            }
        }
        if let Ok(g) = Graph::from_composition(&c) {
            v.push((b.name != executor, g));
        }
    }
    v.sort_by_key(|(other, _)| *other);
    v.into_iter().map(|(_, g)| g).collect()
}

fn fresh_id(g: &Graph, want: &str) -> String {
    if !g.nodes.contains_key(want) {
        return want.to_string();
    }
    (2..).map(|i| format!("{want}-{i}")).find(|k| !g.nodes.contains_key(k)).unwrap()
}

/// Binds node `id`'s unbound required inputs the way template `t`'s node `tid` has them:
/// to a node of the same component already in `g` (shared), else to a copy.
fn fill_from(g: &mut Graph, id: &str, t: &Graph, tid: &str, map: &mut HashMap<String, String>) {
    let n = &g.nodes[id];
    let todo: Vec<(String, String, Option<When>)> = inputs(n.role, &n.component)
        .iter()
        .filter(|p| p.required && p.data != Data::Changes && !n.inputs.contains_key(p.name))
        .filter_map(|p| t.nodes[tid].inputs.get(p.name).map(|src| (p.name.to_string(), src.clone(), t.nodes[tid].timing.get(p.name).copied())))
        .collect();
    for (port, src, when) in todo {
        let Some(at) = import(g, t, &src, map) else { continue };
        let n = g.nodes.get_mut(id).unwrap();
        n.inputs.insert(port.clone(), at);
        if let Some(w) = when {
            n.timing.insert(port, w);
        }
    }
}

fn import(g: &mut Graph, t: &Graph, src: &str, map: &mut HashMap<String, String>) -> Option<String> {
    if let Some(m) = map.get(src) {
        return Some(m.clone());
    }
    let tn = &t.nodes[src];
    if world_side(tn.role) {
        return None;
    }
    if let Some((k, _)) = g.nodes.iter().find(|(_, n)| n.role == tn.role && n.component == tn.component) {
        map.insert(src.into(), k.clone());
        return Some(k.clone());
    }
    let id = fresh_id(g, src);
    g.nodes.insert(id.clone(), Node::new(tn.role, &tn.component));
    map.insert(src.into(), id.clone());
    fill_from(g, &id, t, src, map);
    Some(id)
}

/// Node `id`'s inputs fitted to its component: ports it lacks unbound, required ones it
/// misses filled from each template in turn (one candidate graph per template).
fn completions(g: &Graph, id: &str) -> Vec<Graph> {
    let mut g = g.clone();
    let n = g.nodes.get_mut(id).unwrap();
    let ports = inputs(n.role, &n.component);
    n.inputs.retain(|p, _| ports.iter().any(|x| x.name == p));
    n.timing.retain(|p, _| ports.iter().any(|x| x.name == p));
    let (role, component) = (n.role, n.component.clone());
    let missing = ports.iter().any(|p| p.required && p.data != Data::Changes && !g.nodes[id].inputs.contains_key(p.name));
    if !missing {
        return vec![g];
    }
    let v: Vec<Graph> = templates(role, &component, &g.executor)
        .into_iter()
        .filter_map(|t| {
            let tid = t.nodes.iter().find(|(_, n)| n.role == role && n.component == component)?.0.clone();
            let mut c = g.clone();
            fill_from(&mut c, id, &t, &tid, &mut HashMap::new());
            Some(c)
        })
        .collect();
    if v.is_empty() { vec![g] } else { v }
}

/// The preset shapes with `role` as `component`, the graph's other choices carried over
/// (each role it has once: its component and knobs; the world's side and the post), then
/// the same shapes as they come: for a change whose new shape differs (ray casting to
/// discovery: faces through a drawer instead of hits).
fn regrafts(g: &Graph, role: Role, component: &str) -> Vec<Graph> {
    let mut out = vec![];
    for t in templates(role, component, &g.executor) {
        let mut own = t.clone();
        for tn in own.nodes.values_mut().filter(|n| n.role != role && !world_side(n.role)) {
            if let [m] = g.nodes.values().filter(|n| n.role == tn.role).collect::<Vec<_>>()[..] {
                tn.component = m.component.clone();
                tn.knobs = m.knobs.clone();
            }
        }
        for mut t in [own, t] {
            // The world's side and the post as the graph has them (bound again on loading).
            let shading = t.nodes.iter().find(|(_, n)| n.role == Role::Shading).map(|(k, _)| k.clone());
            for (k, n) in g.nodes.iter().filter(|(_, n)| world_side(n.role) || n.role == Role::Post) {
                t.nodes.retain(|_, m| m.role != n.role);
                let mut n = n.clone();
                n.inputs.clear();
                n.timing.clear();
                if let (Role::Post, Some(s)) = (n.role, &shading) {
                    n.inputs.insert("image".into(), s.clone());
                }
                t.nodes.insert(k.clone(), n);
            }
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

/// The graphs `change` may give, most faithful first.
fn candidates(g: &Graph, change: &Change) -> Vec<Graph> {
    let mut g = g.clone();
    let regraft = match change {
        Change::Component(id, c) => regrafts(&g, g.nodes[id].role, c),
        Change::Bind(_, _, Target::New(role, c)) => regrafts(&g, *role, c),
        _ => vec![],
    };
    let mut out = match change {
        Change::Component(id, c) => {
            let n = g.nodes.get_mut(id).unwrap();
            n.component = c.to_string();
            n.knobs.clear();
            n.api = None;
            completions(&g, id)
        }
        Change::Bind(id, port, target) => {
            let at = match target {
                Target::Unbound => {
                    let n = g.nodes.get_mut(id).unwrap();
                    n.inputs.remove(port);
                    n.timing.remove(port);
                    return finish(vec![g]);
                }
                Target::Node(k) => k.clone(),
                Target::New(role, c) => {
                    let k = fresh_id(&g, role.name());
                    g.nodes.insert(k.clone(), Node::new(*role, c));
                    k
                }
            };
            g.nodes.get_mut(id).unwrap().inputs.insert(port.clone(), at.clone());
            match target {
                Target::New(..) => completions(&g, &at),
                _ => vec![g],
            }
        }
        Change::Device(id, d) => {
            // The node alone, else with everything that ran where it ran (the whole
            // pipeline moved, as switching back ends did).
            let mut all = g.clone();
            if let (Some(d), Ok(ck)) = (d, g.check()) {
                let was = ck.devices.get(id).copied();
                for (k, n) in all.nodes.iter_mut() {
                    if !world_side(n.role) && !output(n.role, &n.component).fused() && ck.devices.get(k).copied() == was {
                        n.device = Some(*d);
                    }
                }
            }
            g.nodes.get_mut(id).unwrap().device = *d;
            if all == g { vec![g] } else { vec![g, all] }
        }
        Change::Api(id, a) => {
            g.nodes.get_mut(id).unwrap().api = *a;
            vec![g]
        }
        Change::Timing(id, port, w) => {
            let n = g.nodes.get_mut(id).unwrap();
            match w {
                When::Now => drop(n.timing.remove(port)),
                w => drop(n.timing.insert(port.clone(), *w)),
            }
            vec![g]
        }
        Change::Knob(id, k, v) => {
            let n = g.nodes.get_mut(id).unwrap();
            match v {
                Some(v) => drop(n.knobs.insert(k.clone(), v.clone())),
                None => drop(n.knobs.remove(k)),
            }
            vec![g]
        }
    };
    // Where the change moves the graph to another preset, the devices that only repeated
    // the old one's placement are let go too; failing that, the changed node's optional
    // inputs (meshes, a prepass) are given up.
    let unpinned: Vec<Graph> = out.iter().map(|g| g.unpinned()).filter(|u| !out.contains(u)).collect();
    out.extend(unpinned);
    if let Change::Component(id, _) | Change::Device(id, _) | Change::Api(id, _) = change {
        let bare: Vec<Graph> = out.iter().filter_map(|g| without_optional(g, id)).filter(|b| !out.contains(b)).collect();
        out.extend(bare);
    }
    out.extend(regraft);
    finish(out)
}

/// Node `id` with its optional inputs unbound (the `changes` and secondary rays aside, which
/// the graph binds itself), if it has any bound.
fn without_optional(g: &Graph, id: &str) -> Option<Graph> {
    let mut g = g.clone();
    let n = g.nodes.get_mut(id)?;
    let drop: Vec<&str> = inputs(n.role, &n.component).iter().filter(|p| !p.required && !p.casts && p.data != Data::Changes && n.inputs.contains_key(p.name)).map(|p| p.name).collect();
    if drop.is_empty() {
        return None;
    }
    for p in drop {
        n.inputs.remove(p);
        n.timing.remove(p);
    }
    Some(g)
}

fn finish(gs: Vec<Graph>) -> Vec<Graph> {
    gs.into_iter().map(|g| g.pruned()).collect()
}

/// Tries changes against one renderer, remembering the outcomes (a picker asks every frame).
pub struct Trial {
    base: RendererSpec,
    graph: Graph,
    memo: HashMap<String, Result<(Graph, RendererSpec), String>>,
}

impl Trial {
    pub fn new(base: RendererSpec, graph: Graph) -> Self {
        Self { base, graph, memo: HashMap::new() }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn base(&self) -> &RendererSpec {
        &self.base
    }

    /// The checked graph and the renderer it loads as, or the reason it cannot run (the
    /// most faithful candidate's).
    pub fn try_change(&mut self, change: &Change, infos: &[RendererInfo]) -> Result<(Graph, RendererSpec), String> {
        let key = format!("{change:?}");
        if let Some(r) = self.memo.get(&key) {
            return r.clone();
        }
        let mut first = None;
        let mut found = None;
        for c in candidates(&self.graph, change) {
            match c.check().map_err(|e| e.to_string()).and_then(|ck| load(&self.base, &ck.graph, infos).map(|s| (ck.graph, s))) {
                Ok(r) => {
                    found = Some(r);
                    break;
                }
                Err(e) => {
                    first.get_or_insert(e);
                }
            }
        }
        let r = found.ok_or_else(|| first.unwrap_or_else(|| "nothing to bind it from".into()));
        self.memo.insert(key, r.clone());
        r
    }
}

/// Every node (other than `id`, and not depending on it within the frame) whose output is
/// `data`, by id.
pub fn producers_in(g: &Graph, data: Data, id: &str) -> Vec<String> {
    g.nodes.iter().filter(|(k, n)| *k != id && output(n.role, &n.component) == data).map(|(k, _)| k.clone()).collect()
}

/// Every component that could be added to produce `data` (not the world's side).
pub fn producers_new(data: Data) -> Vec<(Role, &'static str)> {
    Role::ALL.into_iter().filter(|r| !world_side(*r) && *r != Role::Post).flat_map(|r| role_components(r).into_iter().map(move |c| (r, c))).filter(|(r, c)| output(*r, c) == data).collect()
}

/// The nodes reading node `id`: (reader, port).
pub fn readers(g: &Graph, id: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = g.nodes.iter().flat_map(|(k, n)| n.inputs.iter().filter(|(_, s)| *s == id).map(move |(p, _)| (k.clone(), p.clone()))).collect();
    v.sort();
    v
}

/// The client's and the link's knobs (they are graph knobs, not the executor's parameters).
pub fn world_knobs(role: Role, component: &str, run: &crate::plan::RunSpec) -> Vec<mr_core::ParamSpec> {
    use mr_core::ParamSpec;
    match (role, component) {
        (Role::Client, _) => vec![
            ParamSpec::int("radius", "View radius in chunks this renderer holds (default: the run's view_radius)", 1, 64, run.view_radius as i64, false),
            ParamSpec::int("max_lod", "Coarsest LOD level it holds (default: the run's max_lod, capped by what it supports)", 0, 8, run.max_lod as i64, false),
        ],
        (Role::Link, "net") => vec![
            ParamSpec::float("latency_ms", "One-way delay of every message", 0.0, 1000.0, false, 0.0, false),
            ParamSpec::float("jitter_ms", "Extra delay per message, uniform in [0, jitter]", 0.0, 500.0, false, 0.0, false),
            ParamSpec::float("bandwidth_mbps", "The line's rate, megabits per second (0: unlimited)", 0.0, 10000.0, false, 0.0, false),
            ParamSpec::float("loss", "Fraction of messages lost (each loss delays its message by a timeout)", 0.0, 0.99, false, 0.0, false),
            ParamSpec::bool("reorder", "One ordered stream per chunk instead of one for everything", false, false),
            ParamSpec::int("seed", "Seed of the jitter and the losses", 0, 1 << 30, 0, false),
        ],
        _ => vec![],
    }
}

/// The node whose flat knobs (the executor's parameters owned by its role and component)
/// it shows: the first such node (a second instance's knobs are the graph form's).
pub fn knob_owner(g: &Graph, slot: &str, component: &str) -> Option<String> {
    let mut v: Vec<&String> = g.nodes.iter().filter(|(_, n)| n.role.slot().is_some_and(|s| s.name() == slot) && n.component == component).map(|(k, _)| k).collect();
    v.sort_by_key(|k| (k.as_str() != slot, k.len(), k.to_string()));
    v.first().map(|k| k.to_string())
}

/// The nodes in reading order: from the world's side to the output, each after what it reads.
pub fn order(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeMap::new();
    fn visit(g: &Graph, id: &str, seen: &mut BTreeMap<String, bool>, out: &mut Vec<String>) {
        if seen.contains_key(id) {
            return;
        }
        seen.insert(id.into(), true);
        let n = &g.nodes[id];
        for p in inputs(n.role, &n.component) {
            if let Some(s) = n.inputs.get(p.name).filter(|_| n.when(p.name) == When::Now) {
                visit(g, s, seen, out);
            }
        }
        out.push(id.into());
    }
    let mut roots: Vec<&String> = g.nodes.iter().filter(|(k, _)| !g.nodes.values().any(|n| n.inputs.values().any(|s| s == *k))).map(|(k, _)| k).collect();
    roots.sort();
    for r in roots {
        visit(g, r, &mut seen, &mut out);
    }
    for k in g.nodes.keys() {
        visit(g, k, &mut seen, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(c: &str) -> RendererSpec {
        let mut s = RendererSpec::from_cli(c).unwrap();
        s.resolve().unwrap();
        s
    }

    /// Every preset's graph, written back as a graph, loads as the same pipeline.
    #[test]
    fn presets_load_as_their_graphs() {
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let s = spec(b.name);
            let g = graph_of(&s).unwrap_or_else(|| panic!("{}: no graph", b.name));
            let l = load(&s, &g, &[]).unwrap_or_else(|e| panic!("{}: {e}", b.name));
            assert_eq!(l.pipeline().unwrap().key().unwrap(), s.pipeline().unwrap().key().unwrap(), "{}", b.name);
            assert_eq!(l.name, s.name);
        }
    }

    /// Switching the strategy brings the inputs it needs and drops what is left over.
    #[test]
    fn changes_complete_and_prune() {
        let s = spec("gpu-compute");
        let mut t = Trial::new(s.clone(), graph_of(&s).unwrap());
        let shading = t.graph().nodes.iter().find(|(_, n)| n.role == Role::Shading).unwrap().0.clone();
        // The shading's hits from a face drawer: gpu-raster's shape.
        let (g, l) = t.try_change(&Change::Bind(shading.clone(), "hits".into(), Target::New(Role::Draw, "raster")), &[]).unwrap();
        assert_eq!(l.name, "gpu-raster", "{}", l.label());
        assert!(g.nodes.values().any(|n| n.role == Role::Geometry));
        assert!(!g.nodes.values().any(|n| n.component == "raycast"));
        // A storage of another kind.
        let st = t.graph().nodes.iter().find(|(_, n)| n.role == Role::Storage).unwrap().0.clone();
        let (_, l) = t.try_change(&Change::Component(st, "tree64"), &[]).unwrap();
        assert!(l.label().contains("tree64"), "{}", l.label());
        // The ray caster on the CPU: the cpu preset.
        let vis = t.graph().nodes.iter().find(|(_, n)| n.role == Role::Visibility).unwrap().0.clone();
        let (_, l) = t.try_change(&Change::Device(vis, Some(Device::Cpu)), &[]).unwrap();
        assert_eq!(l.name, "cpu", "{}", l.label());
        // Discovery instead of ray casting: faces through a drawer, the storage kept.
        let vis = t.graph().nodes.iter().find(|(_, n)| n.role == Role::Visibility).unwrap().0.clone();
        let (_, l) = t.try_change(&Change::Component(vis, "quadtree"), &[]).unwrap();
        assert_eq!(l.name, "gpu-discover", "{}", l.label());
        // A palette storage has no GPU implementation: the pipeline moves to the CPU's preset.
        let st = t.graph().nodes.iter().find(|(_, n)| n.role == Role::Storage).unwrap().0.clone();
        let (_, l) = t.try_change(&Change::Component(st, "palette"), &[]).unwrap();
        assert_eq!(l.name, "cpu", "{}", l.label());
        // Shading last frame's hits on the one GPU: refused, with the reason.
        let why = t.try_change(&Change::Timing(shading, "hits".into(), When::Prev), &[]).unwrap_err();
        assert!(why.contains("second GPU"), "{why}");
    }
}
