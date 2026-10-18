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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Api, Composition, Kind, Slot, backend, caps, component};

/// What a node does: the roles of IDEAS §18's pieces that exist so far.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The generator, the authoritative chunk store and its change log (the host today;
    /// PLAN step 14 puts it on a server behind a modelled link).
    World,
    /// The link the server's stream crosses to a client (PLAN step 14c2): `free` (everything
    /// arrives at once, in order) or `net`, a modelled network (`mr_core::link::LinkSpec`
    /// as its knobs). Runs in the client's process, in front of the client.
    Link,
    /// The client of the world's stream (PLAN step 14b): what its renderer holds, derived
    /// on the renderer's side from the one stream the server sends every client. `pick`:
    /// the server's LOD levels, picked by the client's radius and levels.
    Client,
    Storage,
    Geometry,
    Prepass,
    Traversal,
    Visibility,
    /// Discovery's hole detection: which screen tiles the sampler looks at.
    Holes,
    Draw,
    Shading,
    Meshes,
}

impl Role {
    pub const ALL: [Role; 12] = [Role::World, Role::Link, Role::Client, Role::Storage, Role::Geometry, Role::Prepass, Role::Traversal, Role::Visibility, Role::Holes, Role::Draw, Role::Shading, Role::Meshes];

    pub fn name(self) -> &'static str {
        match self {
            Role::World => "world",
            Role::Link => "link",
            Role::Client => "client",
            r => r.slot().unwrap().name(),
        }
    }

    /// The registry slot whose components fill this role (none for the world, the link and
    /// the client).
    pub fn slot(self) -> Option<Slot> {
        Some(match self {
            Role::World | Role::Link | Role::Client => return None,
            Role::Storage => Slot::Storage,
            Role::Geometry => Slot::Geometry,
            Role::Prepass => Slot::Prepass,
            Role::Traversal => Slot::Traversal,
            Role::Visibility => Slot::Visibility,
            Role::Holes => Slot::Holes,
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
    /// The screen tiles discovery looks at (hole detection).
    Tiles,
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
            Data::Tiles => "tiles",
            Data::Image => "image",
        }
    }

    /// Compiled together with the caller (IDEAS §18 fused composition): never crosses devices.
    pub fn fused(self) -> bool {
        matches!(self, Data::Storage | Data::Rays)
    }
}

/// Where a node runs. `gpu` is GPU slot 0 of the run's device configuration; `server` is
/// the world's machine (PLAN step 14), reached from the client's devices over the link.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Device {
    Cpu,
    Gpu(u8),
    Server,
}

impl Device {
    pub fn kind(self) -> Kind {
        match self {
            Device::Cpu | Device::Server => Kind::Cpu,
            Device::Gpu(_) => Kind::Gpu,
        }
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Device::Cpu => f.write_str("cpu"),
            Device::Server => f.write_str("server"),
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
            "server" => Ok(Device::Server),
            "gpu" => Ok(Device::Gpu(0)),
            _ => s.strip_prefix("gpu").and_then(|i| i.parse().ok()).map(Device::Gpu).ok_or_else(|| format!("unknown device `{s}` (cpu, gpu, gpu1, ..., server)")),
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
const SENT: &[Port] = &[port("changes", Data::Changes, true, "the server's stream: the world's change log, the same for every client")];
const STREAM: &[Port] = &[port("changes", Data::Changes, true, "the server's stream as it arrives over the link")];
const ON_STORAGE: &[Port] = &[port("storage", Data::Storage, true, "the storage it reads")];
const ON_GEOMETRY: &[Port] = &[port("geometry", Data::Faces, true, "the faces its acceleration structure is built from")];
const RAYCAST: &[Port] = &[
    port("primary", Data::Rays, true, "one primary ray per pixel"),
    port("start", Data::Starts, false, "where rays may start (a prepass)"),
    port("meshes", Data::Surfaces, false, "meshes capping the rays (mesh mode)"),
];
const DISCOVER: &[Port] = &[port("rays", Data::Rays, true, "discovery rays"), port("exposure", Data::Storage, true, "the storage exposure tests read (which faces are exposed, their blocks)")];
const QUADTREE: &[Port] = &[
    port("rays", Data::Rays, true, "discovery rays"),
    port("exposure", Data::Storage, true, "the storage exposure tests read (which faces are exposed, their blocks)"),
    port("where", Data::Tiles, false, "the screen tiles to look at (hole detection; unbound: every tile, every frame)"),
];
const COVERAGE: &[Port] = &[
    port("covered", Data::Hits, true, "what the drawer covers drawing last frame's faces at this camera (in the loop: the drawer's faces iterate twice)"),
    port("sky", Data::Hits, true, "the previous frame's final image (timing `prev`): sky told apart from holes"),
    port("changes", Data::Changes, true, "the world's change log: edited blocks and changed chunks mark their tiles"),
];
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
        (Role::Link, _) => SENT,
        (Role::Client, _) => STREAM,
        (Role::Storage | Role::Geometry, _) => CHANGES,
        (Role::Meshes, _) => MESHES,
        (Role::Prepass, _) => ON_STORAGE,
        (Role::Traversal, "rayquery") => ON_GEOMETRY,
        (Role::Traversal, _) => ON_STORAGE,
        (Role::Visibility, "raycast") => RAYCAST,
        (Role::Visibility, "quadtree") => QUADTREE,
        (Role::Visibility, "cluster") => DISCOVER,
        (Role::Visibility, "splat") => SPLAT,
        (Role::Visibility, "columns") => ON_STORAGE,
        (Role::Visibility, _) => FACES,
        (Role::Holes, _) => COVERAGE,
        (Role::Draw, _) => DRAW,
        (Role::Shading, "pathtrace") => PATHTRACE,
        (Role::Shading, _) => DIRECT,
    }
}

/// What `role` component `name` outputs.
pub fn output(role: Role, name: &str) -> Data {
    match (role, name) {
        (Role::World | Role::Link | Role::Client, _) => Data::Changes,
        (Role::Storage, _) => Data::Storage,
        (Role::Geometry, _) => Data::Faces,
        (Role::Prepass, _) => Data::Starts,
        (Role::Traversal, _) => Data::Rays,
        (Role::Visibility, "quadtree" | "cluster" | "faces") => Data::Faces,
        (Role::Visibility | Role::Draw, _) => Data::Hits,
        (Role::Holes, _) => Data::Tiles,
        (Role::Shading, _) => Data::Image,
        (Role::Meshes, _) => Data::Surfaces,
    }
}

/// What a stateful component keeps across frames (PLAN step 13, IDEAS §22): owned by the
/// node instance on its device, kept by the executor, reset on resize or a structural
/// change. State only the node reads is not an edge; another node reads it through an
/// output edge with timing [`When::Prev`].
pub fn state(role: Role, name: &str) -> Option<&'static str> {
    Some(match (role, name) {
        (Role::World, _) => "the authoritative chunks and their change log",
        (Role::Link, "net") => "the messages in flight and the line's queue",
        (Role::Client, _) => "a replica of what arrived and what its renderer holds",
        (Role::Storage, _) => "the representation, patched from the change log",
        (Role::Geometry, _) => "the faces, remeshed per changed chunk",
        (Role::Meshes, _) => "the mesh instances",
        (Role::Traversal, "rayquery") => "acceleration structures built from its geometry",
        (Role::Visibility, "quadtree" | "cluster") => "the discovered face set (faces kept `keep_frames` passes)",
        (Role::Shading, "pathtrace") => "accumulated radiance (restarted when the view changes)",
        (Role::Holes, _) => "per-tile reveal radii and the sky mask; when each face last covered a pixel",
        _ => return None,
    })
}

/// When the value on an edge arrives (PLAN step 13, IDEAS §22: state flows both ways).
/// Written per input port in a node's `timing`; unset ports are [`When::Now`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    /// This frame's value: the producer runs first in the frame (or is compiled in).
    Now,
    /// The previous frame's value, kept by the executor across frames. Closes cycles over
    /// frames (history); between devices it lets producer and consumer overlap (pipelining:
    /// the consumer's frame N runs while the producer computes frame N+1).
    Prev,
    /// Within one frame, closing a cycle: the consumer runs `n` times (n >= 2), first on
    /// the producer's value of the previous frame, then on each new one; the nodes between
    /// run n - 1 times. E.g. incremental discovery: the drawer draws last frame's faces,
    /// discovery fills the holes, the drawer draws again (`draw.faces` iterates, 2).
    Iterate(u8),
}

impl When {
    pub fn label(self) -> String {
        match self {
            When::Now => "now".into(),
            When::Prev => "prev".into(),
            When::Iterate(n) => format!("iterate {n}"),
        }
    }
}

/// When a discovery back end's drawer gets its faces: cpu-discover discovers one frame
/// ahead (`prev`; its `pipelined` knob, see [`Graph::with_options`]); with hole detection
/// the faces iterate twice within the frame.
fn faces_timing(executor: &str, holes: bool) -> Option<When> {
    match (executor, holes) {
        (_, true) => Some(When::Iterate(2)),
        ("cpu-discover", false) => Some(When::Prev),
        _ => None,
    }
}

/// Edge timings a back end lets a graph choose, with the values it runs (the rest are its
/// composition's): cpu-discover's faces now or a frame late.
const FREE_TIMINGS: &[(&str, Role, &str, &[When])] = &[("cpu-discover", Role::Draw, "faces", &[When::Now, When::Prev])];

/// Knobs that spell an edge timing (the graph says it; the knob is how plans, the CLI and
/// the GUI write it): back end, knob, the edge's role and port, the timing when the knob
/// is true, when false.
const TIMING_KNOBS: &[(&str, &str, Role, &str, When, When)] = &[("cpu-discover", "pipelined", Role::Draw, "faces", When::Prev, When::Now)];

/// The client's components (`mr_core::client::Levels`): `pick` takes the server's LOD
/// levels, `derive` builds them on the client from LOD 0. The sites form writes a client
/// other than `pick` as `client = "derive"`.
pub const CLIENTS: &[&str] = &["pick", "derive"];

/// The sites form's key for the client's component.
pub const CLIENT: &str = "client";

/// The client's knobs (PLAN step 14c2): its region, the chunks around the camera and the
/// LOD levels beyond (capped by what the renderer supports); unset, the run's
/// `view_radius` and `max_lod`. The workload, not the pipeline: left out of the key.
pub const CLIENT_KNOBS: &[&str] = &["radius", "max_lod"];

/// The link's components: `free`, or `net` with `mr_core::link::LinkSpec`'s fields as its
/// knobs. The sites form writes a link other than `free` as `link = "net(latency_ms=30,
/// loss=0.01)"`.
pub const LINKS: &[&str] = &["free", "net"];

/// The sites form's key for the link.
pub const LINK: &str = "link";

/// `component(knob=value, ...)`, the sites form of a node with knobs (the link, the
/// client): the component and the knobs (values in TOML).
pub fn parse_site_knobs(value: &str) -> Result<(String, toml::Table), String> {
    let Some((name, rest)) = value.split_once('(') else { return Ok((value.trim().to_string(), toml::Table::new())) };
    let body = rest.trim_end().strip_suffix(')').ok_or_else(|| format!("`{value}`: knobs are written `{name}(knob=value, ...)`"))?;
    let mut t = toml::Table::new();
    for kv in body.split(',').map(str::trim).filter(|kv| !kv.is_empty()) {
        let (k, v) = kv.split_once('=').ok_or_else(|| format!("`{value}`: `{kv}` is not `knob=value`"))?;
        let v: toml::Table = toml::from_str(&format!("v = {}", v.trim())).map_err(|e| format!("`{value}`: `{kv}`: {}", e.message()))?;
        t.insert(k.trim().to_string(), v["v"].clone());
    }
    Ok((name.trim().to_string(), t))
}

/// The sites form of a component with knobs (see [`parse_site_knobs`]).
pub fn site_with_knobs(component: &str, knobs: &toml::Table) -> String {
    if knobs.is_empty() {
        return component.to_string();
    }
    format!("{component}({})", knobs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", "))
}

/// A link's sites form (`free`, or `net(..)` with the knobs off their defaults).
pub fn link_site(spec: &mr_core::link::LinkSpec) -> String {
    if spec.free() && !spec.reorder { "free".into() } else { site_with_knobs("net", &link_knobs(spec)) }
}

/// The link node `n` models (its knobs read as a `LinkSpec`; `free`: the free link).
fn link_spec(n: &Node) -> Result<mr_core::link::LinkSpec, String> {
    match n.component.as_str() {
        "free" if n.knobs.is_empty() => Ok(Default::default()),
        "free" => Err("the free link has no knobs (`net` has them)".into()),
        _ => toml::Value::Table(n.knobs.clone()).try_into().map_err(|e: toml::de::Error| format!("link `net`: {}", e.message())),
    }
}

