//! What a node executor builds: per role, the components it has code for, the device kind it
//! runs them on and how many such nodes it takes. `Graph::accept` takes a valid graph
//! (`Graph::check`) on a node executor exactly when it has no other node, else refuses it with
//! the reason; it is never built half-way.

use super::{Checked, Graph, Node, Role, When, inputs, output, role_components};
use crate::impls::{self, ApiChoice, Place};
use crate::{Api, Kind};

/// Nodes of one role an executor builds.
#[derive(Copy, Clone, Debug)]
pub struct Build {
    pub role: Role,
    pub components: &'static [&'static str],
    /// The device kind it runs them on and the APIs it may run them with there.
    pub at: Place,
    /// How many nodes of the role it takes.
    pub least: usize,
    pub most: usize,
}

impl Build {
    /// One node of `role`, one of `components`, run `at`.
    pub const fn one(role: Role, components: &'static [&'static str], at: Place) -> Build {
        Build {
            role,
            components,
            at,
            least: 1,
            most: 1,
        }
    }

    /// Between `least` and `most` nodes.
    pub const fn count(self, least: usize, most: usize) -> Build {
        Build {
            least,
            most,
            ..self
        }
    }
}

/// Inputs an executor reads as one node: from the node of each role, the node reached by
/// following its input ports (e.g. the prepass's storage and the primary kernel's).
#[derive(Copy, Clone, Debug)]
pub struct Same {
    pub a: (Role, &'static [&'static str]),
    pub b: (Role, &'static [&'static str]),
}

/// What an executor reads from an input: on every node of `role` (of one of `of`, or any), the
/// node bound to `port` is one of `components`; `bound`: it must be bound (the executor has no
/// default for it).
#[derive(Copy, Clone, Debug)]
pub struct Only {
    pub role: Role,
    pub of: &'static [&'static str],
    pub port: &'static str,
    pub components: &'static [&'static str],
    pub bound: bool,
}

/// A node a region replaces while it runs: the node of `role` (the one the `at` node's input
/// reads, where the region builds several of the role), its component changed, or the node
/// added or removed. `with`: edge timings that change along with it.
#[derive(Copy, Clone, Debug)]
pub struct Live {
    pub role: Role,
    pub at: Option<(Role, &'static str)>,
    pub with: &'static [(Role, &'static str)],
}

impl Live {
    /// Any node of `role`.
    pub const fn of(role: Role) -> Live {
        Live {
            role,
            at: None,
            with: &[],
        }
    }

    /// The node of the role that input `port` of the `reader` node reads.
    pub const fn read_by(self, reader: Role, port: &'static str) -> Live {
        Live {
            at: Some((reader, port)),
            ..self
        }
    }

    /// The timings of these edges (a role's input) change with it.
    pub const fn with(self, with: &'static [(Role, &'static str)]) -> Live {
        Live { with, ..self }
    }
}

/// The regions of the executors: the parts of each that build one shape of pipeline (the CPU
/// executor's ray caster, rasteriser and column march; the GPU executor's walk, ray queries,
/// meshed faces, the two discovery samplers, splats and column march). An executor picks its
/// code by it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionId {
    CpuRaycast,
    CpuRaster,
    CpuColumns,
    GpuWalk,
    GpuRayQuery,
    GpuMeshing,
    GpuSampler,
    CpuSampler,
    GpuSplat,
    GpuColumns,
}

