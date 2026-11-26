//! The graph model: roles, port data, devices, ports, timings, nodes, graphs.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Api, Kind, component};

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
    /// A gate between two domains on an edge: what crosses it is serialized, the bytes cross
    /// a modelled network (`mr_core::gate::GateSpec` as its knobs: latency, jitter,
    /// bandwidth, loss, reordering, resends), and are deserialized on the far side. It reads
    /// one node (port `in`) and gives what that node gives; any number of gates per graph,
    /// on any edge but the fused ones. The checked graph keeps them apart from its nodes
    /// ([`Checked::gates`]), so every executor builds the graph without them.
    Gate,
}

impl Role {
    pub const ALL: [Role; 15] = [
        Role::World,
        Role::Generator,
        Role::Link,
        Role::Client,
        Role::Storage,
        Role::Geometry,
        Role::Prepass,
        Role::Traversal,
        Role::Visibility,
        Role::Holes,
        Role::Draw,
        Role::Shading,
        Role::Meshes,
        Role::Post,
        Role::Gate,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Role::World => "world",
            Role::Generator => "generator",
            Role::Link => "link",
            Role::Client => "client",
            Role::Post => "post",
            Role::Storage => "storage",
            Role::Geometry => "geometry",
            Role::Prepass => "prepass",
            Role::Traversal => "traversal",
            Role::Visibility => "visibility",
            Role::Holes => "holes",
            Role::Draw => "draw",
            Role::Shading => "shading",
            Role::Meshes => "meshes",
            Role::Gate => "gate",
        }
    }

    /// Whether the registry ([`crate::COMPONENTS`]) holds this role's components (not the
    /// world, its generator, the link, the client and the post: their lists are here).
    pub fn registered(self) -> bool {
        !matches!(
            self,
            Role::World | Role::Generator | Role::Link | Role::Client | Role::Post | Role::Gate
        )
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
            _ => s
                .strip_prefix("gpu")
                .and_then(|i| i.parse().ok())
                .map(Device::Gpu)
                .ok_or_else(|| format!("unknown device `{s}` (cpu, gpu, gpu1, ..., server)")),
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
    Port {
        name,
        data,
        required,
        casts: matches!(data, Data::Rays),
        description,
    }
}

