//! Pipelines as graphs (PLAN step 9, IDEAS §17–§18): a pipeline is a graph of component
//! instances (nodes, each with its own knobs and device) joined by edges from an output to
//! an input *port*, not a fixed list of stages. Any graph whose every required input is
//! bound to a compatible output, and that leads from the world to one image, is a valid
//! pipeline; pipelines need not share the same kinds of components.
//!
//! Ports carry typed data ([`Data`]): the world's change log, a storage, a face set, ray
//! starts, a ray-query service, hits, mesh surfaces, the image. Every place rays are cast
//! is an input port of the component that casts them ([`Port::casts`]: primary rays,
//! discovery rays, shadow and bounce rays, mesh shadows), bound to its own kernel and
//! storage nodes or to shared ones; discovery's exposure tests read a storage port (they
//! look blocks up, they cast no rays). Unbound secondary rays follow the rays before them
//! (shadow rays the strategy's own on the same device, bounces the shadow rays, mesh
//! shadows the shading's). Storages and ray queries are *fused* data: compiled together
//! with their caller, so they never cross devices; everything else may (IDEAS §18, the two
//! kinds of composition).
//!
//! Step 9a: the model, validation with reasons, a canonical form (hashable: the cache key),
//! and the conversion from today's flat [`Composition`]s (so every stored record loads as a
//! graph). Step 9c: back ends consume graphs ([`of_init`], [`Checked::site`]). Each back end
//! has a *lead* site (its strategy's own rays, or a rasteriser's shadow rays: the storage
//! and traversal stages of its compositions) and binds its other sites freely, over the
//! lead's storage only, or only to the lead ([`site_binding`]). [`Graph::accept`] checks
//! the free sites' components are implemented where their rays run and that the rest (the
//! skeleton) is one of the back end's compositions; it returns that composition and whether
//! it is exactly the graph.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Composition, Slot, backend, caps, component};

/// What a node does: the roles of IDEAS §18's pieces that exist so far.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The generator, the authoritative chunk store and its change log (the host today;
    /// PLAN step 14 puts it on a server behind a modelled link).
    World,
    Storage,
    Geometry,
    Prepass,
    Traversal,
    Visibility,
    Draw,
    Shading,
    Meshes,
}

impl Role {
    pub const ALL: [Role; 9] = [Role::World, Role::Storage, Role::Geometry, Role::Prepass, Role::Traversal, Role::Visibility, Role::Draw, Role::Shading, Role::Meshes];

    pub fn name(self) -> &'static str {
        match self {
            Role::World => "world",
            r => r.slot().unwrap().name(),
        }
    }

    /// The registry slot whose components fill this role (none for the world).
    pub fn slot(self) -> Option<Slot> {
        Some(match self {
            Role::World => return None,
            Role::Storage => Slot::Storage,
            Role::Geometry => Slot::Geometry,
            Role::Prepass => Slot::Prepass,
            Role::Traversal => Slot::Traversal,
            Role::Visibility => Slot::Visibility,
            Role::Draw => Slot::Draw,
            Role::Shading => Slot::Shading,
            Role::Meshes => Slot::Meshes,
        })
    }
}

/// The kind of data a port carries.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Data {
    /// Chunk loads, unloads, edits, lights (the world's change log).
    Changes,
    /// A representation answering storage queries (`s_probe`, optional capabilities).
    Storage,
    /// Faces to draw: a geometry component's faces or a discovered face set.
    Faces,
    /// Where each primary ray may start (a prepass).
    Starts,
    /// A ray-query service: a traversal kernel over its storage.
    Rays,
    /// The visible surface per pixel (hit record).
    Hits,
    /// Mesh surfaces joining the voxel image.
    Surfaces,
    Image,
}

impl Data {
    pub fn name(self) -> &'static str {
        match self {
            Data::Changes => "changes",
            Data::Storage => "storage",
            Data::Faces => "faces",
            Data::Starts => "ray starts",
            Data::Rays => "ray queries",
            Data::Hits => "hits",
            Data::Surfaces => "mesh surfaces",
            Data::Image => "image",
        }
    }

    /// Compiled together with the caller (IDEAS §18 fused composition): never crosses devices.
    pub fn fused(self) -> bool {
        matches!(self, Data::Storage | Data::Rays)
    }
}

/// Where a node runs. `gpu` is GPU slot 0 of the run's device configuration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Device {
    Cpu,
    Gpu(u8),
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Device::Cpu => f.write_str("cpu"),
            Device::Gpu(0) => f.write_str("gpu"),
            Device::Gpu(i) => write!(f, "gpu{i}"),
        }
    }
}

impl TryFrom<String> for Device {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        match s.as_str() {
            "cpu" => Ok(Device::Cpu),
            "gpu" => Ok(Device::Gpu(0)),
            _ => s.strip_prefix("gpu").and_then(|i| i.parse().ok()).map(Device::Gpu).ok_or_else(|| format!("unknown device `{s}` (cpu, gpu, gpu1, ...)")),
        }
    }
}

impl From<Device> for String {
    fn from(d: Device) -> String {
        d.to_string()
    }
}

/// An input of a component.
pub struct Port {
    pub name: &'static str,
    pub data: Data,
    pub required: bool,
    /// A ray-casting site: rays cast here through the bound kernel.
    pub casts: bool,
    pub description: &'static str,
}

const fn port(name: &'static str, data: Data, required: bool, description: &'static str) -> Port {
    Port { name, data, required, casts: matches!(data, Data::Rays), description }
}