/// What a region of an executor builds: its nodes, the inputs that must be one node, what it
/// reads from some inputs, and the choices it leaves to the graph (edge timings, a second GPU,
/// an API).
#[derive(Copy, Clone, Debug)]
pub struct Builds {
    pub id: RegionId,
    pub nodes: &'static [Build],
    pub same: &'static [Same],
    pub only: &'static [Only],
    /// Edge timings it lets a graph choose (the edge's role and port, the timings it runs);
    /// every other edge runs now.
    pub free: &'static [(Role, &'static str, &'static [When])],
    /// Knobs that spell an edge timing (the graph says it; the knob is how plans, the CLI and
    /// the GUI write it): the knob, the edge's role and port, the timing when the knob is
    /// true, when false.
    pub timing_knobs: &'static [(&'static str, Role, &'static str, When, When)],
    /// Whether it runs over two GPUs, split at the hit records (the shading on the second).
    pub split: bool,
    /// The choice of API it offers, if any (the CPU ray caster: `isa`, its one hot loop; the
    /// GPU walk: `dispatch`, in a compute or fragment shader).
    pub api_choice: Option<ApiChoice>,
    /// The nodes it replaces while running ([`Graph::live_edits`]); any other change to the
    /// graph's shape is a new renderer.
    pub live: &'static [Live],
}

impl Builds {
    /// Region `id` with nothing built and no choices: the base the declarations fill in.
    const fn plain(id: RegionId) -> Builds {
        Builds {
            id,
            nodes: &[],
            same: &[],
            only: &[],
            free: &[],
            timing_knobs: &[],
            split: false,
            api_choice: None,
            live: &[],
        }
    }

    /// The timings it runs the edge into `port` of a `role` node with, if the graph may choose.
    pub fn free_at(&self, role: Role, port: &str) -> Option<&'static [When]> {
        self.free
            .iter()
            .find(|(r, p, _)| *r == role && *p == port)
            .map(|f| f.2)
    }

    /// Whether it replaces node `id` of graph `g` while running.
    pub fn replaces(&self, g: &Graph, id: &str) -> bool {
        let Some(n) = g.nodes.get(id) else {
            return false;
        };
        self.live.iter().any(|l| {
            l.role == n.role
                && l.at.is_none_or(|(reader, port)| {
                    g.nodes
                        .values()
                        .any(|r| r.role == reader && r.inputs.get(port).is_some_and(|s| s == id))
                })
        })
    }

    /// Whether its API choice sets the nodes of `role`.
    pub fn chooses(&self, role: Role) -> bool {
        self.api_choice.is_some_and(|ch| ch.roles.contains(&role))
    }

    /// The components it declares for `role`, on any device (a graph it builds may need other
    /// nodes changed with one of them: what a node may be is [`Graph::choices`]).
    pub fn declared(&self, role: Role) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = vec![];
        for c in self
            .nodes
            .iter()
            .filter(|b| b.role == role)
            .flat_map(|b| b.components)
        {
            if !v.contains(c) {
                v.push(c);
            }
        }
        v
    }

    /// Lighting knobs belong to the shading node (lit mode is the shading component's job):
    /// marks `shadows` as owned by the shadings it declares, so UIs show it there.
    pub fn lighting_knobs(&self, mut params: Vec<mr_core::ParamSpec>) -> Vec<mr_core::ParamSpec> {
        let shading = self.declared(Role::Shading);
        for p in params
            .iter_mut()
            .filter(|p| p.name == "shadows" && p.owner.is_none())
        {
            *p = p.clone().of(Role::Shading.name(), &shading);
        }
        params
    }

    /// The shading node's placement knob, for the regions that offer it: `inline` shades where
    /// the strategy finds the hit, in the same pass; `deferred` has the strategy write hit
    /// records and one common pass shade each pixel. Fixed at creation: inline pipelines carry
    /// nothing of the deferred one (no record buffer, binding or pass). `default`: the
    /// region's original placement (the CPU rasteriser was deferred).
    pub fn placement_param(&self, default: &str) -> mr_core::ParamSpec {
        mr_core::ParamSpec::choice(
            "placement",
            "Shading placement: inline = shaded where the surface is found, in the same pass (with rasterisers: every fragment that passes the depth test); deferred = the strategy writes a hit record per pixel, then one common pass shades each pixel",
            &["inline", "deferred"],
            default,
            false,
        )
        .of(Role::Shading.name(), &self.declared(Role::Shading))
        .baked(false)
    }

    /// The executor (device kind: `cpu`, `gpu`) it is a region of: where its image is made,
    /// its shading's device.
    pub fn executor(&self) -> &'static str {
        match self
            .nodes
            .iter()
            .find(|b| b.role == Role::Shading)
            .map(|b| b.at.kind)
        {
            Some(Kind::Cpu) => "cpu",
            _ => "gpu",
        }
    }

    /// How messages name it: by its executor, in the shape of the graph at hand (a region is
    /// the code an executor has for one shape of graph).
    pub fn who(&self) -> String {
        format!("the {} executor (for this shape)", self.executor())
    }
}