const CHANGES: &[Port] = &[port(
    "changes",
    Data::Changes,
    true,
    "the world's change log",
)];
const SENT: &[Port] = &[port(
    "changes",
    Data::Changes,
    true,
    "the server's stream: the world's change log, the same for every client",
)];
const STREAM: &[Port] = &[
    port(
        "changes",
        Data::Changes,
        true,
        "the server's stream as it arrives over the link",
    ),
    port(
        "chunks",
        Data::Chunks,
        false,
        "the generator, on the client: chunks generated here (the server then sends edits only)",
    ),
];
const GENERATED: &[Port] = &[port(
    "chunks",
    Data::Chunks,
    false,
    "the generator, on the server (unbound: the generator is on the client)",
)];
const ON_STORAGE: &[Port] = &[port("storage", Data::Storage, true, "the storage it reads")];
const ON_GEOMETRY: &[Port] = &[port(
    "geometry",
    Data::Faces,
    true,
    "the faces its acceleration structure is built from",
)];
const RAYCAST: &[Port] = &[
    port("primary", Data::Rays, true, "one primary ray per pixel"),
    port(
        "start",
        Data::Starts,
        false,
        "where rays may start (a prepass)",
    ),
    port(
        "meshes",
        Data::Surfaces,
        false,
        "meshes capping the rays (mesh mode)",
    ),
];
const DISCOVER: &[Port] = &[
    port("rays", Data::Rays, true, "discovery rays"),
    port(
        "exposure",
        Data::Storage,
        true,
        "the storage exposure tests read (which faces are exposed, their blocks)",
    ),
];
const QUADTREE: &[Port] = &[
    port("rays", Data::Rays, true, "discovery rays"),
    port(
        "exposure",
        Data::Storage,
        true,
        "the storage exposure tests read (which faces are exposed, their blocks)",
    ),
    port(
        "where",
        Data::Tiles,
        false,
        "the screen tiles to look at (hole detection; unbound: every tile, every frame)",
    ),
];
const COVERAGE: &[Port] = &[
    port(
        "covered",
        Data::Hits,
        true,
        "what the drawer covers drawing last frame's faces at this camera (in the loop: the drawer's faces iterate twice)",
    ),
    port(
        "sky",
        Data::Hits,
        true,
        "the previous frame's final image (timing `prev`): sky told apart from holes",
    ),
    port(
        "changes",
        Data::Changes,
        true,
        "the world's change log: edited blocks and changed chunks mark their tiles",
    ),
];
const MESHES: &[Port] = &[
    port("changes", Data::Changes, true, "the world's change log"),
    port(
        "shadow",
        Data::Rays,
        false,
        "shadow rays onto meshes (unbound: the shading's)",
    ),
];
const FACES: &[Port] = &[
    port("faces", Data::Faces, true, "the faces to draw"),
    port(
        "storage",
        Data::Storage,
        false,
        "the storage quads merged across block types look their blocks up in, per pixel (unbound: no lookup, so no such merging)",
    ),
];
const SPLAT: &[Port] = &[port("points", Data::Faces, true, "the points to splat")];
const DRAW: &[Port] = &[
    port("faces", Data::Faces, true, "the faces to draw"),
    port(
        "meshes",
        Data::Surfaces,
        false,
        "meshes in the same depth buffer (mesh mode)",
    ),
];
const DIRECT: &[Port] = &[
    port("hits", Data::Hits, true, "the visible surface per pixel"),
    port(
        "shadow",
        Data::Rays,
        false,
        "shadow rays (unbound: the region's own)",
    ),
];
const POSTED: &[Port] = &[port(
    "image",
    Data::Image,
    true,
    "the shading's linear colour (compiled into its output)",
)];
const GATED: &[Port] = &[port(
    "in",
    Data::Image,
    true,
    "what crosses the gate (any data but storages and ray queries; the type is the source's)",
)];
const PATHTRACE: &[Port] = &[
    port("hits", Data::Hits, true, "the visible surface per pixel"),
    port(
        "shadow",
        Data::Rays,
        false,
        "shadow rays (unbound: the region's own)",
    ),
    port(
        "bounce",
        Data::Rays,
        false,
        "bounce rays (unbound: the region's own)",
    ),
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
        (Role::Traversal, _) if crate::caps::needs(role, name, Role::Geometry, crate::Cap::Accel) => ON_GEOMETRY,
        (Role::Traversal, _) => ON_STORAGE,
        (Role::Visibility, "raycast") => RAYCAST,
        (Role::Visibility, "quadtree") => QUADTREE,
        (Role::Visibility, "cluster") => DISCOVER,
        (Role::Visibility, "splat") => SPLAT,
        (Role::Visibility, "columns") => ON_STORAGE,
        (Role::Visibility, _) => FACES,
        (Role::Holes, _) => COVERAGE,
        (Role::Draw, _) => DRAW,
        (Role::Shading, _) if crate::caps::declares(role, name, crate::Cap::Paths) => PATHTRACE,
        (Role::Shading, _) => DIRECT,
        (Role::Post, _) => POSTED,
        (Role::Gate, _) => GATED,
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
        // A gate gives what it reads; a checked graph has no gates left ([`Checked::gates`]).
        (Role::Gate, _) => Data::Image,
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
        (Role::Gate, _) => "the messages in flight and the line's queue",
        (Role::Client, _) => "a replica of what arrived and what its renderer holds",
        (Role::Storage, _) => "the representation, patched from the change log",
        (Role::Geometry, _) => "the faces, remeshed per changed chunk",
        (Role::Meshes, _) => "the mesh instances",
        (Role::Traversal, _) if crate::caps::needs(role, name, Role::Geometry, crate::Cap::Accel) => {
            "acceleration structures built from its geometry"
        }
        (Role::Visibility, "quadtree" | "cluster") => {
            "the discovered face set (faces kept `keep_frames` passes)"
        }
        (Role::Shading, _) if crate::caps::declares(role, name, crate::Cap::Paths) => {
            "accumulated radiance (restarted when the view changes)"
        }
        (Role::Holes, _) => {
            "per-tile reveal radii and the sky mask; when each face last covered a pixel"
        }
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

impl mr_core::Choices for When {
    const CHOICES: &'static [mr_core::Choice<Self>] = &[
        mr_core::Choice { name: "now", make: || When::Now, about: "This frame's value" },
        mr_core::Choice { name: "prev", make: || When::Prev, about: "The previous frame's value (closes a cycle; across devices: overlaps the two)" },
        mr_core::Choice { name: "iterate", make: || When::Iterate(2), about: "Several passes a frame (here two), the first on last frame's value" },
    ];
    fn choice(&self) -> usize {
        match self {
            When::Now => 0,
            When::Prev => 1,
            When::Iterate(_) => 2,
        }
    }
}

