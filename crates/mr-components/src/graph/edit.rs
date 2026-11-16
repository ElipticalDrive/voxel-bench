//! Edits: the changes a graph is given, one at a time, each made exactly as it says (a new
//! node comes unbound, a node given another component keeps the inputs it still has, nothing
//! is filled in or pruned). The editor makes them from its pickers and plans write them
//! (`edits = [...]` after `graph`); both go through [`Graph::edit`].
//!
//! Also the graph files shipped with the bench ([`SHIPPED`], `pipelines/*.toml`): graphs as
//! anyone saves them, built into the binary so an installed copy has them.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{Device, Graph, Node, Role, When, inputs};
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
                Err("what changes on the node: `component`, `input`, `unbind`, `device`, `api` or `knob`".into())
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
        }
        t
    }
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
        }
        Ok(g)
    }

    /// The graph after `edits`, in order; an edit that cannot be made is named with its
    /// place in the list.
    pub fn with_edits(&self, edits: &[Change]) -> Result<Graph, String> {
        let mut g = self.clone();
        for (i, c) in edits.iter().enumerate() {
            g = g.edit(c).map_err(|e| format!("edit {} (`{}`): {e}", i + 1, toml::Value::Table(c.clone().into())))?;
        }
        Ok(g)
    }
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
        assert_eq!(kinds.len(), 8);
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
}