/// A link's knobs as the canonical form writes them: the `LinkSpec` read back, its fields
/// off their defaults, so `latency_ms = 30` and `30.0` are one network.
fn link_knobs(spec: &mr_core::link::LinkSpec) -> toml::Table {
    let mut t = toml::Table::try_from(spec).expect("a LinkSpec is a table");
    let none = toml::Table::try_from(mr_core::link::LinkSpec::default()).unwrap();
    t.retain(|k, v| none.get(k) != Some(v));
    t
}

/// Whether `role` component `name` exists.
fn known(role: Role, name: &str) -> bool {
    match role.slot() {
        None if role == Role::Client => CLIENTS.contains(&name),
        None if role == Role::Link => LINKS.contains(&name),
        None => name == "world",
        Some(s) => component(s, name).is_some(),
    }
}

/// Where a node runs unless the graph says otherwise: fused nodes run with their callers;
/// the rest where the back end runs that role (its [`crate::Place`]; GPU slot 0), or for a
/// stage it has not (a free site's geometry) where the component runs, if on one kind of
/// device only.
fn default_device(executor: &str, role: Role, component: &str) -> Device {
    match role {
        Role::World => return Device::Server,
        Role::Link | Role::Client => return Device::Cpu,
        _ => {}
    }
    let only = || role.slot().and_then(|s| crate::component(s, component)).and_then(|c| c.impls.iter().all(|i| i.kind == Kind::Cpu).then_some(Device::Cpu));
    match (backend(executor), role.slot()) {
        (Some(b), Some(slot)) if b.has(slot.source()) && b.place(slot).kind == Kind::Gpu => Device::Gpu(0),
        (Some(b), Some(slot)) if b.has(slot.source()) => Device::Cpu,
        (_, Some(_)) => only().unwrap_or(Device::Gpu(0)),
        (_, None) => Device::Cpu,
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
    /// How it runs there (default: with its callers for storages and kernels, else as the
    /// executor chooses; `cpu`: its `isa` knob).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<Api>,
    /// Input port → the node bound to it. Unbound `changes` inputs read the world.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, String>,
    /// Input port → when its value arrives, where not this frame's ([`When`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub timing: BTreeMap<String, When>,
    #[serde(default, skip_serializing_if = "toml::Table::is_empty")]
    pub knobs: toml::Table,
}

impl Node {
    pub fn new(role: Role, component: &str) -> Self {
        Self { role, component: component.into(), device: None, api: None, inputs: BTreeMap::new(), timing: BTreeMap::new(), knobs: toml::Table::new() }
    }

    fn with(mut self, inputs: &[(&str, &str)]) -> Self {
        self.inputs.extend(inputs.iter().map(|&(p, n)| (p.to_string(), n.to_string())));
        self
    }

    /// When input `port`'s value arrives.
    pub fn when(&self, port: &str) -> When {
        self.timing.get(port).copied().unwrap_or(When::Now)
    }
}

/// A pipeline: nodes by id, run by `executor`: the preset its shape and devices name
/// ([`Graph::preset`]; step 13e4), so a graph may leave it out or name another (its
/// nodes without a device run where that one runs their role).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    #[serde(default)]
    pub executor: String,
    pub nodes: BTreeMap<String, Node>,
}