const CHANGES: &[Port] = &[port("changes", Data::Changes, true, "the world's change log")];
const ON_STORAGE: &[Port] = &[port("storage", Data::Storage, true, "the storage it reads")];
const ON_GEOMETRY: &[Port] = &[port("geometry", Data::Faces, true, "the faces its acceleration structure is built from")];
const RAYCAST: &[Port] = &[
    port("primary", Data::Rays, true, "one primary ray per pixel"),
    port("start", Data::Starts, false, "where rays may start (a prepass)"),
    port("meshes", Data::Surfaces, false, "meshes capping the rays (mesh mode)"),
];
const DISCOVER: &[Port] = &[port("rays", Data::Rays, true, "discovery rays"), port("exposure", Data::Storage, true, "the storage exposure tests read (which faces are exposed, their blocks)")];
const MESHES: &[Port] = &[port("changes", Data::Changes, true, "the world's change log"), port("shadow", Data::Rays, false, "shadow rays onto meshes (unbound: the shading's)")];
const FACES: &[Port] = &[port("faces", Data::Faces, true, "the faces to draw")];
const SPLAT: &[Port] = &[port("points", Data::Faces, true, "the points to splat")];
const DRAW: &[Port] = &[port("faces", Data::Faces, true, "the faces to draw"), port("meshes", Data::Surfaces, false, "meshes in the same depth buffer (mesh mode)")];
const DIRECT: &[Port] = &[port("hits", Data::Hits, true, "the visible surface per pixel"), port("shadow", Data::Rays, false, "shadow rays (unbound: the back end's own)")];
const PATHTRACE: &[Port] = &[
    port("hits", Data::Hits, true, "the visible surface per pixel"),
    port("shadow", Data::Rays, false, "shadow rays (unbound: the back end's own)"),
    port("bounce", Data::Rays, false, "bounce rays (unbound: the back end's own)"),
];

/// The inputs of `role` component `name`.
pub fn inputs(role: Role, name: &str) -> &'static [Port] {
    match (role, name) {
        (Role::World, _) => &[],
        (Role::Storage | Role::Geometry, _) => CHANGES,
        (Role::Meshes, _) => MESHES,
        (Role::Prepass, _) => ON_STORAGE,
        (Role::Traversal, "rayquery") => ON_GEOMETRY,
        (Role::Traversal, _) => ON_STORAGE,
        (Role::Visibility, "raycast") => RAYCAST,
        (Role::Visibility, "quadtree" | "cluster") => DISCOVER,
        (Role::Visibility, "splat") => SPLAT,
        (Role::Visibility, "columns") => ON_STORAGE,
        (Role::Visibility, _) => FACES,
        (Role::Draw, _) => DRAW,
        (Role::Shading, "pathtrace") => PATHTRACE,
        (Role::Shading, _) => DIRECT,
    }
}

/// What `role` component `name` outputs.
pub fn output(role: Role, name: &str) -> Data {
    match (role, name) {
        (Role::World, _) => Data::Changes,
        (Role::Storage, _) => Data::Storage,
        (Role::Geometry, _) => Data::Faces,
        (Role::Prepass, _) => Data::Starts,
        (Role::Traversal, _) => Data::Rays,
        (Role::Visibility, "quadtree" | "cluster" | "faces") => Data::Faces,
        (Role::Visibility | Role::Draw, _) => Data::Hits,
        (Role::Shading, _) => Data::Image,
        (Role::Meshes, _) => Data::Surfaces,
    }
}

/// Whether `role` component `name` exists.
fn known(role: Role, name: &str) -> bool {
    match role.slot() {
        None => name == "world",
        Some(s) => component(s, name).is_some(),
    }
}

/// Where a node runs unless the graph says otherwise: fused nodes run with their callers;
/// the rest where the back end runs that role.
fn default_device(executor: &str, role: Role) -> Device {
    match (executor, role) {
        (_, Role::World) => Device::Cpu,
        ("cpu-discover", Role::Draw | Role::Shading | Role::Meshes) => Device::Gpu(0),
        (e, _) if e.starts_with("cpu") => Device::Cpu,
        _ => Device::Gpu(0),
    }
}

/// One component instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub role: Role,
    pub component: String,
    /// Where it runs (default: with its callers for storages and kernels, else where the
    /// back end runs the role).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Device>,
    /// Input port → the node bound to it. Unbound `changes` inputs read the world.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "toml::Table::is_empty")]
    pub knobs: toml::Table,
}

impl Node {
    pub fn new(role: Role, component: &str) -> Self {
        Self { role, component: component.into(), device: None, inputs: BTreeMap::new(), knobs: toml::Table::new() }
    }

    fn with(mut self, inputs: &[(&str, &str)]) -> Self {
        self.inputs.extend(inputs.iter().map(|&(p, n)| (p.to_string(), n.to_string())));
        self
    }
}

/// A pipeline: nodes by id, run by `executor` (a back end; step 13 makes back ends
/// executors placing nodes on devices).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub executor: String,
    pub nodes: BTreeMap<String, Node>,
}

/// A checked graph: the world added, every node's device, the output node.
#[derive(Debug)]
pub struct Checked {
    pub graph: Graph,
    pub devices: HashMap<String, Device>,
    pub output: String,
}

