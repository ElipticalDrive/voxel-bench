//! The pipeline editor's edits: each is a change to the renderer's graph, made exactly and
//! tried as the renderer would load it (a graph that does not run yet is a draft). A choice
//! whose graph cannot run carries the reason.

use std::collections::{BTreeMap, HashMap};

use mr_components::graph::{Data, Device, Graph, Node, Role, When, inputs, output, role_components};
use mr_components::Api;
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
    /// The node gone, the inputs that read it unbound.
    Remove(String),
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

fn fresh_id(g: &Graph, want: &str) -> String {
    if !g.nodes.contains_key(want) {
        return want.to_string();
    }
    (2..).map(|i| format!("{want}-{i}")).find(|k| !g.nodes.contains_key(k)).unwrap()
}

/// `change` made as it says and nothing else: a new node comes unbound, a node given
/// another component keeps the inputs it still has, nothing is pruned.
fn exact(g: &Graph, change: &Change) -> Graph {
    let mut g = g.clone();
    match change {
        Change::Component(id, c) => {
            let n = g.nodes.get_mut(id).unwrap();
            n.component = c.to_string();
            n.knobs.clear();
            n.api = None;
            let ports = inputs(n.role, c);
            n.inputs.retain(|p, _| ports.iter().any(|x| x.name == p));
            n.timing.retain(|p, _| ports.iter().any(|x| x.name == p));
        }
        Change::Bind(id, port, target) => {
            let at = match target {
                Target::Unbound => None,
                Target::Node(k) => Some(k.clone()),
                Target::New(role, c) => {
                    let k = fresh_id(&g, role.name());
                    g.nodes.insert(k.clone(), Node::new(*role, c));
                    Some(k)
                }
            };
            let n = g.nodes.get_mut(id).unwrap();
            match at {
                Some(at) => drop(n.inputs.insert(port.clone(), at)),
                None => {
                    n.inputs.remove(port);
                    n.timing.remove(port);
                }
            }
        }
        Change::Device(id, d) => g.nodes.get_mut(id).unwrap().device = *d,
        Change::Api(id, a) => g.nodes.get_mut(id).unwrap().api = *a,
        Change::Timing(id, port, w) => {
            let n = g.nodes.get_mut(id).unwrap();
            match w {
                When::Now => drop(n.timing.remove(port)),
                w => drop(n.timing.insert(port.clone(), *w)),
            }
        }
        Change::Knob(id, k, v) => {
            let n = g.nodes.get_mut(id).unwrap();
            match v {
                Some(v) => drop(n.knobs.insert(k.clone(), v.clone())),
                None => drop(n.knobs.remove(k)),
            }
        }
        Change::Remove(id) => {
            g.nodes.remove(id);
            for n in g.nodes.values_mut() {
                let gone: Vec<String> = n.inputs.iter().filter(|(_, s)| *s == id).map(|(p, _)| p.clone()).collect();
                for p in gone {
                    n.inputs.remove(&p);
                    n.timing.remove(&p);
                }
            }
        }
    }
    g
}

/// Tries changes against one renderer, remembering the outcomes (a picker asks every frame).
/// A change is made exactly, and one whose graph does not run (yet) can still be made, as a
/// draft the editor shows with the reason while the renderer keeps running its last graph.
pub struct Trial {
    base: RendererSpec,
    graph: Graph,
    draft: Option<(Graph, String)>,
    memo: HashMap<String, Result<(Graph, RendererSpec), String>>,
}

impl Trial {
    pub fn new(base: RendererSpec, graph: Graph) -> Self {
        Self { base, graph, draft: None, memo: HashMap::new() }
    }

    /// The graph edited: the draft if there is one, else the one running.
    pub fn graph(&self) -> &Graph {
        self.draft.as_ref().map_or(&self.graph, |d| &d.0)
    }

    pub fn base(&self) -> &RendererSpec {
        &self.base
    }

