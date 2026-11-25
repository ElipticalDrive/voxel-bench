//! Mesh mode for the CPU renderers (reference semantics, see `mr_core::mesh`): world-space
//! triangles of every instance, two-sided ray intersection, and the surface values the
//! reference shades.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{DVec3, Vec3};
use mr_core::lighting::srgb_to_linear_f;
use mr_core::mesh::{BvhNode, build_bvh};
use mr_core::{MeshData, MeshInstance, MeshUpdate};

struct Tri {
    p0: DVec3,
    e1: DVec3,
    e2: DVec3,
    /// sRGB (0..1) vertex colours.
    c: [Vec3; 3],
}

/// One instance's triangles `tris[start..end]`, bounding sphere and BVH root (`nodes[root]`).
struct Bounds {
    center: DVec3,
    radius: f64,
    root: usize,
}

#[derive(Default)]
pub struct Meshes {
    data: HashMap<u32, Arc<MeshData>>,
    instances: Vec<MeshInstance>,
    tris: Vec<Tri>,
    nodes: Vec<BvhNode>,
    bounds: Vec<Bounds>,
    dirty: bool,
}

pub struct MeshHit {
    pub t: f64,
    /// Geometric normal, facing the ray.
    pub normal: Vec3,
    /// Interpolated sRGB colour (0..1).
    pub color: Vec3,
}

impl MeshHit {
    /// The linear albedo (the colour without its sRGB encoding).
    pub fn albedo(&self) -> Vec3 {
        Vec3::new(
            srgb_to_linear_f(self.color.x),
            srgb_to_linear_f(self.color.y),
            srgb_to_linear_f(self.color.z),
        )
    }
}