/// The component that stands for `role` where a graph has no node of it (the normaliser adds
/// it): its identity, a node that changes nothing. Set to nothing else (no knob off its
/// default, timing, API, device but where it runs unset, input but its first port), such a
/// node is left out of the key and the label, its readers written as reading its input (or,
/// with none, as not reading it): a graph says the same with it as without it.
pub fn identity(role: Role) -> Option<&'static str> {
    match role {
        Role::Generator => Some("procedural"),
        Role::Link => Some("free"),
        Role::Client => Some("pick"),
        Role::Post => Some("clamp"),
        _ => None,
    }
}

/// A component of a role outside the registry (world, generator, link, client, post): its
/// name as graphs spell it and what it does, as the editor shows it. Registry components
/// carry theirs (`ComponentInfo::description`).
pub struct HostComponent {
    pub role: Role,
    pub name: &'static str,
    pub about: &'static str,
}

/// Every host role's components, in the order offered; the one list their names, checks and
/// descriptions come from. The client's are `mr_core::client::Levels`; the link's `net` has
/// `mr_core::link::LinkSpec`'s fields as its knobs; the generator's `procedural` is the
/// world's generator built from its spec (the run's world: terrain, menger, ...); the post's
/// are the tone mappers (`crate::post::Tone`), `clamp` (no tone mapping) the default.
pub const HOST_COMPONENTS: &[HostComponent] = &[
    HostComponent { role: Role::World, name: "world", about: "The authoritative world on the server: chunks and their change log" },
    HostComponent { role: Role::Generator, name: "procedural", about: "Generates the world's chunks from its spec (a shared seed): on the server, or on the client's cpu" },
    HostComponent { role: Role::Link, name: "free", about: "Everything the server sends arrives at once, in order" },
    HostComponent { role: Role::Link, name: "net", about: "A modelled network: latency, jitter, bandwidth, loss, reordering" },
    HostComponent { role: Role::Client, name: "pick", about: "Holds what arrived: the server's LOD levels, picked by its radius and levels" },
    HostComponent { role: Role::Client, name: "derive", about: "Holds what arrived and derives its coarser LOD levels from LOD 0 itself" },
    HostComponent { role: Role::Post, name: "clamp", about: "Linear colour clamped to the output range" },
    HostComponent { role: Role::Post, name: "reinhard", about: "Reinhard tone mapping of the linear colour, compiled into the shading's output" },
    HostComponent { role: Role::Post, name: "aces", about: "ACES filmic tone mapping of the linear colour, compiled into the shading's output" },
    HostComponent { role: Role::Post, name: "hdr", about: "No limit: a float output for HDR displays" },
    HostComponent { role: Role::Gate, name: "net", about: "A gap between two domains on an edge: what crosses is serialized, crosses a modelled network (latency, jitter, bandwidth, loss on the bytes, reordering, resends) and is deserialized" },
];

/// Whether `role` is a host role (its components in [`HOST_COMPONENTS`], not the registry).
pub fn is_host_role(role: Role) -> bool {
    matches!(role, Role::World | Role::Generator | Role::Link | Role::Client | Role::Post | Role::Gate)
}