/// Every region, in the order listings and sweeps go through them.
pub const REGIONS: &[&Builds] = &[
    &GPU_WALK,
    &CPU_RAYCAST,
    &GPU_MESHING,
    &GPU_RAYQUERY,
    &GPU_SAMPLER,
    &CPU_SAMPLER,
    &GPU_SPLAT,
    &GPU_COLUMNS,
    &CPU_COLUMNS,
    &CPU_RASTER,
];

/// The stages one API choice sets, and hit records shaded now or a frame late on a second GPU.
const CAST_ROLES: &[Role] = &[
    Role::Storage,
    Role::Visibility,
    Role::Traversal,
    Role::Shading,
];
const HITS_LATE: &[(Role, &str, &[When])] = &[(Role::Shading, "hits", &[When::Now, When::Prev])];

/// The GPU storages and walking kernels, the geometries ray queries trace, the face drawers.
const GPU_STORAGES: &[&str] = &[
    "bricks", "hier", "prefix", "distance", "rows", "tree64", "svo_dag", "clipmap",
];
const GPU_WALKS: &[&str] = &["refloor", "incremental", "sphere", "rowrun"];
const GPU_KERNELS: &[&str] = &["refloor", "incremental", "sphere", "rowrun", "rayquery"];
const RT_GEOMETRY: &[&str] = &["greedy", "naive", "boxes"];
const DRAWERS: &[&str] = &["raster", "visbuffer", "gbuffer"];
const CPU_STORAGES: &[&str] = &[
    "bricks",
    "u8",
    "bits",
    "hier",
    "prefix",
    "palette",
    "brickmajor",
    "morton",
];
const CPU_WALKS: &[&str] = &["refloor", "incremental"];

/// Ray queries trace an acceleration structure built from one of `RT_GEOMETRY`.
const RT: Only = Only {
    role: Role::Traversal,
    of: &["rayquery"],
    port: "geometry",
    components: RT_GEOMETRY,
    bound: false,
};
/// The shading's shadow rays, walked (`bound`: the executor casts no rays of its own there).
const SHADOW_WALKED: Only = Only {
    role: Role::Shading,
    of: &[],
    port: "shadow",
    components: GPU_WALKS,
    bound: true,
};
const SHADOW_BOUND: Only = Only {
    role: Role::Shading,
    of: &[],
    port: "shadow",
    components: GPU_KERNELS,
    bound: true,
};

/// `raycast-cpu`: one storage (one monomorphised instance; the shadow rays walk it too),
/// its primary and shadow kernels, the ray caster and the shading.
pub const CPU_RAYCAST: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, CPU_STORAGES, impls::CPU_ISAS),
        Build::one(Role::Traversal, CPU_WALKS, impls::CPU_ISAS).count(1, 2),
        Build::one(Role::Visibility, &["raycast"], impls::CPU_ISAS),
        Build::one(Role::Shading, &["direct"], impls::CPU_ISAS),
    ],
    same: &[],
    only: &[],
    api_choice: Some(ApiChoice {
        knob: "isa",
        roles: CAST_ROLES,
        values: &[
            (Api::Scalar, "scalar"),
            (Api::Avx2, "avx2"),
            (Api::Avx512, "avx512"),
        ],
        default: None,
    }),
    ..Builds::plain(RegionId::CpuRaycast)
};

