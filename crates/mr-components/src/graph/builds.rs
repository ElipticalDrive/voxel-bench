//! What a node executor builds: per role, the components it has code for, the device kind it
//! runs them on and how many such nodes it takes. `Graph::accept` takes a valid graph
//! (`Graph::check`) on a node executor exactly when it has no other node, else refuses it with
//! the reason; it is never built half-way.

use super::{Checked, Graph, Node, Role, inputs, output, role_components};
use crate::{Api, Kind};

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

/// `cpu` ray casting: one storage (one monomorphised instance; the shadow rays walk it too),
/// its primary and shadow kernels, the ray caster and the shading.
pub const CPU_RAYCAST: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["bricks", "u8", "bits", "hier", "prefix", "palette", "brickmajor", "morton"], Kind::Cpu),
        Build::one(Role::Traversal, &["refloor", "incremental"], Kind::Cpu).count(1, 2),
        Build::one(Role::Visibility, &["raycast"], Kind::Cpu),
        Build::one(Role::Shading, &["direct"], Kind::Cpu),
    ],
    same: &[],
};

/// `cpu-raster`: the geometry's quads, their culling, the rasteriser and the shading.
pub const CPU_RASTER: Builds = Builds {
    nodes: &[
        Build::one(Role::Geometry, &["greedy", "naive"], Kind::Cpu),
        Build::one(Role::Visibility, &["faces"], Kind::Cpu),
        Build::one(Role::Draw, &["raster"], Kind::Cpu),
        Build::one(Role::Shading, &["direct"], Kind::Cpu),
    ],
    same: &[],
};

/// `cpu-voxelspace`: the heightmap, the column march and the shading.
pub const CPU_COLUMNS: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["heightmap"], Kind::Cpu),
        Build::one(Role::Visibility, &["columns"], Kind::Cpu),
        Build::one(Role::Shading, &["direct"], Kind::Cpu),
    ],
    same: &[],
};

/// The GPU walk (`gpu-compute`): a storage and kernel per walk (primary, shadow, bounce and
/// mesh-shadow rays), a ray-query kernel's geometry (built on the CPU), the mesh records, the
/// beam prepass (over the primary rays' storage, which its shader reads), the walk and the
/// shading (on this GPU or another).
pub const GPU_WALK: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["bricks", "hier", "prefix", "distance", "rows", "tree64", "svo_dag", "clipmap"], Kind::Gpu).count(0, 4),
        Build::one(Role::Traversal, &["refloor", "incremental", "sphere", "rowrun", "rayquery"], Kind::Gpu).count(1, 4),
        Build::one(Role::Geometry, &["greedy", "naive", "boxes"], Kind::Cpu).count(0, 1),
        Build::one(Role::Meshes, &["composite"], Kind::Gpu).count(0, 1),
        Build::one(Role::Prepass, &["beam"], Kind::Gpu).count(0, 1),
        Build::one(Role::Visibility, &["raycast"], Kind::Gpu),
        Build::one(Role::Shading, &["direct", "pathtrace"], Kind::Gpu),
    ],
    same: &[Same { a: (Role::Prepass, &["storage"]), b: (Role::Visibility, &["primary", "storage"]) }],
};

/// GPU ray queries (`gpu-rt`): the geometry of the acceleration structures (on the CPU), the
/// primary ray queries, storages and kernels for walked shadow and bounce rays, the visibility
/// and the shading (on this GPU or another). No meshes and no prepass.
pub const GPU_RAYQUERY: Builds = Builds {
    nodes: &[
        Build::one(Role::Geometry, &["greedy", "naive", "boxes"], Kind::Cpu),
        Build::one(Role::Storage, &["bricks", "hier", "prefix", "distance", "rows", "tree64", "svo_dag", "clipmap"], Kind::Gpu).count(0, 2),
        Build::one(Role::Traversal, &["refloor", "incremental", "sphere", "rowrun", "rayquery"], Kind::Gpu).count(1, 3),
        Build::one(Role::Visibility, &["raycast"], Kind::Gpu),
        Build::one(Role::Shading, &["direct", "pathtrace"], Kind::Gpu),
    ],
    same: &[],
};

/// The roles the worker runs in front of every executor (the world's side), and the post every
/// executor compiles into its shading.
const AROUND: &[Role] = &[Role::World, Role::Generator, Role::Link, Role::Client, Role::Post];

impl Checked {
    /// The node executor that builds this graph and what it declares (`None`: a back end with
    /// its own frame loop). GPU ray casting runs on the ray-query caster when the primary rays'
    /// kernel is a ray query, else on the walk.
    pub fn builder(&self) -> Option<(&'static str, Builds)> {
        // The primary rays' kernel, whichever visibility node casts them.
        let primary = self.graph.nodes.values().filter(|n| n.role == Role::Visibility).find_map(|n| n.inputs.get("primary"));
        Some(match self.graph.executor.as_str() {
            "cpu" => ("cpu", CPU_RAYCAST),
            "cpu-raster" => ("cpu-raster", CPU_RASTER),
            "cpu-voxelspace" => ("cpu-voxelspace", CPU_COLUMNS),
            "gpu-compute" | "gpu-rt" if primary.and_then(|k| self.api(k)) == Some(Api::RayQuery) => ("gpu-rt", GPU_RAYQUERY),
            "gpu-compute" | "gpu-rt" => ("gpu-compute", GPU_WALK),
            _ => return None,
        })
    }

    /// Whether the node executor running this graph builds it (see [`Self::built_by`]).
    pub fn built(&self) -> Result<(), String> {
        self.builder().map_or(Ok(()), |(e, b)| self.built_by(e, b))
    }

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
