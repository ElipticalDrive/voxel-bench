//! Mesh mode: arbitrary triangle meshes, rigidly animated, rendered together
//! with the voxel world.
//!
//! * Geometry ([`MeshData`]) is uploaded once per mesh; instances ([`MeshInstance`]) place
//!   meshes in the world and are resent whenever they move (every frame when animated).
//! * Surfaces: per-vertex sRGB colour, interpolated linearly in sRGB space across each
//!   triangle (barycentric), no textures. Normals are the flat geometric triangle normal,
//!   taken on the side facing the ray, so meshes need not be closed or consistently wound.
//! * Unlit: pixel = interpolated vertex colour. Lit: the same lighting model as voxels
//!   ([`crate::lighting`]). In the reference, meshes are geometry of the whole lighting
//!   model: they shadow voxels and each other, and path-traced bounces land on them (their
//!   linear vertex colour the albedo). Pipelines that lack this report it by their score.
//! * Meshes occlude voxels and voxels occlude meshes, by distance along the primary ray.
//!
//! Renderers opt in with the `meshes` tag in [`crate::RendererInfo::tags`] and receive mesh
//! data through [`crate::Renderer::update_meshes`]. [`crate::FrameRequest::meshes`] says
//! whether to draw them in a given frame.

use std::sync::Arc;

use glam::{DQuat, DVec3, Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// Triangle mesh geometry in mesh-local coordinates (blocks).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshData {
    pub id: u32,
    pub positions: Vec<[f32; 3]>,
    /// sRGB RGBA per vertex (alpha ignored).
    pub colors: Vec<[u8; 4]>,
    /// Three indices per triangle.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Radius of a bounding sphere around the local origin.
    pub fn radius(&self) -> f32 {
        self.positions.iter().map(|p| Vec3::from_array(*p).length()).fold(0.0, f32::max)
    }
}

/// One placed copy of a mesh: `world = position + rotation * (scale * local)`.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshInstance {
    pub id: u32,
    pub mesh: u32,
    pub position: DVec3,
    pub rotation: DQuat,
    pub scale: f64,
}

impl MeshInstance {
    /// Local-to-world transform relative to `origin` (e.g. the camera's block), in f32.
    pub fn matrix_rel(&self, origin: DVec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::splat(self.scale as f32), self.rotation.as_quat(), (self.position - origin).as_vec3())
    }
}

/// A bounding volume hierarchy node: a box around items `[a, b)` (a leaf) or around child
/// nodes `a` and `b`, split on `axis` (`a` the lower side).
#[derive(Copy, Clone, Debug)]
pub struct BvhNode {
    pub lo: DVec3,
    pub hi: DVec3,
    pub a: usize,
    pub b: usize,
    pub leaf: bool,
    pub axis: usize,
}

/// Items per BVH leaf at most.
pub const BVH_LEAF: usize = 4;

/// Builds a BVH over `items` (reordering them; median split on the widest axis of the
/// items' box centres, leaves of at most [`BVH_LEAF`]), appending its nodes to `nodes`, each
/// node before its children; `offset` is added to the leaves' item ranges (`items` a slice
/// of a larger list). `bounds` is an item's box. Returns the root's index.
pub fn build_bvh<T>(items: &mut [T], nodes: &mut Vec<BvhNode>, offset: usize, bounds: &impl Fn(&T) -> (DVec3, DVec3)) -> usize {
    let (lo, hi) = items.iter().map(bounds).fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
    let n = nodes.len();
    nodes.push(BvhNode { lo, hi, a: offset, b: offset + items.len(), leaf: true, axis: 0 });
    if items.len() <= BVH_LEAF {
        return n;
    }
    let centre = |t: &T| {
        let (a, b) = bounds(t);
        a + b
    };
    let (clo, chi) = items.iter().map(centre).fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), c| (lo.min(c), hi.max(c)));
    let ext = chi - clo;
    let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
    let mid = items.len() / 2;
    items.select_nth_unstable_by(mid, |a, b| centre(a)[axis].total_cmp(&centre(b)[axis]));
    let (left, right) = items.split_at_mut(mid);
    let a = build_bvh(left, nodes, offset, bounds);
    let b = build_bvh(right, nodes, offset + mid, bounds);
    nodes[n] = BvhNode { lo, hi, a, b, leaf: false, axis };
    n
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MeshUpdate {
    /// Adds or replaces mesh geometry (by id).
    Meshes(Vec<Arc<MeshData>>),
    /// The complete set of instances, replacing the previous one.
    Instances(Vec<MeshInstance>),
}