/// `raster-cpu`: the geometry's quads, their culling, the rasteriser and the shading, its
/// shadow rays through a storage and kernel of the ray caster's.
pub const CPU_RASTER: Builds = Builds {
    nodes: &[
        Build::one(Role::Geometry, &["greedy", "naive"], impls::CPU_SCALAR),
        Build::one(Role::Visibility, &["faces"], impls::CPU_SCALAR),
        Build::one(Role::Draw, &["raster"], impls::CPU_SCALAR),
        Build::one(Role::Storage, CPU_STORAGES, impls::CPU_SCALAR),
        Build::one(Role::Traversal, CPU_WALKS, impls::CPU_SCALAR),
        Build::one(Role::Shading, &["direct"], impls::CPU_SCALAR),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Shading,
            of: &[],
            port: "shadow",
            components: CPU_WALKS,
            bound: true,
        },
        // Its geometry merges only faces of one block: no lookup.
        Only {
            role: Role::Visibility,
            of: &[],
            port: "storage",
            components: &[],
            bound: false,
        },
    ],
    live: &[Live::of(Role::Geometry)],
    ..Builds::plain(RegionId::CpuRaster)
};

/// `columns-cpu`: the heightmap, its own ray march (the shadow rays), the column march and the shading.
pub const CPU_COLUMNS: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["heightmap"], impls::CPU_SCALAR),
        Build::one(Role::Traversal, &["heightfield"], impls::CPU_SCALAR),
        Build::one(Role::Visibility, &["columns"], impls::CPU_SCALAR),
        Build::one(Role::Shading, &["direct"], impls::CPU_SCALAR),
    ],
    same: &[],
    only: &[],
    ..Builds::plain(RegionId::CpuColumns)
};

/// The GPU walk (`raycast-gpu`): a storage and kernel per walk (primary, shadow, bounce and
/// mesh-shadow rays), a ray-query kernel's geometry (built on the CPU), the mesh records, the
/// beam prepass (over the primary rays' storage, which its shader reads), the walk and the
/// shading (on this GPU or another).
pub const GPU_WALK: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, GPU_STORAGES, impls::GPU_DISPATCH).count(0, 4),
        Build::one(Role::Traversal, GPU_KERNELS, impls::GPU_DISPATCH).count(1, 4),
        Build::one(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR).count(0, 1),
        Build::one(Role::Meshes, &["composite"], impls::GPU_RASTER).count(0, 1),
        Build::one(Role::Prepass, &["beam"], impls::GPU_COMPUTE).count(0, 1),
        Build::one(Role::Visibility, &["raycast"], impls::GPU_DISPATCH),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_DISPATCH),
    ],
    same: &[Same {
        a: (Role::Prepass, &["storage"]),
        b: (Role::Visibility, &["primary", "storage"]),
    }],
    only: &[],
    free: HITS_LATE,
    split: true,
    api_choice: Some(ApiChoice {
        knob: "dispatch",
        roles: CAST_ROLES,
        values: &[(Api::Compute, "compute"), (Api::Raster, "fragment")],
        default: Some(Api::Compute),
    }),
    live: &[
        Live::of(Role::Traversal).read_by(Role::Visibility, "primary"),
        Live::of(Role::Shading),
        Live::of(Role::Prepass),
        Live::of(Role::Meshes),
    ],
    ..Builds::plain(RegionId::GpuWalk)
};

/// GPU ray queries (`rayquery-gpu`): the geometry of the acceleration structures (on the CPU), the
/// primary ray queries, storages and kernels for walked shadow and bounce rays, the visibility
/// and the shading (on this GPU or another). No meshes and no prepass.
pub const GPU_RAYQUERY: Builds = Builds {
    nodes: &[
        Build::one(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR),
        Build::one(Role::Storage, GPU_STORAGES, impls::GPU_RAYQUERY).count(0, 2),
        Build::one(Role::Traversal, GPU_KERNELS, impls::GPU_RAYQUERY).count(1, 3),
        Build::one(Role::Visibility, &["raycast"], impls::GPU_RAYQUERY),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_RAYQUERY),
    ],
    same: &[],
    only: &[],
    free: HITS_LATE,
    split: true,
    live: &[Live::of(Role::Shading)],
    ..Builds::plain(RegionId::GpuRayQuery)
};

