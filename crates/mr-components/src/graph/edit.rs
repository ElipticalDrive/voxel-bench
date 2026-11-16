//! Edits: the changes a graph is given, one at a time, each made exactly as it says (a new
//! node comes unbound, a node given another component keeps the inputs it still has, nothing
//! is filled in or pruned). The editor makes them from its pickers and plans write them
//! (`edits = [...]` after `graph`); both go through [`Graph::edit`].
//!
//! Also the graph files shipped with the bench ([`SHIPPED`], `pipelines/*.toml`): graphs as
//! anyone saves them, built into the binary so an installed copy has them.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{Checked, Device, Graph, Node, Role, When, inputs};
use crate::Api;

/// What an input port is bound to after an edit.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Node(String),
    /// A new node of this role and component, under an id not taken yet (the role's name,
    /// then `<role>-2`, ...).
    New(Role, String),
    Unbound,
}

/// One change to a graph. In TOML, one inline table each:
///
/// ```toml
/// { node = "storage", component = "tree64" }
/// { node = "shading", input = "bounce", from = "kernel" }      # an existing node
/// { node = "shading", input = "bounce", from = { role = "traversal", component = "sphere" } }
/// { node = "shading", unbind = "bounce" }
/// { node = "shading", device = "gpu1" }                        # or "default"
/// { node = "kernel", api = "rayquery" }                        # or "default"
/// { node = "draw", input = "faces", when = "prev" }            # "now", { iterate = 2 }
/// { node = "shading", knob = "bounces", value = 2 }            # no value: unset
/// { node = "link", knobs = { latency_ms = 30, loss = 0.01 } }  # all its knobs: these
/// { add = "far", role = "storage", component = "tree64" }
/// { remove = "prepass" }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "toml::Table", into = "toml::Table")]
pub enum Change {
    /// A new node under this id, unbound.
    Add(String, Role, String),
    /// The node gone, the inputs that read it unbound.
    Remove(String),
    Component(String, String),
    Bind(String, String, Target),
    Device(String, Option<Device>),
    Api(String, Option<Api>),
    Timing(String, String, When),
    Knob(String, String, Option<toml::Value>),
    /// The node's knobs are these and no others.
    Knobs(String, toml::Table),
}

impl Change {
    /// Node `node` is component `component`.
    pub fn component(node: &str, component: &str) -> Change {
        Change::Component(node.into(), component.into())
    }

    /// Input `input` of `node` reads node `from`.
    pub fn bind(node: &str, input: &str, from: &str) -> Change {
        Change::Bind(node.into(), input.into(), Target::Node(from.into()))
    }

    /// A new node `id`, unbound.
    pub fn add(id: &str, role: Role, component: &str) -> Change {
        Change::Add(id.into(), role, component.into())
    }
}

const DEFAULT: &str = "default";

fn text(t: &mut toml::Table, k: &str) -> Result<Option<String>, String> {
    match t.remove(k) {
        None => Ok(None),
        Some(toml::Value::String(s)) => Ok(Some(s)),
        Some(v) => Err(format!("`{k}` is a string, not `{v}`")),
    }
}

fn typed<T: DeserializeOwned>(k: &str, v: toml::Value) -> Result<T, String> {
    v.clone().try_into().map_err(|e| format!("`{k} = {v}`: {}", e.to_string().trim()))
}

/// A value that may be written `"default"` (none set).
fn or_default<T: DeserializeOwned>(k: &str, v: toml::Value) -> Result<Option<T>, String> {
    if v.as_str() == Some(DEFAULT) { Ok(None) } else { typed(k, v).map(Some) }
}

fn new_node(k: &str, t: &mut toml::Table) -> Result<(Role, String), String> {
    let role = typed("role", t.remove("role").ok_or_else(|| format!("`{k}` needs `role` and `component`"))?)?;
    let component = text(t, "component")?.ok_or_else(|| format!("`{k}` needs `role` and `component`"))?;
    Ok((role, component))
}

fn rest(t: &toml::Table) -> Result<(), String> {
    match t.keys().next() {
        Some(k) => Err(format!("`{k}` does not belong in this edit (one change per edit)")),
        None => Ok(()),
    }
}

impl TryFrom<toml::Table> for Change {
    type Error = String;