    /// Why the draft does not run, if there is one.
    pub fn draft(&self) -> Option<&str> {
        self.draft.as_ref().map(|d| d.1.as_str())
    }

    /// Makes `change` whose graph does not run: the draft (a graph that runs is loaded
    /// instead, from `try_change`).
    pub fn make_draft(&mut self, change: &Change, infos: &[RendererInfo]) {
        let g = exact(self.graph(), change);
        if let Err(why) = self.run(&g, infos) {
            self.draft = Some((g, why));
            self.memo.clear();
        }
    }

    /// Back to the graph running.
    pub fn drop_draft(&mut self) {
        if self.draft.take().is_some() {
            self.memo.clear();
        }
    }

    fn run(&self, g: &Graph, infos: &[RendererInfo]) -> Result<(Graph, RendererSpec), String> {
        g.check().map_err(|e| e.to_string()).and_then(|ck| load(&self.base, &ck.graph, infos).map(|s| (ck.graph, s)))
    }

    /// The checked graph and the renderer it loads as, or the reason it cannot run.
    pub fn try_change(&mut self, change: &Change, infos: &[RendererInfo]) -> Result<(Graph, RendererSpec), String> {
        let key = format!("{change:?}");
        if let Some(r) = self.memo.get(&key) {
            return r.clone();
        }
        let r = self.run(&exact(self.graph(), change), infos);
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
    use mr_components::BACKENDS;

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

    fn node(g: &Graph, r: Role) -> String {
        g.nodes.iter().find(|(_, n)| n.role == r).unwrap().0.clone()
    }

    /// A change is made exactly: nothing filled in, nothing dropped; one that does not run can still be made as a draft, and editing on from the
    /// draft reaches graphs that run again.
    #[test]
    fn free_changes_are_exact() {
        let s = spec("raycast-gpu");
        let g = graph_of(&s).unwrap();
        let mut t = Trial::new(s.clone(), g.clone());
        let (st, vis, shading) = (node(&g, Role::Storage), node(&g, Role::Visibility), node(&g, Role::Shading));
        // Another storage: only that node changes.
        let (h, l) = t.try_change(&Change::Component(st.clone(), "tree64"), &[]).unwrap();
        assert_eq!(l.name, "raycast-gpu");
        let mut want = g.clone();
        want.nodes.get_mut(&st).unwrap().component = "tree64".into();
        assert_eq!(h, want);
        // The hits from a new drawer: raycast-gpu builds no draw node, and the ray caster is
        // left reaching no image. Refused; as a draft the drawer comes unbound, the caster stays.
        let to_draw = Change::Bind(shading.clone(), "hits".into(), Target::New(Role::Draw, "raster"));
        let why = t.try_change(&to_draw, &[]).unwrap_err();
        t.make_draft(&to_draw, &[]);
        assert_eq!(t.draft(), Some(why.as_str()));
        let draw = node(t.graph(), Role::Draw);
        assert!(t.graph().nodes[&draw].inputs.is_empty());
        assert!(t.graph().nodes.contains_key(&vis));
        // Removing the drawer leaves the hits unbound: still a draft.
        let rm = Change::Remove(draw.clone());
        assert!(t.try_change(&rm, &[]).is_err());
        t.make_draft(&rm, &[]);
        assert!(!t.graph().nodes.contains_key(&draw) && !t.graph().nodes[&shading].inputs.contains_key("hits"));
        // Bound to the caster again: the graph running, as it was.
        let (h, _) = t.try_change(&Change::Bind(shading.clone(), "hits".into(), Target::Node(vis.clone())), &[]).unwrap();
        assert_eq!(h, g);
        t.drop_draft();
        assert!(t.draft().is_none() && *t.graph() == g);
        // Discovery on raycast-gpu: refused as it is (no regraft onto discover-gpu's shape).
        assert!(t.try_change(&Change::Component(vis, "quadtree"), &[]).is_err());
    }
}
