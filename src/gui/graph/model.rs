//! The pipeline editor's edits: each is a change to the renderer's graph
//! (`mr_components::graph::Change`, the edits plans write too), made exactly and
//! tried as the renderer would load it (a graph that does not run yet is a draft). A choice
//! whose graph cannot run carries the reason.

use std::collections::{BTreeMap, HashMap};

pub use mr_components::graph::{Change, Target};
use mr_components::graph::{Data, Graph, Role, When, inputs, output, role_components};
use mr_core::RendererInfo;

use crate::plan::RendererSpec;

/// The roles of the world's side: one node each, kept whatever reads them.
pub fn world_side(role: Role) -> bool {
    matches!(role, Role::World | Role::Generator | Role::Link | Role::Client)
}

/// The renderer `base` running graph `g`, as a plan would load it (its options pruned to
/// those its executor knows), or why it cannot run.
pub fn load(base: &RendererSpec, g: &Graph, infos: &[RendererInfo]) -> Result<RendererSpec, String> {
    let mut s = base.clone();
    s.graph = Some(g.clone());
    s.name.clear();
    s.resolve().map_err(|e| format!("{e:#}").trim_start_matches("renderer graph: ").to_string())?;
    // Only its knobs are read: any devices do.
    if let Some(info) = s.info(&crate::plan::RunSpec::default(), infos) {
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

/// Tries changes against one renderer, remembering the outcomes (a picker asks every frame).
/// A change is made exactly, and one whose graph does not run (yet) can still be made, as a
/// draft the editor shows with the reason while the renderer keeps running its last graph.
pub struct Trial {
    base: RendererSpec,
    graph: Graph,
    draft: Option<(Graph, String)>,
    memo: HashMap<String, Result<(Graph, RendererSpec), String>>,
    /// What the live lane running this graph says is wrong with its nodes (`faults`), set
    /// each frame by whoever shows the lane; empty for a plan's renderer.
    pub faults: BTreeMap<String, Vec<String>>,
}

impl Trial {
    pub fn new(base: RendererSpec, graph: Graph) -> Self {
        Self { base, graph, draft: None, memo: HashMap::new(), faults: BTreeMap::new() }
    }

    /// The graph edited: the draft if there is one, else the one running.
    pub fn graph(&self) -> &Graph {
        self.draft.as_ref().map_or(&self.graph, |d| &d.0)
    }

    /// Why the draft does not run, if there is one.
    pub fn draft(&self) -> Option<&str> {
        self.draft.as_ref().map(|d| d.1.as_str())
    }

    /// Makes `change` whose graph does not run: the draft (a graph that runs is loaded
    /// instead, from `try_change`).
    pub fn make_draft(&mut self, change: &Change, infos: &[RendererInfo]) {
        let Ok(g) = self.graph().edit(change) else { return };
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
        let r = self.graph().edit(change).and_then(|g| self.run(&g, infos));
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
/// The client's `max_lod` offers the levels the pipeline holds (`features` of its graph;
/// `None`: not known, every level).
pub fn world_knobs(role: Role, component: &str, run: &crate::plan::RunSpec, features: Option<&mr_core::Features>) -> Vec<mr_core::ParamSpec> {
    use mr_core::{Feature, ParamSpec};
    match (role, component) {
        (Role::Client, _) => {
            let (max, why) = features.map_or((8, String::new()), |f| (f.max_lod(), f.why_not(Feature::Lod).map_or_else(String::new, |w| format!("\nThis pipeline holds no LOD levels: {w}"))));
            vec![
                ParamSpec::int("radius", "View radius in chunks this renderer holds (default: the run's view_radius)", 1, 64, run.view_radius as i64, false),
                ParamSpec::int("max_lod", &format!("Coarsest LOD level it holds (default: the run's max_lod, capped by what it supports){why}"), 0, max as i64, (run.max_lod.min(max)) as i64, false),
            ]
        }
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
    let mut v: Vec<&String> = g.nodes.iter().filter(|(_, n)| n.role.registered() && n.role.name() == slot && n.component == component).map(|(k, _)| k).collect();
    v.sort_by_key(|k| (k.as_str() != slot, k.len(), k.to_string()));
    v.first().map(|k| k.to_string())
}

/// What a live lane's failure and refused knobs say about its nodes: node id -> the
/// messages, for the editor to show on the node at fault. A failure names a node by its
/// component (`clipmap: ...`, the convention of build, device-limit and memory errors: a
/// component's error starts with its name), by its role when the code at fault serves the
/// role whatever its component (`storage: the voxel pool is full`), or by its id in
/// backticks, alone or before a port (`` `shading`.hits prev: ... ``, as `accept` words its
/// refusals); each `: `-separated
/// part of the message is tried, so the host's prefixes (`failed: worker error: `) do not
/// matter. A component used by several nodes marks each. A refused knob marks the node that
/// owns it. What names no node stays the lane's (its tile and panel show every failure).
pub fn faults(g: &Graph, failure: Option<&str>, refused: &[(String, String)], params: &[mr_core::ParamSpec]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(e) = failure {
        let parts: Vec<&str> = e.lines().next().unwrap_or("").split(": ").map(str::trim).collect();
        for (id, n) in &g.nodes {
            let tick = format!("`{id}`");
            let by_id = |p: &str| p.strip_prefix(tick.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', ' ']));
            let named = |p: &&str| *p == n.component || (n.role.registered() && *p == n.role.name()) || by_id(p);
            if !n.component.is_empty() && parts.iter().any(named) {
                out.entry(id.clone()).or_default().push(e.to_string());
            }
        }
    }
    for (name, error) in refused {
        let owners: Vec<String> = match params.iter().find(|s| s.name == *name).and_then(|s| s.owner.as_ref()) {
            Some((slot, comps)) => comps.iter().filter_map(|c| knob_owner(g, slot, c)).collect(),
            None => name.split_once('.').map(|(node, _)| node).filter(|node| g.nodes.contains_key(*node)).map(String::from).into_iter().collect(),
        };
        for id in owners {
            let what = format!("refused {name}: {error}");
            let v = out.entry(id).or_default();
            if !v.contains(&what) {
                v.push(what);
            }
        }
    }
    out
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
    use mr_components::graph::SHIPPED;

    fn spec(c: &str) -> RendererSpec {
        let mut s = RendererSpec::from_cli(c).unwrap();
        s.resolve().unwrap();
        s
    }

    /// Every shipped file's graph, written back as a graph, loads as the same pipeline.
    #[test]
    fn files_load_as_their_graphs() {
        assert!(SHIPPED.len() >= 10);
        for (name, _) in SHIPPED {
            let s = spec(name);
            let g = graph_of(&s).unwrap_or_else(|| panic!("{name}: no graph"));
            let l = load(&s, &g, &[]).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(l.pipeline().unwrap().key().unwrap(), s.pipeline().unwrap().key().unwrap(), "{name}");
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
        let (h, l) = t.try_change(&Change::Component(st.clone(), "tree64".into()), &[]).unwrap();
        assert!(l.same_kind(&s));
        let mut want = g.clone();
        want.nodes.get_mut(&st).unwrap().component = "tree64".into();
        assert_eq!(h, want);
        // The hits from a new drawer: raycast-gpu builds no draw node, and the ray caster is
        // left reaching no image. Refused; as a draft the drawer comes unbound, the caster stays.
        let to_draw = Change::Bind(shading.clone(), "hits".into(), Target::New(Role::Draw, "raster".into()));
        let why = t.try_change(&to_draw, &[]).unwrap_err();
        t.make_draft(&to_draw, &[]);
        assert_eq!(t.draft(), Some(why.as_str()));
        let draw = node(t.graph(), Role::Draw);
        // The reason names the node at fault (`draw`, unbound), which the canvas marks.
        assert_eq!(faults(t.graph(), Some(&why), &[], &[]).keys().collect::<Vec<_>>(), [&draw], "{why}");
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
        assert!(t.try_change(&Change::Component(vis, "quadtree".into()), &[]).is_err());
    }
}