    fn try_from(mut t: toml::Table) -> Result<Self, String> {
        let whole = toml::Value::Table(t.clone()).to_string();
        let mut read = || -> Result<Change, String> {
            if let Some(id) = text(&mut t, "add")? {
                let (role, component) = new_node("add", &mut t)?;
                return Ok(Change::Add(id, role, component));
            }
            if let Some(id) = text(&mut t, "remove")? {
                return Ok(Change::Remove(id));
            }
            let id = text(&mut t, "node")?.ok_or("an edit names its `node` (or is `add` / `remove`)")?;
            if let Some(c) = text(&mut t, "component")? {
                Ok(Change::Component(id, c))
            } else if let Some(p) = text(&mut t, "unbind")? {
                Ok(Change::Bind(id, p, Target::Unbound))
            } else if let Some(v) = t.remove("device") {
                Ok(Change::Device(id, or_default("device", v)?))
            } else if let Some(v) = t.remove("api") {
                Ok(Change::Api(id, or_default("api", v)?))
            } else if let Some(k) = text(&mut t, "knob")? {
                Ok(Change::Knob(id, k, t.remove("value")))
            } else if let Some(k) = t.remove("knobs") {
                Ok(Change::Knobs(id, typed("knobs", k)?))
            } else if let Some(p) = text(&mut t, "input")? {
                if let Some(w) = t.remove("when") {
                    return Ok(Change::Timing(id, p, typed("when", w)?));
                }
                match t.remove("from") {
                    Some(toml::Value::String(s)) => Ok(Change::Bind(id, p, Target::Node(s))),
                    Some(toml::Value::Table(mut n)) => {
                        let (role, component) = new_node("from", &mut n)?;
                        rest(&n)?;
                        Ok(Change::Bind(id, p, Target::New(role, component)))
                    }
                    Some(v) => Err(format!("`from` is a node's id or a new node (`{{ role, component }}`), not `{v}`")),
                    None => Err("`input` needs `from` (what it is bound to) or `when` (its timing)".into()),
                }
            } else {
                Err("what changes on the node: `component`, `input`, `unbind`, `device`, `api`, `knob` or `knobs`".into())
            }
        };
        read().and_then(|c| rest(&t).map(|_| c)).map_err(|e| format!("edit `{whole}`: {e}"))
    }
}

impl From<Change> for toml::Table {
    fn from(c: Change) -> toml::Table {
        fn value<T: Serialize>(v: T) -> toml::Value {
            toml::Value::try_from(v).expect("a TOML value")
        }
        let mut t = toml::Table::new();
        let mut set = |k: &str, v: toml::Value| drop(t.insert(k.into(), v));
        match c {
            Change::Add(id, role, component) => {
                set("add", id.into());
                set("role", value(role));
                set("component", component.into());
            }
            Change::Remove(id) => set("remove", id.into()),
            Change::Component(id, c) => {
                set("node", id.into());
                set("component", c.into());
            }
            Change::Bind(id, port, target) => {
                set("node", id.into());
                match target {
                    Target::Unbound => set("unbind", port.into()),
                    Target::Node(k) => {
                        set("input", port.into());
                        set("from", k.into());
                    }
                    Target::New(role, component) => {
                        set("input", port.into());
                        let mut n = toml::Table::new();
                        n.insert("role".into(), value(role));
                        n.insert("component".into(), component.into());
                        set("from", n.into());
                    }
                }
            }
            Change::Device(id, d) => {
                set("node", id.into());
                set("device", d.map_or(DEFAULT.into(), value));
            }
            Change::Api(id, a) => {
                set("node", id.into());
                set("api", a.map_or(DEFAULT.into(), value));
            }
            Change::Timing(id, port, w) => {
                set("node", id.into());
                set("input", port.into());
                set("when", value(w));
            }
            Change::Knob(id, k, v) => {
                set("node", id.into());
                set("knob", k.into());
                if let Some(v) = v {
                    set("value", v);
                }
            }
            Change::Knobs(id, knobs) => {
                set("node", id.into());
                set("knobs", knobs.into());
            }
        }
        t
    }
}

/// As plans write an edit: an inline table, its keys in reading order.
impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const ORDER: [&str; 14] = ["add", "remove", "node", "role", "component", "input", "unbind", "from", "when", "device", "api", "knob", "value", "knobs"];
        let t: toml::Table = self.clone().into();
        let parts: Vec<String> = ORDER.iter().filter_map(|k| t.get(*k).map(|v| format!("{k} = {v}"))).collect();
        write!(f, "{{ {} }}", parts.join(", "))
    }
}

/// Most combinations one `vary` may expand into (sweeps are bounded).
pub const MAX_VARIED: usize = 100_000;

/// The keys of an edit that hold what it sets, any of which a sweep may give as a list.
const VALUES: [&str; 6] = ["component", "from", "when", "device", "api", "value"];

/// The nodes `vary` adds, with their roles (so `*` can ask what a new node may be).
fn added(v: &toml::Value, out: &mut Vec<(String, Role)>) {
    match v {
        toml::Value::Array(a) => a.iter().for_each(|x| added(x, out)),
        toml::Value::Table(t) => {
            if let (Some(id), Some(role)) = (t.get("add").and_then(|x| x.as_str()), t.get("role").and_then(|r| typed::<Role>("role", r.clone()).ok())) {
                out.push((id.to_string(), role));
            }
            t.get("any").into_iter().for_each(|x| added(x, out));
        }
        _ => {}
    }
}