impl Graph {
    /// The graph of a flat composition (its back end's defaults filled in). Primary and
    /// secondary rays share kernel and storage nodes where the composition says `same`;
    /// secondary rays on another device than the primary ones get their own nodes.
    pub fn from_composition(c: &Composition) -> Result<Graph, String> {
        let mut c = c.clone();
        c.fill_defaults();
        let b = backend(&c.backend).ok_or_else(|| format!("unknown back end `{}`", c.backend))?;
        let has = |s: Slot| b.has(s) && !c.get(s).is_empty();
        let mut nodes = BTreeMap::new();
        let mut add = |id: &str, role: Role, name: &str, inputs: &[(&str, &str)]| {
            nodes.insert(id.to_string(), Node::new(role, name).with(inputs));
        };
        add("world", Role::World, "world", &[]);
        let changes = [("changes", "world")];
        for (slot, role) in [(Slot::Storage, Role::Storage), (Slot::Geometry, Role::Geometry)] {
            if has(slot) {
                add(slot.name(), role, c.get(slot), &changes);
            }
        }
        let prepass = has(Slot::Prepass) && c.prepass != "none";
        if prepass {
            add("prepass", Role::Prepass, &c.prepass, &[("storage", "storage")]);
        }
        let meshes = has(Slot::Meshes) && c.meshes != "none";
        if meshes {
            add("meshes", Role::Meshes, &c.meshes, &changes);
        }
        if has(Slot::Traversal) {
            let on = if c.traversal == "rayquery" { ("geometry", "geometry") } else { ("storage", "storage") };
            add("kernel", Role::Traversal, &c.traversal, &[on]);
        }
        let drawn = has(Slot::Draw);
        let mut vis: Vec<(&str, &str)> = match c.visibility.as_str() {
            "raycast" => vec![("primary", "kernel")],
            "quadtree" | "cluster" => vec![("rays", "kernel"), ("exposure", "storage")],
            "splat" => vec![("points", "geometry")],
            "columns" => vec![("storage", "storage")],
            _ => vec![("faces", "geometry")],
        };
        if prepass {
            vis.push(("start", "prepass"));
        }
        if meshes && !drawn {
            vis.push(("meshes", "meshes"));
        }
        add("visibility", Role::Visibility, &c.visibility, &vis);
        if drawn {
            add("draw", Role::Draw, &c.draw, if meshes { &[("faces", "visibility"), ("meshes", "meshes")] } else { &[("faces", "visibility")] });
        }
        let shadow = if b.has(Slot::ShadowTraversal) {
            // On the primary rays' device, a secondary stage naming the primary's component is `same`.
            let apart = b.rays != b.name;
            let shared = |s: Slot| !apart && c.resolved(s) == c.get(s.source());
            if shared(Slot::ShadowStorage) && shared(Slot::ShadowTraversal) {
                Some("kernel")
            } else {
                let st = if shared(Slot::ShadowStorage) {
                    "storage"
                } else {
                    add("shadow.storage", Role::Storage, c.resolved(Slot::ShadowStorage), &changes);
                    "shadow.storage"
                };
                add("shadow.kernel", Role::Traversal, c.resolved(Slot::ShadowTraversal), &[("storage", st)]);
                Some("shadow.kernel")
            }
        } else {
            has(Slot::Traversal).then_some("kernel")
        };
        let mut shade = vec![("hits", if drawn { "draw" } else { "visibility" })];
        if let Some(k) = shadow {
            shade.extend(inputs(Role::Shading, &c.shading).iter().filter(|p| p.casts).map(|p| (p.name, k)));
        }
        add("shading", Role::Shading, &c.shading, &shade);
        // Mesh shadows are the shading's shadow rays.
        if let (true, Some(k)) = (meshes, shadow) {
            nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), k.into());
        }
        Ok(Graph { executor: c.backend, nodes })
    }

    /// The world node added if missing; unbound `changes` inputs bound to it.
    fn normalised(&self) -> Result<Graph, String> {
        let mut g = self.clone();
        let worlds: Vec<String> = g.nodes.iter().filter(|(_, n)| n.role == Role::World).map(|(k, _)| k.clone()).collect();
        let world = match worlds.as_slice() {
            [] => {
                if g.nodes.contains_key("world") {
                    return Err("node `world` must be the world (role = \"world\")".into());
                }
                g.nodes.insert("world".into(), Node::new(Role::World, "world"));
                "world".to_string()
            }
            [w] => w.clone(),
            _ => return Err(format!("one world per graph (have: {})", worlds.join(", "))),
        };
        for n in g.nodes.values_mut() {
            for p in inputs(n.role, &n.component).iter().filter(|p| p.data == Data::Changes) {
                n.inputs.entry(p.name.into()).or_insert_with(|| world.clone());
            }
        }
        // Unbound secondary rays follow the rays before them: the shading's shadow rays the
        // strategy's own (on the same device), bounces the shadow rays, mesh shadows the
        // shading's shadow rays.
        if let Some(sid) = g.nodes.iter().find(|(_, n)| n.role == Role::Shading).map(|(k, _)| k.clone()) {
            let has = |g: &Graph, id: &str, port: &str| g.nodes.get(id).is_some_and(|n| inputs(n.role, &n.component).iter().any(|p| p.name == port));
            if has(&g, &sid, "shadow") && !g.nodes[&sid].inputs.contains_key("shadow") {
                if let Some(k) = g.lead_kernel(&sid) {
                    g.nodes.get_mut(&sid).unwrap().inputs.insert("shadow".into(), k);
                }
            }
            if let Some(k) = g.nodes[&sid].inputs.get("shadow").cloned() {
                if has(&g, &sid, "bounce") {
                    g.nodes.get_mut(&sid).unwrap().inputs.entry("bounce".into()).or_insert(k.clone());
                }
                for n in g.nodes.values_mut().filter(|n| n.role == Role::Meshes) {
                    if inputs(n.role, &n.component).iter().any(|p| p.name == "shadow") {
                        n.inputs.entry("shadow".into()).or_insert(k.clone());
                    }
                }
            }
        }
        Ok(g)
    }

    /// The kernel the strategy feeding shading node `shading` casts its own rays with
    /// (primary or discovery rays), if it casts any on the shading's device.
    fn lead_kernel(&self, shading: &str) -> Option<String> {
        let mut id = self.nodes.get(shading)?.inputs.get("hits")?;
        if self.nodes.get(id)?.role == Role::Draw {
            id = self.nodes[id].inputs.get("faces")?;
        }
        let vis = self.nodes.get(id)?;
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role));
        if vis.role != Role::Visibility || dev(vis) != dev(&self.nodes[shading]) {
            return None;
        }
        inputs(vis.role, &vis.component).iter().filter(|p| p.casts).find_map(|p| vis.inputs.get(p.name).cloned())
    }

    /// Checks the graph is well formed (not yet whether its back end runs it: see
    /// [`Self::validate`]), with the reasons when it isn't.
    pub fn check(&self) -> Result<Checked, String> {
        let b = backend(&self.executor).ok_or_else(|| format!("unknown back end `{}`", self.executor))?;
        let g = self.normalised()?;
        let mut why = Vec::new();
        for (id, n) in &g.nodes {
            if !known(n.role, &n.component) {
                let have: Vec<&str> = n.role.slot().map(|s| crate::components(s).map(|c| c.name).collect()).unwrap_or_default();
                why.push(format!("`{id}`: unknown {} component `{}` (have: {})", n.role.name(), n.component, have.join(", ")));
                continue;
            }
            let ports = inputs(n.role, &n.component);
            for (p, src) in &n.inputs {
                let Some(port) = ports.iter().find(|x| x.name == p) else {
                    why.push(format!("`{id}` ({} `{}`) has no input `{p}` (has: {})", n.role.name(), n.component, ports.iter().map(|x| x.name).collect::<Vec<_>>().join(", ")));
                    continue;
                };
                let Some(s) = g.nodes.get(src) else {
                    why.push(format!("`{id}`.{p}: no node `{src}`"));
                    continue;
                };
                if !known(s.role, &s.component) {
                    continue;
                }
                let out = output(s.role, &s.component);
                if out != port.data {
                    why.push(format!("`{id}`.{p} takes {}, but `{src}` ({} `{}`) gives {}", port.data.name(), s.role.name(), s.component, out.name()));
                    continue;
                }
                // Capability needs on what this port reads: met natively, by an adapter, or not at all.
                let needs = n.role.slot().and_then(|sl| component(sl, &n.component)).map_or(&[][..], |c| c.needs);
                for &(slot, cap) in needs.iter().filter(|(slot, _)| s.role.slot() == Some(*slot)) {
                    if !caps::provides(slot, &s.component, cap) && caps::adapter(slot, cap).is_none() {
                        why.push(format!("`{id}` ({}) needs {} from `{src}`: {} `{}` lacks it and no adapter gives it", n.component, cap.name(), slot.name(), s.component));
                    }
                }
            }
            for p in ports.iter().filter(|p| p.required && !n.inputs.contains_key(p.name)) {
                why.push(format!("`{id}` ({} `{}`): input `{}` ({}) is not bound", n.role.name(), n.component, p.name, p.description));
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        let outs: Vec<&String> = g.nodes.iter().filter(|(_, n)| output(n.role, &n.component) == Data::Image).map(|(k, _)| k).collect();
        let [out] = outs.as_slice() else {
            return Err(format!("a graph has one image output (have: {})", if outs.is_empty() { "none".into() } else { outs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") }));
        };
        let out = out.to_string();
        // Producers before consumers (post-order from the output), rejecting cycles.
        let mut order = Vec::new();
        let mut state: HashMap<&str, bool> = HashMap::new(); // false: on the stack, true: done
        fn visit<'a>(g: &'a Graph, id: &'a str, state: &mut HashMap<&'a str, bool>, order: &mut Vec<&'a str>) -> Result<(), String> {
            match state.get(id) {
                Some(true) => return Ok(()),
                Some(false) => return Err(format!("the graph has a cycle through `{id}`")),
                None => {}
            }
            state.insert(id, false);
            for src in g.nodes[id].inputs.values() {
                visit(g, src, state, order)?;
            }
            state.insert(id, true);
            order.push(id);
            Ok(())
        }
        visit(&g, &out, &mut state, &mut order)?;
        let dangling: Vec<&str> = g.nodes.keys().map(|k| k.as_str()).filter(|k| !state.contains_key(k)).collect();
        if !dangling.is_empty() {
            return Err(format!("nodes that lead to no image: {}", dangling.join(", ")));
        }
        // Devices, consumers first: a fused node runs with its callers.
        let mut devices: HashMap<String, Device> = HashMap::new();
        for &id in order.iter().rev() {
            let n = &g.nodes[id];
            let d = match n.device {
                Some(d) => d,
                None if output(n.role, &n.component).fused() => {
                    let callers: Vec<(&str, Device)> = g.nodes.iter().filter(|(_, c)| c.inputs.values().any(|s| s == id)).map(|(k, _)| (k.as_str(), devices[k.as_str()])).collect();
                    let d = callers[0].1;
                    if let Some((other, od)) = callers.iter().find(|(_, x)| *x != d) {
                        return Err(format!("`{id}` is compiled into callers on two devices (`{}` on {d}, `{other}` on {od}): give each device its own instance", callers[0].0));
                    }
                    d
                }
                None => default_device(&g.executor, n.role),
            };
            devices.insert(id.to_string(), d);
        }
        for (id, n) in &g.nodes {
            for (p, src) in &n.inputs {
                let s = &g.nodes[src];
                if output(s.role, &s.component).fused() && devices[src] != devices[id] {
                    return Err(format!("`{id}`.{p} on {} cannot read `{src}` on {}: {} are compiled into their caller", devices[id], devices[src], output(s.role, &s.component).name()));
                }
            }
        }
        if b.renderer.is_none() {
            return Err(format!("back end `{}` is not available yet", b.name));
        }
        Ok(Checked { graph: g, devices, output: out })
    }

    /// The canonical form: nodes renamed in visiting order from the output (inputs by port
    /// name), devices resolved, knobs sorted; equal for graphs that differ only in node ids.
    pub fn canonical(&self) -> Result<String, String> {
        let c = self.check()?;
        let g = &c.graph;
        let mut ids: HashMap<&str, usize> = HashMap::new();
        let mut order: Vec<&str> = Vec::new();
        let mut stack = vec![c.output.as_str()];
        while let Some(id) = stack.pop() {
            if ids.contains_key(id) {
                continue;
            }
            ids.insert(id, order.len());
            order.push(id);
            // Reversed so the first port by name is visited first.
            stack.extend(g.nodes[id].inputs.values().rev().map(|s| s.as_str()));
        }
        let mut s = format!("{}|", g.executor);
        for id in &order {
            let n = &g.nodes[*id];
            let ins: Vec<String> = n.inputs.iter().map(|(p, src)| format!("{p}=n{}", ids[src.as_str()])).collect();
            let knobs: Vec<String> = n.knobs.iter().map(|(k, v)| format!("{k}={v}")).collect();
            s += &format!("n{}={}.{}@{}({}){{{}}};", ids[id], n.role.name(), n.component, c.devices[*id], ins.join(","), knobs.join(","));
        }
        Ok(s)
    }

    /// A stable hash of the canonical form (FNV-1a, 64 bits): the cache key.
    pub fn key(&self) -> Result<String, String> {
        let h = self.canonical()?.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        Ok(format!("{h:016x}"))
    }

    /// The ray-casting sites (and exposure tests' storages) bound in this graph: caster
    /// node, port, the node bound to it.
    fn sites(&self) -> Vec<(String, &'static str, String)> {
        let mut v = Vec::new();
        for (id, n) in &self.nodes {
            for p in inputs(n.role, &n.component).iter().filter(|p| p.casts || p.name == "exposure") {
                if let Some(src) = n.inputs.get(p.name) {
                    v.push((id.clone(), p.name, src.clone()));
                }
            }
        }
        v
    }

    /// The lead site: the strategy's own rays (primary or discovery rays), else the shading's
    /// shadow rays (rasterisers, whose storage and kernel stages are the shadow rays').
    fn lead(&self) -> Option<(String, &'static str)> {
        let sites = self.sites();
        let of = |role: Role| sites.iter().find(|(c, p, _)| self.nodes[c].role == role && *p != "exposure").map(|(c, p, _)| (c.clone(), *p));
        of(Role::Visibility).or_else(|| of(Role::Shading).filter(|(_, p)| *p == "shadow"))
    }

    /// The node a kernel walks (its storage, or geometry for ray queries).
    fn walked(&self, kernel: &str) -> Option<&str> {
        let n = self.nodes.get(kernel)?;
        n.inputs.get("storage").or(n.inputs.get("geometry")).map(String::as_str)
    }

    /// The skeleton: free sites rebound as a flat composition binds them (exposure tests
    /// to the lead kernel's storage, secondary rays unbound so they follow the lead), and
    /// the nodes only they reached dropped.
    fn skeleton(&self) -> Graph {
        let mut g = self.clone();
        let lead = g.lead();
        let lead_storage = lead.as_ref().and_then(|(c, p)| g.nodes[c].inputs.get(*p)).and_then(|k| g.walked(k)).map(String::from);
        for (caster, port, _) in g.sites() {
            if lead.as_ref().is_some_and(|(c, p)| *c == caster && *p == port) || site_binding(&g.executor, g.nodes[&caster].role, port) == Bind::Lead {
                continue;
            }
            let n = g.nodes.get_mut(&caster).unwrap();
            match (port, &lead_storage) {
                ("exposure", Some(s)) => drop(n.inputs.insert(port.into(), s.clone())),
                ("exposure", None) => {}
                _ => drop(n.inputs.remove(port)),
            }
        }
        // Keep what the output reaches.
        let Some(out) = g.nodes.iter().find(|(_, n)| output(n.role, &n.component) == Data::Image).map(|(k, _)| k.clone()) else { return g };
        let mut keep = std::collections::HashSet::new();
        let mut stack = vec![out];
        while let Some(id) = stack.pop() {
            if keep.insert(id.clone()) {
                stack.extend(g.nodes.get(&id).into_iter().flat_map(|n| n.inputs.values().cloned()));
            }
        }
        g.nodes.retain(|k, n| keep.contains(k) || n.role == Role::World);
        g
    }

    /// Why a site bound away from the lead cannot run on this back end, if it can't.
    fn site_blocker(&self, ck: &Checked) -> Option<String> {
        let g = &ck.graph;
        let b = backend(&g.executor)?;
        let lead = g.lead();
        let lead_kernel = lead.as_ref().and_then(|(c, p)| g.nodes[c].inputs.get(*p)).cloned();
        let lead_storage = lead_kernel.as_deref().and_then(|k| g.walked(k)).map(String::from);
        let mut why = Vec::new();
        for (caster, port, src) in g.sites() {
            if lead.as_ref().is_some_and(|(c, p)| *c == caster && *p == port) {
                continue;
            }
            let role = g.nodes[&caster].role;
            let at = format!("`{caster}`.{port}");
            match site_binding(&g.executor, role, port) {
                Bind::Lead if port == "exposure" => {
                    if Some(&src) != lead_storage.as_ref() {
                        why.push(format!("{at}: `{}` reads exposure from the storage its rays walk (`{}`)", g.executor, lead_storage.as_deref().unwrap_or("-")));
                    }
                }
                Bind::Lead => {
                    if Some(&src) != lead_kernel.as_ref() {
                        why.push(format!("{at}: `{}` casts these rays through its {} kernel (`{}`)", g.executor, lead.as_ref().map_or("own", |l| l.1), lead_kernel.as_deref().unwrap_or("-")));
                    }
                }
                bind => {
                    // Secondary rays run where the shading runs (cpu-discover: on the GPU).
                    let on = if matches!(role, Role::Shading | Role::Meshes) { b.rays } else { b.name };
                    let need = |slot: Slot, id: &str| crate::unavailable(on, slot, &g.nodes[id].component).map(|w| format!("{at} → `{id}`: {w}"));
                    if port == "exposure" {
                        why.extend(need(Slot::Storage, &src));
                        continue;
                    }
                    why.extend(need(Slot::Traversal, &src));
                    if let Some(st) = g.walked(&src) {
                        if let Some(slot) = g.nodes[st].role.slot() {
                            why.extend(need(slot, st));
                        }
                        if bind == Bind::SameStorage && Some(st) != lead_storage.as_deref() {
                            why.push(format!("{at}: `{}` walks these rays over the storage of its {} rays (`{}`; another storage needs a pipeline compiled for it)", g.executor, lead.as_ref().map_or("own", |l| l.1), lead_storage.as_deref().unwrap_or("-")));
                        }
                    }
                }
            }
        }
        // Back ends that cast shadow rays through a kernel need one bound.
        if b.has(Slot::Traversal) {
            if let Some(n) = g.nodes.values().find(|n| n.role == Role::Shading) {
                if !n.inputs.contains_key("shadow") {
                    why.push(format!("shading `{}`: input `shadow` (shadow rays) is not bound, and `{}` has no rays of its own on the shading's device", n.component, g.executor));
                }
            }
        }
        (!why.is_empty()).then(|| why.join("; "))
    }

    /// The flat composition of the graph's skeleton (everything but its free sites), and
    /// whether it is exactly the graph (else the graph is run as such: the composition is
    /// the nearest, for labels and the flat options). Fails with the reasons if the back end
    /// cannot run the graph.
    pub fn accept(&self) -> Result<(Composition, bool), String> {
        let ck = self.check()?;
        if let Some(why) = self.site_blocker(&ck) {
            return Err(why);
        }
        let skel = ck.graph.skeleton();
        let mut c = skel.skeleton_composition()?;
        let strip = |g: &Graph| {
            let mut g = g.clone();
            g.nodes.values_mut().for_each(|n| n.knobs.clear());
            g.canonical()
        };
        if strip(&Graph::from_composition(&c)?.skeleton())? != strip(&skel)? {
            return Err(format!("`{}` cannot run this graph; the nearest it runs is `{}`", self.executor, c.label()));
        }
        // The secondary-ray stages where the composition has them.
        let b = backend(&self.executor).unwrap();
        let g = &ck.graph;
        let shading = ck.output.as_str();
        if let (true, Some(s), Some(k)) = (b.has(Slot::ShadowTraversal), g.nodes[shading].inputs.get("shadow"), g.lead().and_then(|(c, p)| g.nodes[&c].inputs.get(p).cloned())) {
            let mut e = c.clone();
            if *s == k {
                e.shadow_storage = "same".into();
                e.shadow_traversal = "same".into();
            } else {
                e.shadow_traversal = if g.nodes[s].component == g.nodes[&k].component { "same".into() } else { g.nodes[s].component.clone() };
                let (ss, ps) = (g.walked(s).unwrap_or(""), g.walked(&k).unwrap_or(""));
                e.shadow_storage = if ss == ps { "same".into() } else { g.nodes.get(ss).map_or(String::new(), |n| n.component.clone()) };
            }
            if e.validate().is_ok() {
                c = e;
            }
        }
        let exact = strip(&Graph::from_composition(&c)?)? == strip(self)?;
        Ok((c, exact))
    }

    /// The composition the back end runs the graph as (see [`Self::accept`]).
    pub fn to_composition(&self) -> Result<Composition, String> {
        self.accept().map(|(c, _)| c)
    }

    /// A skeleton's flat composition: each stage node in its slot.
    fn skeleton_composition(&self) -> Result<Composition, String> {
        let ck = self.check()?;
        let g = &ck.graph;
        let n = |id: &str| &g.nodes[id];
        let input = |id: &str, p: &str| n(id).inputs.get(p).map(String::as_str);
        let mut c = Composition { backend: g.executor.clone(), ..Composition::new("") };
        let b = backend(&g.executor).unwrap();
        let shading = ck.output.as_str();
        c.shading = n(shading).component.clone();
        let hits = input(shading, "hits").unwrap();
        let vis = if n(hits).role == Role::Draw {
            c.draw = n(hits).component.clone();
            input(hits, "faces").unwrap()
        } else {
            hits
        };
        if n(vis).role != Role::Visibility {
            return Err(format!("`{}` draws faces straight from `{vis}`: this back end needs a visibility stage between them", g.executor));
        }
        c.visibility = n(vis).component.clone();
        let take = |c: &mut Composition, id: &str| {
            if let Some(slot) = n(id).role.slot() {
                c.set(slot, &n(id).component);
            }
        };
        n(vis).inputs.values().for_each(|src| take(&mut c, src));
        if n(hits).role == Role::Draw {
            input(hits, "meshes").into_iter().for_each(|m| take(&mut c, m));
        }
        // The lead kernel's storage; rasterisers' only rays are the shadow rays.
        if let Some((caster, port)) = g.lead() {
            let k = input(&caster, port).unwrap();
            take(&mut c, k);
            if let Some(st) = g.walked(k) {
                take(&mut c, st);
            }
        }
        // Stages a back end has but the graph leaves out are at their "off" component.
        for (slot, off) in [(Slot::Prepass, "none"), (Slot::Meshes, "none")] {
            if b.has(slot) && c.get(slot).is_empty() {
                c.set(slot, off);
            }
        }
        c.fill_defaults();
        c.validate()?;
        Ok(c)
    }

    /// Checks the graph and that its back end runs it.
    pub fn validate(&self) -> Result<(), String> {
        self.accept().map(|_| ())
    }

    /// The knobs of the skeleton's nodes as the back end's flat options (the nodes on free
    /// sites keep theirs: back ends read them per instance). Two skeleton nodes with
    /// different values of one knob cannot be expressed yet.
    pub fn flat_knobs(&self) -> Result<toml::Table, String> {
        let skel = self.normalised()?.skeleton();
        let mut t = toml::Table::new();
        for (id, n) in self.nodes.iter().filter(|(id, _)| skel.nodes.contains_key(*id)) {
            for (k, v) in &n.knobs {
                if let Some(old) = t.insert(k.clone(), v.clone()).filter(|old| old != v) {
                    return Err(format!("knob `{k}` set to {old} and {v} (`{id}`): `{}` has one `{k}` for its stages", self.executor));
                }
            }
        }
        Ok(t)
    }
}

/// The graph back end `backend` runs (PLAN step 9c): the host's
/// ([`mr_core::InitParams::graph_json`]), else the graph of the flat composition in its
/// options (its slots by name), checked and accepted.
pub fn of_init(backend: &str, p: &mr_core::InitParams) -> anyhow::Result<Checked> {
    let g: Graph = if p.graph_json.trim().is_empty() {
        let opts: serde_json::Map<String, serde_json::Value> = if p.options_json.trim().is_empty() { Default::default() } else { serde_json::from_str(&p.options_json)? };
        let mut c = Composition::new(backend);
        for s in c.slots().collect::<Vec<_>>() {
            if let Some(v) = opts.get(s.name()).and_then(|v| v.as_str()) {
                c.set(s, v);
            }
        }
        Graph::from_composition(&c).map_err(anyhow::Error::msg)?
    } else {
        serde_json::from_str(&p.graph_json)?
    };
    anyhow::ensure!(g.executor == backend, "a `{}` graph handed to `{backend}`", g.executor);
    g.validate().map_err(anyhow::Error::msg)?;
    g.check().map_err(anyhow::Error::msg)
}

/// How a back end binds a site that is not its lead.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Bind {
    /// Any kernel and storage implemented where the rays run.
    Free,
    /// Its own kernel over the lead's storage (the `cpu` back end's precompiled build: one
    /// storage type per instance).
    SameStorage,
    /// The lead's kernel (or for exposure tests, the lead kernel's storage).
    Lead,
}

/// How back end `executor` binds the `port` site of its `role` node (PLAN step 9c).
fn site_binding(executor: &str, role: Role, port: &str) -> Bind {
    match (executor, role, port) {
        ("gpu-compute", Role::Shading | Role::Meshes, _) => Bind::Free,
        ("gpu-raster" | "gpu-splat", Role::Shading, "bounce") | ("gpu-raster", Role::Meshes, "shadow") => Bind::Free,
        ("gpu-discover", Role::Visibility, "exposure") | ("gpu-discover" | "cpu-discover", Role::Shading, _) => Bind::Free,
        ("cpu", Role::Shading, "shadow") => Bind::SameStorage,
        _ => Bind::Lead,
    }
}

/// The ray-casting sites of a checked graph, for the back ends that run it.
impl Checked {
    pub fn node(&self, id: &str) -> &Node {
        &self.graph.nodes[id]
    }

    /// The node of `role` (strategy, shading, draw, prepass, meshes: one each).
    pub fn find(&self, role: Role) -> Option<&str> {
        self.graph.nodes.iter().find(|(_, n)| n.role == role).map(|(k, _)| k.as_str())
    }

    /// The node bound to input `port` of `role`'s node.
    pub fn input(&self, role: Role, port: &str) -> Option<&str> {
        self.graph.nodes.get(self.find(role)?)?.inputs.get(port).map(String::as_str)
    }

    /// Ray-casting site `port` of `role`'s node: its kernel and what the kernel walks.
    pub fn site(&self, role: Role, port: &str) -> Option<Site<'_>> {
        let kernel = self.input(role, port)?;
        Some(Site { kernel, storage: self.graph.walked(kernel)? })
    }
}

/// A ray-casting site's binding: node ids of its kernel and of the storage (or geometry)
/// the kernel walks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Site<'a> {
    pub kernel: &'a str,
    pub storage: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BACKENDS, names_on};

    /// Every composition of every back end (each slot through all its components).
    fn every_composition() -> Vec<Composition> {
        let mut all = Vec::new();
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let mut combos = vec![Composition::new(b.name)];
            for slot in Slot::ALL.into_iter().filter(|&s| b.has(s)) {
                combos = combos
                    .iter()
                    .flat_map(|c| {
                        names_on(b.name, slot).into_iter().map(move |x| {
                            let mut c = c.clone();
                            c.set(slot, x);
                            c
                        })
                    })
                    .collect();
            }
            all.extend(combos.into_iter().filter(|c| c.validate().is_ok()));
        }
        all
    }

    /// Today's compositions load as graphs that validate, and convert back to a composition
    /// with the same graph.
    #[test]
    fn compositions_round_trip() {
        let all = every_composition();
        assert!(all.len() > 1000, "{}", all.len());
        let mut keys = HashMap::new();
        for c in &all {
            let g = Graph::from_composition(c).unwrap();
            let back = g.to_composition().unwrap_or_else(|e| panic!("{}: {e}\n{g:#?}", c.label()));
            assert_eq!(Graph::from_composition(&back).unwrap().canonical(), g.canonical(), "{}", c.label());
            // Exactly, except `same` and an explicit name meaning one graph.
            let same = |c: &Composition| {
                let mut c = c.clone();
                for s in [Slot::ShadowStorage, Slot::ShadowTraversal] {
                    if b_has(&c, s) {
                        let r = c.resolved(s).to_string();
                        c.set(s, &r);
                    }
                }
                c
            };
            assert_eq!(same(&back), same(c), "{}", c.label());
            keys.entry(g.key().unwrap()).or_insert_with(Vec::new).push(same(c));
        }
        // Distinct compositions are distinct graphs, unless they differ only in `same`.
        let shared = keys.values().filter(|v| v.len() > 1).count();
        for (k, cs) in &keys {
            assert!(cs.iter().all(|c| *c == cs[0]), "{k}: {cs:?}");
        }
        eprintln!("{} compositions, {} graphs, {shared} reached by several spellings", all.len(), keys.len());
    }

    fn b_has(c: &Composition, s: Slot) -> bool {
        backend(&c.backend).is_some_and(|b| b.has(s))
    }

    #[test]
    fn canonical_ignores_ids() {
        let g = Graph::from_composition(&Composition { storage: "prefix".into(), shadow_storage: "rows".into(), ..Default::default() }).unwrap();
        let mut renamed = Graph { executor: g.executor.clone(), nodes: BTreeMap::new() };
        let rename = |id: &str| format!("x_{id}");
        for (id, n) in &g.nodes {
            let mut n = n.clone();
            n.inputs.values_mut().for_each(|s| *s = rename(s));
            renamed.nodes.insert(rename(id), n);
        }
        assert_eq!(renamed.canonical(), g.canonical());
        assert_eq!(renamed.key(), g.key());
        assert_ne!(Graph::from_composition(&Composition::default()).unwrap().key(), g.key());
    }

    /// A graph written by hand, without the world (implicit), loads and runs as the
    /// composition it is.
    #[test]
    fn written_graph() {
        let g: Graph = toml::from_str(
            r#"
            executor = "gpu-compute"
            [nodes.s]
            role = "storage"
            component = "prefix"
            [nodes.k]
            role = "traversal"
            component = "incremental"
            inputs = { storage = "s" }
            knobs = { exact = true }
            [nodes.v]
            role = "visibility"
            component = "raycast"
            inputs = { primary = "k" }
            [nodes.out]
            role = "shading"
            component = "direct"
            inputs = { hits = "v", shadow = "k" }
            "#,
        )
        .unwrap();
        let c = g.to_composition().unwrap();
        assert_eq!(c, Composition { storage: "prefix".into(), traversal: "incremental".into(), meshes: "none".into(), ..Default::default() });
        assert_eq!(g.flat_knobs().unwrap().get("exact"), Some(&toml::Value::Boolean(true)));
        // Shadow rays through their own kernel over the same storage.
        let mut sep = g.clone();
        sep.nodes.insert("k2".into(), Node::new(Role::Traversal, "sphere").with(&[("storage", "s")]));
        sep.nodes.get_mut("out").unwrap().inputs.insert("shadow".into(), "k2".into());
        let c = sep.to_composition().unwrap();
        assert_eq!((c.shadow_storage.as_str(), c.shadow_traversal.as_str()), ("same", "sphere"));
    }

    /// Invalid graphs say why.
    #[test]
    fn reasons() {
        let base = Graph::from_composition(&Composition::default()).unwrap();
        let err = |f: &dyn Fn(&mut Graph)| {
            let mut g = base.clone();
            f(&mut g);
            g.validate().unwrap_err()
        };
        let e = err(&|g| {
            g.nodes.get_mut("visibility").unwrap().inputs.insert("primary".into(), "storage".into());
        });
        assert!(e.contains("takes ray queries, but `storage`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().inputs.clear();
        });
        assert!(e.contains("input `storage`") && e.contains("not bound"), "{e}");
        let e = err(&|g| {
            g.nodes.insert("extra".into(), Node::new(Role::Storage, "rows"));
        });
        assert!(e.contains("lead to no image: extra"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().device = Some(Device::Cpu);
        });
        assert!(e.contains("compiled into their caller"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("kernel").unwrap().inputs.insert("storage".into(), "kernel".into());
        });
        assert!(e.contains("takes storage"), "{e}");
        // A shape gpu-compute has no composition for: two primary-ray storages.
        let e = err(&|g| {
            g.nodes.insert("s2".into(), Node::new(Role::Storage, "rows"));
            g.nodes.insert("k2".into(), Node::new(Role::Traversal, "refloor").with(&[("storage", "s2")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "k2".into());
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "kernel".into());
        });
        assert!(e.contains("no input `bounce`"), "{e}");
        let e = err(&|g| {
            g.nodes.get_mut("shading").unwrap().component = "pathtrace".into();
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "visibility".into());
        });
        assert!(e.contains("takes ray queries"), "{e}");
        // Capability needs: sphere needs distances; storages get them through an adapter,
        // so only a storage-less source would fail, which types already reject.
        assert!(Graph::from_composition(&Composition { traversal: "sphere".into(), ..Default::default() }).unwrap().validate().is_ok());
        // Discovery across devices: CPU discovery rays and GPU shadow rays cannot share a kernel.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "kernel".into());
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(e.contains("two devices"), "{e}");
    }

    /// Step 9c: sites off the lead bind their own kernel and storage where the back end
    /// runs them so; elsewhere they are rejected with the reason.
    #[test]
    fn free_sites() {
        let with = |c: Composition, f: &dyn Fn(&mut Graph)| {
            let mut g = Graph::from_composition(&c).unwrap();
            f(&mut g);
            g
        };
        let own = |g: &mut Graph, id: &str, st: &str, k: &str, at: &[(&str, &str)]| {
            g.nodes.insert(format!("{id}.storage"), Node::new(Role::Storage, st));
            g.nodes.insert(format!("{id}.kernel"), Node::new(Role::Traversal, k).with(&[("storage", &format!("{id}.storage"))]));
            for (node, port) in at {
                g.nodes.get_mut(*node).unwrap().inputs.insert(port.to_string(), format!("{id}.kernel"));
            }
        };
        // gpu-compute: primary, shadow, bounce and mesh shadows each their own.
        let pt = Composition { shading: "pathtrace".into(), ..Default::default() };
        let g = with(pt.clone(), &|g| {
            own(g, "s", "distance", "sphere", &[("shading", "shadow")]);
            own(g, "b", "rows", "rowrun", &[("shading", "bounce")]);
            own(g, "m", "tree64", "refloor", &[("meshes", "shadow")]);
        });
        let (c, exact) = g.accept().unwrap();
        assert!(!exact && c.shadow_storage == "distance" && c.shadow_traversal == "sphere", "{c:?}");
        // The same shadow and bounce walk is a composition again.
        let g = with(pt.clone(), &|g| own(g, "s", "distance", "sphere", &[("shading", "shadow"), ("shading", "bounce"), ("meshes", "shadow")]));
        assert!(g.accept().unwrap().1);
        // Unbound bounces follow the shadow rays.
        let mut h = g.clone();
        h.nodes.get_mut("shading").unwrap().inputs.remove("bounce");
        assert_eq!(h.canonical(), g.canonical());
        // gpu-raster: bounces apart from the shadow rays; a kernel it lacks is rejected.
        let r = Composition { shading: "pathtrace".into(), ..Composition::new("gpu-raster") };
        assert!(!with(r.clone(), &|g| own(g, "b", "prefix", "incremental", &[("shading", "bounce")])).accept().unwrap().1);
        let e = with(r, &|g| {
            g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "rq".into());
        })
        .validate()
        .unwrap_err();
        assert!(e.contains("no gpu-raster implementation"), "{e}");
        // gpu-discover: exposure tests read another storage than the discovery rays walk.
        let d = with(Composition { storage: "distance".into(), traversal: "sphere".into(), ..Composition::new("gpu-discover") }, &|g| {
            g.nodes.insert("x".into(), Node::new(Role::Storage, "bricks"));
            g.nodes.get_mut("visibility").unwrap().inputs.insert("exposure".into(), "x".into());
        });
        assert!(!d.accept().unwrap().1);
        // cpu-discover: exposure is the discovery storage; GPU shadow rays their own.
        let cd = Composition::new("cpu-discover");
        let e = with(cd.clone(), &|g| {
            g.nodes.insert("x".into(), Node::new(Role::Storage, "hier"));
            g.nodes.get_mut("visibility").unwrap().inputs.insert("exposure".into(), "x".into());
        })
        .validate()
        .unwrap_err();
        assert!(e.contains("reads exposure from the storage its rays walk"), "{e}");
        assert!(with(Composition { shading: "pathtrace".into(), ..cd }, &|g| own(g, "b", "rows", "rowrun", &[("shading", "bounce")])).validate().is_ok());
        // cpu: the shadow kernel of its own over the primary storage, not another storage.
        let cpu = Composition::new("cpu");
        let g = with(cpu.clone(), &|g| {
            g.nodes.insert("sk".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "storage")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "sk".into());
        });
        assert!(!g.accept().unwrap().1);
        let e = with(cpu, &|g| own(g, "s", "hier", "refloor", &[("shading", "shadow")])).validate().unwrap_err();
        assert!(e.contains("over the storage of its primary rays"), "{e}");
        // gpu-rt: every ray is a ray query through its one acceleration structure.
        let e = with(Composition { shading: "pathtrace".into(), ..Composition::new("gpu-rt") }, &|g| {
            g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "rq".into());
        })
        .validate()
        .unwrap_err();
        assert!(e.contains("through its primary kernel"), "{e}");
        // Knobs of free-site nodes stay with their nodes.
        let mut g = with(Composition::default(), &|g| own(g, "s", "prefix", "incremental", &[("shading", "shadow")]));
        g.nodes.get_mut("s.kernel").unwrap().knobs.insert("exact".into(), toml::Value::Boolean(false));
        g.nodes.get_mut("kernel").unwrap().knobs.insert("exact".into(), toml::Value::Boolean(true));
        assert_eq!(g.flat_knobs().unwrap().get("exact"), Some(&toml::Value::Boolean(true)));
        // cpu-discover without GPU shadow rays cannot shade lit.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("shading").unwrap().inputs.remove("shadow");
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(e.contains("input `shadow`"), "{e}");
    }
}