/// Faces with the meshing source (`raster-gpu`): the geometry the drawer draws (and one for
/// ray-queried bounce rays), its culling, the drawer, meshes into the depth buffer lit through
/// walks, and the shading, whose shadow rays (and the meshes') are walks.
pub const GPU_MESHING: Builds = Builds {
    nodes: &[
        Build::one(
            Role::Geometry,
            &["greedy", "naive", "cubes", "boxes"],
            impls::CPU_SCALAR,
        )
        .count(1, 2),
        Build::one(Role::Visibility, &["faces"], impls::GPU_RASTER),
        Build::one(Role::Draw, DRAWERS, impls::GPU_RASTER),
        Build::one(Role::Meshes, &["depth"], impls::GPU_RASTER).count(0, 1),
        Build::one(Role::Storage, GPU_STORAGES, impls::GPU_DRAWN).count(1, 3),
        Build::one(Role::Traversal, GPU_KERNELS, impls::GPU_DRAWN).count(1, 3),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_DRAWN),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Visibility,
            of: &[],
            port: "faces",
            components: &["greedy", "naive", "cubes"],
            bound: true,
        },
        Only {
            role: Role::Draw,
            of: &[],
            port: "faces",
            components: &["faces"],
            bound: true,
        },
        Only {
            role: Role::Meshes,
            of: &[],
            port: "shadow",
            components: GPU_WALKS,
            bound: false,
        },
        Only {
            role: Role::Visibility,
            of: &[],
            port: "storage",
            components: GPU_STORAGES,
            bound: false,
        },
        SHADOW_WALKED,
        RT,
    ],
    free: HITS_LATE,
    live: &[
        Live::of(Role::Geometry).read_by(Role::Visibility, "faces"),
        Live::of(Role::Draw),
        Live::of(Role::Shading),
        Live::of(Role::Meshes),
    ],
    split: true,
    ..Builds::plain(RegionId::GpuMeshing)
};

/// GPU discovery (`discover-gpu`): the sampler (its rays and exposure tests over any GPU
/// storage), its hole detection, the drawer of the face set, the shading (any rays, a
/// ray-query geometry on the CPU).
pub const GPU_SAMPLER: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, GPU_STORAGES, impls::GPU_DRAWN).count(1, 4),
        Build::one(Role::Traversal, GPU_KERNELS, impls::GPU_DRAWN).count(1, 3),
        Build::one(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR).count(0, 1),
        Build::one(Role::Visibility, &["quadtree"], impls::GPU_COMPUTE),
        Build::one(Role::Holes, &["coverage"], impls::GPU_COMPUTE).count(0, 1),
        Build::one(Role::Draw, DRAWERS, impls::GPU_RASTER),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_DRAWN),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Draw,
            of: &[],
            port: "faces",
            components: &["quadtree"],
            bound: true,
        },
        Only {
            role: Role::Visibility,
            of: &[],
            port: "rays",
            components: GPU_WALKS,
            bound: true,
        },
        SHADOW_BOUND,
        RT,
    ],
    // Hole detection closes a cycle through the drawer, which draws twice a frame, and reads
    // the last frame's sky mask.
    free: &[
        (Role::Draw, "faces", &[When::Now, When::Iterate(2)]),
        (Role::Holes, "sky", &[When::Prev]),
        HITS_LATE[0],
    ],
    live: &[
        Live::of(Role::Holes).with(&[(Role::Draw, "faces")]),
        Live::of(Role::Draw),
    ],
    split: true,
    ..Builds::plain(RegionId::GpuSampler)
};