/// A checked graph: the world added, every node's device, the output node.
#[derive(Debug)]
pub struct Checked {
    pub graph: Graph,
    pub devices: HashMap<String, Device>,
    /// The API of every node that has one set (its own, or a fused node's callers');
    /// the others run as the executor chooses.
    pub apis: HashMap<String, Api>,
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
        // Hole detection: a loop through the drawer (it draws last frame's faces, discovery
        // looks where they leave holes, it draws again), and the last frame's image.
        let holes = has(Slot::Holes) && c.holes != "none";
        if holes {
            vis.push(("where", "holes"));
            add("holes", Role::Holes, &c.holes, &[("covered", "draw"), ("sky", "draw"), ("changes", "world")]);
        }
        add("visibility", Role::Visibility, &c.visibility, &vis);
        if drawn {
            add("draw", Role::Draw, &c.draw, if meshes { &[("faces", "visibility"), ("meshes", "meshes")] } else { &[("faces", "visibility")] });
        }
        let shadow = if b.has(Slot::ShadowTraversal) {
            // On the primary rays' device, a secondary stage naming the primary's component is `same`.
            let apart = b.place(Slot::ShadowTraversal).kind != b.place(Slot::Traversal).kind;
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
        if holes {
            nodes.get_mut("holes").unwrap().timing.insert("sky".into(), When::Prev);
        }
        if let (true, Some(w)) = (drawn, faces_timing(&c.backend, holes)) {
            nodes.get_mut("draw").unwrap().timing.insert("faces".into(), w);
        }
        // Mesh shadows are the shading's shadow rays.
        if let (true, Some(k)) = (meshes, shadow) {
            nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), k.into());
        }
        Ok(Graph { executor: c.backend, nodes })
    }

    /// The preset this graph's shape and devices name (PLAN step 13e4: the back ends'
    /// names are presets of graphs, not what a graph is written for): the face drawer's
    /// source (meshing: `gpu-raster` / `cpu-raster` by the drawer's device; discovery:
    /// `gpu-discover` / `cpu-discover` by the sampler's), else the visibility node's
    /// component and device (ray casting: `cpu`, `gpu-rt` when its primary rays are ray
    /// queries, else `gpu-compute`; `columns`; `splat`). Nodes without a device run where
    /// the written executor runs their role. `None` for a shape no preset has.
    pub fn preset(&self) -> Option<&'static str> {
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role, &n.component)).kind();
        let shading = self.nodes.values().find(|n| n.role == Role::Shading)?;
        let hits = self.nodes.get(shading.inputs.get("hits")?)?;
        let gpu = |n: &Node| dev(n) == Kind::Gpu;
        if hits.role == Role::Draw {
            let src = self.nodes.get(hits.inputs.get("faces")?)?;
            return match (src.role, src.component.as_str()) {
                (Role::Visibility, "quadtree" | "cluster") if gpu(hits) => Some(if gpu(src) { "gpu-discover" } else { "cpu-discover" }),
                (Role::Visibility, "faces") => Some(if gpu(hits) { "gpu-raster" } else { "cpu-raster" }),
                _ => None,
            };
        }
        if hits.role != Role::Visibility {
            return None;
        }
        match (hits.component.as_str(), gpu(hits)) {
            ("columns", g) => Some(if g { "gpu-voxelspace" } else { "cpu-voxelspace" }),
            ("splat", true) => Some("gpu-splat"),
            ("raycast", false) => Some("cpu"),
            ("raycast", true) => {
                let rq = hits.inputs.get("primary").and_then(|k| self.nodes.get(k)).is_some_and(|k| k.component == "rayquery");
                Some(if rq { "gpu-rt" } else { "gpu-compute" })
            }
            _ => None,
        }
    }

    /// The graph run by the preset its shape names ([`Self::preset`]; unchanged if it names
    /// none or the written one). Where the preset changes, nodes in stages the written back
    /// end has keep the device it runs them on (fused nodes still follow their callers), so
    /// the graph means what it was written as; the others run where the preset runs them.
    pub fn placed(&self) -> Graph {
        let Some(p) = self.preset().filter(|p| *p != self.executor) else { return self.clone() };
        let mut g = self.clone();
        if let Some(b) = backend(&self.executor) {
            for n in g.nodes.values_mut().filter(|n| n.role.slot().is_some_and(|s| b.has(s)) && known(n.role, &n.component) && !output(n.role, &n.component).fused()) {
                n.device.get_or_insert_with(|| default_device(&self.executor, n.role, &n.component));
            }
        }
        g.executor = p.into();
        g
    }

    /// The world, the link and the client added if missing, in a chain: the link reads the
    /// world, the client the link, and every other `changes` input (unbound, or bound to the
    /// world or the link) the client: the world's change log reaches a renderer only over
    /// its link, through its client (PLAN steps 14b, 14c2).
    fn normalised(&self) -> Result<Graph, String> {
        let mut g = self.placed();
        let one = |g: &mut Graph, role: Role, component: &str| -> Result<String, String> {
            let ids: Vec<String> = g.nodes.iter().filter(|(_, n)| n.role == role).map(|(k, _)| k.clone()).collect();
            match ids.as_slice() {
                [] => {
                    let id = role.name();
                    if g.nodes.contains_key(id) {
                        return Err(format!("node `{id}` must be the {id} (role = \"{id}\")"));
                    }
                    g.nodes.insert(id.into(), Node::new(role, component));
                    Ok(id.to_string())
                }
                [one] => Ok(one.clone()),
                _ => Err(format!("one {} per graph (have: {})", role.name(), ids.join(", "))),
            }
        };
        let world = one(&mut g, Role::World, "world")?;
        let link = one(&mut g, Role::Link, "free")?;
        let client = one(&mut g, Role::Client, "pick")?;
        for (id, n) in g.nodes.iter_mut() {
            n.timing.retain(|_, w| *w != When::Now);
            let from = if *id == link {
                &world
            } else if *id == client {
                &link
            } else {
                &client
            };
            for p in inputs(n.role, &n.component).iter().filter(|p| p.data == Data::Changes) {
                let src = n.inputs.entry(p.name.into()).or_insert_with(|| from.clone());
                if *src == world || *src == link {
                    *src = from.clone();
                }
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
        let dev = |n: &Node| n.device.unwrap_or_else(|| default_device(&self.executor, n.role, &n.component));
        if vis.role != Role::Visibility || dev(vis) != dev(&self.nodes[shading]) {
            return None;
        }
        inputs(vis.role, &vis.component).iter().filter(|p| p.casts).find_map(|p| vis.inputs.get(p.name).cloned())
    }

    /// This frame's edges into `id`: inputs with timing `now` (`prev` and `iterate` edges
    /// carry values from before; they close cycles).
    fn now_inputs<'a>(&'a self, id: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        let n = &self.nodes[id];
        n.inputs.iter().filter(|(p, _)| n.when(p) == When::Now).map(|(p, s)| (p.as_str(), s.as_str()))
    }

    /// Whether `id` depends on `on` within the frame (through `now` edges; a node on itself).
    fn depends(&self, id: &str, on: &str) -> bool {
        let mut seen = HashSet::new();
        let mut stack = vec![id];
        while let Some(x) = stack.pop() {
            if x == on {
                return true;
            }
            if seen.insert(x) {
                stack.extend(self.now_inputs(x).map(|(_, s)| s));
            }
        }
        false
    }

    /// Producers before consumers over this frame's edges (post-order from the output,
    /// then from nodes only reached through `prev` / `iterate` edges); a cycle of `now`
    /// edges is an error.
    fn frame_order<'a>(&'a self, out: &'a str) -> Result<Vec<&'a str>, String> {
        let mut order = Vec::new();
        let mut state: HashMap<&str, bool> = HashMap::new(); // false: on the stack, true: done
        fn visit<'a>(g: &'a Graph, id: &'a str, state: &mut HashMap<&'a str, bool>, order: &mut Vec<&'a str>) -> Result<(), String> {
            match state.get(id) {
                Some(true) => return Ok(()),
                Some(false) => return Err(format!("the graph has a cycle through `{id}` within one frame: give one of its edges the timing `prev` (the previous frame's value) or `iterate` (passes within the frame)")),
                None => {}
            }
            state.insert(id, false);
            for (_, src) in g.now_inputs(id) {
                visit(g, src, state, order)?;
            }
            state.insert(id, true);
            order.push(id);
            Ok(())
        }
        visit(self, out, &mut state, &mut order)?;
        // Nodes only reached through earlier values, latest first (from the output's side).
        let mut stack: Vec<&str> = vec![out];
        let mut seen: HashSet<&str> = HashSet::new();
        while let Some(id) = stack.pop() {
            if seen.insert(id) {
                if !state.contains_key(id) {
                    visit(self, id, &mut state, &mut order)?;
                }
                stack.extend(self.nodes[id].inputs.values().map(String::as_str));
            }
        }
        Ok(order)
    }

    /// Checks the graph is well formed (not yet whether its back end runs it: see
    /// [`Self::validate`]), with the reasons when it isn't.
    pub fn check(&self) -> Result<Checked, String> {
        let placed = self.placed();
        if placed.executor != self.executor {
            return placed.check();
        }
        let b = backend(&self.executor).ok_or_else(|| format!("unknown back end `{}`", self.executor))?;
        let g = self.normalised()?;
        let mut why = Vec::new();
        for (id, n) in &g.nodes {
            if !known(n.role, &n.component) {
                let have: Vec<&str> = match n.role {
                    Role::Client => CLIENTS.to_vec(),
                    Role::Link => LINKS.to_vec(),
                    r => r.slot().map(|s| crate::components(s).map(|c| c.name).collect()).unwrap_or_default(),
                };
                why.push(format!("`{id}`: unknown {} component `{}` (have: {})", n.role.name(), n.component, have.join(", ")));
                continue;
            }
            match n.role {
                Role::Link => {
                    if let Err(e) = link_spec(n) {
                        why.push(format!("`{id}`: {e}"));
                    }
                }
                Role::Client => {
                    for (k, v) in &n.knobs {
                        match (CLIENT_KNOBS.iter().position(|c| c == k), v.as_integer()) {
                            (None, _) => why.push(format!("`{id}`: the client has no knob `{k}` (has: {})", CLIENT_KNOBS.join(", "))),
                            (Some(0), Some(1..=1024)) | (Some(1), Some(0..=15)) => {}
                            (Some(0), _) => why.push(format!("`{id}`.radius: chunks around the camera, 1 to 1024 (not {v})")),
                            _ => why.push(format!("`{id}`.max_lod: LOD levels beyond LOD 0, 0 to 15 (not {v})")),
                        }
                    }
                }
                _ => {}
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
            }
            for (p, w) in &n.timing {
                match (ports.iter().find(|x| x.name == p), n.inputs.contains_key(p)) {
                    (Some(port), true) if port.data.fused() => why.push(format!("`{id}`.{p}: {} are compiled into their caller, so they arrive within the frame (no timing)", port.data.name())),
                    (_, true) => {}
                    _ => why.push(format!("`{id}`: a timing for `{p}`, which is not a bound input")),
                }
                if matches!(w, When::Iterate(k) if *k < 2) {
                    why.push(format!("`{id}`.{p}: an iteration runs its consumer at least twice (iterate >= 2)"));
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
        // Every node leads to the image, through edges of any timing.
        let mut reached: HashSet<&str> = HashSet::new();
        let mut stack = vec![out.as_str()];
        while let Some(id) = stack.pop() {
            if reached.insert(id) {
                stack.extend(g.nodes[id].inputs.values().map(String::as_str));
            }
        }
        let dangling: Vec<&str> = g.nodes.keys().map(|k| k.as_str()).filter(|k| !reached.contains(k)).collect();
        if !dangling.is_empty() {
            return Err(format!("nodes that lead to no image: {}", dangling.join(", ")));
        }
        let order = g.frame_order(&out)?;
        // An iteration closes a cycle of this frame's edges.
        for (id, n) in &g.nodes {
            for (p, src) in n.inputs.iter().filter(|(p, _)| matches!(n.when(p), When::Iterate(_))) {
                if !g.depends(src, id) {
                    why.push(format!("`{id}`.{p} iterates, but `{src}` does not depend on `{id}` within the frame (no cycle to iterate: use `prev`)"));
                }
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        // Devices, consumers first: a fused node runs with its callers.
        let mut why = Vec::new();
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
                None => default_device(&g.executor, n.role, &n.component),
            };
            devices.insert(id.to_string(), d);
            if (n.role == Role::World) != (d == Device::Server) {
                why.push(if n.role == Role::World { format!("`{id}`: the world runs on the server, not on {d}") } else { format!("`{id}`: only the world runs on the server") });
            } else if n.role == Role::Client && d != Device::Cpu {
                why.push(format!("`{id}`: the client runs on the cpu (the renderer's process), not on {d}"));
            } else if n.role == Role::Link && d != Device::Cpu {
                why.push(format!("`{id}`: the link runs on the cpu (the client's process, in front of the client), not on {d}"));
            }
        }
        for (id, n) in &g.nodes {
            for (p, src) in &n.inputs {
                let s = &g.nodes[src];
                if output(s.role, &s.component).fused() && devices[src] != devices[id] {
                    return Err(format!("`{id}`.{p} on {} cannot read `{src}` on {}: {} are compiled into their caller", devices[id], devices[src], output(s.role, &s.component).name()));
                }
            }
        }
        // Capability needs on what each port reads (on the reader's device): met natively,
        // by an adapter, or not at all.
        for (id, n) in &g.nodes {
            let needs = n.role.slot().and_then(|sl| component(sl, &n.component)).map_or(&[][..], |c| c.needs);
            for src in n.inputs.values() {
                let s = &g.nodes[src];
                for &(slot, cap) in needs.iter().filter(|(slot, _)| s.role.slot() == Some(*slot)) {
                    if !caps::provides(devices[src].kind(), slot, &s.component, cap) && caps::adapter(slot, cap).is_none() {
                        why.push(format!("`{id}` ({}) needs {} from `{src}`: {} `{}` lacks it on the {} and no adapter gives it", n.component, cap.name(), slot.name(), s.component, devices[src].kind().name()));
                    }
                }
            }
        }
        // APIs: a node's own, else (fused) its callers', else the executor's choice.
        let mut apis: HashMap<String, Api> = HashMap::new();
        for &id in order.iter().rev() {
            let n = &g.nodes[id];
            let fused = output(n.role, &n.component).fused();
            // Inherited when every caller runs one API (a module compiled into callers on two
            // APIs of one device runs as each of them).
            let inherited = || {
                let mut callers = g.nodes.iter().filter(|(_, c)| c.inputs.values().any(|s| s == id)).map(|(k, _)| apis.get(k.as_str()).copied());
                let first = callers.next().flatten();
                callers.all(|a| a == first).then_some(first).flatten()
            };
            let api = match n.api {
                Some(a) => {
                    if a.kind() != devices[id].kind() {
                        why.push(format!("`{id}`: api {} runs on a {}, the node is on {}", a.name(), a.kind().name(), devices[id]));
                    } else if let Some(c) = n.role.slot().and_then(|sl| component(sl, &n.component)).filter(|c| !c.impls.iter().any(|i| i.kind == a.kind() && i.apis.contains(&a))) {
                        why.push(format!("`{id}`: {} `{}` has no {} implementation (has: {})", n.role.name(), c.name, a.name(), c.impls_label()));
                    }
                    if let Some(ca) = inherited().filter(|ca| fused && *ca != a) {
                        why.push(format!("`{id}` (api {}) is compiled into callers running {}", a.name(), ca.name()));
                    }
                    Some(a)
                }
                None if fused => inherited(),
                None => None,
            };
            if let Some(a) = api {
                apis.insert(id.to_string(), a);
            }
        }
        if !why.is_empty() {
            return Err(why.join("; "));
        }
        if b.renderer.is_none() {
            return Err(format!("back end `{}` is not available yet", b.name));
        }
        Ok(Checked { graph: g, devices, apis, output: out })
    }

    /// The canonical form: nodes renamed in visiting order from the output (inputs by port
    /// name), devices resolved, APIs where set and a choice, knobs sorted; equal for graphs
    /// that differ only in node ids.
    pub fn canonical(&self) -> Result<String, String> {
        let c = self.check()?;
        let g = &c.graph;
        // The client's region is the workload, not the pipeline (its knobs left out); the
        // default link (`free`) and client (`pick`), nothing else set, are left out: their
        // readers are written as reading what they read, as before steps 14b and 14c2,
        // which keeps the keys.
        let knobs = |n: &Node| -> toml::Table {
            match n.role {
                Role::Client => n.knobs.iter().filter(|(k, _)| !CLIENT_KNOBS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect(),
                Role::Link => link_spec(n).map(|l| link_knobs(&l)).unwrap_or_default(),
                _ => n.knobs.clone(),
            }
        };
        let plain = |id: &str| {
            let n = &g.nodes[id];
            matches!((n.role, n.component.as_str()), (Role::Client, "pick") | (Role::Link, "free")) && knobs(n).is_empty() && n.timing.is_empty() && n.api.is_none() && n.device.is_none_or(|d| d == Device::Cpu)
        };
        let src = |s: &'_ str| -> String {
            let mut s = s;
            while plain(s) {
                s = &g.nodes[s].inputs["changes"];
            }
            s.to_string()
        };
        let mut ids: HashMap<String, usize> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut stack = vec![c.output.clone()];
        while let Some(id) = stack.pop() {
            if ids.contains_key(&id) {
                continue;
            }
            ids.insert(id.clone(), order.len());
            // Reversed so the first port by name is visited first.
            stack.extend(g.nodes[&id].inputs.values().rev().map(|s| src(s)));
            order.push(id);
        }
        // APIs only where the executor offers a choice (elsewhere a set API is checked to be
        // the one it runs, so it says nothing more).
        // (and not the choice's default: an API set to it is the same pipeline as one unset).
        let chosen = |n: &Node, a: &Api| backend(&g.executor).is_some_and(|b| n.role.slot().is_some_and(|sl| b.chooses(sl)) && b.api_choice.is_some_and(|ch| ch.default != Some(*a)));
        let mut s = format!("{}|", g.executor);
        for id in &order {
            let n = &g.nodes[id];
            let when = |p: &str| match n.when(p) {
                When::Now => String::new(),
                When::Prev => "~prev".into(),
                When::Iterate(k) => format!("~iterate{k}"),
            };
            let ins: Vec<String> = n.inputs.iter().map(|(p, s)| format!("{p}=n{}{}", ids[&src(s)], when(p))).collect();
            let knobs: Vec<String> = knobs(n).iter().map(|(k, v)| format!("{k}={v}")).collect();
            let api = c.apis.get(id).filter(|a| chosen(n, a)).map_or(String::new(), |a| format!(":{}", a.name()));
            // (The server is written `cpu`, the world's device before step 14, which keeps
            // the keys: only the world runs there.)
            let dev = match c.devices[id] {
                Device::Server => Device::Cpu,
                d => d,
            };
            s += &format!("n{}={}.{}@{}{api}({}){{{}}};", ids[id], n.role.name(), n.component, dev, ins.join(","), knobs.join(","));
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
        g.pruned()
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
                    // On the GPU, any GPU implementation (the GPU executor compiles walks into
                    // any of its lanes and builds ray queries' acceleration structures). Else
                    // secondary rays run where the back end runs its shadow stages, the others
                    // with its storage and kernel.
                    let secondary = matches!(role, Role::Shading | Role::Meshes);
                    let need = |slot: Slot, id: &str| {
                        let comp = &g.nodes[id].component;
                        if ck.devices[id].kind() == Kind::Gpu && slot != Slot::Geometry {
                            let c = component(slot, comp)?;
                            return (!c.impls.iter().any(|i| i.kind == Kind::Gpu)).then(|| format!("{at} → `{id}`: `{comp}` has no GPU implementation (has: {})", c.impls_label()));
                        }
                        let at_slot = match slot {
                            Slot::Storage if secondary => Slot::ShadowStorage,
                            Slot::Traversal if secondary => Slot::ShadowTraversal,
                            s => s,
                        };
                        crate::unimplemented_at(&b.place(at_slot), slot, comp).map(|w| format!("{at} → `{id}`: {w}"))
                    };
                    if port == "exposure" {
                        why.extend(need(Slot::Storage, &src));
                        continue;
                    }
                    why.extend(need(Slot::Traversal, &src));
                    // Ray queries trace acceleration structures the GPU executor builds on the
                    // CPU from a geometry node (one per pipeline), not in the face drawer's
                    // mesh pipelines.
                    let query = g.walked(&src).filter(|w| g.nodes[*w].role == Role::Geometry);
                    if let Some(geo) = query {
                        let name = &g.nodes[geo].component;
                        if crate::geometry::rt(name).is_none() {
                            why.push(format!("{at} → `{geo}`: geometry `{name}` builds no acceleration structure for ray queries (have: greedy, naive, boxes)"));
                        }
                        if ck.devices[geo] != Device::Cpu {
                            why.push(format!("{at} → `{geo}`: acceleration structures are built from geometry meshed on the CPU (here: {})", ck.devices[geo]));
                        }
                        if role == Role::Meshes && b.has(Slot::Draw) {
                            why.push(format!("{at}: `{}` lights meshes through walks (its mesh pipelines have no ray queries)", g.executor));
                        }
                        let other = g.sites().into_iter().find_map(|(_, _, k)| g.walked(&k).filter(|w| g.nodes[*w].role == Role::Geometry && *w != geo).map(String::from));
                        if let Some(o) = other {
                            why.push(format!("{at}: ray queries over two geometry nodes (`{geo}`, `{o}`): one acceleration structure per pipeline"));
                        }
                        continue;
                    }
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
        let placed = self.placed();
        if placed.executor != self.executor {
            return placed.accept();
        }
        let ck = self.check()?;
        // Timings a back end lets the graph choose; the others must be its composition's.
        let free = |n: &Node, p: &str| FREE_TIMINGS.iter().find(|(e, r, port, _)| *e == self.executor && *r == n.role && *port == p).map(|f| f.3);
        for (id, n) in &ck.graph.nodes {
            for (p, w) in &n.timing {
                if let Some(ok) = free(n, p).filter(|ok| !ok.contains(w)) {
                    return Err(format!("`{id}`.{p}: `{}` runs this edge {}, not {}", self.executor, ok.iter().map(|w| w.label()).collect::<Vec<_>>().join(" or "), w.label()));
                }
            }
        }
        if let Some(why) = self.site_blocker(&ck).or_else(|| device_blocker(&ck)) {
            return Err(why);
        }
        let skel = ck.graph.skeleton();
        let mut c = skel.skeleton_composition()?;
        // Without knobs and APIs (the composition has neither; `cpu` reads its API as `isa`)
        // and free timings (its knobs: see `TIMING_KNOBS`); `all` makes every timed edge
        // `prev` (the graphs' shapes compared, whatever their timings).
        let strip_with = |g: &Graph, all: bool| {
            let mut g = g.clone();
            g.nodes.values_mut().for_each(|n| {
                n.knobs.clear();
                n.api = None;
                // The worker runs the link and the client, whichever they are; the back end
                // sees the client's messages.
                match n.role {
                    Role::Client => n.component = "pick".into(),
                    Role::Link => n.component = "free".into(),
                    _ => {}
                }
                let role = n.role;
                n.timing.retain(|p, _| all || !FREE_TIMINGS.iter().any(|(e, r, port, _)| *e == self.executor && *r == role && port == p));
                if all {
                    n.timing.values_mut().for_each(|w| *w = When::Prev);
                }
            });
            g.canonical()
        };
        let strip = |g: &Graph| strip_with(g, false);
        // The shape on one GPU: which GPU each node runs on is `device_blocker`'s.
        let one_gpu = |g: &Graph| {
            let mut g = g.clone();
            g.nodes.values_mut().filter(|n| matches!(n.device, Some(Device::Gpu(_)))).for_each(|n| n.device = Some(Device::Gpu(0)));
            g
        };
        let expected = Graph::from_composition(&c)?;
        if strip(&expected.skeleton())? != strip(&one_gpu(&skel))? {
            if strip_with(&expected.skeleton(), true)? == strip_with(&one_gpu(&skel), true)? {
                let timed = |g: &Graph| {
                    let v: Vec<String> = g.nodes.iter().flat_map(|(id, n)| n.timing.iter().map(move |(p, w)| format!("`{id}`.{p} {}", w.label()))).collect();
                    if v.is_empty() { "every edge now".to_string() } else { v.join(", ") }
                };
                return Err(format!("`{}` runs `{}` with {} (here: {})", self.executor, c.label(), timed(&expected), timed(&ck.graph)));
            }
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
        if let Some(why) = api_blocker(b, &ck, &c) {
            return Err(why);
        }
        let exact = strip(&Graph::from_composition(&c)?)? == strip(self)?;
        Ok((c, exact))
    }

    /// The graph with the edge timings its knobs spell set (`TIMING_KNOBS`: cpu-discover's
    /// `pipelined = false` makes the drawer's faces arrive `now`). Knobs not given keep the
    /// graph's timing.
    pub fn with_options(self, options: &toml::Table) -> Graph {
        let mut g = self.placed();
        for &(_, knob, role, port, on, off) in TIMING_KNOBS.iter().filter(|k| k.0 == g.executor) {
            let Some(v) = options.get(knob).and_then(|v| v.as_bool()) else { continue };
            for n in g.nodes.values_mut().filter(|n| n.role == role && n.inputs.contains_key(port)) {
                n.timing.insert(port.into(), if v { on } else { off });
                n.timing.retain(|_, w| *w != When::Now);
            }
        }
        g
    }

    /// The knobs spelling this graph's timings, as `with_options` reads them.
    pub fn timing_options(&self) -> Vec<(&'static str, bool)> {
        let g = self.placed();
        let mut v = Vec::new();
        for &(_, knob, role, port, on, _) in TIMING_KNOBS.iter().filter(|k| k.0 == g.executor) {
            if let Some(n) = g.nodes.values().find(|n| n.role == role && n.inputs.contains_key(port)) {
                v.push((knob, n.when(port) == on));
            }
        }
        v
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
        let placed = self.placed();
        if placed.executor != self.executor {
            return placed.flat_knobs();
        }
        let skel = self.normalised()?.skeleton();
        let mut t = toml::Table::new();
        if let Some((b, ch)) = backend(&self.executor).and_then(|b| b.api_choice.map(|ch| (b, ch))) {
            if let Some(v) = self.nodes.values().filter(|n| n.role.slot().is_some_and(|sl| b.chooses(sl))).find_map(|n| n.api).and_then(|a| ch.value(a)) {
                t.insert(ch.knob.into(), v.into());
            }
        }
        for (k, v) in self.normalised()?.timing_options() {
            t.insert(k.into(), v.into());
        }
        // (The link's and the client's knobs are theirs: the worker's, not the back end's.)
        for (id, n) in self.nodes.iter().filter(|(id, n)| skel.nodes.contains_key(*id) && n.role.slot().is_some()) {
            for (k, v) in &n.knobs {
                if let Some(old) = t.insert(k.clone(), v.clone()).filter(|old| old != v) {
                    return Err(format!("knob `{k}` set to {old} and {v} (`{id}`): `{}` has one `{k}` for its stages", self.executor));
                }
            }
        }
        Ok(t)
    }
}

impl Graph {
    /// The graph with flat knob values placed on the nodes they belong to (step 10: what a
    /// compiled build bakes in, for its cache key): each `(spec, value)` goes to the
    /// skeleton's nodes its `owner` names (the back end's stages; nodes on free sites keep
    /// their own knobs). A knob whose owner is not in the graph is moot here and left out;
    /// a knob without an owner (the back end's own) has no node and is an error.
    pub fn with_knobs(&self, knobs: &[(&mr_core::ParamSpec, toml::Value)]) -> Result<Graph, String> {
        let mut g = self.normalised()?;
        let skel = g.skeleton();
        for (spec, v) in knobs {
            let Some((slot, comps)) = &spec.owner else {
                return Err(format!("knob `{}` belongs to the back end, not to a node", spec.name));
            };
            for (_, n) in g.nodes.iter_mut().filter(|(id, _)| skel.nodes.contains_key(*id)) {
                if n.role.name() == slot && comps.iter().any(|c| *c == n.component) {
                    n.knobs.entry(spec.name.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        Ok(g)
    }
}

/// Why the GPUs `ck` places its nodes on cannot run, if they can't (PLAN step 13e4c): a
/// pipeline runs on one GPU (any slot), or split over two at the hit records: ray casting's
/// shading (`placement = deferred`, with the walks compiled into it) on another GPU than the
/// rest, the records crossing between them (the schedule's transfer).
fn device_blocker(ck: &Checked) -> Option<String> {
    let g = &ck.graph;
    let mut slots: Vec<u8> = ck.devices.values().filter_map(|d| if let Device::Gpu(i) = d { Some(*i) } else { None }).collect();
    slots.sort();
    slots.dedup();
    if slots.len() < 2 {
        return None;
    }
    let on = |d: Device| {
        let mut v: Vec<&str> = g.nodes.keys().map(String::as_str).filter(|id| ck.devices[*id] == d).collect();
        v.sort();
        v.iter().map(|id| format!("`{id}`")).collect::<Vec<_>>().join(", ")
    };
    let used = slots.iter().map(|&i| format!("{} ({})", Device::Gpu(i), on(Device::Gpu(i)))).collect::<Vec<_>>().join("; ");
    if !matches!(g.executor.as_str(), "gpu-compute" | "gpu-rt") {
        return Some(format!("`{}` runs on one GPU (here: {used}); a pipeline over two GPUs is split at the hit records, ray casting's deferred shading on the second", g.executor));
    }
    let (Some(vis), Some(shading)) = (g.nodes.iter().find(|(_, n)| n.role == Role::Visibility), g.nodes.iter().find(|(_, n)| n.role == Role::Shading)) else {
        return Some(format!("a pipeline over two GPUs is split between its visibility and shading nodes (here: {used})"));
    };
    let (near, far) = (ck.devices[vis.0], ck.devices[shading.0]);
    // The nodes on the shading's GPU are the shading and what is compiled into it (fused
    // nodes follow their callers), the rest on the visibility's.
    let stray: Vec<String> = g.nodes.iter().filter(|(id, n)| matches!(ck.devices[*id], Device::Gpu(_)) && ck.devices[*id] != near && (far == near || (*id != shading.0 && !output(n.role, &n.component).fused()))).map(|(id, _)| format!("`{id}` on {}", ck.devices[id])).collect();
    if slots.len() > 2 || far == near || !stray.is_empty() {
        return Some(format!("only the shading and the walks compiled into it run on a second GPU (here: {used}{})", if stray.is_empty() { String::new() } else { format!("; not so: {}", stray.join(", ")) }));
    }
    let mut why = Vec::new();
    if shading.1.knobs.get("placement").and_then(|v| v.as_str()) != Some("deferred") {
        why.push(format!("`{}` on {far} shades the hit records `{}` writes on {near}: placement = deferred", shading.0, vis.0));
    }
    if let Some((id, n)) = g.nodes.iter().find(|(_, n)| n.role == Role::Meshes && n.component != "none") {
        why.push(format!("`{id}` ({}): mesh surfaces are rasterised on {near}, and the shading on {far} has not their triangles (meshes = none)", n.component));
    }
    (!why.is_empty()).then(|| why.join("; "))
}

/// Why the APIs set in `ck` cannot run on back end `b` running composition `c`, if they
/// can't: a non-fused node's API must be the one the back end runs its stage with, or one
/// of its choices; the stages its `api_choice` sets run with one API.
fn api_blocker(b: &crate::BackendInfo, ck: &Checked, c: &Composition) -> Option<String> {
    let g = &ck.graph;
    let mut why = Vec::new();
    for (id, &a) in ck.apis.iter().filter(|(id, _)| g.nodes[*id].api.is_some()) {
        // A fused node runs as the stages calling it (through other fused nodes) do.
        let mut stages = vec![id.as_str()];
        while let Some(i) = stages.iter().position(|s| output(g.nodes[*s].role, &g.nodes[*s].component).fused()) {
            let s = stages.remove(i);
            stages.extend(g.nodes.iter().filter(|(_, n)| n.inputs.values().any(|x| x == s)).map(|(k, _)| k.as_str()));
        }
        stages.sort();
        stages.dedup();
        for st in stages {
            let Some(slot) = g.nodes[st].role.slot() else { continue };
            let place = b.place(slot);
            let at = if st == id { format!("`{id}`") } else { format!("`{id}` (in `{st}`)") };
            match b.api(slot, c) {
                Some(x) if x != a => why.push(format!("{at}: `{}` runs its {} with {} here, not {}", b.name, slot.name(), x.name(), a.name())),
                None if !place.apis.contains(&a) => why.push(format!("{at}: `{}` runs its {} with {}, not {}", b.name, slot.name(), place.label(), a.name())),
                _ => {}
            }
        }
    }
    if let Some(ch) = b.api_choice {
        let mut set: Vec<Api> = ck.apis.iter().filter(|(id, _)| g.nodes[*id].role.slot().is_some_and(|sl| b.chooses(sl))).map(|(_, a)| *a).collect();
        set.sort();
        set.dedup();
        if set.len() > 1 {
            why.push(format!("`{}` runs its {} with one api, its `{}` (have: {})", b.name, ch.slots.iter().map(|s| s.name()).collect::<Vec<_>>().join(", "), ch.knob, set.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")));
        }
    }
    (!why.is_empty()).then(|| why.join("; "))
}

/// Whether `p` is a site: a ray-casting port, or discovery's exposure tests (their storage).
fn is_site(p: &Port) -> bool {
    p.casts || p.name == "exposure"
}

/// The short name of `role`'s site `port`: the port's name for the strategy and the
/// shading (`primary`, `rays`, `exposure`, `shadow`, `bounce`), else `<role>-<port>`
/// (`meshes-shadow`).
pub fn site_name(role: Role, port: &str) -> String {
    match role {
        Role::Visibility | Role::Shading => port.to_string(),
        r => format!("{}-{port}", r.name()),
    }
}

/// Step 9d: the *sites form* of a graph, its flat composition plus a binding per site that
/// the composition binds otherwise (`shadow = "tree64/sphere"`), for the CLI
/// (`gpu-compute:prefix+incremental,bounce=tree64/sphere`), plans (`sites = { ... }`),
/// labels, sweeps and the GUI. A site's value is one of:
/// - another site's name: the same kernel (an exposure site: the same storage);
/// - `<kernel>`: a kernel of its own over the lead's storage;
/// - `<storage>/<kernel>`: a kernel of its own over a storage of its own, or `<site>/<kernel>`
///   over that site's storage;
/// - for exposure tests, `<storage>`: a storage of their own.
///
/// Unlisted sites follow the rays before them, as unbound ports do: rebinding the shadow
/// rays takes the bounces and mesh shadows along unless they are listed too.
impl Graph {
    /// The sites of the graph's nodes (bound or not): name, caster node, port. The lead
    /// first, then by role and port order.
    pub fn site_ports(&self) -> Vec<(String, String, &'static Port)> {
        let mut v: Vec<(String, String, &'static Port)> = Vec::new();
        for (id, n) in &self.nodes {
            for p in inputs(n.role, &n.component).iter().filter(|p| is_site(p)) {
                v.push((site_name(n.role, p.name), id.clone(), p));
            }
        }
        let lead = self.lead();
        let rank = |(_, c, p): &(String, String, &'static Port)| {
            let n = &self.nodes[c];
            let at = inputs(n.role, &n.component).iter().position(|x| x.name == p.name).unwrap_or(0);
            (lead.as_ref().is_none_or(|(lc, lp)| lc != c || *lp != p.name), n.role, at)
        };
        v.sort_by_key(|s| rank(s));
        v
    }

    /// The node bound to site `name` (a kernel; for exposure tests a storage).
    pub fn site_node(&self, name: &str) -> Option<&str> {
        let (_, caster, port) = self.site_ports().into_iter().find(|(n, ..)| n == name)?;
        self.nodes.get(&caster)?.inputs.get(port.name).map(String::as_str)
    }

    /// The storage site `name` reads: its kernel's storage, or the exposure tests' own.
    fn site_storage(&self, name: &str) -> Option<String> {
        let (_, caster, port) = self.site_ports().into_iter().find(|(n, ..)| n == name)?;
        let b = self.nodes.get(&caster)?.inputs.get(port.name)?;
        if port.data == Data::Rays { self.walked(b).map(String::from) } else { Some(b.clone()) }
    }

    /// The site whose rays a site follows when it is not bound (or listed) itself.
    fn followed(&self, name: &str) -> Option<String> {
        match name {
            "bounce" | "meshes-shadow" => Some("shadow".into()),
            "shadow" | "exposure" => self.lead().map(|(c, p)| site_name(self.nodes[&c].role, p)).filter(|l| l != name),
            _ => None,
        }
    }

    /// The graph with `sites` bound as given (see the sites form above).
    pub fn with_sites(&self, sites: &BTreeMap<String, String>) -> Result<Graph, String> {
        let mut g = self.normalised()?;
        let mut sites = sites.clone();
        for (key, role, have) in [(CLIENT, Role::Client, CLIENTS), (LINK, Role::Link, LINKS)] {
            if let Some(v) = sites.remove(key) {
                let (c, knobs) = parse_site_knobs(&v)?;
                if !have.contains(&c.as_str()) {
                    return Err(format!("unknown {key} `{c}` (have: {})", have.join(", ")));
                }
                for n in g.nodes.values_mut().filter(|n| n.role == role) {
                    n.component = c.clone();
                    n.knobs = knobs.clone();
                }
            }
        }
        let sites = &sites;
        let names: Vec<String> = g.site_ports().into_iter().map(|(n, ..)| n).collect();
        if let Some(bad) = sites.keys().find(|k| !names.contains(k)) {
            return Err(format!("no site `{bad}` in this pipeline (have: {}, {LINK}, {CLIENT})", names.join(", ")));
        }
        // A value naming another listed site waits until that one is bound.
        let refers = |v: &str, n: &str| v == n || v.split_once('/').is_some_and(|(s, _)| s == n);
        let mut pending: Vec<(&String, &String)> = sites.iter().collect();
        while !pending.is_empty() {
            let Some(i) = pending.iter().position(|(_, v)| !pending.iter().any(|(n, _)| refers(v, n))) else {
                return Err(format!("sites refer to each other in a cycle: {}", pending.iter().map(|(n, v)| format!("{n} = {v}")).collect::<Vec<_>>().join(", ")));
            };
            let (name, value) = pending.remove(i);
            g.bind_site(name, value, sites)?;
        }
        Ok(g.pruned())
    }

    /// Binds site `name` to `value`; the sites following it (not in `listed`) and bound as
    /// it was come along.
    fn bind_site(&mut self, name: &str, value: &str, listed: &BTreeMap<String, String>) -> Result<(), String> {
        let ports = self.site_ports();
        let (_, caster, port) = ports.iter().find(|(n, ..)| n == name).cloned().unwrap();
        let is_site = |s: &str| ports.iter().any(|(n, ..)| n == s);
        let lead_storage = self.lead().and_then(|(c, p)| self.site_storage(&site_name(self.nodes[&c].role, p)));
        let fresh = |g: &Graph, id: String| {
            let mut out = id.clone();
            let mut i = 2;
            while g.nodes.contains_key(&out) {
                out = format!("{id}{i}");
                i += 1;
            }
            out
        };
        let add = |g: &mut Graph, id: String, node: Node| {
            let id = fresh(g, id);
            g.nodes.insert(id.clone(), node);
            id
        };
        let unknown = |role: Role, c: &str| format!("site `{name}`: unknown {} component `{c}` (have: {})", role.name(), crate::components(role.slot().unwrap()).map(|c| c.name).collect::<Vec<_>>().join(", "));
        let old = self.nodes[&caster].inputs.get(port.name).cloned();
        let target = if port.data == Data::Storage {
            if is_site(value) {
                self.site_storage(value).ok_or_else(|| format!("site `{name}` = `{value}`: that site is not bound"))?
            } else if known(Role::Storage, value) {
                add(self, format!("{name}.storage"), Node::new(Role::Storage, value).with(&[("changes", "world")]))
            } else {
                return Err(unknown(Role::Storage, value));
            }
        } else if is_site(value) {
            self.site_node(value).map(String::from).ok_or_else(|| format!("site `{name}` = `{value}`: that site is not bound"))?
        } else {
            let (st, k) = match value.split_once('/') {
                Some((s, k)) => (Some(s), k),
                None => (None, value),
            };
            if !known(Role::Traversal, k) {
                return Err(unknown(Role::Traversal, k));
            }
            let walks = inputs(Role::Traversal, k).iter().find(|p| p.required).unwrap();
            let src = match st {
                None => lead_storage.clone().ok_or_else(|| format!("site `{name}` = `{value}`: no lead storage to walk; name one (`<storage>/{k}`)"))?,
                Some(s) if is_site(s) => self.site_storage(s).ok_or_else(|| format!("site `{name}` = `{value}`: site `{s}` is not bound"))?,
                Some(s) => {
                    let role = if walks.data == Data::Faces { Role::Geometry } else { Role::Storage };
                    if !known(role, s) {
                        return Err(unknown(role, s));
                    }
                    add(self, format!("{name}.{}", role.name()), Node::new(role, s).with(&[("changes", "world")]))
                }
            };
            add(self, format!("{name}.kernel"), Node::new(Role::Traversal, k).with(&[(walks.name, &src)]))
        };
        self.nodes.get_mut(&caster).unwrap().inputs.insert(port.name.into(), target.clone());
        // Followers bound as this site was come along (unless listed themselves).
        if let Some(old) = old {
            for (f, fc, fp) in &ports {
                if f != name && !listed.contains_key(f) && self.followed(f).as_deref() == Some(name) && self.nodes[fc].inputs.get(fp.name) == Some(&old) && fp.data == port.data {
                    self.bind_site(f, name, listed)?;
                }
            }
        }
        Ok(())
    }

    /// Without the nodes that lead to no image (the world stays).
    fn pruned(mut self) -> Graph {
        let Some(out) = self.nodes.iter().find(|(_, n)| known(n.role, &n.component) && output(n.role, &n.component) == Data::Image).map(|(k, _)| k.clone()) else { return self };
        let mut keep = std::collections::HashSet::new();
        let mut stack = vec![out];
        while let Some(id) = stack.pop() {
            if keep.insert(id.clone()) {
                stack.extend(self.nodes.get(&id).into_iter().flat_map(|n| n.inputs.values().cloned()));
            }
        }
        self.nodes.retain(|k, n| keep.contains(k) || matches!(n.role, Role::World | Role::Link | Role::Client));
        self
    }

    /// The client node's component (`pick` when the graph has none).
    pub fn client(&self) -> &str {
        self.nodes.values().find(|n| n.role == Role::Client).map_or("pick", |n| n.component.as_str())
    }

    /// The client's knob `name` ([`CLIENT_KNOBS`]), if set.
    pub fn client_knob(&self, name: &str) -> Option<i64> {
        self.nodes.values().find(|n| n.role == Role::Client).and_then(|n| n.knobs.get(name)).and_then(|v| v.as_integer())
    }

    /// The network the link node models (free when the graph has none).
    pub fn link(&self) -> Result<mr_core::link::LinkSpec, String> {
        self.nodes.values().find(|n| n.role == Role::Link).map_or(Ok(Default::default()), link_spec)
    }

    /// The sites form's value for the link or the client (`key`: [`LINK`], [`CLIENT`]),
    /// the default written out.
    pub fn sites_value(&self, key: &str) -> String {
        let g = self.normalised().unwrap_or_else(|_| self.clone());
        g.world_sites().remove(key).unwrap_or_else(|| if key == LINK { "free" } else { "pick" }.into())
    }

    /// The link and the client in the sites form (`link = "net(latency_ms=30)"`, `client =
    /// "derive(radius=4)"`): those other than the default, nothing set.
    fn world_sites(&self) -> BTreeMap<String, String> {
        let mut t = BTreeMap::new();
        for n in self.nodes.values() {
            let (key, default) = match n.role {
                Role::Link => (LINK, "free"),
                Role::Client => (CLIENT, "pick"),
                _ => continue,
            };
            let knobs = if n.role == Role::Link { link_spec(n).map(|l| link_knobs(&l)).unwrap_or_else(|_| n.knobs.clone()) } else { n.knobs.clone() };
            if n.component != default || !knobs.is_empty() {
                t.insert(key.to_string(), site_with_knobs(&n.component, &knobs));
            }
        }
        t
    }

    /// The lead site's name (the strategy's own rays, or a rasteriser's shadow rays).
    pub fn lead_site(&self) -> Option<String> {
        self.lead().map(|(c, p)| site_name(self.nodes[&c].role, p))
    }

    /// Every bound site but the lead, in site order, described as in the sites form
    /// (nothing left out: what the composition binds so too).
    pub fn site_values(&self) -> Vec<(String, String)> {
        let me = self;
        // Every site described, the lead first so the others can name what they share.
        let ports = me.site_ports();
        let lead = me.lead().map(|(c, p)| site_name(me.nodes[&c].role, p));
        let lead_storage = lead.as_deref().and_then(|l| me.site_storage(l));
        let mut kernels: HashMap<String, String> = HashMap::new(); // kernel node → first site using it
        let mut storages: HashMap<String, String> = HashMap::new(); // storage node → first site reading it
        let mut sites = Vec::new();
        for (name, caster, port) in &ports {
            let Some(b) = me.nodes[caster].inputs.get(port.name) else { continue };
            let st = me.site_storage(name);
            if Some(name) == lead.as_ref() {
                kernels.insert(b.clone(), name.clone());
                st.into_iter().for_each(|s| drop(storages.insert(s, name.clone())));
                continue;
            }
            let value = if port.data == Data::Storage {
                storages.get(b).cloned().unwrap_or_else(|| me.nodes[b].component.clone())
            } else if let Some(owner) = kernels.get(b) {
                owner.clone()
            } else {
                let k = me.nodes[b].component.clone();
                match &st {
                    s if *s == lead_storage => k,
                    Some(s) => format!("{}/{k}", storages.get(s).cloned().unwrap_or_else(|| me.nodes[s].component.clone())),
                    None => k,
                }
            };
            if port.data == Data::Rays {
                kernels.entry(b.clone()).or_insert(name.clone());
            }
            if let Some(s) = st {
                storages.entry(s).or_insert(name.clone());
            }
            sites.push((name.clone(), value));
        }
        sites
    }

    /// The graph in its sites form: the composition the back end runs it as and the sites
    /// bound otherwise (`None` if it cannot be written so: nodes on free sites have knobs,
    /// or a shape the form has no words for).
    pub fn sites_form(&self) -> Result<Option<(Composition, BTreeMap<String, String>)>, String> {
        let (c, exact) = self.accept()?;
        // A link or client other than the default (which `accept` leaves to the worker) is
        // written out.
        let client = self.normalised()?.world_sites();
        if exact && client.is_empty() {
            return Ok(Some((c, client)));
        }
        let me = self.normalised()?.pruned();
        let skel = me.skeleton();
        if me.nodes.iter().any(|(id, n)| !skel.nodes.contains_key(id) && !n.knobs.is_empty()) {
            return Ok(None);
        }
        let strip = |g: &Graph| {
            let mut g = g.clone();
            g.nodes.values_mut().for_each(|n| n.knobs.clear());
            g.canonical()
        };
        let target = strip(&me)?;
        let base = Graph::from_composition(&c)?;
        let described = me.site_values();
        let order: Vec<String> = described.iter().map(|(n, _)| n.clone()).collect();
        let mut sites: BTreeMap<String, String> = described.into_iter().chain(client).collect();
        let same = |s: &BTreeMap<String, String>| base.with_sites(s).and_then(|g| strip(&g)).is_ok_and(|x| x == target);
        if !same(&sites) {
            return Ok(None);
        }
        // Only what differs from the composition: drop the sites it binds so anyway, last first.
        for name in order.iter().rev() {
            let mut fewer = sites.clone();
            fewer.remove(name);
            // Another site naming this one keeps it.
            if !fewer.values().any(|v| v == name || v.split_once('/').is_some_and(|(s, _)| s == name)) && same(&fewer) {
                sites = fewer;
            }
        }
        Ok(Some((c, sites)))
    }

    /// A label: the composition's, then the sites bound otherwise
    /// (`gpu-compute: prefix + incremental, bounce = tree64/sphere`); a graph without a sites
    /// form gets its key.
    pub fn label(&self) -> Result<String, String> {
        Ok(match self.sites_form()? {
            Some((c, s)) => sites_label(&c, &s),
            None => format!("{} [graph {}]", self.accept()?.0.label(), &self.key()?[..8]),
        })
    }

    /// The values site `name` could take on its own (for sweeps' `"*"` and the GUI; not
    /// checked against the back end): the other sites it could share, a kernel over the
    /// lead's storage, and each storage with each kernel (exposure tests: each storage).
    pub fn site_choices(&self, name: &str) -> Vec<String> {
        if name == CLIENT {
            return CLIENTS.iter().map(|c| c.to_string()).collect();
        }
        if name == LINK {
            return LINKS.iter().map(|c| c.to_string()).collect();
        }
        let ports = self.site_ports();
        let Some((_, _, port)) = ports.iter().find(|(n, ..)| n == name) else { return vec![] };
        let others = ports.iter().filter(|(n, _, p)| n != name && p.data == port.data).map(|(n, ..)| n.clone());
        let storages = || crate::components(Slot::Storage).map(|c| c.name.to_string());
        if port.data == Data::Storage {
            return others.chain(storages()).collect();
        }
        let kernels: Vec<String> = crate::components(Slot::Traversal).map(|c| c.name.to_string()).collect();
        let geometries: Vec<String> = crate::components(Slot::Geometry).map(|c| c.name.to_string()).collect();
        let mut v: Vec<String> = others.chain(kernels.iter().cloned()).collect();
        for k in &kernels {
            let on = if inputs(Role::Traversal, k).iter().any(|p| p.data == Data::Faces) { geometries.clone() } else { storages().collect() };
            v.extend(on.iter().map(|s| format!("{s}/{k}")));
        }
        v
    }
}

/// A composition's label with sites bound otherwise.
pub fn sites_label(c: &Composition, sites: &BTreeMap<String, String>) -> String {
    let mut s = c.label();
    for (k, v) in sites {
        s += &format!(", {k} = {v}");
    }
    s
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
        let knobs: toml::Table = serde_json::from_value(serde_json::Value::Object(opts)).unwrap_or_default();
        Graph::from_composition(&c).map_err(anyhow::Error::msg)?.with_options(&knobs)
    } else {
        serde_json::from_str(&p.graph_json)?
    };
    let g = g.placed();
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
        ("gpu-compute", Role::Shading | Role::Meshes, _) | ("gpu-rt", Role::Shading, _) => Bind::Free,
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

    /// The API node `id` runs with: its own or its callers' (`apis`), else its component's
    /// only one on the node's device (e.g. `rayquery`'s ray queries).
    pub fn api(&self, id: &str) -> Option<Api> {
        self.apis.get(id).copied().or_else(|| {
            let n = self.node(id);
            let kind = self.devices[id].kind();
            let apis: Vec<Api> = component(n.role.slot()?, &n.component)?.impls.iter().filter(|i| i.kind == kind).flat_map(|i| i.apis.iter().copied()).collect();
            match apis[..] {
                [a] => Some(a),
                _ => None,
            }
        })
    }

    /// The node of `role` (strategy, shading, draw, prepass, meshes: one each).
    pub fn find(&self, role: Role) -> Option<&str> {
        self.graph.nodes.iter().find(|(_, n)| n.role == role).map(|(k, _)| k.as_str())
    }

    /// The ids of the nodes of `roles`, in that order: what a timed stage runs (PLAN step
    /// 11; callers only, the storages and kernels fused into them are timed with them).
    /// A role without a node (added live, e.g. a prepass switched on) is named as
    /// [`Graph::from_composition`] names it: by the role.
    pub fn ids(&self, roles: &[Role]) -> Vec<String> {
        roles.iter().map(|&r| self.find(r).unwrap_or(r.name()).to_string()).collect()
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

/// How an executor runs a checked graph (PLAN step 13): computed once when the pipeline is
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
        // Work of its own: a non-fused output, or an update from a non-fused input.
        let is_pass = |id: &str| {
            let n = &g.nodes[id];
            !out_of(id).fused() || n.inputs.is_empty() || n.inputs.values().any(|s| !out_of(s).fused())
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
            assert_eq!(g.preset(), Some(c.backend.as_str()), "{}", c.label());
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
    /// Step 10: a node's `api`, checked against its device, its component's implementations
    /// and the executor; written into the canonical form only where it is a choice.
    #[test]
    fn apis() {
        let set = |c: &Composition, at: &[(&str, Api)]| {
            let mut g = Graph::from_composition(c).unwrap();
            for (id, a) in at {
                g.nodes.get_mut(*id).unwrap().api = Some(*a);
            }
            g
        };
        // cpu: one API for its one loop, read as its `isa` knob; fused nodes follow.
        let cpu = Composition::new("cpu");
        let g = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx2)]);
        let (c, exact) = g.accept().unwrap();
        assert!(exact && c == Composition::new("cpu"));
        assert_eq!(g.flat_knobs().unwrap().get("isa"), Some(&toml::Value::String("avx2".into())));
        let ck = g.check().unwrap();
        assert_eq!(ck.apis.get("kernel"), Some(&Api::Avx2), "the kernel runs with its caller");
        // A node without an API of its own or its callers' runs its component's only one on
        // its device (what the ray-casting executor picks its lane by).
        // A walk's kernel has two (compute, raster: `dispatch`), so it has none.
        for (backend, api) in [("gpu-rt", Some(Api::RayQuery)), ("gpu-compute", None)] {
            let ck = Graph::from_composition(&Composition::new(backend)).unwrap().check().unwrap();
            assert_eq!(ck.api(ck.input(Role::Visibility, "primary").unwrap()), api, "{backend}");
        }
        assert!(g.canonical().unwrap().contains(":avx2") && g.key().unwrap() != Graph::from_composition(&cpu).unwrap().key().unwrap());
        let e = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx512)]).validate().unwrap_err();
        assert!(e.contains("with one api, its `isa`"), "{e}");
        let e = set(&cpu, &[("visibility", Api::Avx2), ("shading", Api::Avx2), ("kernel", Api::Scalar)]).validate().unwrap_err();
        assert!(e.contains("`kernel` (api scalar) is compiled into callers running avx2"), "{e}");
        let e = set(&cpu, &[("shading", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("runs on a gpu"), "{e}");
        // GPU back ends: the API they run the stage with, and only that.
        let gc = Composition::default();
        assert!(set(&gc, &[("visibility", Api::Compute)]).validate().is_ok());
        assert_eq!(set(&gc, &[("visibility", Api::Compute)]).key(), Graph::from_composition(&gc).unwrap().key(), "the default: not in the key");
        // gpu-compute's walk: a compute thread or a fragment shader per pixel (`dispatch`).
        let frag = set(&gc, &[("visibility", Api::Raster)]);
        assert_eq!(frag.flat_knobs().unwrap().get("dispatch"), Some(&toml::Value::String("fragment".into())));
        assert!(frag.validate().is_ok() && frag.key() != Graph::from_composition(&gc).unwrap().key());
        let e = set(&gc, &[("visibility", Api::Raster), ("shading", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("with one api, its `dispatch`"), "{e}");
        let e = set(&Composition { prepass: "beam".into(), ..Default::default() }, &[("prepass", Api::Raster)]).validate().unwrap_err();
        assert!(e.contains("`beam` has no raster implementation"), "{e}");
        let e = set(&Composition::new("gpu-rt"), &[("visibility", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("runs its visibility with rayquery here, not compute"), "{e}");
        let e = set(&Composition::new("gpu-raster"), &[("shading", Api::Compute)]).validate().unwrap_err();
        assert!(e.contains("runs its shading with raster here, not compute"), "{e}");
        let e = set(&gc, &[("kernel", Api::RayQuery)]).validate().unwrap_err();
        assert!(e.contains("`kernel` (in `meshes`): `gpu-compute` runs its meshes with raster here, not rayquery"), "{e}");
        // A face drawer's shading: in the fragment shader for `raster`, else deferred compute.
        let r = Composition::new("gpu-raster");
        assert!(set(&r, &[("shading", Api::Raster)]).validate().is_ok());
        assert_eq!(set(&r, &[("shading", Api::Raster)]).key(), Graph::from_composition(&r).unwrap().key(), "no choice: not in the key");
        assert!(set(&r, &[("shading", Api::Compute)]).validate().is_err());
        let vb = Composition { draw: "visbuffer".into(), ..Composition::new("gpu-raster") };
        assert!(set(&vb, &[("shading", Api::Compute)]).validate().is_ok());
        // Implementations: cpu-raster is scalar only; pathtrace has no CPU implementation.
        let e = set(&Composition::new("cpu-raster"), &[("shading", Api::Avx2)]).validate().unwrap_err();
        assert!(e.contains("runs its shading with scalar here, not avx2"), "{e}");
        let mut g = Graph::from_composition(&Composition::new("cpu")).unwrap();
        g.nodes.get_mut("shading").unwrap().component = "pathtrace".into();
        assert!(g.validate().is_err());
        // Devices follow where the back end runs a role: rasterisers build faces on the CPU.
        let ck = Graph::from_composition(&r).unwrap().check().unwrap();
        assert_eq!((ck.devices["geometry"], ck.devices["draw"], ck.devices["kernel"]), (Device::Cpu, Device::Gpu(0), Device::Gpu(0)));
        // Plans and records: `api` round-trips through TOML.
        let t = toml::to_string(&set(&cpu, &[("visibility", Api::Avx512)])).unwrap();
        assert!(t.contains("api = \"avx512\""), "{t}");
        assert_eq!(toml::from_str::<Graph>(&t).unwrap(), set(&cpu, &[("visibility", Api::Avx512)]));
    }

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
        // gpu-raster: bounces apart from the shadow rays, also as ray queries over the
        // geometry it draws (step 13e4b; before, a kernel it lacked).
        let r = Composition { shading: "pathtrace".into(), ..Composition::new("gpu-raster") };
        assert!(!with(r.clone(), &|g| own(g, "b", "prefix", "incremental", &[("shading", "bounce")])).accept().unwrap().1);
        let q = with(r, &|g| {
            g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "rq".into());
        });
        assert!(!q.accept().unwrap().1);
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
        // gpu-rt: its shading's rays free (step 13e4b): a kernel node of their own over its
        // one acceleration structure.
        let g = with(Composition { shading: "pathtrace".into(), ..Composition::new("gpu-rt") }, &|g| {
            g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "geometry")]));
            g.nodes.get_mut("shading").unwrap().inputs.insert("bounce".into(), "rq".into());
        });
        assert!(!g.accept().unwrap().1);
        // Knobs of free-site nodes stay with their nodes.
        let mut g = with(Composition::default(), &|g| own(g, "s", "prefix", "incremental", &[("shading", "shadow")]));
        g.nodes.get_mut("s.kernel").unwrap().knobs.insert("exact".into(), toml::Value::Boolean(false));
        g.nodes.get_mut("kernel").unwrap().knobs.insert("exact".into(), toml::Value::Boolean(true));
        assert_eq!(g.flat_knobs().unwrap().get("exact"), Some(&toml::Value::Boolean(true)));
    }

    /// Step 9d: every graph one or two site bindings away from a back end's compositions
    /// has a sites form, and that form gives the graph back.
    #[test]
    fn sites_form_round_trips() {
        let mut bases = Vec::new();
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let mut c = Composition::new(b.name);
            c.fill_defaults();
            bases.push(c.clone());
            if b.has(Slot::Shading) && names_on(b.name, Slot::Shading).contains(&"pathtrace") {
                c.shading = "pathtrace".into();
                if b.has(Slot::Meshes) && names_on(b.name, Slot::Meshes).len() > 1 {
                    c.meshes = names_on(b.name, Slot::Meshes).into_iter().find(|m| *m != "none").unwrap().into();
                }
                bases.push(c);
            }
        }
        let (mut graphs, mut keys) = (0, std::collections::HashSet::new());
        let check = |c: &Composition, sites: &BTreeMap<String, String>, keys: &mut std::collections::HashSet<String>| -> bool {
            let base = Graph::from_composition(c).unwrap();
            let Ok(g) = base.with_sites(sites) else { return false };
            if g.accept().is_err() {
                return false;
            }
            let (c2, s2) = g.sites_form().unwrap().unwrap_or_else(|| panic!("{}: {sites:?} has no sites form", c.label()));
            let again = Graph::from_composition(&c2).unwrap().with_sites(&s2).unwrap();
            assert_eq!(again.canonical(), g.canonical(), "{} {sites:?} → {} {s2:?}", c.label(), c2.label());
            assert!(g.label().unwrap().starts_with(&c2.label()));
            keys.insert(g.key().unwrap());
            true
        };
        for c in &bases {
            let base = Graph::from_composition(c).unwrap();
            let names: Vec<String> = base.site_ports().into_iter().map(|(n, ..)| n).collect();
            for n in &names {
                for v in base.site_choices(n) {
                    graphs += check(c, &BTreeMap::from([(n.clone(), v.clone())]), &mut keys) as usize;
                }
            }
            // Pairs: shadow and bounce rays on gpu-compute, a sample of values.
            if c.backend == "gpu-compute" && c.shading == "pathtrace" {
                for a in ["primary", "sphere", "rows/rowrun", "tree64/refloor"] {
                    for b in ["shadow", "incremental", "distance/sphere", "shadow/sphere", "primary"] {
                        graphs += check(c, &BTreeMap::from([("shadow".into(), a.into()), ("bounce".into(), b.into())]), &mut keys) as usize;
                    }
                }
            }
        }
        eprintln!("{graphs} site bindings run, {} distinct graphs", keys.len());
        assert!(graphs > 300, "{graphs}");
        // Labels and errors.
        let pt = Composition { shading: "pathtrace".into(), ..Default::default() };
        let g = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("bounce".into(), "tree64/sphere".into())])).unwrap();
        assert_eq!(g.label().unwrap(), "gpu-compute: bricks + refloor + pathtrace, bounce = tree64/sphere");
        // Rebinding the shadow rays takes the bounces along: a composition again.
        let g = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("shadow".into(), "rows/rowrun".into())])).unwrap();
        assert_eq!(g.label().unwrap(), "gpu-compute: bricks + refloor + shadow rows + shadow rowrun + pathtrace");
        let e = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("nope".into(), "x".into())])).unwrap_err();
        assert!(e.contains("no site `nope`"), "{e}");
        let e = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("bounce".into(), "x/refloor".into())])).unwrap_err();
        assert!(e.contains("unknown storage component `x`"), "{e}");
        let e = Graph::from_composition(&pt).unwrap().with_sites(&BTreeMap::from([("bounce".into(), "shadow".into()), ("shadow".into(), "bounce".into())])).unwrap_err();
        assert!(e.contains("cycle"), "{e}");
    }

    /// Step 13e4: the back ends' names are presets, named by a graph's shape and devices,
    /// whatever executor it is written for (or none).
    #[test]
    fn presets() {
        let key = |c: Composition| Graph::from_composition(&c).unwrap().key().unwrap();
        let of = |b: &str| Graph::from_composition(&Composition::new(b)).unwrap();
        // Discovery's sampler moved to the CPU: the shading's shadow rays get their own
        // instances on the GPU, the drawer its faces a frame late or now.
        let mut g = of("gpu-discover");
        g.nodes.get_mut("visibility").unwrap().device = Some(Device::Cpu);
        for (id, n) in [("shadow.kernel", g.nodes["kernel"].clone().with(&[("storage", "shadow.storage")])), ("shadow.storage", g.nodes["storage"].clone())] {
            g.nodes.insert(id.into(), Node { device: Some(Device::Gpu(0)), ..n });
        }
        g.nodes.get_mut("shading").unwrap().inputs.insert("shadow".into(), "shadow.kernel".into());
        assert_eq!(g.placed().executor, "cpu-discover");
        let now: toml::Table = toml::from_str("pipelined = false").unwrap();
        assert_eq!(g.key().unwrap(), of("cpu-discover").with_options(&now).key().unwrap());
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Prev);
        assert_eq!(g.key().unwrap(), key(Composition::new("cpu-discover")));
        assert_eq!(g.to_composition().unwrap().backend, "cpu-discover");
        // The drawer's face source a choice: discovery in place of meshing.
        let mut g = Graph::from_composition(&Composition { meshes: "none".into(), ..Composition::new("gpu-raster") }).unwrap();
        g.nodes.retain(|k, _| k != "geometry");
        g.nodes.insert("visibility".into(), Node::new(Role::Visibility, "quadtree").with(&[("rays", "kernel"), ("exposure", "storage")]));
        assert_eq!(g.key().unwrap(), key(Composition::new("gpu-discover")));
        // Ray queries for the primary rays, written for gpu-compute or for no executor.
        let mut g = of("gpu-rt");
        g.executor = "gpu-compute".into();
        assert_eq!(g.key().unwrap(), of("gpu-rt").key().unwrap());
        for b in ["gpu-compute", "gpu-splat", "gpu-voxelspace", "gpu-raster", "gpu-discover"] {
            let mut g = of(b);
            g.executor.clear();
            assert_eq!(g.key().unwrap(), of(b).key().unwrap(), "{b}");
        }
        // Without an executor, nodes run on the GPU unless they say otherwise.
        let mut g = of("cpu");
        g.executor.clear();
        assert_eq!(g.placed().executor, "gpu-compute");
        g.nodes.values_mut().filter(|n| matches!(n.role, Role::Visibility | Role::Shading)).for_each(|n| n.device = Some(Device::Cpu));
        assert_eq!(g.key().unwrap(), of("cpu").key().unwrap());
        // A shape no preset has keeps its reasons.
        let mut g = of("cpu-raster");
        g.nodes.get_mut("draw").unwrap().device = Some(Device::Gpu(0));
        assert_eq!(g.placed().executor, "gpu-raster");
        assert!(g.validate().is_err());
    }
    /// A pipeline on one GPU runs on any slot; over two it is split at the hit records (ray
    /// The client's component is a choice of the sites form (`client = "derive"`): the back
    /// end runs the graph as before, its key and label say so; `pick` stays out of both.
    #[test]
    fn client_in_the_sites_form() {
        let c = Composition::new("gpu-compute");
        let pick = Graph::from_composition(&c).unwrap();
        let sites: BTreeMap<String, String> = [(CLIENT.to_string(), "derive".to_string())].into();
        let derive = pick.with_sites(&sites).unwrap();
        assert_eq!(derive.client(), "derive");
        assert_eq!(derive.to_composition().unwrap().label(), pick.to_composition().unwrap().label());
        assert_ne!(derive.key().unwrap(), pick.key().unwrap());
        assert_eq!(derive.sites_form().unwrap().unwrap().1, sites);
        assert!(derive.label().unwrap().ends_with(", client = derive"), "{}", derive.label().unwrap());
        assert!(pick.sites_form().unwrap().unwrap().1.is_empty());
        let with_shadow: BTreeMap<String, String> = [(CLIENT.to_string(), "derive".to_string()), ("shadow".to_string(), "tree64/sphere".to_string())].into();
        let g = pick.with_sites(&with_shadow).unwrap();
        let (c, form) = g.sites_form().unwrap().unwrap();
        assert_eq!(form.get(CLIENT).map(String::as_str), Some("derive"));
        assert_eq!(Graph::from_composition(&c).unwrap().with_sites(&form).unwrap().key().unwrap(), g.key().unwrap());
        let bad: BTreeMap<String, String> = [(CLIENT.to_string(), "fetch".to_string())].into();
        assert!(pick.with_sites(&bad).unwrap_err().contains("unknown client"));
    }

    /// The link as a node (PLAN step 14c2): `link = "net(..)"` in the sites form; the free
    /// link and the client's region leave the key alone, each network has its own; the
    /// back end runs the graph as its composition.
    #[test]
    fn link_and_client_knobs() {
        let pick = Graph::from_composition(&Composition::new("gpu-compute")).unwrap();
        let with = |kv: &[(&str, &str)]| pick.with_sites(&kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        let key = |g: &Graph| g.key().unwrap();
        assert_eq!(key(&with(&[(LINK, "free")]).unwrap()), key(&pick));
        assert_eq!(key(&with(&[(CLIENT, "pick(radius=4, max_lod=3)")]).unwrap()), key(&pick), "the region is not in the key");
        let home = with(&[(LINK, "net(latency_ms=30, jitter_ms=10, loss=0.01, reorder=true)")]).unwrap();
        assert_ne!(key(&home), key(&pick));
        assert_eq!(key(&with(&[(LINK, "net(reorder=true, latency_ms=30.0, loss=0.01, jitter_ms=10)")]).unwrap()), key(&home), "knob order and 30 / 30.0");
        assert_ne!(key(&with(&[(LINK, "net(latency_ms=31, jitter_ms=10, loss=0.01, reorder=true)")]).unwrap()), key(&home));
        assert_eq!(home.link().unwrap(), mr_core::link::LinkSpec { latency_ms: 30.0, jitter_ms: 10.0, loss: 0.01, reorder: true, ..Default::default() });
        assert_eq!(home.to_composition().unwrap().label(), pick.to_composition().unwrap().label());
        let (_, form) = home.sites_form().unwrap().unwrap();
        assert_eq!(form[LINK], "net(jitter_ms=10.0, latency_ms=30.0, loss=0.01, reorder=true)");
        assert_eq!(key(&pick.with_sites(&form).unwrap()), key(&home));
        let region = with(&[(CLIENT, "derive(radius=4)"), (LINK, "net(latency_ms=1)")]).unwrap();
        assert_eq!((region.client(), region.client_knob("radius"), region.client_knob("max_lod")), ("derive", Some(4), None));
        assert_eq!(region.sites_form().unwrap().unwrap().1[CLIENT], "derive(radius=4)");
        assert!(region.label().unwrap().ends_with(", client = derive(radius=4), link = net(latency_ms=1.0)"), "{}", region.label().unwrap());
        // The chain: world -> link -> client -> the renderer's readers.
        let ck = home.check().unwrap();
        let reads = |id: &str| ck.graph.nodes[id].inputs["changes"].as_str();
        assert_eq!((reads("link"), reads("client")), ("world", "link"));
        assert_eq!(ck.devices["link"], Device::Cpu);
        for (bad, why) in [("net(latency=3)", "unknown field"), ("free(latency_ms=3)", "no knobs"), ("udp", "unknown link"), ("net(latency_ms=3", "knobs are written")] {
            assert!(with(&[(LINK, bad)]).and_then(|g| g.check().map(|_| ())).unwrap_err().contains(why), "{bad}");
        }
        for bad in ["pick(radius=0)", "pick(levels=2)", "pick(max_lod=99)"] {
            assert!(with(&[(CLIENT, bad)]).unwrap().check().is_err(), "{bad}");
        }
        // An old graph with the client reading the world gets the link in between.
        let mut old = pick.normalised().unwrap();
        old.nodes.remove("link");
        old.nodes.get_mut("client").unwrap().inputs.insert("changes".into(), "world".into());
        assert_eq!(old.normalised().unwrap().nodes["client"].inputs["changes"], "link");
        assert_eq!(key(&old), key(&pick));
    }

    /// casting's deferred shading, with its walks, on the second GPU); else the reasons
    /// (PLAN step 13e4c).
    #[test]
    fn two_gpus() {
        let split = |b: &str, placement: &str| {
            let mut g = Graph::from_composition(&Composition::new(b)).unwrap();
            g.nodes.retain(|_, n| n.role != Role::Meshes);
            g.nodes.values_mut().for_each(|n| drop(n.inputs.remove("meshes")));
            g.nodes.insert("s.storage".into(), Node::new(Role::Storage, "prefix").with(&[("changes", "world")]));
            g.nodes.insert("s.kernel".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "s.storage")]));
            let sh = g.nodes.get_mut("shading").unwrap();
            sh.inputs.insert("shadow".into(), "s.kernel".into());
            sh.device = Some(Device::Gpu(1));
            sh.knobs.insert("placement".into(), toml::Value::String(placement.into()));
            g
        };
        for b in ["gpu-compute", "gpu-rt"] {
            let g = split(b, "deferred");
            let (_, exact) = g.accept().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert!(!exact, "{b}: the split is no flat composition (its devices would be lost)");
            let ck = g.check().unwrap();
            assert_eq!((ck.devices["s.kernel"], ck.devices["visibility"]), (Device::Gpu(1), Device::Gpu(0)));
            let sched = ck.schedule().unwrap().to_string();
            assert!(sched.contains("gpu -> gpu1"), "{b}: {sched}");
            let why = split(b, "inline").accept().unwrap_err();
            assert!(why.contains("placement = deferred"), "{b}: {why}");
        }
        // The whole pipeline on the second GPU.
        let mut g = Graph::from_composition(&Composition::new("gpu-compute")).unwrap();
        g.nodes.values_mut().filter(|n| n.role != Role::World).for_each(|n| n.device = Some(Device::Gpu(1)));
        g.accept().unwrap();
        // Meshes on the first GPU with the shading on the second; three GPUs; a drawer's shading.
        let mut g = split("gpu-compute", "deferred");
        g.nodes.insert("m.kernel".into(), Node::new(Role::Traversal, "refloor").with(&[("storage", "storage")]));
        g.nodes.insert("meshes".into(), Node::new(Role::Meshes, "composite").with(&[("changes", "world"), ("shadow", "m.kernel")]));
        g.nodes.get_mut("visibility").unwrap().inputs.insert("meshes".into(), "meshes".into());
        let why = g.accept().unwrap_err();
        assert!(why.contains("meshes = none"), "{why}");
        let mut g = split("gpu-compute", "deferred");
        g.nodes.get_mut("storage").unwrap().device = Some(Device::Gpu(2));
        assert!(g.accept().is_err());
        let mut g = Graph::from_composition(&Composition::new("gpu-raster")).unwrap();
        g.nodes.insert("s.storage".into(), Node::new(Role::Storage, "prefix").with(&[("changes", "world")]));
        g.nodes.insert("s.kernel".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "s.storage")]));
        let sh = g.nodes.get_mut("shading").unwrap();
        sh.inputs.insert("shadow".into(), "s.kernel".into());
        sh.device = Some(Device::Gpu(1));
        let why = g.accept().unwrap_err();
        assert!(why.contains("runs on one GPU"), "{why}");
    }


    /// Step 13e4b: any GPU ray-casting site binds any kernel with a GPU implementation: ray
    /// queries over a geometry node's acceleration structures (one per pipeline) beside
    /// walks, walks beside ray queries; the reasons where not.
    #[test]
    fn ray_query_sites() {
        let of = |b: &str| Graph::from_composition(&Composition { shading: "pathtrace".into(), ..Composition::new(b) }).unwrap();
        let rq = |g: &mut Graph, geo: &str, k: &str| {
            g.nodes.insert(geo.into(), Node::new(Role::Geometry, "greedy"));
            g.nodes.insert(k.into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", geo)]));
        };
        let walk = |g: &mut Graph| {
            g.nodes.insert("st".into(), Node::new(Role::Storage, "prefix"));
            g.nodes.insert("wk".into(), Node::new(Role::Traversal, "incremental").with(&[("storage", "st")]));
        };
        let bind = |g: &mut Graph, port: &str, k: &str| drop(g.nodes.get_mut("shading").unwrap().inputs.insert(port.into(), k.into()));
        // Shadow rays as ray queries in a walk, in GPU discovery, in CPU discovery's drawing.
        for b in ["gpu-compute", "gpu-discover", "cpu-discover"] {
            let mut g = of(b);
            rq(&mut g, "geo", "rq");
            bind(&mut g, "shadow", "rq");
            bind(&mut g, "bounce", "rq");
            let g = g.pruned();
            g.validate().unwrap_or_else(|e| panic!("{b}: {e}"));
            assert_eq!(g.check().unwrap().devices["geo"], Device::Cpu, "{b}");
        }
        // Bounces as ray queries in the rasterisers; walks for gpu-rt's shadow rays.
        for b in ["gpu-raster", "gpu-splat"] {
            let mut g = of(b);
            rq(&mut g, "geo2", "rq");
            bind(&mut g, "bounce", "rq");
            g.validate().unwrap_or_else(|e| panic!("{b}: {e}"));
        }
        let mut g = of("gpu-rt");
        walk(&mut g);
        bind(&mut g, "shadow", "wk");
        g.validate().unwrap();
        // Reasons: geometry without acceleration structures, two of them, mesh shadows of the
        // face drawer, a CPU-only storage on the GPU (its capabilities checked first).
        let mut g = of("gpu-compute");
        g.nodes.insert("pts".into(), Node::new(Role::Geometry, "points"));
        g.nodes.insert("rq".into(), Node::new(Role::Traversal, "rayquery").with(&[("geometry", "pts")]));
        bind(&mut g, "shadow", "rq");
        assert!(g.validate().unwrap_err().contains("builds no acceleration structure"));
        let mut g = of("gpu-rt");
        rq(&mut g, "geo2", "rq2");
        bind(&mut g, "shadow", "rq2");
        assert!(g.validate().unwrap_err().contains("one acceleration structure per pipeline"));
        let mut g = of("gpu-raster");
        rq(&mut g, "geo2", "rq");
        g.nodes.get_mut("meshes").unwrap().inputs.insert("shadow".into(), "rq".into());
        assert!(g.validate().unwrap_err().contains("its mesh pipelines have no ray queries"));
        let mut g = of("gpu-compute");
        g.nodes.insert("st".into(), Node::new(Role::Storage, "palette"));
        g.nodes.insert("wk".into(), Node::new(Role::Traversal, "refloor").with(&[("storage", "st")]));
        bind(&mut g, "shadow", "wk");
        let e = g.validate().unwrap_err();
        assert!(e.contains("storage `palette` lacks it on the gpu"), "{e}");
    }

    #[test]
    fn cpu_discover_needs_shadow_rays() {
        // cpu-discover without GPU shadow rays cannot shade lit.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("shading").unwrap().inputs.remove("shadow");
        g.nodes.retain(|k, _| !k.starts_with("shadow."));
        let e = g.validate().unwrap_err();
        assert!(e.contains("input `shadow`"), "{e}");
    }

    /// Step 13a/b: edge timings. Hole detection is a loop through the drawer (it draws last
    /// frame's faces, the holes node finds where they leave holes, the sampler looks there,
    /// the drawer draws again) plus last frame's image read `prev`; and the reasons for what
    /// is not a pipeline.
    #[test]
    fn timings() {
        let it = Graph::from_composition(&Composition { holes: "coverage".into(), ..Composition::new("gpu-discover") }).unwrap();
        assert_eq!(it.nodes["draw"].when("faces"), When::Iterate(2));
        assert_eq!(it.nodes["holes"].when("sky"), When::Prev);
        assert_eq!(it.nodes["visibility"].inputs.get("where").map(String::as_str), Some("holes"));
        // Written as TOML and as JSON (the worker's form), canonical with its timings.
        let toml_text = toml::to_string(&it).unwrap();
        assert!(toml_text.contains("iterate = 2") && toml_text.contains("sky = \"prev\""), "{toml_text}");
        assert_eq!(toml::from_str::<Graph>(&toml_text).unwrap(), it);
        assert_eq!(serde_json::from_str::<Graph>(&serde_json::to_string(&it).unwrap()).unwrap(), it);
        let canon = it.canonical().unwrap();
        assert!(canon.contains("~iterate2") && canon.contains("~prev"), "{canon}");
        // An explicit `now` is the same pipeline as none.
        let base = Graph::from_composition(&Composition::new("gpu-discover")).unwrap();
        let mut now = base.clone();
        now.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Now);
        assert_eq!(now.key().unwrap(), base.key().unwrap());
        // The schedule: one loop, the consumer first, the shading after it.
        let sched = it.check().unwrap().schedule().unwrap();
        let names = |ps: &[Pass]| ps.iter().map(|p| p.node.clone()).collect::<Vec<_>>();
        let loops: Vec<&Step> = sched.steps.iter().filter(|s| matches!(s, Step::Loop { .. })).collect();
        let [Step::Loop { passes, body }] = loops.as_slice() else { panic!("one loop\n{sched}") };
        assert_eq!((*passes, names(body)), (2, vec!["draw".to_string(), "holes".to_string(), "visibility".to_string()]), "{sched}");
        assert!(matches!(sched.steps.last(), Some(Step::Pass(p)) if p.node == "shading"), "{sched}");
        assert!(sched.kept.iter().any(|e| e.to == "draw" && e.port == "faces") && sched.kept.iter().any(|e| e.to == "holes" && e.port == "sky"), "{sched}");
        // gpu-discover runs it, and it is the composition.
        assert_eq!(it.accept().unwrap(), (Composition { holes: "coverage".into(), ..Composition::new("gpu-discover") }, true));
        // Reasons. A cycle within the frame says how to break it.
        let mut g = it.clone();
        g.nodes.get_mut("draw").unwrap().timing.clear();
        let e = g.check().unwrap_err();
        assert!(e.contains("cycle through") && e.contains("`prev`"), "{e}");
        // Faces a frame late instead of the loop: a valid graph gpu-discover does not run.
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Prev);
        let e = g.validate().unwrap_err();
        assert!(e.contains("runs `gpu-discover: bricks + quadtree + coverage` with") && e.contains("iterate 2"), "{e}");
        let mut g = base.clone();
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Iterate(2));
        let e = g.check().unwrap_err();
        assert!(e.contains("no cycle to iterate"), "{e}");
        let mut g = it.clone();
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Iterate(1));
        let e = g.check().unwrap_err();
        assert!(e.contains("at least twice"), "{e}");
        let mut g = base.clone();
        g.nodes.get_mut("kernel").unwrap().timing.insert("storage".into(), When::Prev);
        let e = g.check().unwrap_err();
        assert!(e.contains("compiled into their caller") && e.contains("no timing"), "{e}");
        let mut g = base.clone();
        g.nodes.get_mut("visibility").unwrap().timing.insert("where".into(), When::Prev);
        let e = g.check().unwrap_err();
        assert!(e.contains("not a bound input"), "{e}");
        // A free timing outside what the back end runs.
        let mut g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        g.nodes.get_mut("draw").unwrap().timing.insert("faces".into(), When::Iterate(2));
        g.nodes.get_mut("visibility").unwrap().inputs.insert("where".into(), "draw".into());
        assert!(g.validate().is_err());
    }

    /// The schedule across devices. cpu-discover: discovery on the CPU, drawing on the GPU;
    /// faces and the change log cross devices; by default the faces arrive a frame late and
    /// the devices overlap (`pipelined = true`); `pipelined = false` spells `now`.
    #[test]
    fn schedule_across_devices() {
        let g = Graph::from_composition(&Composition::new("cpu-discover")).unwrap();
        let sched = g.check().unwrap().schedule().unwrap();
        let crosses = |s: &Schedule, from: &str, to: &str| s.transfers.iter().find(|e| e.from == from && e.to == to).map(|e| (e.from_device, e.to_device, e.when));
        assert_eq!(crosses(&sched, "visibility", "draw"), Some((Device::Cpu, Device::Gpu(0), When::Prev)), "{sched}");
        // The link: the server's stream into the link (in the client's process), the
        // client's messages to the GPU.
        assert_eq!(crosses(&sched, "world", "link"), Some((Device::Server, Device::Cpu, When::Now)), "{sched}");
        assert_eq!(crosses(&sched, "link", "client"), None, "{sched}");
        assert_eq!(crosses(&sched, "client", "shadow.storage"), Some((Device::Cpu, Device::Gpu(0), When::Now)), "{sched}");
        assert_eq!(crosses(&sched, "client", "storage"), None, "{sched}");
        assert!(sched.pipelined(), "{sched}");
        let vis = sched.passes().find(|p| p.node == "visibility").unwrap();
        assert_eq!((vis.device, vis.fused.clone()), (Device::Cpu, vec!["kernel".to_string(), "storage".to_string()]));
        assert!(vis.state.is_some());
        assert_eq!(g.timing_options(), vec![("pipelined", true)]);
        let off: toml::Table = toml::from_str("pipelined = false").unwrap();
        let sync = g.clone().with_options(&off);
        let sched = sync.check().unwrap().schedule().unwrap();
        assert_eq!(crosses(&sched, "visibility", "draw"), Some((Device::Cpu, Device::Gpu(0), When::Now)), "{sched}");
        assert!(!sched.pipelined());
        assert_eq!(sync.timing_options(), vec![("pipelined", false)]);
        // Both run, as the same composition; the knob carries the difference.
        assert_eq!(sync.accept().unwrap(), (Composition::new("cpu-discover"), true));
        assert_eq!(sync.flat_knobs().unwrap().get("pipelined"), Some(&toml::Value::Boolean(false)));
        assert_ne!(sync.key().unwrap(), g.key().unwrap());
    }

    /// Every composition schedules: the output after what it reads, walking kernels only compiled in, every other
    /// node a pass of its own, fused nodes compiled into a pass on their device.
    #[test]
    fn every_composition_schedules() {
        for c in every_composition() {
            let ck = Graph::from_composition(&c).unwrap().check().unwrap();
            let s = ck.schedule().unwrap_or_else(|e| panic!("{}: {e}", c.label()));
            let passes: Vec<&Pass> = s.passes().collect();
            // The output after everything it reads within the frame (discovery a frame ahead
            // runs after it).
            let at = |id: &str| passes.iter().position(|p| p.node == id);
            let out = at(&ck.output).unwrap();
            assert!(ck.graph.nodes.keys().filter(|id| ck.graph.depends(&ck.output, id)).filter_map(|id| at(id)).all(|i| i <= out), "{}\n{s}", c.label());
            // Kept values and loops only where discovery's timings are.
            let looped = s.steps.iter().any(|x| matches!(x, Step::Loop { .. }));
            assert_eq!(looped, c.holes == "coverage", "{}\n{s}", c.label());
            assert_eq!(s.kept.is_empty(), c.holes != "coverage" && c.backend != "cpu-discover", "{}\n{s}", c.label());
            for (id, n) in &ck.graph.nodes {
                let is_pass = passes.iter().any(|p| p.node == *id);
                // Ray queries build their acceleration structures from faces: a pass too.
                assert_eq!(is_pass, n.role != Role::Traversal || n.component == "rayquery", "{}: `{id}`\n{s}", c.label());
                if output(n.role, &n.component).fused() {
                    assert!(passes.iter().any(|p| p.fused.contains(id) && p.device == ck.devices[id.as_str()]), "{}: `{id}` compiled into nothing\n{s}", c.label());
                }
            }
            for e in &s.transfers {
                assert!(!e.data.fused() && e.from_device != e.to_device);
            }
        }
    }
}
