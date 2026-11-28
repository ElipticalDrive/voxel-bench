//! What a node executor builds: per role, the components it has code for, the device kind it
//! runs them on and how many such nodes it takes. `Graph::accept` takes a valid graph
//! (`Graph::check`) on a node executor exactly when it has no other node, else refuses it with
//! the reason; it is never built half-way.

use super::{Checked, Graph, Node, Role, When, inputs, output, role_components};
use crate::impls::{self, ApiChoice, Place};
use crate::{Api, Cap, Kind};

/// Which components of a node's role a region takes there: named, or those the registry
/// implements on the node's device kind (every one, or those with a capability, or those
/// needing one of another role), or the union of several picks. A component registered with
/// what a pick other than `Named` asks for is taken without another edit.
#[derive(Copy, Clone, Debug)]
pub enum Pick {
    Named(&'static [&'static str]),
    Providing(Cap),
    /// Every component of the role implemented on the kind.
    Every,
    /// The components implemented on the kind that need the capability of the role (the
    /// walks: a storage's `Voxels`).
    Needing(Role, Cap),
    /// The components any of these take (in the order given, each once).
    Any(&'static [Pick]),
}

impl Pick {
    /// The components it takes for a node of `role` on device kind `kind`: in registry order,
    /// or as named.
    pub fn names(&self, role: Role, kind: Kind) -> Vec<&'static str> {
        match *self {
            Pick::Named(v) => v.to_vec(),
            Pick::Any(v) => {
                let mut out: Vec<&'static str> = vec![];
                for n in v.iter().flat_map(|p| p.names(role, kind)) {
                    if !out.contains(&n) {
                        out.push(n);
                    }
                }
                out
            }
            _ => crate::components(role)
                .filter(|c| self.takes(role, kind, c.name))
                .map(|c| c.name)
                .collect(),
        }
    }

    /// Whether it takes `name` for a node of `role` on device kind `kind`.
    pub fn takes(&self, role: Role, kind: Kind, name: &str) -> bool {
        let implemented = || crate::component(role, name).and_then(|c| c.on(kind));
        match *self {
            Pick::Named(v) => v.contains(&name),
            Pick::Providing(cap) => crate::caps::provides(kind, role, name, cap),
            Pick::Every => implemented().is_some(),
            Pick::Needing(of, cap) => {
                implemented().is_some()
                    && crate::component(role, name).is_some_and(|c| c.needs.contains(&(of, cap)))
            }
            Pick::Any(v) => v.iter().any(|p| p.takes(role, kind, name)),
        }
    }
}

