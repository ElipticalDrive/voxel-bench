//! Capabilities: every interface between stages has a required
//! core, which each component implements, plus optional capabilities. A component declares
//! the capabilities it provides and the ones it needs from other stages (`(role, cap)`); a
//! feature (a knob such as LOD) can need capabilities too. A need is met by the component in
//! that role, or by an *adapter*: compile-time code that gives the component the capability
//! from its core, slower. A need met by neither makes the combination invalid, with the
//! reason; nothing substitutes silently. Adapters are compiled in only where the chosen
//! component lacks the capability, so capabilities nobody needs cost nothing.
//!
//! So far the storage interface on the GPU (`storage/adapters.wgsl`) and the hit record's
//! optional fields (`Lod`, `Steps`; `crates/mr-gpu/src/voxels.wgsl`). Capabilities are
//! declared per implementation ([`crate::Impl`]), not per preset: `bricks` has
//! `Lod` on the GPU, not on the CPU (CPU walks have no LOD). CPU implementations declare
//! theirs as associated consts (`Repr::CAPS`, `Kernel::CAPS`), which the registry reads.
//!
//! Storages come with one of two query interfaces, also capabilities: `Voxels` (`s_probe`,
//! the walks', exposure tests' and prepasses' core) and `Heights` (a heightmap's columns,
//! the column march's). No adapter turns one into the other yet.

use crate::{Kind, Role, component};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cap {
    /// Storage: the voxel query interface (`s_probe`, `s_occupied`, `s_block`; on the CPU
    /// `Repr`): what ray walks, exposure tests and prepasses read.
    Voxels,
    /// Storage: the column interface (`hm_top`, `hm_trace`): the top block per (x, z).
    Heights,
    /// Storage: `s_dist(h, l)`, the Chebyshev distance to the nearest solid voxel.
    Dist,
    /// Storage: `s_row(h, x0, y, z)`, the occupancy of the 32 cells from x0 along x.
    Row,
    /// Storage: LOD chunks (`s_top_lod`, cells 2^lod blocks wide). Traversal: a walk that
    /// reads them with distance. Prepass: correct with them.
    Lod,
    /// Traversal: the walk counts its loop iterations (`ct_iters`; the `steps` views).
    Steps,
    /// Draw: the shading's rays (shadows, bounces) meet mesh triangles, not only voxels (the
    /// deferred pass binds them; a forward drawer's fragment shader does not).
    MeshRays,
    /// More than one surface per pixel: what [`mr_core::Feature::Transparency`] needs of every
    /// visibility and draw node (a walk continuing past a see-through block, k records per
    /// pixel), of the primary rays' kernel (it resumes a walk from any cell) and of the
    /// shading (each surface's colour in linear light, to composite).
    Layers,
    /// Draw: writes a hit record (or a face id) per pixel, which a deferred pass shades (so the
    /// records can cross to a second GPU, and the shading runs in a compute pass); a drawer
    /// without it shades forward, in its fragment shader.
    Records,
    /// Shading: path traces: bounce rays (a `bounce` input), per-pixel state accumulated
    /// over frames, the knobs `bounces`, `spp` and `accumulate`; its reference traces paths.
    Paths,
    /// Shading (with `Paths`): its secondary rays measure ambient occlusion (the share of
    /// rays that meet nothing within the knob `reach`) instead of bouncing; its reference
    /// computes that, not paths.
    Occlusion,
    /// Geometry: quads in the `mr_mesh` encoding, grouped per face: what a rasteriser draws
    /// as strips and a ray query traces as two triangles each.
    Quads,
    /// Geometry: one cube instance per block, 36 vertices, the faces derived on the GPU.
    Cubes,
    /// Geometry: what a ray query's acceleration structure is built from (quads as two
    /// triangles each, or one AABB per occupied brick resolved by a DDA through the voxel
    /// pool). Traversal (`needs`): a ray query, tracing it.
    Accel,
    /// Geometry: one point per surface block, which splatting draws.
    Points,
    /// Meshes: rasterised into a record and depth target of their own, at whose depth each
    /// primary ray stops (a ray caster's meshes).
    RayLimit,
    /// Meshes: drawn into the voxel faces' own depth buffer (a rasteriser's meshes).
    FaceDepth,
}

impl Cap {
    pub fn name(self) -> &'static str {
        match self {
            Cap::Voxels => "voxels",
            Cap::Heights => "heights",
            Cap::Dist => "s_dist",
            Cap::Row => "s_row",
            Cap::Lod => "lod",
            Cap::Steps => "steps",
            Cap::MeshRays => "mesh_rays",
            Cap::Layers => "layers",
            Cap::Records => "records",
            Cap::Paths => "paths",
            Cap::Occlusion => "occlusion",
            Cap::Quads => "quads",
            Cap::Cubes => "cubes",
            Cap::Accel => "accel",
            Cap::Points => "points",
            Cap::RayLimit => "ray_limit",
            Cap::FaceDepth => "face_depth",
        }
    }
}