/// CPU discovery (`discover-cpu`): the sampler over a CPU storage (exposure tests over the one
/// its rays walk), the face set drawn on the GPU, the shading there (any GPU rays, a ray-query
/// geometry on the CPU).
pub const CPU_SAMPLER: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, CPU_STORAGES, impls::CPU_SCALAR),
        Build::one(Role::Traversal, &["refloor"], impls::CPU_SCALAR),
        Build::one(
            Role::Visibility,
            &["quadtree", "cluster"],
            impls::CPU_SCALAR,
        ),
        Build::one(Role::Draw, DRAWERS, impls::GPU_RASTER),
        Build::one(Role::Storage, GPU_STORAGES, impls::GPU_DRAWN).count(0, 2),
        Build::one(Role::Traversal, GPU_KERNELS, impls::GPU_DRAWN).count(1, 2),
        Build::one(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR).count(0, 1),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_DRAWN),
    ],
    same: &[Same {
        a: (Role::Visibility, &["exposure"]),
        b: (Role::Visibility, &["rays", "storage"]),
    }],
    only: &[
        Only {
            role: Role::Draw,
            of: &[],
            port: "faces",
            components: &["quadtree", "cluster"],
            bound: true,
        },
        SHADOW_BOUND,
        RT,
    ],
    // Discovery a frame ahead of the drawer, or within its frame.
    free: &[(Role::Draw, "faces", &[When::Now, When::Prev]), HITS_LATE[0]],
    timing_knobs: &[("pipelined", Role::Draw, "faces", When::Prev, When::Now)],
    live: &[Live::of(Role::Visibility), Live::of(Role::Draw)],
    split: true,
    ..Builds::plain(RegionId::CpuSampler)
};

/// Point splatting (`splat-gpu`): the points (and a geometry for ray-queried bounce rays), the
/// splat passes, the shading (its shadow rays walked).
pub const GPU_SPLAT: Builds = Builds {
    nodes: &[
        Build::one(
            Role::Geometry,
            &["points", "greedy", "naive", "boxes"],
            impls::CPU_SCALAR,
        )
        .count(1, 2),
        Build::one(Role::Visibility, &["splat"], impls::GPU_COMPUTE),
        Build::one(Role::Storage, GPU_STORAGES, impls::GPU_COMPUTE).count(1, 2),
        Build::one(Role::Traversal, GPU_KERNELS, impls::GPU_COMPUTE).count(1, 2),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_COMPUTE),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Visibility,
            of: &[],
            port: "points",
            components: &["points"],
            bound: true,
        },
        SHADOW_WALKED,
        RT,
    ],
    ..Builds::plain(RegionId::GpuSplat)
};

/// Voxel Space on the GPU (`columns-gpu`): the heightmap, its own ray march (the shadow and
/// bounce rays), the column march, the shading.
pub const GPU_COLUMNS: Builds = Builds {
    nodes: &[
        Build::one(Role::Storage, &["heightmap"], impls::GPU_COMPUTE),
        Build::one(Role::Traversal, &["heightfield"], impls::GPU_COMPUTE),
        Build::one(Role::Visibility, &["columns"], impls::GPU_COMPUTE),
        Build::one(Role::Shading, &["direct", "pathtrace"], impls::GPU_COMPUTE),
    ],
    same: &[],
    only: &[],
    ..Builds::plain(RegionId::GpuColumns)
};

/// The roles the worker runs in front of every executor (the world's side), and the post every
/// executor compiles into its shading.
const AROUND: &[Role] = &[
    Role::World,
    Role::Generator,
    Role::Link,
    Role::Client,
    Role::Post,
];

impl Checked {
    /// The region of the graph's executor that builds this graph (the one the graph's shape
    /// picks: [`Graph::region`]) and what it declares; `None`: a shape no region has, or one
    /// another device's executor runs. GPU ray casting runs on the
    /// ray-query caster when the primary rays' kernel is a ray query, else on the walk.
    pub fn builder(&self) -> Option<Builds> {
        self.region
            .filter(|b| b.executor() == self.graph.runs_on())
            .copied()
    }