/// Nodes of one role an executor builds.
#[derive(Copy, Clone, Debug)]
pub struct Build {
    pub role: Role,
    pub components: Pick,
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
            components: Pick::Named(components),
            at,
            least: 1,
            most: 1,
        }
    }

    /// One node of `role`, any component implemented `at`'s kind with capability `cap`.
    pub const fn providing(role: Role, cap: Cap, at: Place) -> Build {
        Build::picking(role, Pick::Providing(cap), at)
    }

    /// One node of `role`, any component `components` takes on `at`'s kind.
    pub const fn picking(role: Role, components: Pick, at: Place) -> Build {
        Build {
            components,
            ..Build::one(role, &[], at)
        }
    }

    /// The components it takes.
    pub fn names(&self) -> Vec<&'static str> {
        self.components.names(self.role, self.at.kind)
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
/// node bound to `port` is one of `components` (`Named(&[])`: none, the input stays unbound);
/// `bound`: it must be bound (the executor has no default for it).
#[derive(Copy, Clone, Debug)]
pub struct Only {
    pub role: Role,
    pub of: &'static [&'static str],
    pub port: &'static str,
    pub components: Pick,
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
    GpuSlices,
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
    /// Whether it runs every edge that is not fused data `prev` too (the reader reads the
    /// producer's value of the last frame, with that frame's camera): the generic CPU
    /// executor's edge resources, which it keeps across frames. `free` lists the rest.
    pub late: bool,
    /// Knobs that spell an edge timing (the graph says it; the knob is how plans, the CLI and
    /// the GUI write it): the knob, the edge's role and port, the timing when the knob is
    /// true, when false.
    pub timing_knobs: &'static [(&'static str, Role, &'static str, When, When)],
    /// Whether it runs over two GPUs, split at the hit records (the shading on the second).
    pub split: bool,
    /// The edges (the reader's role and port) whose data it holds in GPU resources between
    /// two passes, so a gate on the edge reads them back, sends them through and writes the
    /// last that arrived for the reader's pass. A late edge among them (`prev`, `iterate`)
    /// is sent where its value is final and what arrived is written, at the next frame's
    /// start, into resources of the reader's own ([`late_crossings`](Self::late_crossings)).
    pub crossings: &'static [(Role, &'static str)],
    /// The late edges (reader's role and port, timing) among `crossings` whose earlier
    /// value the reader reads from resources of its own when a gate sits on the edge.
    pub late_crossings: &'static [(Role, &'static str, When)],
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
            late: false,
            timing_knobs: &[],
            split: false,
            crossings: &[],
            late_crossings: &[],
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
            .or(self.late.then_some(&[When::Now, When::Prev][..]))
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
            .flat_map(|b| b.names())
        {
            if !v.contains(&c) {
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
    &GPU_SLICES,
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

/// Storages, walks, drawers and shadings are not listed: a region takes any storage the
/// registry holds a voxel implementation of where the region runs it
/// (`Pick::Providing(Cap::Voxels)`), any walk (a traversal reading a storage's voxels:
/// `WALKS`), any drawer and any shading implemented there (`Pick::Every`). The kernels are the
/// walks and the ray queries (a traversal reading a geometry's acceleration structure). Geometries
/// and meshes are taken by the form they declare: quads and cubes drawn, an acceleration
/// structure's (quads, boxes) ray-queried, points splatted; meshes stopping a ray caster's
/// rays or drawn into a rasteriser's depth buffer.
const WALKS: Pick = Pick::Needing(Role::Storage, Cap::Voxels);
const GPU_KERNELS: Pick = Pick::Any(&[WALKS, Pick::Needing(Role::Geometry, Cap::Accel)]);
const RT_GEOMETRY: Pick = Pick::Providing(Cap::Accel);
/// The column storages' ray marches (a traversal reading a storage's heights).
pub(crate) const MARCHES: Pick = Pick::Needing(Role::Storage, Cap::Heights);
const DRAWN_GEOMETRY: Pick = Pick::Any(&[Pick::Providing(Cap::Quads), Pick::Providing(Cap::Cubes)]);
/// The CPU rasteriser's storages: the shadow rays' (voxels) and a face source's lookup (any
/// storage it keeps as a pass of its own: voxels or heights).
const LOOKUPS: Pick = Pick::Any(&[Pick::Providing(Cap::Voxels), Pick::Providing(Cap::Heights)]);

/// Ray queries trace an acceleration structure built from one of `RT_GEOMETRY` (only ray
/// queries have a `geometry` port).
const RT: Only = Only {
    role: Role::Traversal,
    of: &[],
    port: "geometry",
    components: RT_GEOMETRY,
    bound: false,
};
/// The shading's shadow rays, walked (`bound`: the executor casts no rays of its own there).
const SHADOW_WALKED: Only = Only {
    role: Role::Shading,
    of: &[],
    port: "shadow",
    components: WALKS,
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
        Build::providing(Role::Storage, Cap::Voxels, impls::CPU_ISAS),
        Build::picking(Role::Traversal, WALKS, impls::CPU_ISAS).count(1, 2),
        Build::one(Role::Visibility, &["raycast"], impls::CPU_ISAS),
        Build::picking(Role::Shading, Pick::Every, impls::CPU_ISAS),
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
    late: true,
    ..Builds::plain(RegionId::CpuRaycast)
};

/// `raster-cpu`: the geometry's quads, their culling, the rasteriser and the shading, its
/// shadow rays through a storage and kernel of the ray caster's. Each node is a pass of its
/// own: a drawer reading the geometry's faces straight transforms them itself (unculled), a
/// culling may read another's culled set, a shading without a shadow site lights without
/// shadow rays, a face source's lookup storage is a pass of its own (its geometry merges
/// only faces of one block: nothing is looked up in it). The faces may come from CPU
/// discovery instead (`quadtree`, `cluster`: the face set it found, drawn from this frame's
/// camera; its rays walk with `refloor` over the storage it tests exposure in). Boxes and
/// points are drawn as the exposed block faces they stand for (a box's: those the ray
/// queries' search inside it finds; a point's: its exposed faces).
pub const CPU_RASTER: Builds = Builds {
    nodes: &[
        // Quads, or cubes drawn as their six faces, or boxes' and points' exposed faces.
        Build::picking(Role::Geometry, Pick::Any(&[DRAWN_GEOMETRY, Pick::Named(&["boxes", "points"])]), impls::CPU_SCALAR).count(0, 1),
        Build::one(Role::Visibility, &["faces", "quadtree", "cluster"], impls::CPU_SCALAR).count(0, 2),
        Build::picking(Role::Draw, Pick::Every, impls::CPU_SCALAR),
        Build::picking(Role::Storage, LOOKUPS, impls::CPU_SCALAR).count(0, 2),
        Build::picking(Role::Traversal, WALKS, impls::CPU_SCALAR).count(0, 2),
        Build::picking(Role::Shading, Pick::Every, impls::CPU_SCALAR),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Visibility,
            of: &["quadtree", "cluster"],
            port: "exposure",
            components: Pick::Providing(Cap::Voxels),
            bound: true,
        },
        Only {
            role: Role::Visibility,
            of: &["quadtree", "cluster"],
            port: "rays",
            components: Pick::Named(&["refloor"]),
            bound: true,
        },
        Only {
            role: Role::Shading,
            of: &[],
            port: "shadow",
            components: WALKS,
            bound: false,
        },
        Only {
            role: Role::Visibility,
            of: &[],
            port: "storage",
            components: LOOKUPS,
            bound: false,
        },
    ],
    live: &[Live::of(Role::Geometry)],
    late: true,
    ..Builds::plain(RegionId::CpuRaster)
};

/// `columns-cpu`: the heightmap, its own ray march (the shadow rays), the column march and the
/// shading. Or, in the column march's place, the ray caster over a heightmap's march (one ray
/// per pixel through the `heightfield` kernel bound to its `primary` port).
pub const CPU_COLUMNS: Builds = Builds {
    nodes: &[
        // The march's heightmap and the one the shadow rays walk (the same or another: a
        // pass each), or no shadow rays (no march bound to the shading).
        Build::providing(Role::Storage, Cap::Heights, impls::CPU_SCALAR).count(1, 2),
        Build::picking(Role::Traversal, MARCHES, impls::CPU_SCALAR).count(0, 2),
        Build::one(Role::Visibility, &["columns", "raycast"], impls::CPU_SCALAR),
        Build::picking(Role::Shading, Pick::Every, impls::CPU_SCALAR),
    ],
    same: &[],
    only: &[Only {
        role: Role::Visibility,
        of: &["raycast"],
        port: "primary",
        components: MARCHES,
        bound: true,
    }],
    late: true,
    ..Builds::plain(RegionId::CpuColumns)
};

/// The GPU walk (`raycast-gpu`): a storage and kernel per walk (primary, shadow, bounce and
/// mesh-shadow rays), a ray-query kernel's geometry (built on the CPU), the mesh records, the
/// beam prepass (over the primary rays' storage, which its shader reads), the walk and the
/// shading (on this GPU or another).
pub const GPU_WALK: Builds = Builds {
    nodes: &[
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_DISPATCH).count(0, 4),
        Build::picking(Role::Traversal, GPU_KERNELS, impls::GPU_DISPATCH).count(1, 4),
        Build::picking(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR).count(0, 1),
        Build::providing(Role::Meshes, Cap::RayLimit, impls::GPU_RASTER).count(0, 1),
        Build::picking(Role::Prepass, Pick::Every, impls::GPU_COMPUTE).count(0, 1),
        Build::one(Role::Visibility, &["raycast"], impls::GPU_DISPATCH),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_DISPATCH),
    ],
    same: &[Same {
        a: (Role::Prepass, &["storage"]),
        b: (Role::Visibility, &["primary", "storage"]),
    }],
    only: &[],
    free: HITS_LATE,
    split: true,
    crossings: &[(Role::Visibility, "start"), (Role::Visibility, "meshes")],
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
        Build::picking(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR),
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_RAYQUERY).count(0, 2),
        Build::picking(Role::Traversal, GPU_KERNELS, impls::GPU_RAYQUERY).count(1, 3),
        Build::one(Role::Visibility, &["raycast"], impls::GPU_RAYQUERY),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_RAYQUERY),
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
        Build::picking(
            Role::Geometry,
            Pick::Any(&[DRAWN_GEOMETRY, RT_GEOMETRY]),
            impls::CPU_SCALAR,
        )
        .count(1, 2),
        Build::one(Role::Visibility, &["faces"], impls::GPU_RASTER),
        Build::picking(Role::Draw, Pick::Every, impls::GPU_RASTER),
        Build::providing(Role::Meshes, Cap::FaceDepth, impls::GPU_RASTER).count(0, 1),
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_DRAWN).count(1, 3),
        Build::picking(Role::Traversal, GPU_KERNELS, impls::GPU_DRAWN).count(1, 3),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_DRAWN),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Visibility,
            of: &[],
            port: "faces",
            components: DRAWN_GEOMETRY,
            bound: true,
        },
        Only {
            role: Role::Draw,
            of: &[],
            port: "faces",
            components: Pick::Named(&["faces"]),
            bound: true,
        },
        Only {
            role: Role::Meshes,
            of: &[],
            port: "shadow",
            components: WALKS,
            bound: false,
        },
        Only {
            role: Role::Visibility,
            of: &[],
            port: "storage",
            components: Pick::Providing(Cap::Voxels),
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
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_DRAWN).count(1, 4),
        Build::picking(Role::Traversal, GPU_KERNELS, impls::GPU_DRAWN).count(1, 3),
        Build::picking(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR).count(0, 1),
        Build::one(Role::Visibility, &["quadtree"], impls::GPU_COMPUTE),
        Build::picking(Role::Holes, Pick::Every, impls::GPU_COMPUTE).count(0, 1),
        Build::picking(Role::Draw, Pick::Every, impls::GPU_RASTER),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_DRAWN),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Draw,
            of: &[],
            port: "faces",
            components: Pick::Named(&["quadtree"]),
            bound: true,
        },
        Only {
            role: Role::Visibility,
            of: &[],
            port: "rays",
            components: WALKS,
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
    // Hole detection reads last frame's sky mask (`prev`) and draws last frame's faces (the
    // loop's first iteration): behind a gate, what crossed.
    crossings: &[(Role::Visibility, "where"), (Role::Draw, "faces"), (Role::Holes, "sky")],
    late_crossings: &[(Role::Holes, "sky", When::Prev), (Role::Draw, "faces", When::Iterate(2))],
    ..Builds::plain(RegionId::GpuSampler)
};

