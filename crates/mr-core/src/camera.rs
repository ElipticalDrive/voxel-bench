use glam::{DVec3, Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// Pinhole camera. World space is right-handed with +Y up.
///
/// * `yaw = 0, pitch = 0` looks down -Z; positive yaw turns right (towards +X);
///   positive pitch looks up.
/// * Pixel `(0, 0)` is the top-left pixel; the primary ray of pixel `(x, y)` passes
///   through the pixel centre `(x + 0.5, y + 0.5)`.
/// * Position is `f64` so that huge worlds keep precision. Renderers are expected to
///   work camera-relative (see [`Camera::split_position`]).
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Geometry closer than this along the view axis may be clipped.
    pub near: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { position: DVec3::ZERO, yaw: 0.0, pitch: 0.0, fov_y: 70f32.to_radians(), near: 0.05 }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cp * sy, sp, -cp * cy)
    }

    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, sy)
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    /// Camera looking from `position` towards `target`.
    pub fn look_at(position: DVec3, target: DVec3, fov_y_deg: f32) -> Self {
        let d = (target - position).as_vec3().normalize_or(Vec3::NEG_Z);
        let pitch = d.y.clamp(-1.0, 1.0).asin();
        let yaw = d.x.atan2(-d.z);
        Self { position, yaw, pitch, fov_y: fov_y_deg.to_radians(), ..Default::default() }
    }

    /// Normalised direction of the primary ray through pixel `(px, py)` for a `w x h` image.
    pub fn ray_dir(&self, px: u32, py: u32, w: u32, h: u32) -> Vec3 {
        self.ray_dir_at(px as f32 + 0.5, py as f32 + 0.5, w, h)
    }

    /// Ray direction through an arbitrary image-plane position (pixel units, `(0, 0)` is
    /// the top-left corner of the image), e.g. for mouse picking.
    pub fn ray_dir_at(&self, x: f32, y: f32, w: u32, h: u32) -> Vec3 {
        let (f, r, u) = (self.forward(), self.right(), self.up());
        let t = (self.fov_y * 0.5).tan();
        let aspect = w as f32 / h as f32;
        let sx = (2.0 * x / w as f32 - 1.0) * t * aspect;
        let sy = (1.0 - 2.0 * y / h as f32) * t;
        (f + sx * r + sy * u).normalize()
    }

    /// Splits the position into an integer block coordinate and a fractional offset in
    /// `[0, 1)`, so that renderers can build camera-relative structures without losing
    /// precision far from the origin.
    pub fn split_position(&self) -> (glam::IVec3, Vec3) {
        let floor = self.position.floor();
        (floor.as_ivec3(), (self.position - floor).as_vec3())
    }

    /// View matrix with the eye at the origin (camera-relative rendering).
    pub fn view_rotation(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(Vec3::ZERO, self.forward(), self.up())
    }

    /// Reverse-Z infinite perspective projection (depth 1 at near, 0 at infinity) for
    /// WebGPU/DirectX NDC (Y up, Z in [0, 1]), consistent with [`Camera::ray_dir`].
    /// Raw Vulkan users must flip Y (or use a negative viewport height).
    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective_infinite_reverse(self.fov_y, aspect, self.near)
    }
}
