//! What a node executor builds: per role, the components it has code for, the device kind it
//! runs them on and how many such nodes it takes. A valid graph (`Graph::check`) with any
//! other node is refused at creation with the reason, never built half-way.

use super::{Checked, Graph, Node, Role, inputs, output, role_components};
use crate::Kind;

/// Nodes of one role an executor builds.
#[derive(Copy, Clone, Debug)]
pub struct Build {
    pub role: Role,
    pub components: &'static [&'static str],
    pub kind: Kind,
    /// How many nodes of the role it takes.
    pub least: usize,
    pub most: usize,
}

impl Build {
    /// One node of `role`, one of `components`, on `kind`.
    pub const fn one(role: Role, components: &'static [&'static str], kind: Kind) -> Build {
        Build { role, components, kind, least: 1, most: 1 }
    }

    /// Between `least` and `most` nodes.
    pub const fn count(self, least: usize, most: usize) -> Build {
        Build { least, most, ..self }
    }
}

/// Inputs an executor reads as one node: from the node of each role, the node reached by
/// following its input ports (e.g. the prepass's storage and the primary kernel's).
#[derive(Copy, Clone, Debug)]
pub struct Same {
    pub a: (Role, &'static [&'static str]),
    pub b: (Role, &'static [&'static str]),
}

/// What an executor builds: its nodes, and the inputs that must be one node.
#[derive(Copy, Clone, Debug)]
pub struct Builds {
    pub nodes: &'static [Build],
    pub same: &'static [Same],
}

/// The roles the worker runs in front of every executor (the world's side), and the post every
/// executor compiles into its shading.
const AROUND: &[Role] = &[Role::World, Role::Generator, Role::Link, Role::Client, Role::Post];

impl Checked {
    /// Whether `executor`, building `builds`, builds this graph; else the first node (or role)
    /// it cannot, with the reason.
    pub fn built_by(&self, executor: &str, builds: Builds) -> Result<(), String> {
        let Builds { nodes: builds, same } = builds;
        let list = |v: &[&str]| v.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
        for (id, n) in self.graph.nodes.iter().filter(|(_, n)| !AROUND.contains(&n.role)) {
            let Some(b) = builds.iter().find(|b| b.role == n.role) else {
                let roles: Vec<&str> = builds.iter().map(|b| b.role.name()).collect();
                return Err(format!("`{id}`: `{executor}` builds no {} node (only {})", n.role.name(), roles.join(", ")));
            };
            if !b.components.contains(&n.component.as_str()) {
                return Err(format!("`{id}`: `{executor}` builds {} {}, not `{}`", n.role.name(), list(b.components), n.component));
            }
            let kind = self.devices[id.as_str()].kind();
            if kind != b.kind {
                return Err(format!("`{id}`: `{executor}` builds {} on {}, not on {}", n.role.name(), b.kind.name(), kind.name()));
            }
        }
        for b in builds {
            let ids: Vec<&str> = self.graph.nodes.iter().filter(|(_, n)| n.role == b.role).map(|(id, _)| id.as_str()).collect();
            if ids.len() < b.least || ids.len() > b.most {
                let want = if b.least == b.most { format!("{}", b.least) } else if ids.len() < b.least { format!("at least {}", b.least) } else { format!("at most {}", b.most) };
                let here = if ids.is_empty() { "none".into() } else { list(&ids) };
                return Err(format!("`{executor}` builds {want} {} node{} (here: {here})", b.role.name(), if want == "1" { "" } else { "s" }));
            }
        }
        let path = |s: (Role, &[&str])| -> Option<&str> { s.1.iter().try_fold(self.find(s.0)?, |id, p| self.graph.nodes[id].inputs.get(*p).map(String::as_str)) };
        let name = |s: (Role, &[&str])| format!("{}.{}", s.0.name(), s.1.join("."));
        for s in same {
            if let (Some(a), Some(b)) = (path(s.a), path(s.b))
                && a != b
            {
                return Err(format!("`{executor}` reads {} and {} as one node (here: `{a}` and `{b}`)", name(s.a), name(s.b)));
            }
        }
        Ok(())
    }
}

/// The graphs one edit from `g` (unchecked): each input rebound to another node of its type,
/// an optional input unbound, an input bound to a new node of any component with its inputs
/// bound to the first nodes of their types, each node's component swapped. Tests feed them
/// (and their edits) to the executors.
pub fn edits(g: &Graph) -> Vec<Graph> {
    let mut v = Vec::new();
    let ids: Vec<&String> = g.nodes.keys().collect();
    let out = |id: &str| output(g.nodes[id].role, &g.nodes[id].component);
    for (id, n) in &g.nodes {
        let with = |f: &dyn Fn(&mut Graph)| {
            let mut h = g.clone();
            f(&mut h);
            h
        };
        for p in inputs(n.role, &n.component) {
            for src in ids.iter().filter(|s| **s != id && n.inputs.get(p.name) != Some(**s) && out(s) == p.data) {
                v.push(with(&|h| _ = h.nodes.get_mut(id).unwrap().inputs.insert(p.name.into(), src.to_string())));
            }
            if !p.required && n.inputs.contains_key(p.name) {
                v.push(with(&|h| _ = h.nodes.get_mut(id).unwrap().inputs.remove(p.name)));
            }
            for role in Role::ALL {
                for comp in role_components(role).into_iter().filter(|c| output(role, c) == p.data) {
                    let nid = format!("new-{comp}");
                    if g.nodes.contains_key(&nid) {
                        continue;
                    }
                    let mut nn = Node::new(role, comp);
                    for q in inputs(role, comp) {
                        if let Some(src) = ids.iter().find(|s| out(s) == q.data) {
                            nn.inputs.insert(q.name.into(), src.to_string());
                        }
                    }
                    v.push(with(&|h| {
                        h.nodes.insert(nid.clone(), nn.clone());
                        h.nodes.get_mut(id).unwrap().inputs.insert(p.name.into(), nid.clone());
                    }));
                }
            }
        }
        for comp in role_components(n.role).into_iter().filter(|c| *c != n.component) {
            v.push(with(&|h| h.nodes.get_mut(id).unwrap().component = comp.into()));
        }
    }
    v
}

/// The valid graphs one or two [`edits`] from the default graphs of `executors` (and their
/// path-tracing ones), placed, with the executor each lands on; each once.
pub fn near(executors: &[&str]) -> Vec<Checked> {
    let mut bases = vec![];
    for &e in executors {
        let mut c = crate::Composition::new(e);
        c.fill_defaults();
        bases.push(c.clone());
        if crate::names_on(e, crate::Slot::Shading).contains(&"pathtrace") {
            c.shading = "pathtrace".into();
            bases.push(c);
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut v = vec![];
    for c in &bases {
        let g = Graph::from_composition(c).expect("a default composition is a graph");
        for h in edits(&g).into_iter().map(Graph::pruned) {
            let two: Vec<Graph> = edits(&h).into_iter().map(Graph::pruned).collect();
            for x in std::iter::once(h).chain(two) {
                let Ok(ck) = x.placed().check() else { continue };
                if executors.contains(&ck.graph.executor.as_str()) && seen.insert(ck.graph.canonical().unwrap_or_default()) {
                    v.push(ck);
                }
            }
        }
    }
    v
}