/// CPU discovery (`discover-cpu`): the sampler over a CPU storage (exposure tests over the one
/// its rays walk), the face set drawn on the GPU, the shading there (any GPU rays, a ray-query
/// geometry on the CPU).
pub const CPU_SAMPLER: Builds = Builds {
    nodes: &[
        Build::providing(Role::Storage, Cap::Voxels, impls::CPU_SCALAR),
        Build::one(Role::Traversal, &["refloor"], impls::CPU_SCALAR),
        Build::one(
            Role::Visibility,
            &["quadtree", "cluster"],
            impls::CPU_SCALAR,
        ),
        Build::picking(Role::Draw, Pick::Every, impls::GPU_RASTER),
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_DRAWN).count(0, 2),
        Build::picking(Role::Traversal, GPU_KERNELS, impls::GPU_DRAWN).count(1, 2),
        Build::picking(Role::Geometry, RT_GEOMETRY, impls::CPU_SCALAR).count(0, 1),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_DRAWN),
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
            components: Pick::Named(&["quadtree", "cluster"]),
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
        Build::picking(
            Role::Geometry,
            Pick::Any(&[Pick::Providing(Cap::Points), RT_GEOMETRY]),
            impls::CPU_SCALAR,
        )
        .count(1, 2),
        Build::one(Role::Visibility, &["splat"], impls::GPU_COMPUTE),
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_COMPUTE).count(1, 2),
        Build::picking(Role::Traversal, GPU_KERNELS, impls::GPU_COMPUTE).count(1, 2),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_COMPUTE),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Visibility,
            of: &[],
            port: "points",
            components: Pick::Providing(Cap::Points),
            bound: true,
        },
        SHADOW_WALKED,
        RT,
    ],
    split: true,
    ..Builds::plain(RegionId::GpuSplat)
};