    /// Whether the node executor running this graph builds it (see [`Self::built_by`]).
    pub fn built(&self) -> Result<(), String> {
        let b = self.builder().ok_or_else(|| match self.region {
            Some(r) => format!("the {} executor builds no pipeline of this shape (the {} executor does)", self.graph.runs_on(), r.executor()),
            None => format!("no region of the {} executor builds this shape (an image from a shading over hits from a visibility node or a drawer)", self.graph.runs_on()),
        })?;
        self.built_by(b)
    }

    /// Whether region `builds` builds this graph; else the first node (or role) it cannot,
    /// with the reason.
    pub fn built_by(&self, builds: Builds) -> Result<(), String> {
        let executor = builds.who();
        let Builds {
            nodes: builds,
            same,
            only,
            ..
        } = builds;
        let list = |v: &[&str]| {
            v.iter()
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        for (id, n) in self
            .graph
            .nodes
            .iter()
            .filter(|(_, n)| !AROUND.contains(&n.role))
        {
            let of_role: Vec<&Build> = builds.iter().filter(|b| b.role == n.role).collect();
            if of_role.is_empty() {
                let mut roles: Vec<&str> = builds.iter().map(|b| b.role.name()).collect();
                roles.dedup();
                return Err(format!(
                    "`{id}`: {executor} builds no {} node (only {})",
                    n.role.name(),
                    roles.join(", ")
                ));
            }
            let kind = self.devices[id.as_str()].kind();
            let Some(b) = of_role.iter().find(|b| b.at.kind == kind) else {
                let kinds: Vec<&str> = of_role.iter().map(|b| b.at.kind.name()).collect();
                return Err(format!(
                    "`{id}`: {executor} builds {} on {}, not on {}",
                    n.role.name(),
                    kinds.join(" and "),
                    kind.name()
                ));
            };
            if !b.components.contains(&n.component.as_str()) {
                return Err(format!(
                    "`{id}`: {executor} builds {} {}{}, not `{}`",
                    n.role.name(),
                    list(b.components),
                    if of_role.len() > 1 {
                        format!(" on {}", kind.name())
                    } else {
                        String::new()
                    },
                    n.component
                ));
            }
        }
        for b in builds {
            let ids: Vec<&str> = self
                .graph
                .nodes
                .iter()
                .filter(|(id, n)| n.role == b.role && self.devices[id.as_str()].kind() == b.at.kind)
                .map(|(id, _)| id.as_str())
                .collect();
            if ids.len() < b.least || ids.len() > b.most {
                let want = if b.least == b.most {
                    format!("{}", b.least)
                } else if ids.len() < b.least {
                    format!("at least {}", b.least)
                } else {
                    format!("at most {}", b.most)
                };
                let here = if ids.is_empty() {
                    "none".into()
                } else {
                    list(&ids)
                };
                let on = if builds.iter().filter(|o| o.role == b.role).count() > 1 {
                    format!(" on {}", b.at.kind.name())
                } else {
                    String::new()
                };
                let plural = if ids.len() < b.least { b.least } else { b.most } != 1;
                return Err(format!(
                    "{executor} builds {want} {} node{}{on} (here: {here})",
                    b.role.name(),
                    if plural { "s" } else { "" }
                ));
            }
        }
        for o in only {
            for (id, n) in self.graph.nodes.iter().filter(|(_, n)| {
                n.role == o.role && (o.of.is_empty() || o.of.contains(&n.component.as_str()))
            }) {
                match n.inputs.get(o.port) {
                    None if o.bound => {
                        return Err(format!(
                            "`{id}`.{}: not bound, and {executor} has nothing of its own to read there",
                            o.port
                        ));
                    }
                    Some(src) if o.components.is_empty() => {
                        return Err(format!(
                            "`{id}`.{} → `{src}`: {executor} reads nothing there (unbind it)",
                            o.port
                        ));
                    }
                    Some(src)
                        if !o
                            .components
                            .contains(&self.graph.nodes[src].component.as_str()) =>
                    {
                        return Err(format!(
                            "`{id}`.{} → `{src}`: {executor} reads {} here, not `{}`",
                            o.port,
                            list(o.components),
                            self.graph.nodes[src].component
                        ));
                    }
                    _ => {}
                }
            }
        }
        let path = |s: (Role, &[&str])| -> Option<&str> {
            s.1.iter().try_fold(self.find(s.0)?, |id, p| {
                self.graph.nodes[id].inputs.get(*p).map(String::as_str)
            })
        };
        let name = |s: (Role, &[&str])| format!("{}.{}", s.0.name(), s.1.join("."));
        for s in same {
            if let (Some(a), Some(b)) = (path(s.a), path(s.b))
                && a != b
            {
                return Err(format!(
                    "{executor} reads {} and {} as one node (here: `{a}` and `{b}`)",
                    name(s.a),
                    name(s.b)
                ));
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
            for src in ids
                .iter()
                .filter(|s| **s != id && n.inputs.get(p.name) != Some(**s) && out(s) == p.data)
            {
                v.push(with(&|h| {
                    _ = h
                        .nodes
                        .get_mut(id)
                        .unwrap()
                        .inputs
                        .insert(p.name.into(), src.to_string())
                }));
            }
            if !p.required && n.inputs.contains_key(p.name) {
                v.push(with(&|h| {
                    _ = h.nodes.get_mut(id).unwrap().inputs.remove(p.name)
                }));
            }
            for role in Role::ALL {
                for comp in role_components(role)
                    .into_iter()
                    .filter(|c| output(role, c) == p.data)
                {
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
                        h.nodes
                            .get_mut(id)
                            .unwrap()
                            .inputs
                            .insert(p.name.into(), nid.clone());
                    }));
                }
            }
        }
        for comp in role_components(n.role)
            .into_iter()
            .filter(|c| *c != n.component)
        {
            v.push(with(&|h| {
                h.nodes.get_mut(id).unwrap().component = comp.into()
            }));
        }
    }
    v
}

