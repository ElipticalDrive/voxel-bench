//! Scene meshes (mesh mode): procedural shapes, placement and scripted rigid animation.
//! Everything is a deterministic function of the frame index.

use std::sync::Arc;

use glam::{DQuat, DVec3, Vec3};
use mr_core::{MeshData, MeshInstance};
use serde::{Deserialize, Serialize};

use crate::noise::{hash3, unit};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeshSpec {
    pub shape: Shape,
    pub position: [f64; 3],
    /// Uniform scale: the shapes have radius ~1.
    #[serde(default = "one")]
    pub scale: f64,
    /// sRGB colour (0..1); default picked per mesh.
    pub color: Option<[f32; 3]>,
    /// Tessellation (segments for sphere and torus).
    #[serde(default = "detail")]
    pub detail: u32,
    /// Copies scattered in `position +- spread` (deterministic), each with its own phase.
    #[serde(default = "one_u")]
    pub count: u32,
    #[serde(default)]
    pub spread: [f64; 3],
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub animate: Vec<Animation>,
}

fn one() -> f64 {
    1.0
}
fn one_u() -> u32 {
    1
}
fn detail() -> u32 {
    24
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Box,
    Sphere,
    Torus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Animation {
    /// Rotate about `axis` once every `period` frames.
    Spin {
        #[serde(default = "up")]
        axis: [f64; 3],
        period: u32,
    },
    /// Circle around `center` (default: the mesh's position) in the horizontal plane.
    Orbit { center: Option<[f64; 3]>, radius: f64, period: u32 },
    /// Move up and down by `amplitude`.
    Bob { amplitude: f64, period: u32 },
}

fn up() -> [f64; 3] {
    [0.0, 1.0, 0.0]
}

const PALETTE: [[f32; 3]; 6] = [[0.85, 0.25, 0.2], [0.2, 0.55, 0.9], [0.95, 0.75, 0.15], [0.6, 0.3, 0.8], [0.9, 0.9, 0.9], [0.2, 0.75, 0.45]];

fn srgb8(c: Vec3) -> [u8; 4] {
    let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [q(c.x), q(c.y), q(c.z), 255]
}

/// Geometry of `spec` (mesh id `id`). Colours vary across the surface so shapes read in
/// unlit mode too.
pub fn build(spec: &MeshSpec, id: u32) -> MeshData {
    let base = Vec3::from_array(spec.color.unwrap_or(PALETTE[id as usize % PALETTE.len()]));
    let n = spec.detail.max(3);
    let (mut positions, mut colors, mut indices) = (vec![], vec![], vec![]);
    match spec.shape {
        Shape::Box => {
            // Per face 4 vertices, shaded per face and by height.
            let faces: [(Vec3, Vec3, Vec3); 6] = [
                (Vec3::X, Vec3::Y, Vec3::Z),
                (-Vec3::X, Vec3::Z, Vec3::Y),
                (Vec3::Y, Vec3::Z, Vec3::X),
                (-Vec3::Y, Vec3::X, Vec3::Z),
                (Vec3::Z, Vec3::X, Vec3::Y),
                (-Vec3::Z, Vec3::Y, Vec3::X),
            ];
            for (k, (nrm, u, v)) in faces.iter().enumerate() {
                let b = positions.len() as u32;
                for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let p = *nrm + *u * su + *v * sv;
                    positions.push((p * 0.75).to_array());
                    colors.push(srgb8(base * (0.7 + 0.05 * k as f32 + 0.15 * (p.y + 1.0) * 0.5)));
                }
                indices.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
            }
        }
        Shape::Sphere => {
            let rings = n / 2;
            for i in 0..=rings {
                let th = std::f32::consts::PI * i as f32 / rings as f32;
                for j in 0..=n {
                    let ph = std::f32::consts::TAU * j as f32 / n as f32;
                    let p = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
                    positions.push(p.to_array());
                    let stripe = if (j / 2) % 2 == 0 { 1.0 } else { 0.8 };
                    colors.push(srgb8(base * stripe * (0.75 + 0.25 * p.y)));
                }
            }
            for i in 0..rings {
                for j in 0..n {
                    let a = i * (n + 1) + j;
                    let b = a + n + 1;
                    indices.extend([a, b, a + 1, a + 1, b, b + 1]);
                }
            }
        }
        Shape::Torus => {
            let (r_major, r_minor) = (0.7, 0.3);
            let m = (n / 2).max(3);
            for i in 0..=n {
                let u = std::f32::consts::TAU * i as f32 / n as f32;
                for j in 0..=m {
                    let v = std::f32::consts::TAU * j as f32 / m as f32;
                    let r = r_major + r_minor * v.cos();
                    positions.push([r * u.cos(), r_minor * v.sin(), r * u.sin()]);
                    let band = if (i / 3) % 2 == 0 { 1.0 } else { 0.7 };
                    colors.push(srgb8(base * band * (0.8 + 0.2 * v.cos())));
                }
            }
            for i in 0..n {
                for j in 0..m {
                    let a = i * (m + 1) + j;
                    let b = a + m + 1;
                    indices.extend([a, b, a + 1, a + 1, b, b + 1]);
                }
            }
        }
    }
    MeshData { id, positions, colors, indices }
}