/// Plane sweeping (`slices-gpu`): the storage the planes look up (one for the shadow rays'
/// walk too, or the same), the planes' raster pass writing records, the shading in the common
/// deferred pass (its shadow rays walked).
pub const GPU_SLICES: Builds = Builds {
    nodes: &[
        Build::providing(Role::Storage, Cap::Voxels, impls::GPU_DRAWN).count(1, 2),
        Build::one(Role::Visibility, &["slices"], impls::GPU_RASTER),
        Build::picking(Role::Traversal, WALKS, impls::GPU_DRAWN).count(1, 2),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_COMPUTE),
    ],
    same: &[],
    only: &[
        Only {
            role: Role::Visibility,
            of: &[],
            port: "storage",
            components: Pick::Providing(Cap::Voxels),
            bound: true,
        },
        SHADOW_WALKED,
    ],
    ..Builds::plain(RegionId::GpuSlices)
};

/// Voxel Space on the GPU (`columns-gpu`): the heightmap, its own ray march (the shadow and
/// bounce rays), the column march, the shading.
pub const GPU_COLUMNS: Builds = Builds {
    nodes: &[
        Build::providing(Role::Storage, Cap::Heights, impls::GPU_COMPUTE),
        Build::picking(Role::Traversal, MARCHES, impls::GPU_COMPUTE),
        Build::one(Role::Visibility, &["columns"], impls::GPU_COMPUTE),
        Build::picking(Role::Shading, Pick::Every, impls::GPU_COMPUTE),
    ],
    same: &[],
    only: &[],
    crossings: &[(Role::Shading, "hits")],
    ..Builds::plain(RegionId::GpuColumns)
};

