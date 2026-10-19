//! The graph model: roles, port data, devices, ports, timings, nodes, graphs.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Api, Kind, Slot, backend, component};


/// What a node does: the roles of the pipeline's pieces that exist so far.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The authoritative chunk store and its change log, on the server.
    World,
    /// The world's generator: on the server (the default), feeding the world,
    /// which streams its chunks; or on the client's `cpu`, feeding the client from the
    /// world's spec (a shared seed), the server then sending that client edits only.
    Generator,
    /// The link the server's stream crosses to a client: `free` (everything
    /// arrives at once, in order) or `net`, a modelled network (`mr_core::link::LinkSpec`
    /// as its knobs). Runs in the client's process, in front of the client.
    Link,
    /// The client of the world's stream: what its renderer holds, derived
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
    /// What turns the shading's linear colour into the output's: a tone
    /// mapper (`crate::post::Tone`), compiled into the shading's output on its device.
    Post,
}

impl Role {
    pub const ALL: [Role; 14] = [Role::World, Role::Generator, Role::Link, Role::Client, Role::Storage, Role::Geometry, Role::Prepass, Role::Traversal, Role::Visibility, Role::Holes, Role::Draw, Role::Shading, Role::Meshes, Role::Post];

    pub fn name(self) -> &'static str {
        match self {
            Role::World => "world",
            Role::Generator => "generator",
            Role::Link => "link",
            Role::Client => "client",
            Role::Post => "post",
            r => r.slot().unwrap().name(),
        }
    }

    /// The registry slot whose components fill this role (none for the world, its generator,
    /// the link, the client and the post).
    pub fn slot(self) -> Option<Slot> {
        Some(match self {
            Role::World | Role::Generator | Role::Link | Role::Client | Role::Post => return None,
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
    /// Generated chunks (a generator's: any chunk on request).
    Chunks,
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
            Data::Chunks => "generated chunks",
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

    /// Compiled together with the caller: never crosses devices.
    pub fn fused(self) -> bool {
        matches!(self, Data::Storage | Data::Rays)
    }
}

/// Where a node runs. `gpu` is GPU slot 0 of the run's device configuration; `server` is
/// the world's machine, reached from the client's devices over the link.
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
const STREAM: &[Port] = &[
    port("changes", Data::Changes, true, "the server's stream as it arrives over the link"),
    port("chunks", Data::Chunks, false, "the generator, on the client: chunks generated here (the server then sends edits only)"),
];
const GENERATED: &[Port] = &[port("chunks", Data::Chunks, false, "the generator, on the server (unbound: the generator is on the client)")];
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
const POSTED: &[Port] = &[port("image", Data::Image, true, "the shading's linear colour (compiled into its output)")];
const PATHTRACE: &[Port] = &[
    port("hits", Data::Hits, true, "the visible surface per pixel"),
    port("shadow", Data::Rays, false, "shadow rays (unbound: the back end's own)"),
    port("bounce", Data::Rays, false, "bounce rays (unbound: the back end's own)"),
];

/// The inputs of `role` component `name`.
pub fn inputs(role: Role, name: &str) -> &'static [Port] {
    match (role, name) {
        (Role::World, _) => GENERATED,
        (Role::Generator, _) => &[],
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
        (Role::Post, _) => POSTED,
    }
}

/// What `role` component `name` outputs.
pub fn output(role: Role, name: &str) -> Data {
    match (role, name) {
        (Role::World | Role::Link | Role::Client, _) => Data::Changes,
        (Role::Generator, _) => Data::Chunks,
        (Role::Storage, _) => Data::Storage,
        (Role::Geometry, _) => Data::Faces,
        (Role::Prepass, _) => Data::Starts,
        (Role::Traversal, _) => Data::Rays,
        (Role::Visibility, "quadtree" | "cluster" | "faces") => Data::Faces,
        (Role::Visibility | Role::Draw, _) => Data::Hits,
        (Role::Holes, _) => Data::Tiles,
        (Role::Shading | Role::Post, _) => Data::Image,
        (Role::Meshes, _) => Data::Surfaces,
    }
}

/// What a stateful component keeps across frames: owned by the
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

/// When the value on an edge arrives (state flows both ways).
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
pub(super) fn faces_timing(executor: &str, holes: bool) -> Option<When> {
    match (executor, holes) {
        (_, true) => Some(When::Iterate(2)),
        ("cpu-discover", false) => Some(When::Prev),
        _ => None,
    }
}

/// Edge timings a back end lets a graph choose, with the values it runs (the rest are its
/// composition's): cpu-discover's faces now or a frame late; ray casting's shading on a
/// second GPU shading this frame's hit records or the last frame's (the GPUs
/// then overlap; `device_blocker` says why not on one GPU).
pub(super) const FREE_TIMINGS: &[(&str, Role, &str, &[When])] = &[
    ("cpu-discover", Role::Draw, "faces", &[When::Now, When::Prev]),
    ("gpu-compute", Role::Shading, "hits", &[When::Now, When::Prev]),
    ("gpu-rt", Role::Shading, "hits", &[When::Now, When::Prev]),
];

/// Knobs that spell an edge timing (the graph says it; the knob is how plans, the CLI and
/// the GUI write it): back end, knob, the edge's role and port, the timing when the knob
/// is true, when false.
pub(super) const TIMING_KNOBS: &[(&str, &str, Role, &str, When, When)] = &[("cpu-discover", "pipelined", Role::Draw, "faces", When::Prev, When::Now)];

/// The client's components (`mr_core::client::Levels`): `pick` takes the server's LOD
/// levels, `derive` builds them on the client from LOD 0. The sites form writes a client
/// other than `pick` as `client = "derive"`.
pub const CLIENTS: &[&str] = &["pick", "derive"];

/// The sites form's key for the client's component.
pub const CLIENT: &str = "client";

/// The client's knobs: its region, the chunks around the camera and the
/// LOD levels beyond (capped by what the renderer supports); unset, the run's
/// `view_radius` and `max_lod`. The workload, not the pipeline: left out of the key.
pub const CLIENT_KNOBS: &[&str] = &["radius", "max_lod"];

/// The link's components: `free`, or `net` with `mr_core::link::LinkSpec`'s fields as its
/// knobs. The sites form writes a link other than `free` as `link = "net(latency_ms=30,
/// loss=0.01)"`.
pub const LINKS: &[&str] = &["free", "net"];

/// The sites form's key for the link.
pub const LINK: &str = "link";

/// The generator's components: `procedural`, the world's generator built
/// from its spec (the run's world: terrain, menger, ...).
pub const GENERATORS: &[&str] = &["procedural"];

/// The sites form's key for the generator: its device, `server` (streamed, the default) or
/// `cpu` (on the client: `generator = "cpu"`).
pub const GENERATOR: &str = "generator";

/// The post's components: the tone mappers (`crate::post::Tone`), `clamp`
/// (no tone mapping) the default, and `hdr` (no limit: a float output for HDR displays).
/// The sites form writes another as `post = "aces"`.
pub const POSTS: &[&str] = &["clamp", "reinhard", "aces", "hdr"];

/// The sites form's key for the post.
pub const POST: &str = "post";


/// The components that can fill `role` (the world has none to choose; "none" means the
/// optional input it would feed is left unbound).
pub fn role_components(role: Role) -> Vec<&'static str> {
    match role.slot() {
        None if role == Role::Client => CLIENTS.to_vec(),
        None if role == Role::Link => LINKS.to_vec(),
        None if role == Role::Generator => GENERATORS.to_vec(),
        None if role == Role::Post => POSTS.to_vec(),
        None => vec![],
        Some(s) => crate::components(s).map(|c| c.name).filter(|n| *n != "none").collect(),
    }
}