/// Möller–Trumbore, two-sided: `(t, u, v)` when the ray meets the triangle at any `t`.
fn intersect(tri: &Tri, o: DVec3, d: DVec3) -> Option<(f64, f64, f64)> {
    let p = d.cross(tri.e2);
    let det = tri.e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - tri.p0;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(tri.e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some((tri.e2.dot(q) * inv, u, v))
}

impl Tri {
    fn lo(&self) -> DVec3 {
        self.p0.min(self.p0 + self.e1).min(self.p0 + self.e2)
    }

    fn hi(&self) -> DVec3 {
        self.p0.max(self.p0 + self.e1).max(self.p0 + self.e2)
    }
}

impl Meshes {
    pub fn apply(&mut self, u: MeshUpdate) {
        match u {
            MeshUpdate::Meshes(ms) => {
                for m in ms {
                    self.data.insert(m.id, m);
                }
            }
            MeshUpdate::Instances(i) => self.instances = i,
        }
        self.dirty = true;
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    /// Rebuilds world-space triangles and their BVHs after updates; call before tracing.
    pub fn prepare(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.tris.clear();
        self.nodes.clear();
        self.bounds.clear();
        for inst in &self.instances {
            let Some(m) = self.data.get(&inst.mesh) else {
                continue;
            };
            let start = self.tris.len();
            let xf = |p: [f32; 3]| {
                inst.position + inst.rotation * (Vec3::from_array(p).as_dvec3() * inst.scale)
            };
            let col = |i: u32| {
                let c = m.colors[i as usize];
                Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.0
            };
            for t in m.indices.as_chunks::<3>().0 {
                let (a, b, c) = (
                    xf(m.positions[t[0] as usize]),
                    xf(m.positions[t[1] as usize]),
                    xf(m.positions[t[2] as usize]),
                );
                self.tris.push(Tri {
                    p0: a,
                    e1: b - a,
                    e2: c - a,
                    c: [col(t[0]), col(t[1]), col(t[2])],
                });
            }
            if self.tris.len() == start {
                continue;
            }
            let end = self.tris.len();
            let root = build_bvh(
                &mut self.tris[start..end],
                &mut self.nodes,
                start,
                &|t: &Tri| (t.lo(), t.hi()),
            );
            self.bounds.push(Bounds {
                center: inst.position,
                radius: m.radius() as f64 * inst.scale * 1.0001 + 1e-6,
                root,
            });
        }
    }

    /// Nearest triangle hit with `0 < t < max_t` (two-sided, f64).
    pub fn trace(&self, o: DVec3, d: DVec3, max_t: f64) -> Option<MeshHit> {
        let (t, i, u, v) = self.walk(o, d, max_t, false)?;
        let tri = &self.tris[i];
        let mut n = tri.e1.cross(tri.e2).normalize();
        if n.dot(d) > 0.0 {
            n = -n;
        }
        let (u, v) = (u as f32, v as f32);
        Some(MeshHit {
            t,
            normal: n.as_vec3(),
            color: tri.c[0] * (1.0 - u - v) + tri.c[1] * u + tri.c[2] * v,
        })
    }

    /// Whether any triangle is hit with `0 < t < max_t` (a shadow ray's test; stops at the
    /// first).
    pub fn occluded(&self, o: DVec3, d: DVec3, max_t: f64) -> bool {
        self.walk(o, d, max_t, true).is_some()
    }

    /// The nearest hit `(t, triangle, u, v)`, or with `any` the first found.
    fn walk(&self, o: DVec3, d: DVec3, max_t: f64, any: bool) -> Option<(f64, usize, f64, f64)> {
        let mut best = None;
        let mut limit = max_t;
        let inv = d.recip();
        let mut stack = Vec::with_capacity(64);
        for b in &self.bounds {
            // Ray vs bounding sphere (d is unit length).
            let oc = o - b.center;
            let bb = oc.dot(d);
            let disc = bb * bb - (oc.length_squared() - b.radius * b.radius);
            if disc < 0.0 || -bb - disc.sqrt() > limit || -bb + disc.sqrt() < 0.0 {
                continue;
            }
            stack.push(b.root);
            while let Some(n) = stack.pop() {
                let node = &self.nodes[n];
                let (t0, t1) = ((node.lo - o) * inv, (node.hi - o) * inv);
                // A NaN (0 * inf, a ray in a slab's plane) is skipped: f64 min / max take the other.
                if t0.min(t1).max_element().max(0.0) > t0.max(t1).min_element().min(limit) {
                    continue;
                }
                if !node.leaf {
                    stack.extend([node.a, node.b]);
                    continue;
                }
                for (i, tri) in self.tris[node.a..node.b].iter().enumerate() {
                    if let Some((t, u, v)) =
                        intersect(tri, o, d).filter(|h| h.0 > 1e-9 && h.0 < limit)
                    {
                        let i = node.a + i;
                        limit = t;
                        best = Some((t, i, u, v));
                        if any {
                            return best;
                        }
                    }
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DQuat;

    /// One triangle in the z = 0 plane, coloured red / green / blue at its corners.
    fn tri_scene(inst: MeshInstance) -> Meshes {
        let m = MeshData {
            id: 0,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            colors: vec![[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
            indices: vec![0, 1, 2],
        };
        let mut s = Meshes::default();
        s.apply(MeshUpdate::Meshes(vec![Arc::new(m)]));
        s.apply(MeshUpdate::Instances(vec![inst]));
        s.prepare();
        s
    }

    fn inst(position: DVec3, rotation: DQuat, scale: f64) -> MeshInstance {
        MeshInstance {
            id: 0,
            mesh: 0,
            position,
            rotation,
            scale,
        }
    }

    #[test]
    fn hits_misses_normals_and_colours() {
        let s = tri_scene(inst(DVec3::ZERO, DQuat::IDENTITY, 1.0));
        // From +z looking down -z at (0.25, 0.25): t = 5, normal +z, colour mix.
        let h = s
            .trace(DVec3::new(0.25, 0.25, 5.0), -DVec3::Z, 100.0)
            .expect("hit");
        assert!((h.t - 5.0).abs() < 1e-9);
        assert!((h.normal - Vec3::Z).length() < 1e-6);
        assert!(
            (h.color - Vec3::new(0.5, 0.25, 0.25)).length() < 1e-5,
            "{:?}",
            h.color
        );
        // From -z: two-sided, normal flips to face the ray.
        let h = s
            .trace(DVec3::new(0.25, 0.25, -2.0), DVec3::Z, 100.0)
            .expect("hit back side");
        assert!((h.normal + Vec3::Z).length() < 1e-6);
        // Outside the triangle, beyond max_t, and pointing away.
        assert!(
            s.trace(DVec3::new(0.8, 0.8, 5.0), -DVec3::Z, 100.0)
                .is_none()
        );
        assert!(
            s.trace(DVec3::new(0.25, 0.25, 5.0), -DVec3::Z, 4.0)
                .is_none()
        );
        assert!(
            s.trace(DVec3::new(0.25, 0.25, 5.0), DVec3::Z, 100.0)
                .is_none()
        );
    }

    #[test]
    fn instance_transform() {
        // Scaled 4x, rotated 90 degrees about y (local +x -> world -z), moved to (10, 0, 0).
        let s = tri_scene(inst(
            DVec3::new(10.0, 0.0, 0.0),
            DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
            4.0,
        ));
        // The triangle now lies in the x = 10 plane, spanning z in [-4, 0] and y in [0, 4].
        let h = s
            .trace(DVec3::new(20.0, 1.0, -1.0), -DVec3::X, 100.0)
            .expect("hit");
        assert!((h.t - 10.0).abs() < 1e-9);
        assert!(
            s.trace(DVec3::new(20.0, 1.0, 1.0), -DVec3::X, 100.0)
                .is_none()
        );
    }

    /// The BVH finds what testing every triangle finds: the same nearest hit, and a hit for
    /// `occluded` exactly when there is one (random triangles, random rays through them).
    #[test]
    fn bvh_matches_brute_force() {
        let mut rng = crate::Rng::new(7);
        let mut r = |s: f64| (rng.unit() * 2.0 - 1.0) * s;
        let mut positions = vec![];
        for _ in 0..300 {
            let c = [r(4.0), r(4.0), r(4.0)];
            for _ in 0..3 {
                positions.push([
                    (c[0] + r(0.7)) as f32,
                    (c[1] + r(0.7)) as f32,
                    (c[2] + r(0.7)) as f32,
                ]);
            }
        }
        let n = positions.len() as u32;
        let m = MeshData {
            id: 0,
            positions,
            colors: vec![[200, 100, 50, 255]; n as usize],
            indices: (0..n).collect(),
        };
        let mut s = Meshes::default();
        s.apply(MeshUpdate::Meshes(vec![Arc::new(m)]));
        s.apply(MeshUpdate::Instances(vec![inst(
            DVec3::new(1.0, 2.0, 3.0),
            DQuat::from_rotation_z(0.4),
            1.5,
        )]));
        s.prepare();
        assert!(s.nodes.len() > 100, "a tree, not one leaf");
        let mut hits = 0;
        for _ in 0..2000 {
            let o = DVec3::new(r(12.0), r(12.0), r(12.0));
            let d =
                (DVec3::new(1.0, 2.0, 3.0) + DVec3::new(r(5.0), r(5.0), r(5.0)) - o).normalize();
            let max_t = 4.0 + r(1.0).abs() * 20.0;
            let brute = s
                .tris
                .iter()
                .filter_map(|t| intersect(t, o, d))
                .map(|h| h.0)
                .filter(|&t| t > 1e-9 && t < max_t)
                .min_by(f64::total_cmp);
            assert_eq!(s.trace(o, d, max_t).map(|h| h.t), brute);
            assert_eq!(s.occluded(o, d, max_t), brute.is_some());
            hits += brute.is_some() as u32;
        }
        assert!(hits > 200, "{hits} rays hit");
    }
}