/// The names of host role `role`'s components, in the order offered.
pub fn host_components(role: Role) -> Vec<&'static str> {
    HOST_COMPONENTS.iter().filter(|c| c.role == role).map(|c| c.name).collect()
}

/// The id of the client node a graph gets when one is added to it (by the host, setting a
/// client knob).
pub const CLIENT: &str = "client";

/// The client's knobs: its region, the chunks around the camera and the
/// LOD levels beyond (capped by what the renderer supports); unset, the run's
/// `view_radius` and `max_lod`. The workload, not the pipeline: left out of the key.
pub const CLIENT_KNOBS: &[&str] = &["radius", "max_lod"];

/// What a host role's component does, as the editor shows it; `None` for a name the role
/// does not have.
pub fn host_description(role: Role, name: &str) -> Option<&'static str> {
    HOST_COMPONENTS.iter().find(|c| c.role == role && c.name == name).map(|c| c.about)
}

/// The components that can fill `role` (the world has none to choose; "none" means the
/// optional input it would feed is left unbound).
pub fn role_components(role: Role) -> Vec<&'static str> {
    match role {
        Role::World => Vec::new(),
        r if is_host_role(r) => host_components(r),
        r => crate::components(r)
            .map(|c| c.name)
            .filter(|n| *n != "none")
            .collect(),
    }
}

/// Whether `role` component `name` exists.
pub(super) fn known(role: Role, name: &str) -> bool {
    match role {
        r if is_host_role(r) => host_description(r, name).is_some(),
        r => component(r, name).is_some(),
    }
}

/// Where a node runs unless the graph says otherwise (`executor`: the graph's, `cpu` or
/// `gpu`, none = `gpu`): the world and its generator on the server, the link and the client on
/// the CPU; a component on the executor's device if it has an implementation there (GPU slot
/// 0), else on the one kind of device it has. Fused nodes follow their callers ([`Graph::check`]).
pub(super) fn default_device(executor: &str, role: Role, component: &str) -> Device {
    match role {
        Role::World | Role::Generator => return Device::Server,
        Role::Link | Role::Client | Role::Gate => return Device::Cpu,
        _ => {}
    }
    let want = if executor == "cpu" {
        Kind::Cpu
    } else {
        Kind::Gpu
    };
    let on = |k: Kind| {
        if k == Kind::Cpu {
            Device::Cpu
        } else {
            Device::Gpu(0)
        }
    };
    match crate::component(role, component) {
        Some(c) if !c.impls.iter().any(|i| i.kind == want) => {
            c.impls.first().map_or(on(want), |i| on(i.kind))
        }
        _ => on(want),
    }
}

/// One component instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub role: Role,
    pub component: String,
    /// Where it runs (default: with its callers for storages and kernels, else where the
    /// preset runs the role).
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
        Self {
            role,
            component: component.into(),
            device: None,
            api: None,
            inputs: BTreeMap::new(),
            timing: BTreeMap::new(),
            knobs: toml::Table::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn with(mut self, inputs: &[(&str, &str)]) -> Self {
        self.inputs
            .extend(inputs.iter().map(|&(p, n)| (p.to_string(), n.to_string())));
        self
    }

    /// When input `port`'s value arrives.
    pub fn when(&self, port: &str) -> When {
        self.timing.get(port).copied().unwrap_or(When::Now)
    }
}

/// A pipeline: nodes by id, run by the executor of a device kind (`cpu`, `gpu`; none = `gpu`),
/// where its nodes without a device run if they can ([`default_device`]). Which of the
/// executor's regions builds it is the graph's shape ([`Graph::preset`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    #[serde(default)]
    pub executor: String,
    pub nodes: BTreeMap<String, Node>,
}

impl Graph {
    /// The executor that runs it: `cpu` or `gpu` (also when none is written).
    pub fn runs_on(&self) -> &str {
        if self.executor.is_empty() {
            "gpu"
        } else {
            &self.executor
        }
    }
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
    /// The region its shape names ([`Graph::region`]), which builds it.
    pub region: Option<&'static super::Builds>,
    /// The gates on its edges, taken out of `graph` (whose readers read what the gates read).
    pub gates: Vec<super::EdgeGates>,
}