/// The roles the worker runs in front of every executor (the world's side), and the post every
/// executor compiles into its shading.
const AROUND: &[Role] = &[
    Role::World,
    Role::Generator,
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

    /// Why the hits `reader`.`port` reads cannot leave the GPU pass that writes them as hit
    /// records (to cross a gate), if they can't: the reader is the shading, and its source
    /// writes records (rays deferred, a drawer writing records, splats; mesh records name
    /// triangles of the far shading's own copy of the meshes).
    fn records_cross(&self, reader: &str, port: &str) -> Option<String> {
        let g = &self.graph;
        let sn = g.nodes.get(reader)?;
        if sn.role != Role::Shading || port != "hits" {
            return Some(format!("on a GPU only the shading's hits cross as records, not `{reader}`.{port}"));
        }
        let hid = sn.inputs.get("hits")?;
        let hn = g.nodes.get(hid)?;
        match hn.role {
            Role::Draw if !crate::caps::declares(Role::Draw, &hn.component, crate::Cap::Records) => {
                return Some(format!("`{hid}` draws no hit records to send (draw = {})", super::accept::records_drawers()));
            }
            Role::Draw => {}
            Role::Visibility if hn.component == "splat" => {}
            _ if sn.knobs.get("placement").and_then(|v| v.as_str()) != Some("deferred") => {
                return Some(format!("with `placement = inline` the hits never leave the visibility pass; set `placement = \"deferred\"` on `{reader}`"));
            }
            _ => {}
        }
        None
    }

    /// Whether region `builds` builds this graph; else the first node (or role) it cannot,
    /// with the reason.
    pub fn built_by(&self, builds: Builds) -> Result<(), String> {
        let executor = builds.who();
        // Gates on the world's side (its changes into and out of the client, a client
        // generator's chunks) and behind the output (the frame) run in the worker, for every
        // executor; gates inside the pipeline are the executor's, on the edges it gives a
        // serialized form.
        for e in &self.gates {
            // A gate on a previous-frame edge carries the value its reader gets a frame late:
            // where the executor hands that value over itself (the CPU discoverer's faces,
            // the hit records between two GPUs).
            let when = self.graph.nodes.get(&e.reader).map_or(When::Now, |n| n.when(&e.port));
            let role = self.graph.nodes.get(&e.reader).map(|n| n.role);
            let late_ok = role.is_some_and(|r| builds.late_crossings.contains(&(r, e.port.as_str(), when)))
                || when == When::Prev
                    && match e.data {
                        super::Data::Faces => builds.id == RegionId::CpuSampler,
                        super::Data::Hits => builds.split && role == Some(Role::Shading),
                        _ => false,
                    };
            if when != When::Now && !late_ok {
                return Err(format!(
                    "`{}` on `{}`: a gate on an edge with timing `{}` is not modelled for {} in {executor} (gates on late edges run on the CPU discoverer's faces, the hit records a split's second GPU shades, and GPU discovery's hole detection: its sky mask and the drawer's faces in its loop)",
                    e.gates[0].0,
                    e.edge(),
                    when.label(),
                    e.data.name()
                ));
            }
            // The loop's first iteration (hole detection) draws single faces; with the row
            // merge the drawer draws merged runs, which hole detection cannot draw.
            if matches!(when, When::Iterate(_)) && role == Some(Role::Draw) {
                let knob = |r: Role, k: &str| self.graph.nodes.values().find(|n| n.role == r).and_then(|n| n.knobs.get(k)).and_then(|v| v.as_str().map(str::to_string));
                if knob(Role::Visibility, "merge").as_deref() == Some("rows") && knob(Role::Draw, "order").as_deref() != Some("painter") {
                    return Err(format!(
                        "`{}` on `{}`: with `merge = \"rows\"` the drawer draws merged runs, and hole detection (the loop's first iteration) draws single faces: what crosses could not feed both; set `merge = \"none\"`",
                        e.gates[0].0,
                        e.edge()
                    ));
                }
            }
        }
        for e in self.gates.iter().filter(|e| !e.in_worker()) {
            let at = format!("`{}` on `{}`", e.gates[0].0, e.edge());
            match e.data {
                super::Data::Image if self.graph.nodes.get(&e.reader).is_some_and(|n| n.role == Role::Post) => {
                    return Err(format!(
                        "{at}: the post is compiled into its shading, no image crosses that edge; put the gate after the post (on the frame)"
                    ));
                }
                super::Data::Hits if matches!(builds.id, RegionId::CpuRaycast | RegionId::CpuColumns | RegionId::CpuRaster) => {
                    let placement = self.graph.nodes.get(&e.reader).and_then(|n| n.knobs.get("placement")).and_then(|v| v.as_str());
                    if self.graph.nodes.get(&e.reader).is_some_and(|n| n.role == Role::Shading) && placement != Some("deferred") {
                        return Err(format!(
                            "{at}: with `placement = inline` the hits never leave the visibility pass; set `placement = \"deferred\"` on `{}`",
                            e.reader
                        ));
                    }
                }
                // Held between two passes of one GPU executor: read back before the reader's
                // pass, through the gates, the last that arrived written in their place.
                d if self.graph.nodes.get(&e.reader).is_some_and(|n| builds.crossings.contains(&(n.role, e.port.as_str()))) => {
                    let placement = self.graph.nodes.get(&e.reader).and_then(|n| n.knobs.get("placement")).and_then(|v| v.as_str());
                    let shading = self.graph.nodes.get(&e.reader).is_some_and(|n| n.role == Role::Shading);
                    if d == super::Data::Hits && shading && placement != Some("deferred") {
                        return Err(format!(
                            "{at}: with `placement = inline` the hits never leave the march; set `placement = \"deferred\"` on `{}`",
                            e.reader
                        ));
                    }
                }
                // On a GPU the hits cross as the records a split's second GPU shades: read
                // back, through the gates, written to the shading's own copy of the edge.
                super::Data::Hits if builds.split => {
                    if let Some(why) = self.records_cross(&e.reader, &e.port) {
                        return Err(format!("{at}: {why}"));
                    }
                }
                // The CPU rasteriser's faces: the geometry's meshes, the visibility's
                // screen triangles.
                super::Data::Faces if matches!(builds.id, RegionId::CpuRaster) => {}
                // The CPU discoverer's face set, on its way to the GPU that draws it.
                super::Data::Faces if builds.id == RegionId::CpuSampler => {}
                // The GPU rasteriser's faces: the geometry's, each chunk's at their upload; the
                // culling's, each frame's draws at their upload.
                super::Data::Faces if builds.id == RegionId::GpuMeshing => {}
                // The splats' points: each chunk's points between the rebuild and the upload.
                super::Data::Faces if builds.id == RegionId::GpuSplat && self.graph.nodes.get(&e.reader).is_some_and(|n| n.role == Role::Visibility) => {}
                // Ray queries' geometry: each chunk's primitives between the meshing and the
                // BLAS build. One set of acceleration structures serves every ray query over
                // the geometry, so every edge out of it crosses the same gates.
                super::Data::Faces
                    if builds.executor() == "gpu"
                        && self.graph.nodes.get(&e.source).is_some_and(|n| n.role == Role::Geometry)
                        && self.graph.nodes.get(&e.reader).is_some_and(|n| n.role == Role::Traversal && crate::traversal::rt_wgsl(&n.component).is_some()) =>
                {
                    for (id, n) in &self.graph.nodes {
                        for (port, src) in &n.inputs {
                            if *src == e.source && self.gates_on(id, port).is_none_or(|o| o.gates != e.gates) {
                                return Err(format!(
                                    "{at}: the acceleration structures are built once for every ray query over `{}`, so every edge out of it crosses the same gates; `{id}`.{port} has other gates (or none)",
                                    e.source
                                ));
                            }
                        }
                    }
                }
                super::Data::Hits => {
                    return Err(format!(
                        "{at}: {executor} gives its hits no serialized form yet (gates on hits run where the hit records can be split off: the CPU ray caster, rasteriser and column marcher, GPU ray casting, the GPU column marcher, face drawers and splats)"
                    ));
                }
                super::Data::Faces if self.graph.nodes.get(&e.source).is_some_and(|n| matches!(n.role, Role::Geometry)) && builds.executor() == "gpu" => {
                    return Err(format!(
                        "{at}: the geometry is built on the CPU and uploaded into the GPU's buffers as chunks change, not sent whole each frame; no serialized form at that upload yet (gates on faces run in the CPU rasteriser, the CPU discoverer, the GPU rasteriser (geometry and culled draws), the splats' points, ray queries' geometry and between GPU discovery and its drawer)"
                    ));
                }
                super::Data::Faces => {
                    return Err(format!("{at}: {executor} gives its faces no serialized form yet (gates on faces run in the CPU rasteriser, the CPU discoverer, the GPU rasteriser (geometry and culled draws), the splats' points, ray queries' geometry and between GPU discovery and its drawer)"));
                }
                super::Data::Surfaces if builds.executor() == "gpu" => {
                    return Err(format!(
                        "{at}: {executor} rasterises mesh surfaces into its own pass's targets, beside its faces (no edge resource to read back); gates on mesh surfaces run in GPU ray casting (the mesh records the rays read)"
                    ));
                }
                d => return Err(format!("{at}: {executor} runs no gates on {} edges yet", d.name())),
            }
        }
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
            if !b.components.takes(n.role, kind, &n.component) {
                return Err(format!(
                    "`{id}`: {executor} builds {} {}{}, not `{}`",
                    n.role.name(),
                    list(&b.names()),
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
                    Some(src) if matches!(o.components, Pick::Named(&[])) => {
                        return Err(format!(
                            "`{id}`.{} → `{src}`: {executor} reads nothing there (unbind it)",
                            o.port
                        ));
                    }
                    Some(src) => {
                        let s = &self.graph.nodes[src];
                        let kind = self.devices[src.as_str()].kind();
                        if !o.components.takes(s.role, kind, &s.component) {
                            return Err(format!(
                                "`{id}`.{} → `{src}`: {executor} reads {} here, not `{}`",
                                o.port,
                                list(&o.components.names(s.role, kind)),
                                s.component
                            ));
                        }
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