/// All mesh geometry of a scene, one entry per spec (ids = spec index).
pub fn build_all(specs: &[MeshSpec]) -> Vec<Arc<MeshData>> {
    specs.iter().enumerate().map(|(i, s)| Arc::new(build(s, i as u32))).collect()
}

/// Every instance at `frame`.
pub fn instances(specs: &[MeshSpec], frame: u32) -> Vec<MeshInstance> {
    let mut out = vec![];
    for (mi, s) in specs.iter().enumerate() {
        for c in 0..s.count.max(1) {
            let r = |k: i32| unit(hash3(s.seed, mi as i32, c as i32, k)) as f64 * 2.0 - 1.0;
            let home = DVec3::from_array(s.position) + DVec3::new(r(0), r(1), r(2)) * DVec3::from_array(s.spread);
            // Copies start their animations at different phases.
            let phase = if s.count > 1 { unit(hash3(s.seed, mi as i32, c as i32, 7)) as f64 } else { 0.0 };
            let mut position = home;
            let mut rotation = DQuat::IDENTITY;
            for a in &s.animate {
                let t = |period: u32| std::f64::consts::TAU * (frame as f64 / period.max(1) as f64 + phase);
                match a {
                    Animation::Spin { axis, period } => rotation = DQuat::from_axis_angle(DVec3::from_array(*axis).normalize(), t(*period)) * rotation,
                    Animation::Orbit { center, radius, period } => {
                        let cen = center.map(DVec3::from_array).unwrap_or(home);
                        let a = t(*period);
                        position = DVec3::new(cen.x + radius * a.cos(), position.y, cen.z + radius * a.sin());
                    }
                    Animation::Bob { amplitude, period } => position.y += amplitude * t(*period).sin(),
                }
            }
            out.push(MeshInstance { id: out.len() as u32, mesh: mi as u32, position, rotation, scale: s.scale });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(shape: Shape) -> MeshSpec {
        MeshSpec { shape, position: [0.0; 3], scale: 1.0, color: None, detail: 12, count: 1, spread: [0.0; 3], seed: 0, animate: vec![] }
    }

    /// Shapes are well formed: indices in range, triangles non-degenerate, radius ~1.
    #[test]
    fn shapes_are_valid() {
        for shape in [Shape::Box, Shape::Sphere, Shape::Torus] {
            let m = build(&spec(shape), 3);
            assert_eq!(m.positions.len(), m.colors.len());
            assert_eq!(m.indices.len() % 3, 0);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
            let degenerate = m.indices.chunks(3).filter(|t| {
                let p = |i: u32| Vec3::from_array(m.positions[i as usize]);
                (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).length() < 1e-7
            });
            // UV spheres have degenerate pole triangles; everything else must not.
            if shape != Shape::Sphere {
                assert_eq!(degenerate.count(), 0, "{shape:?}");
            }
            assert!(m.radius() > 0.9 && m.radius() < 1.5, "{shape:?} radius {}", m.radius());
        }
    }

    #[test]
    fn animation_is_deterministic_and_moves() {
        let mut s = spec(Shape::Torus);
        s.animate = vec![Animation::Spin { axis: [0.0, 1.0, 0.0], period: 60 }, Animation::Bob { amplitude: 2.0, period: 40 }];
        s.count = 3;
        s.spread = [5.0, 0.0, 5.0];
        let a = instances(&[s.clone()], 10);
        assert_eq!(a, instances(&[s.clone()], 10));
        assert_ne!(a, instances(&[s], 11));
        assert_eq!(a.len(), 3);
    }
}