/// An optional capability of the stored hit record: bits of the
/// record's id word (or, for layers, more records per pixel) that pipelines without it never
/// write, and code compiled in only with it. Not every one is built yet.
pub struct RecordCap {
    pub name: &'static str,
    /// The id word's bits it takes (`mr::voxels`, `pack_hit`); none for a capability of
    /// the ray services rather than the record.
    pub bits: Option<std::ops::RangeInclusive<u32>>,
    pub description: &'static str,
    /// Implemented; the others are named here so the record leaves room for them.
    pub built: bool,
}

/// The record's optional capabilities. Voxel records use bits 0-22 (hit, face, block, cell
/// parity); a mesh record reuses bits 1-22 for its triangle under its own flag.
pub const RECORD_CAPS: &[RecordCap] = &[
    RecordCap {
        name: "meshes",
        bits: Some(23..=23),
        description: "Mesh surfaces: bit 23 flags a mesh triangle's hit, its index in bits 1-22 (`pack_mesh_hit`; `visbuffer`: `VIS_MESH | tri`); decoded from the mesh vertices and shaded by the shading component (`shade_mesh`). Compiled in with a meshes stage (`MESHES`).",
        built: true,
    },
    RecordCap {
        name: "layers",
        bits: Some(24..=26),
        description: "Transparency: several surfaces per pixel, `LAYERS` (8) records front to back, each its index here (unused slots the sky); the shading pass composites them (OVER). Written by a walk that continues past see-through blocks (raycast-cpu; `visibility::record`).",
        built: true,
    },
    RecordCap {
        name: "exit",
        bits: Some(27..=27),
        description: "Transparency: the layer is the exit face of a see-through block (seen from inside; its shading normal faces back along the ray).",
        built: true,
    },
    RecordCap {
        name: "opaque-only",
        bits: None,
        description: "Transparency: the cheap declared variant of the shadow rays (a ray service, no record bits): they stop at the first occupied cell, see-through blocks included (fully dark shadows). What a shadow kernel without layers gives (a ray query; every pipeline in a world without see-through blocks); with layers they carry the light through instead (`transmit`, the walk's `1 - coverage` per see-through surface).",
        built: true,
    },
];

/// Compile-time code that gives a component lacking `cap` the capability.
pub struct Adapter {
    pub role: Role,
    pub cap: Cap,
    pub description: &'static str,
}

/// Every adapter. Storage adapters are WGSL sections of `storage/adapters.wgsl`, named by
/// the capability (`//@s_dist`), appended in this order.
pub const ADAPTERS: &[Adapter] = &[
    Adapter {
        role: Role::Storage,
        cap: Cap::Dist,
        description: "`s_dist` from `s_probe`: the distance to the boundary of the empty aligned cube the storage reports",
    },
    Adapter {
        role: Role::Storage,
        cap: Cap::Row,
        description: "`s_row` from 32 voxel-level `s_occupied` queries",
    },
];

pub fn adapter(role: Role, cap: Cap) -> Option<&'static Adapter> {
    ADAPTERS.iter().find(|a| a.role == role && a.cap == cap)
}

/// Whether component `name` in `role` provides `cap` natively on device kind `kind`.
pub fn provides(kind: Kind, role: Role, name: &str, cap: Cap) -> bool {
    component(role, name)
        .and_then(|c| c.on(kind))
        .is_some_and(|i| i.provides.contains(&cap))
}

/// Whether component `name` in `role` provides `cap` on some device kind (what a component is,
/// wherever it runs: a drawer writing records, a path-tracing shading).
pub fn declares(role: Role, name: &str, cap: Cap) -> bool {
    component(role, name).is_some_and(|c| c.impls.iter().any(|i| i.provides.contains(&cap)))
}

/// Whether component `name` in `role` needs `cap` from an input of role `of` (a ray query, the
/// acceleration structure of a geometry).
pub fn needs(role: Role, name: &str, of: Role, cap: Cap) -> bool {
    component(role, name).is_some_and(|c| c.needs.contains(&(of, cap)))
}

#[cfg(test)]
mod tests {
    use super::RECORD_CAPS;

    /// The record's capabilities take distinct bits above the voxel record's (0-22), and the
    /// built ones match the WGSL encoding.
    #[test]
    fn record_caps_are_disjoint() {
        let mut used = 0u32;
        for c in RECORD_CAPS {
            for b in c.bits.clone().into_iter().flatten() {
                assert!(
                    b > 22 && b < 32 && used & (1 << b) == 0,
                    "{} bit {b}",
                    c.name
                );
                used |= 1 << b;
            }
        }
        let mesh = RECORD_CAPS.iter().find(|c| c.name == "meshes").unwrap();
        assert!(
            mesh.built
                && mr_gpu::voxels::VOXELS_WGSL.contains(&format!(
                    "const REC_MESH: u32 = {:#x}u;",
                    1u32 << mesh.bits.clone().unwrap().start()
                ))
        );
    }
}