/// Every value key `k` of edit `t` may be given: what the graph has to offer there
/// (whether a pipeline takes it is `accept`'s to say, per combination).
fn every(base: &Graph, new: &[(String, Role)], t: &toml::Table, k: &str) -> Result<Vec<toml::Value>, String> {
    let node = t.get("node").or(t.get("add")).and_then(|x| x.as_str()).unwrap_or_default();
    let names = |v: Vec<&str>| Ok(v.into_iter().map(toml::Value::from).collect());
    match k {
        "component" => {
            let role = match t.get("role") {
                Some(r) => typed::<Role>("role", r.clone())?,
                None => base.nodes.get(node).map(|n| n.role).or_else(|| new.iter().find(|(id, _)| id == node).map(|(_, r)| *r)).ok_or_else(|| format!("no node `{node}` to vary (the graph has {})", base.nodes.keys().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")))?,
            };
            names(super::role_components(role))
        }
        "api" => names(Api::ALL.map(|a| a.name()).to_vec()),
        "when" => names(vec!["now", "prev"]),
        "from" => Ok(base.nodes.keys().map(String::as_str).chain(new.iter().map(|(id, _)| id.as_str())).filter(|id| *id != node).map(toml::Value::from).collect()),
        _ => Err(format!("`{k} = \"*\"`: there is no list of every {k} to ask for; give the values as a list")),
    }
}

/// The edits one varied edit stands for: one per value of the key given as a list or `*`.
fn alternatives(base: &Graph, new: &[(String, Role)], v: &toml::Value) -> Result<Vec<Change>, String> {
    let t = v.as_table().ok_or_else(|| format!("an edit is a table, not `{v}`"))?;
    let list = |x: &toml::Value| x.is_array() || x.as_str() == Some("*");
    let Some(k) = VALUES.into_iter().find(|k| t.get(*k).is_some_and(list)) else { return Change::try_from(t.clone()).map(|c| vec![c]) };
    let values = match &t[k] {
        toml::Value::Array(a) => a.clone(),
        _ => every(base, new, t, k).map_err(|e| format!("edit `{v}`: {e}"))?,
    };
    values.into_iter().map(|x| {
        let mut t = t.clone();
        t.insert(k.into(), x);
        Change::try_from(t)
    }).collect()
}

/// Every way of taking one entry of each list, the first list varying slowest.
fn product(lists: Vec<Vec<Vec<Change>>>) -> Result<Vec<Vec<Change>>, String> {
    let n = lists.iter().map(Vec::len).try_fold(1usize, |a, b| a.checked_mul(b)).filter(|n| *n <= MAX_VARIED);
    if n.is_none() {
        return Err(format!("more than {MAX_VARIED} combinations ({}); vary less at once", lists.iter().map(|l| l.len().to_string()).collect::<Vec<_>>().join(" x ")));
    }
    let mut out = vec![vec![]];
    for list in &lists {
        out = out.iter().flat_map(|c: &Vec<Change>| list.iter().map(move |x| c.iter().chain(x).cloned().collect())).collect();
    }
    Ok(out)
}

/// The edit lists a sweep stands for (`vary = [...]` after `graph` and `edits`): every
/// combination of one choice per entry. An entry is an edit whose value (`component`,
/// `from`, `when`, `device`, `api`, a knob's `value`) is a list, or `"*"` for everything
/// the graph offers there (a role's components, the APIs, `now` and `prev`, the other
/// nodes); or `{ any = [[edits...], ...] }`, a choice between lists of edits (an empty
/// list: nothing changes), whose edits may hold lists themselves. Whether a combination
/// can be made and is a pipeline is for the caller to ask (`Graph::with_edits`, `accept`).
///
/// ```toml
/// vary = [
///     { node = "storage", component = "*" },
///     { node = "kernel", component = ["refloor", "sphere"] },
///     { any = [[], [{ remove = "meshes" }]] },
/// ]
/// ```
pub fn varied(base: &Graph, vary: &[toml::Value]) -> Result<Vec<Vec<Change>>, String> {
    let mut new = vec![];
    vary.iter().for_each(|v| added(v, &mut new));
    let one = |v: &toml::Value| alternatives(base, &new, v).map(|a| a.into_iter().map(|c| vec![c]).collect::<Vec<_>>());
    let mut axes = vec![];
    for v in vary {
        let Some(any) = v.get("any") else {
            axes.push(one(v)?);
            continue;
        };
        if v.as_table().is_some_and(|t| t.len() != 1) {
            return Err(format!("`{v}`: `any` stands alone (a choice between lists of edits)"));
        }
        let mut axis = vec![];
        for alt in any.as_array().ok_or_else(|| format!("`{v}`: `any` is a list of alternatives, each a list of edits"))? {
            let edits = alt.as_array().ok_or_else(|| format!("`{v}`: each alternative of `any` is a list of edits, not `{alt}`"))?;
            axis.extend(product(edits.iter().map(one).collect::<Result<_, _>>()?)?);
        }
        axes.push(axis);
    }
    product(axes)
}

fn fresh_id(g: &Graph, want: &str) -> String {
    if !g.nodes.contains_key(want) {
        return want.to_string();
    }
    (2..).map(|i| format!("{want}-{i}")).find(|k| !g.nodes.contains_key(k)).unwrap()
}

impl Graph {
    fn node_mut(&mut self, id: &str) -> Result<&mut Node, String> {
        if !self.nodes.contains_key(id) {
            return Err(format!("no node `{id}` (the graph has {})", self.nodes.keys().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")));
        }
        Ok(self.nodes.get_mut(id).unwrap())
    }

    /// The node's input `port`, which its component must have.
    fn port_mut(&mut self, id: &str, port: &str) -> Result<&mut Node, String> {
        let n = self.node_mut(id)?;
        let ports = inputs(n.role, &n.component);
        if !ports.iter().any(|p| p.name == port) {
            return Err(format!("`{id}` ({} {}) has no input `{port}` (its inputs: {})", n.role.name(), n.component, ports.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")));
        }
        Ok(n)
    }

    /// The graph with `change` made as it says and nothing else. Whether the result is a
    /// pipeline is `check`'s and `accept`'s to say; refused here is only a change that names
    /// what the graph does not have (a node, an input) or an id it has already.
    pub fn edit(&self, change: &Change) -> Result<Graph, String> {
        let mut g = self.clone();
        match change {
            Change::Add(id, role, c) => {
                if g.nodes.contains_key(id) {
                    return Err(format!("there is a node `{id}` already"));
                }
                g.nodes.insert(id.clone(), Node::new(*role, c));
            }
            Change::Remove(id) => {
                g.node_mut(id)?;
                g.nodes.remove(id);
                for n in g.nodes.values_mut() {
                    let gone: Vec<String> = n.inputs.iter().filter(|(_, s)| *s == id).map(|(p, _)| p.clone()).collect();
                    for p in gone {
                        n.inputs.remove(&p);
                        n.timing.remove(&p);
                    }
                }
            }
            Change::Component(id, c) => {
                let n = g.node_mut(id)?;
                n.component = c.clone();
                n.knobs.clear();
                n.api = None;
                let ports = inputs(n.role, c);
                n.inputs.retain(|p, _| ports.iter().any(|x| x.name == p));
                n.timing.retain(|p, _| ports.iter().any(|x| x.name == p));
            }
            Change::Bind(id, port, target) => {
                g.port_mut(id, port)?;
                let at = match target {
                    Target::Unbound => None,
                    Target::Node(k) => {
                        g.node_mut(k)?;
                        Some(k.clone())
                    }
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
            Change::Device(id, d) => g.node_mut(id)?.device = *d,
            Change::Api(id, a) => g.node_mut(id)?.api = *a,
            Change::Timing(id, port, w) => {
                let n = g.port_mut(id, port)?;
                match w {
                    When::Now => drop(n.timing.remove(port)),
                    w => drop(n.timing.insert(port.clone(), *w)),
                }
            }
            Change::Knob(id, k, v) => {
                let n = g.node_mut(id)?;
                match v {
                    Some(v) => drop(n.knobs.insert(k.clone(), v.clone())),
                    None => drop(n.knobs.remove(k)),
                }
            }
            Change::Knobs(id, knobs) => g.node_mut(id)?.knobs = knobs.clone(),
        }
        Ok(g)
    }

    /// The graph after `edits`, in order; an edit that cannot be made is named with its
    /// place in the list.
    pub fn with_edits(&self, edits: &[Change]) -> Result<Graph, String> {
        let mut g = self.clone();
        for (i, c) in edits.iter().enumerate() {
            g = g.edit(c).map_err(|e| format!("edit {} (`{c}`): {e}", i + 1))?;
        }
        Ok(g)
    }

    /// The edits that make this graph `to` (`self.with_edits(..) == to`): nodes gone, nodes
    /// new, then per node what differs. `None` when no edits can: another executor, or an
    /// input `to` binds that its node's component does not have.
    pub fn diff(&self, to: &Graph) -> Option<Vec<Change>> {
        if self.executor != to.executor {
            return None;
        }
        let mut v = vec![];
        for (id, n) in &self.nodes {
            if to.nodes.get(id).is_none_or(|t| t.role != n.role) {
                v.push(Change::Remove(id.clone()));
            }
        }
        for (id, t) in &to.nodes {
            match self.nodes.get(id).filter(|n| n.role == t.role) {
                None => v.push(Change::Add(id.clone(), t.role, t.component.clone())),
                Some(n) if n.component != t.component => v.push(Change::Component(id.clone(), t.component.clone())),
                Some(_) => {}
            }
        }
        let g = self.with_edits(&v).ok()?;
        for (id, t) in &to.nodes {
            let n = &g.nodes[id];
            let id = || id.clone();
            v.extend(n.inputs.keys().filter(|p| !t.inputs.contains_key(*p)).map(|p| Change::Bind(id(), p.clone(), Target::Unbound)));
            v.extend(t.inputs.iter().filter(|(p, s)| n.inputs.get(*p) != Some(s)).map(|(p, s)| Change::Bind(id(), p.clone(), Target::Node(s.clone()))));
            if n.device != t.device {
                v.push(Change::Device(id(), t.device));
            }
            if n.api != t.api {
                v.push(Change::Api(id(), t.api));
            }
            v.extend(n.timing.keys().filter(|p| !t.timing.contains_key(*p)).map(|p| Change::Timing(id(), p.clone(), When::Now)));
            v.extend(t.timing.iter().filter(|(p, w)| n.timing.get(*p) != Some(w)).map(|(p, w)| Change::Timing(id(), p.clone(), *w)));
            let knobs: Vec<Change> = n.knobs.keys().filter(|k| !t.knobs.contains_key(*k)).map(|k| Change::Knob(id(), k.clone(), None)).chain(t.knobs.iter().filter(|(k, x)| n.knobs.get(*k) != Some(x)).map(|(k, x)| Change::Knob(id(), k.clone(), Some(x.clone())))).collect();
            match knobs.len() {
                0 | 1 => v.extend(knobs),
                _ => v.push(Change::Knobs(id(), t.knobs.clone())),
            }
        }
        let made = self.with_edits(&v).ok()?;
        debug_assert!(made == *to);
        Some(v)
    }
}

/// A node a running executor replaces: its role and its new component (`None`: the node is
/// gone).
#[derive(Clone, Debug, PartialEq)]
pub struct Switch {
    pub node: String,
    pub role: Role,
    pub component: Option<String>,
}

impl Graph {
    /// The edits from this graph to `to` that its running executor takes, and the nodes they
    /// replace: every edit changes a node the region replaces while running
    /// ([`super::Builds::live`]: its component, the node added or removed, its inputs), an
    /// input that read such a node or reads a new one, or a timing that changes with one.
    /// Knob values are left out (knobs are set as knobs). Fails with the first edit that
    /// needs a new renderer, or when `to` is not a pipeline of the same region.
    pub fn live_edits(&self, to: &Graph) -> Result<(Vec<Change>, Vec<Switch>), String> {
        let region = self.region().ok_or("no region builds the running graph")?;
        to.accept()?;
        if to.region().map(|b| b.id) != Some(region.id) {
            return Err(format!("the pipeline is no longer one `{}` builds", region.name));
        }
        let edits: Vec<Change> = self.diff(to).ok_or("another executor")?.into_iter().filter(|c| !matches!(c, Change::Knob(..) | Change::Knobs(..))).collect();
        let mut switches: Vec<Switch> = vec![];
        for c in &edits {
            match c {
                Change::Remove(id) if region.replaces(self, id) => switches.push(Switch { node: id.clone(), role: self.nodes[id].role, component: None }),
                Change::Add(id, role, c) if region.replaces(to, id) => switches.push(Switch { node: id.clone(), role: *role, component: Some(c.clone()) }),
                Change::Component(id, c) if region.replaces(self, id) && region.replaces(to, id) => switches.push(Switch { node: id.clone(), role: to.nodes[id].role, component: Some(c.clone()) }),
                Change::Remove(_) | Change::Add(..) | Change::Component(..) => return Err(format!("`{c}` needs a new renderer (`{}` replaces {} while running)", region.name, live_roles(region))),
                _ => {}
            }
        }
        let replaced = |id: &str| switches.iter().any(|s| s.node == id);
        let came = |id: &str| replaced(id) && !self.nodes.contains_key(id);
        for c in &edits {
            let ok = match c {
                Change::Remove(_) | Change::Add(..) | Change::Component(..) | Change::Knob(..) | Change::Knobs(..) => true,
                Change::Bind(id, port, target) => {
                    let was = self.nodes.get(id).and_then(|n| n.inputs.get(port));
                    region.replaces(to, id) || was.is_some_and(|s| replaced(s)) || matches!(target, Target::Node(s) if replaced(s))
                }
                Change::Timing(id, port, _) => came(id) || to.nodes.get(id).is_some_and(|n| region.live.iter().any(|l| l.with.contains(&(n.role, port.as_str())) && switches.iter().any(|s| s.role == l.role))),
                Change::Device(id, _) | Change::Api(id, _) => came(id),
            };
            if !ok {
                return Err(format!("`{c}` needs a new renderer"));
            }
        }
        Ok((edits, switches))
    }
}

fn live_roles(region: &super::Builds) -> String {
    match region.live {
        [] => "no node".into(),
        live => live.iter().map(|l| l.role.name()).collect::<Vec<_>>().join(", "),
    }
}

/// `edits` as the text a host sends its worker ([`mr_core::Renderer::edit`]).
pub fn edits_text(edits: &[Change]) -> String {
    serde_json::to_string(edits).expect("edits are tables")
}

/// For an executor taking `text` ([`edits_text`]) while running graph `g`: the graph it is to
/// run, checked, and the nodes to replace. Refuses edits that need a new renderer.
pub fn live(g: &Graph, text: &str) -> anyhow::Result<(Checked, Vec<Switch>)> {
    let edits: Vec<Change> = serde_json::from_str(text)?;
    let to = g.with_edits(&edits).map_err(anyhow::Error::msg)?;
    let (_, switches) = g.live_edits(&to).map_err(anyhow::Error::msg)?;
    Ok((to.check().map_err(anyhow::Error::msg)?, switches))
}

/// The graph files shipped with the bench (`pipelines/<name>.toml`), by name: starting
/// points like any saved graph, built in so an installed copy has them.
pub const SHIPPED: &[(&str, &str)] = &[
    ("raycast-gpu", include_str!("../../../../pipelines/raycast-gpu.toml")),
    ("raycast-cpu", include_str!("../../../../pipelines/raycast-cpu.toml")),
    ("rayquery-gpu", include_str!("../../../../pipelines/rayquery-gpu.toml")),
    ("raster-gpu", include_str!("../../../../pipelines/raster-gpu.toml")),
    ("raster-cpu", include_str!("../../../../pipelines/raster-cpu.toml")),
    ("discover-gpu", include_str!("../../../../pipelines/discover-gpu.toml")),
    ("discover-cpu", include_str!("../../../../pipelines/discover-cpu.toml")),
    ("splat-gpu", include_str!("../../../../pipelines/splat-gpu.toml")),
    ("columns-gpu", include_str!("../../../../pipelines/columns-gpu.toml")),
    ("columns-cpu", include_str!("../../../../pipelines/columns-cpu.toml")),
];

impl Graph {
    /// The components node `id` may be instead, each leaving a pipeline its executor builds
    /// (`accept`); its own among them when the graph is one. Empty: no such node.
    pub fn choices(&self, id: &str) -> Vec<&'static str> {
        let Some(n) = self.nodes.get(id) else { return vec![] };
        super::role_components(n.role).into_iter().filter(|c| self.edit(&Change::Component(id.into(), c.to_string())).is_ok_and(|g| g.accept().is_ok())).collect()
    }
}

/// The graph files a renderer can be started from (the shipped ones), each with what its
/// file says of it (its first comment line).
pub fn start_files() -> impl Iterator<Item = (&'static str, &'static str)> {
    SHIPPED.iter().map(|(name, text)| (*name, text.lines().next().unwrap_or_default().trim_start_matches('#').trim()))
}

/// The shipped graph file `name`, if there is one.
pub fn shipped(name: &str) -> Option<Graph> {
    let (_, text) = SHIPPED.iter().find(|(n, _)| *n == name)?;
    Some(toml::from_str(text).unwrap_or_else(|e| panic!("pipelines/{name}.toml: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Composition;
    use crate::graph::REGIONS;

    /// The knobs of the shading node are owned by the shadings its region declares
    /// (`Builds::declared`): exactly what the shading node of the region's file may be.
    #[test]
    fn declared_shadings_are_the_choices() {
        assert_eq!(SHIPPED.len(), REGIONS.len());
        for (name, _) in SHIPPED {
            let g = shipped(name).unwrap();
            let b = g.region().unwrap();
            let (id, _) = g.nodes.iter().find(|(_, n)| n.role == Role::Shading).unwrap();
            let (mut declared, mut choices) = (b.declared(Role::Shading), g.choices(id));
            declared.sort();
            choices.sort();
            assert!(!choices.is_empty() && declared == choices, "{name}: declares {declared:?}, the node may be {choices:?}");
        }
    }

    /// The edits a running executor takes are its region's: another component on exactly
    /// the nodes it replaces, such a node removed or added back; the edits arrive as text.
    #[test]
    fn live_edits_are_the_regions() {
        let mut replaced = vec![];
        for (name, _) in SHIPPED {
            let g = shipped(name).unwrap();
            let region = g.region().unwrap();
            for id in g.nodes.keys() {
                for c in g.choices(id).into_iter().filter(|c| *c != g.nodes[id].component) {
                    let to = g.edit(&Change::component(id, c)).unwrap();
                    match g.live_edits(&to) {
                        Ok((edits, switches)) => {
                            assert!(region.replaces(&g, id), "{name} {id}={c}");
                            assert_eq!(switches, [Switch { node: id.clone(), role: g.nodes[id].role, component: Some(c.to_string()) }]);
                            let (ck, sw) = live(&g, &edits_text(&edits)).unwrap();
                            assert_eq!((ck.graph.key(), sw), (to.key(), switches));
                            replaced.push(format!("{name}.{id}"));
                        }
                        Err(why) => assert!(!region.replaces(&g, id), "{name} {id}={c}: {why}"),
                    }
                }
            }
        }
        replaced.dedup();
        assert_eq!(
            replaced,
            ["raycast-gpu.kernel", "raycast-gpu.shading", "rayquery-gpu.shading", "raster-gpu.draw", "raster-gpu.geometry", "raster-gpu.shading", "raster-cpu.geometry", "discover-gpu.draw", "discover-cpu.draw", "discover-cpu.visibility"]
        );
        // A node that comes and goes: the walk's mesh records, with the input that read them.
        let g = shipped("raycast-gpu").unwrap();
        let without = g.edit(&Change::Remove("meshes".into())).unwrap();
        let gone = |s: &[Switch]| s.iter().map(|s| (s.role, s.component.clone())).collect::<Vec<_>>();
        assert_eq!(gone(&g.live_edits(&without).unwrap().1), [(Role::Meshes, None)]);
        let (back, switches) = without.live_edits(&g).unwrap();
        assert_eq!(gone(&switches), [(Role::Meshes, Some("composite".to_string()))]);
        assert_eq!(without.with_edits(&back).unwrap(), g);
        // A knob alone is no edit (knobs are set as knobs).
        let knob = g.edit(&Change::Knob("shading".into(), "bounces".into(), Some(toml::Value::Integer(2)))).unwrap();
        assert_eq!(g.live_edits(&knob).unwrap().0, vec![]);
    }

    fn change(s: &str) -> Result<Change, String> {
        #[derive(Deserialize)]
        struct One {
            e: Change,
        }
        toml::from_str::<One>(&format!("e = {s}")).map(|o| o.e).map_err(|e| e.message().to_string())
    }

    /// Every shipped file is a region's default pipeline, key for key, and every region's
    /// default pipeline is shipped; the directory holds no file the binary lacks.
    #[test]
    fn shipped_files_are_the_default_pipelines() {
        assert_eq!(SHIPPED.len(), REGIONS.len());
        for b in REGIONS {
            let g = shipped(b.name).unwrap_or_else(|| panic!("no pipelines/{}.toml", b.name));
            g.accept().unwrap_or_else(|e| panic!("{}: {e}", b.name));
            let d = Graph::from_composition(&Composition::new(b.name)).unwrap();
            assert_eq!(g.key().unwrap(), d.key().unwrap(), "{}", b.name);
            assert_eq!(g, d, "{}", b.name);
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../pipelines");
        let mut files: Vec<String> = std::fs::read_dir(dir).unwrap().map(|f| f.unwrap().file_name().into_string().unwrap()).collect();
        files.sort();
        let mut want: Vec<String> = SHIPPED.iter().map(|(n, _)| format!("{n}.toml")).collect();
        want.sort();
        assert_eq!(files, want);
    }

    /// Writes the shipped files from the default compositions (run by hand:
    /// `cargo test -p mr-components write_shipped -- --ignored`).
    #[test]
    #[ignore]
    fn write_shipped() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../pipelines");
        std::fs::create_dir_all(&dir).unwrap();
        for b in REGIONS {
            let g = Graph::from_composition(&Composition::new(b.name)).unwrap();
            let text = format!("# {}\n# A pipeline graph: start a renderer from it with `graph = \"{}\"` and `edits`.\n\n{}", b.description, b.name, toml::to_string(&g).unwrap());
            std::fs::write(dir.join(format!("{}.toml", b.name)), text).unwrap();
        }
    }

    /// Every kind of edit reads from TOML, writes back as it was read, and is refused with
    /// what is wrong.
    #[test]
    fn edits_read_and_write() {
        let all = [
            r#"{ node = "storage", component = "tree64" }"#,
            r#"{ node = "shading", input = "bounce", from = "kernel" }"#,
            r#"{ node = "shading", input = "bounce", from = { role = "traversal", component = "sphere" } }"#,
            r#"{ node = "shading", unbind = "bounce" }"#,
            r#"{ node = "shading", device = "gpu1" }"#,
            r#"{ node = "shading", device = "default" }"#,
            r#"{ node = "kernel", api = "rayquery" }"#,
            r#"{ node = "kernel", api = "default" }"#,
            r#"{ node = "draw", input = "faces", when = "prev" }"#,
            r#"{ node = "draw", input = "faces", when = { iterate = 2 } }"#,
            r#"{ node = "shading", knob = "bounces", value = 2 }"#,
            r#"{ node = "shading", knob = "bounces" }"#,
            r#"{ node = "link", knobs = { latency_ms = 30, loss = 0.01 } }"#,
            r#"{ add = "far", role = "storage", component = "tree64" }"#,
            r#"{ remove = "prepass" }"#,
        ];
        let mut kinds = std::collections::HashSet::new();
        for s in all {
            let c = change(s).unwrap_or_else(|e| panic!("{s}: {e}"));
            kinds.insert(std::mem::discriminant(&c));
            let back = toml::Value::Table(c.clone().into());
            assert_eq!(change(&back.to_string()).unwrap(), c, "{s} -> {back}");
        }
        assert_eq!(kinds.len(), 9);
        assert_eq!(change(r#"{ node = "kernel", api = "rayquery" }"#).unwrap(), Change::Api("kernel".into(), Some(Api::RayQuery)));
        for (s, why) in [
            (r#"{ component = "tree64" }"#, "names its `node`"),
            (r#"{ node = "storage" }"#, "what changes on the node"),
            (r#"{ node = "storage", component = "tree64", device = "gpu" }"#, "`device` does not belong"),
            (r#"{ node = "shading", input = "bounce" }"#, "`input` needs `from`"),
            (r#"{ node = "shading", device = "moon" }"#, "device"),
            (r#"{ add = "far", role = "storage" }"#, "`add` needs `role` and `component`"),
        ] {
            let e = change(s).unwrap_err();
            assert!(e.contains(why), "{s}: {e}");
        }
    }

    /// Edits are made exactly, in order, and one naming what the graph lacks is refused
    /// with its place in the list.
    #[test]
    fn edits_are_exact() {
        let g = shipped("raycast-gpu").unwrap();
        let h = g.with_edits(&[change(r#"{ node = "storage", component = "tree64" }"#).unwrap()]).unwrap();
        let mut want = g.clone();
        want.nodes.get_mut("storage").unwrap().component = "tree64".into();
        assert_eq!(h, want);
        assert_eq!(h.key().unwrap(), Graph::from_composition(&Composition { storage: "tree64".into(), ..Composition::new("raycast-gpu") }).unwrap().key().unwrap());
        // A new node bound, then removed: the input is left unbound.
        let new = r#"{ node = "shading", input = "shadow", from = { role = "traversal", component = "sphere" } }"#;
        let bound = g.with_edits(&[change(new).unwrap()]).unwrap();
        assert_eq!(bound.nodes["shading"].inputs["shadow"], "traversal");
        assert!(bound.nodes["traversal"].inputs.is_empty());
        assert_eq!(bound.nodes.len(), g.nodes.len() + 1);
        let unbound = g.edit(&change(r#"{ node = "shading", unbind = "shadow" }"#).unwrap()).unwrap();
        assert!(!unbound.nodes["shading"].inputs.contains_key("shadow"));
        assert_eq!(bound.edit(&Change::Remove("traversal".into())).unwrap(), unbound);
        // Add then bind by id is the same as binding a new node.
        let add = r#"{ add = "traversal", role = "traversal", component = "sphere" }"#;
        let two = g.with_edits(&[change(add).unwrap(), change(r#"{ node = "shading", input = "shadow", from = "traversal" }"#).unwrap()]).unwrap();
        assert_eq!(two, bound);
        for (s, why) in [
            (r#"{ node = "nowhere", component = "tree64" }"#, "edit 2 (`"),
            (r#"{ node = "nowhere", component = "tree64" }"#, "no node `nowhere`"),
            (r#"{ node = "shading", input = "faces", from = "storage" }"#, "has no input `faces`"),
            (r#"{ node = "shading", input = "shadow", from = "nowhere" }"#, "no node `nowhere`"),
            (r#"{ add = "storage", role = "storage", component = "tree64" }"#, "already"),
        ] {
            let e = g.with_edits(&[Change::Knob("shading".into(), "bounces".into(), None), change(s).unwrap()]).unwrap_err();
            assert!(e.contains(why), "{s}: {e}");
        }
    }

    /// `diff` gives the edits that make one graph another: every shipped graph into every
    /// graph one change away, and into every other shipped graph of its executor.
    #[test]
    fn diff_gives_the_edits() {
        let mut n = 0;
        for (name, _) in SHIPPED {
            let g = shipped(name).unwrap();
            assert_eq!(g.diff(&g), Some(vec![]));
            let others = SHIPPED.iter().map(|(o, _)| shipped(o).unwrap());
            for h in crate::graph::edits(&g).into_iter().chain(others) {
                match g.diff(&h) {
                    Some(d) => {
                        assert_eq!(g.with_edits(&d).unwrap_or_else(|e| panic!("{name}: {e}")), h, "{name}: {}", d.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "));
                        n += 1;
                    }
                    None => assert!(g.executor != h.executor || h.nodes.values().any(|x| x.inputs.keys().any(|p| !inputs(x.role, &x.component).iter().any(|i| i.name == p)))),
                }
            }
        }
        assert!(n > 500, "{n}");
        // Several knobs of a node are one edit; an edit prints as plans write it.
        let g = shipped("raycast-gpu").unwrap();
        let mut h = g.clone();
        h.nodes.get_mut("shading").unwrap().knobs = toml::toml! { bounces = 2 paths = 4 };
        let d = g.diff(&h).unwrap();
        assert_eq!(d.iter().map(|c| c.to_string()).collect::<Vec<_>>(), [r#"{ node = "shading", knobs = { bounces = 2, paths = 4 } }"#]);
        assert_eq!(Change::Add("far".into(), Role::Storage, "tree64".into()).to_string(), r#"{ add = "far", role = "storage", component = "tree64" }"#);
    }

    /// A sweep's edit lists: one choice per entry, the first entry varying slowest.
    #[test]
    fn varied_edits() {
        let g = shipped("raycast-gpu").unwrap();
        let vary = |s: &str| {
            let t: toml::Table = toml::from_str(&format!("v = {s}")).unwrap();
            varied(&g, t["v"].as_array().unwrap()).map(|v| v.into_iter().map(|c| c.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>())
        };
        assert_eq!(vary("[]").unwrap(), [""]);
        assert_eq!(
            vary(r#"[{ node = "storage", component = ["tree64", "rows"] }, { node = "kernel", api = "default" }, { any = [[], [{ remove = "meshes" }]] }]"#).unwrap(),
            [
                r#"{ node = "storage", component = "tree64" } { node = "kernel", api = "default" }"#,
                r#"{ node = "storage", component = "tree64" } { node = "kernel", api = "default" } { remove = "meshes" }"#,
                r#"{ node = "storage", component = "rows" } { node = "kernel", api = "default" }"#,
                r#"{ node = "storage", component = "rows" } { node = "kernel", api = "default" } { remove = "meshes" }"#,
            ]
        );
        // `*`: what the graph offers there, for a node it has, one the sweep adds, and each kind of value.
        let storages = crate::graph::role_components(Role::Storage).len();
        assert_eq!(vary(r#"[{ node = "storage", component = "*" }]"#).unwrap().len(), storages);
        assert_eq!(vary(r#"[{ any = [[{ add = "far", role = "storage", component = "*" }, { node = "far", component = "*" }]] }]"#).unwrap().len(), storages * storages);
        assert_eq!(vary(r#"[{ node = "kernel", api = "*" }]"#).unwrap().len(), Api::ALL.len());
        assert_eq!(vary(r#"[{ node = "shading", input = "hits", when = "*" }]"#).unwrap().len(), 2);
        assert_eq!(vary(r#"[{ node = "shading", input = "shadow", from = "*" }]"#).unwrap().len(), g.nodes.len() - 1);
        assert_eq!(vary(r#"[{ node = "shading", device = ["gpu", "gpu1"] }, { node = "shading", knob = "bounces", value = [1, 2, 3] }]"#).unwrap().len(), 6);
        // Alternatives hold lists themselves.
        assert_eq!(vary(r#"[{ any = [[], [{ node = "storage", component = ["tree64", "rows"] }, { node = "kernel", component = ["sphere", "rowrun"] }]] }]"#).unwrap().len(), 5);
        for (s, why) in [
            (r#"[{ node = "nowhere", component = "*" }]"#, "no node `nowhere` to vary"),
            (r#"[{ node = "shading", device = "*" }]"#, "give the values as a list"),
            (r#"[{ node = "storage", component = ["tree64"], device = "gpu" }]"#, "`device` does not belong"),
            (r#"[{ any = [{ remove = "meshes" }] }]"#, "each alternative of `any` is a list of edits"),
            (r#"[{ any = [[]], remove = "meshes" }]"#, "`any` stands alone"),
            (r#"[3]"#, "an edit is a table"),
            (r#"[{ add = "a", role = "storage", component = "*" }, { add = "b", role = "storage", component = "*" }, { add = "c", role = "storage", component = "*" }, { add = "d", role = "storage", component = "*" }, { add = "e", role = "storage", component = "*" }]"#, "more than 100000 combinations"),
        ] {
            let e = vary(s).unwrap_err();
            assert!(e.contains(why), "{s}: {e}");
        }
    }
}