/// Whether `role` component `name` exists.
pub(super) fn known(role: Role, name: &str) -> bool {
    match role.slot() {
        None if role == Role::Client => CLIENTS.contains(&name),
        None if role == Role::Link => LINKS.contains(&name),
        None if role == Role::Generator => GENERATORS.contains(&name),
        None if role == Role::Post => POSTS.contains(&name),
        None => name == "world",
        Some(s) => component(s, name).is_some(),
    }
}

/// Where a node runs unless the graph says otherwise: fused nodes run with their callers;
/// the rest where the back end runs that role (its [`crate::Place`]; GPU slot 0), or for a
/// stage it has not (a free site's geometry) where the component runs, if on one kind of
/// device only.
pub(super) fn default_device(executor: &str, role: Role, component: &str) -> Device {
    match role {
        Role::World | Role::Generator => return Device::Server,
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

    pub(super) fn with(mut self, inputs: &[(&str, &str)]) -> Self {
        self.inputs.extend(inputs.iter().map(|&(p, n)| (p.to_string(), n.to_string())));
        self
    }

    /// When input `port`'s value arrives.
    pub fn when(&self, port: &str) -> When {
        self.timing.get(port).copied().unwrap_or(When::Now)
    }
}

/// A pipeline: nodes by id, run by `executor`: the preset its shape and devices name
/// ([`Graph::preset`]), so a graph may leave it out or name another (its
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
