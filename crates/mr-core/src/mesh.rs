//! Mesh mode (IDEAS §13): arbitrary triangle meshes, rigidly animated, rendered together
//! with the voxel world.
//!
//! * Geometry ([`MeshData`]) is uploaded once per mesh; instances ([`MeshInstance`]) place
//!   meshes in the world and are resent whenever they move (every frame when animated).
//! * Surfaces: per-vertex sRGB colour, interpolated linearly in sRGB space across each
//!   triangle (barycentric), no textures. Normals are the flat geometric triangle normal,
//!   taken on the side facing the ray, so meshes need not be closed or consistently wound.
//! * Unlit: pixel = interpolated vertex colour. Lit: the same lighting model as voxels
//!   ([`crate::lighting`]), with the voxel world shadowing meshes. Meshes cast no shadows in
//!   this baseline (neither on voxels nor on each other); see PLAN step 7d.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MeshUpdate {
    /// Adds or replaces mesh geometry (by id).
    Meshes(Vec<Arc<MeshData>>),
    /// The complete set of instances, replacing the previous one.
    Instances(Vec<MeshInstance>),
}