/// The valid graphs one or two [`edits`] from the shipped graph files `regions` build (and
/// from those with each other shading their shading node may be), checked, with the region
/// each lands on among them; each once.
pub fn near(regions: &[&Builds]) -> Vec<Checked> {
    let ours = |g: &Graph| {
        g.region()
            .is_some_and(|b| regions.iter().any(|r| r.id == b.id))
    };
    let mut bases = vec![];
    for (name, _) in super::SHIPPED {
        let g = super::shipped(name).expect("a shipped file");
        if !ours(&g) {
            continue;
        }
        let shading = g
            .check()
            .expect("a shipped graph is valid")
            .shading()
            .to_string();
        let others = g
            .choices(&shading)
            .into_iter()
            .filter(|c| *c != g.nodes[&shading].component);
        // An input only the other shading has (path tracing's bounce rays) reads what the
        // node's input of the same type reads.
        let others: Vec<Graph> = others
            .map(|c| {
                let mut h = g
                    .edit(&super::Change::Component(shading.clone(), c.into()))
                    .expect("a choice");
                let n = h.nodes.get_mut(&shading).expect("the shading");
                for p in inputs(n.role, c) {
                    let like = inputs(n.role, c)
                        .iter()
                        .find(|q| q.data == p.data && n.inputs.contains_key(q.name))
                        .map(|q| n.inputs[q.name].clone());
                    if let Some(src) = like.filter(|_| !n.inputs.contains_key(p.name)) {
                        n.inputs.insert(p.name.into(), src);
                    }
                }
                h
            })
            .collect();
        bases.push(g);
        bases.extend(others);
    }
    let mut seen = std::collections::HashSet::new();
    let mut v = vec![];
    for g in &bases {
        for h in edits(g).into_iter().map(Graph::pruned) {
            let two: Vec<Graph> = edits(&h).into_iter().map(Graph::pruned).collect();
            for x in std::iter::once(h).chain(two) {
                let Ok(ck) = x.check() else { continue };
                if ours(&ck.graph) && seen.insert(ck.graph.canonical().unwrap_or_default()) {
                    v.push(ck);
                }
            }
        }
    }
    v
}
