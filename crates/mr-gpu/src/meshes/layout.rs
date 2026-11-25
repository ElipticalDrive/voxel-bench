//! The two-level layout of the meshes' storage buffer: shapes (each mesh's triangles in its
//! own local space, with its bottom-level BVH) written when meshes arrive, and per frame
//! only the placed instances (their transforms and a top-level BVH over them).
//!
//! The buffer, in words:
//! - a header of [`HEAD`] words ([`Header`]);
//! - the shapes' vertices, [`VERT_WORDS`] each, three per triangle, every shape's triangles
//!   in its BLAS's leaf order (local positions, the local triangle normal, sRGB colour);
//! - the BLAS nodes of every shape ([`NODE_WORDS`] each; children by index in this list,
//!   leaves name triangles local to their shape);
//! - the instance table ([`INST_WORDS`] each, ordered by shape, so each shape's instances
//!   are consecutive and drawn by one instanced draw: [`Placed::draws`]): local ->
//!   camera-block-relative as three rows of a 3x4 matrix, its inverse likewise, the shape's
//!   BLAS root, first vertex, the instance's first global triangle and its triangle count;
//! - the TLAS nodes (leaves name items), then its items (instance indices).
//!
//! The shapes' part is fixed between mesh updates; a moved instance or a new camera block
//! rewrites the header and what follows the shapes. A record's triangle id is global (the
//! instance's first triangle + the local index), so the 2^22 a record can name counts placed
//! triangles; the instance of an id is found by a binary search over the table's bases.
//!
//! BVH node words (both levels): its box (lo, hi: 6 f32), then a leaf's first item and
//! count, or an inner node's two children (the second in bits 0-28, the split axis in bits
//! 29-30, bit 31 set; the first child on the axis' lower side).

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use glam::{DMat4, DVec3, Vec3};
use mr_core::mesh::{BvhNode, build_bvh};
use mr_core::{MeshData, MeshInstance};

use super::{MESH_TRIS, Vertex};

/// Words before the vertices.
pub const HEAD: usize = 8;
/// Words per vertex (position, sRGB colour, normal).
pub const VERT_WORDS: usize = size_of::<Vertex>() / 4;
/// Words per BVH node.
pub const NODE_WORDS: usize = 8;
/// Words per instance (40 used, padded to a multiple of 4: the table is also an
/// instance-rate vertex buffer).
pub const INST_WORDS: usize = 32;
/// Word offsets inside an instance.
pub const INST_INVERSE: usize = 12;
pub const INST_ROOT: usize = 24;
pub const INST_FIRST_VERTEX: usize = 25;
pub const INST_BASE: usize = 26;
pub const INST_TRIS: usize = 27;

/// The header: where each part starts (words from the buffer's start) and their counts.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Header {
    pub blas: u32,
    pub instances: u32,
    pub instance_count: u32,
    pub tlas: u32,
    pub items: u32,
    pub triangles: u32,
}

impl Header {
    pub fn words(&self) -> [u32; HEAD] {
        [
            self.blas,
            self.instances,
            self.instance_count,
            self.tlas,
            self.items,
            self.triangles,
            0,
            0,
        ]
    }
}

/// One shape in the buffer.
#[derive(Copy, Clone, Debug)]
struct Span {
    first_vertex: u32,
    tris: u32,
    /// Its BLAS root (an index in the BLAS node list).
    root: u32,
    /// Its local box.
    lo: DVec3,
    hi: DVec3,
}

/// The shapes' part: every mesh's triangles and BLAS, built once per mesh update.
pub struct Shapes {
    spans: HashMap<u32, Span>,
    pub verts: Vec<Vertex>,
    /// The BLAS nodes' words.
    pub nodes: Vec<u32>,
}

impl Shapes {
    /// Lays out `meshes` (by id; meshes without triangles get no shape).
    pub fn build(meshes: &HashMap<u32, Arc<MeshData>>) -> Self {
        let mut ids: Vec<u32> = meshes.keys().copied().collect();
        ids.sort_unstable();
        let (mut spans, mut verts, mut nodes) = (HashMap::new(), vec![], vec![]);
        let bounds = |t: &[Vertex; 3]| {
            let p = t.map(|v| Vec3::from_array(v.pos).as_dvec3());
            (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]))
        };
        for id in ids {
            let m = &meshes[&id];
            let mut tris: Vec<[Vertex; 3]> = m
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| {
                    let p: [Vec3; 3] = t.map(|k| Vec3::from_array(m.positions[k as usize]));
                    let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                    std::array::from_fn(|k| Vertex {
                        pos: p[k].to_array(),
                        color: m.colors[t[k] as usize],
                        normal: n.to_array(),
                    })
                })
                .collect();
            if tris.is_empty() {
                continue;
            }
            let root = build_bvh(&mut tris, &mut nodes, 0, &bounds);
            let (lo, hi) = (nodes[root].lo, nodes[root].hi);
            spans.insert(
                id,
                Span {
                    first_vertex: verts.len() as u32,
                    tris: tris.len() as u32,
                    root: root as u32,
                    lo,
                    hi,
                },
            );
            verts.extend(tris.into_iter().flatten());
        }
        Self {
            spans,
            verts,
            nodes: node_words(&nodes),
        }
    }

    /// Where the BLAS nodes start (words).
    pub fn blas_start(&self) -> usize {
        HEAD + self.verts.len() * VERT_WORDS
    }

    /// Where the instances' part starts (words): the shapes' end.
    pub fn end(&self) -> usize {
        self.blas_start() + self.nodes.len()
    }

    /// The placed instances relative to `origin` (the camera's block).
    pub fn place(&self, instances: &[MeshInstance], origin: DVec3) -> Placed {
        self.place_within(instances, origin, MESH_TRIS)
    }

    /// [`Self::place`] with at most `limit` triangles placed.
    fn place_within(&self, instances: &[MeshInstance], origin: DVec3, limit: usize) -> Placed {
        let mut order: Vec<(&MeshInstance, Span)> = instances
            .iter()
            .filter_map(|i| self.spans.get(&i.mesh).map(|s| (i, *s)))
            .collect();
        order.sort_by_key(|(i, _)| i.mesh);
        let mut table = vec![];
        let mut boxes = vec![];
        let mut draws: Vec<Draw> = vec![];
        let (mut base, mut dropped) = (0u32, 0usize);
        for (inst, s) in order {
            if base as usize + s.tris as usize > limit {
                dropped += s.tris as usize;
                continue;
            }
            let m = DMat4::from_scale_rotation_translation(
                DVec3::splat(inst.scale),
                inst.rotation,
                inst.position - origin,
            );
            let k = (table.len() / INST_WORDS) as u32;
            let mut w = [0u32; INST_WORDS];
            rows(&m, &mut w[..12]);
            rows(&m.inverse(), &mut w[INST_INVERSE..INST_INVERSE + 12]);
            (
                w[INST_ROOT],
                w[INST_FIRST_VERTEX],
                w[INST_BASE],
                w[INST_TRIS],
            ) = (s.root, s.first_vertex, base, s.tris);
            table.extend_from_slice(&w);
            boxes.push(world_box(&m, s.lo, s.hi));
            match draws.last_mut() {
                Some(d) if d.first_vertex == s.first_vertex => d.instances.end = k + 1,
                _ => draws.push(Draw {
                    first_vertex: s.first_vertex,
                    vertices: s.tris * 3,
                    instances: k..k + 1,
                }),
            }
            base += s.tris;
        }
        let mut items: Vec<u32> = (0..boxes.len() as u32).collect();
        let mut nodes = vec![];
        if !items.is_empty() {
            build_bvh(&mut items, &mut nodes, 0, &|&i: &u32| boxes[i as usize]);
        }
        let tlas = node_words(&nodes);
        let start = self.end() as u32;
        let header = Header {
            blas: self.blas_start() as u32,
            instances: start,
            instance_count: boxes.len() as u32,
            tlas: start + table.len() as u32,
            items: start + (table.len() + tlas.len()) as u32,
            triangles: base,
        };
        let mut words = table;
        words.extend(tlas);
        words.extend(items);
        Placed {
            header,
            words,
            draws,
            dropped,
        }
    }
}

/// The instances' part for one frame.
pub struct Placed {
    pub header: Header,
    /// The instance table, the TLAS nodes and the TLAS items, written at [`Shapes::end`].
    pub words: Vec<u32>,
    /// One instanced draw per shape.
    pub draws: Vec<Draw>,
    /// Triangles left out: beyond the [`MESH_TRIS`] a record can name.
    pub dropped: usize,
}

/// One instanced draw: the shape's vertices, over its instances (table indices).
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub first_vertex: u32,
    pub vertices: u32,
    pub instances: Range<u32>,
}

/// A 3x4 affine matrix's rows as f32 words.
fn rows(m: &DMat4, out: &mut [u32]) {
    for r in 0..3 {
        for c in 0..4 {
            out[r * 4 + c] = (m.col(c)[r] as f32).to_bits();
        }
    }
}

/// The box around the local box (`lo`, `hi`) moved by `m`, padded so that it holds the
/// shape however f32 rounds the transforms.
fn world_box(m: &DMat4, lo: DVec3, hi: DVec3) -> (DVec3, DVec3) {
    let (mut a, mut b) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
    for k in 0..8 {
        let c = DVec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let p = m.transform_point3(c);
        (a, b) = (a.min(p), b.max(p));
    }
    let pad = (b - a).max_element() * 1e-5 + a.abs().max(b.abs()).max_element() * 1e-6 + 1e-6;
    (a - pad, b + pad)
}

/// BVH nodes as words (see the module).
fn node_words(nodes: &[BvhNode]) -> Vec<u32> {
    let mut words = vec![0u32; nodes.len() * NODE_WORDS];
    for (n, w) in nodes.iter().zip(words.as_chunks_mut::<NODE_WORDS>().0) {
        // Rounded outwards: the boxes hold their items in f32.
        for i in 0..3 {
            w[i] = down(n.lo[i]).to_bits();
            w[3 + i] = up(n.hi[i]).to_bits();
        }
        (w[6], w[7]) = if n.leaf {
            (n.a as u32, (n.b - n.a) as u32)
        } else {
            (n.a as u32, n.b as u32 | (n.axis as u32) << 29 | 1 << 31)
        };
    }
    words
}

fn down(x: f64) -> f32 {
    let f = x as f32;
    if f as f64 > x { f.next_down() } else { f }
}

fn up(x: f64) -> f32 {
    let f = x as f32;
    if (f as f64) < x { f.next_up() } else { f }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DQuat;

    struct Rng(u32);

    impl Rng {
        /// Uniform in (-s, s).
        fn f(&mut self, s: f32) -> f32 {
            self.0 = self.0.wrapping_mul(747796405).wrapping_add(2891336453);
            ((self.0 >> 8) as f32 / 16777216.0 * 2.0 - 1.0) * s
        }

        fn v(&mut self, s: f32) -> Vec3 {
            Vec3::new(self.f(s), self.f(s), self.f(s))
        }
    }

    fn scene(r: &mut Rng) -> (HashMap<u32, Arc<MeshData>>, Vec<MeshInstance>) {
        let mut meshes = HashMap::new();
        for (id, n) in [(3u32, 60usize), (7, 1), (11, 200)] {
            let mut positions = vec![];
            for _ in 0..n {
                let c = r.v(2.0);
                positions.extend((0..3).map(|_| (c + r.v(0.6)).to_array()));
            }
            let colors = vec![[200, 100, 50, 255]; positions.len()];
            meshes.insert(
                id,
                Arc::new(MeshData {
                    id,
                    positions,
                    colors,
                    indices: (0..3 * n as u32).collect(),
                }),
            );
        }
        // An empty mesh and an instance of a mesh never sent: neither is placed.
        meshes.insert(
            5,
            Arc::new(MeshData {
                id: 5,
                positions: vec![],
                colors: vec![],
                indices: vec![],
            }),
        );
        let mut instances = vec![];
        for id in 0..14u32 {
            let mesh = [11, 3, 7, 11, 5, 3, 99][id as usize % 7];
            let axis = r.v(1.0).as_dvec3().normalize_or(DVec3::Y);
            let rotation = DQuat::from_axis_angle(axis, r.f(3.0) as f64);
            let position = DVec3::new(1000.0, 64.0, -2000.0) + r.v(9.0).as_dvec3();
            instances.push(MeshInstance {
                id,
                mesh,
                position,
                rotation,
                scale: 0.5 + r.f(1.0).abs() as f64 * 1.5,
            });
        }
        (meshes, instances)
    }

    /// The whole buffer, as the GPU gets it.
    fn buffer(shapes: &Shapes, placed: &Placed) -> Vec<u32> {
        [
            &placed.header.words()[..],
            bytemuck::cast_slice(&shapes.verts),
            &shapes.nodes,
            &placed.words,
        ]
        .concat()
    }

    fn f(buf: &[u32], w: u32) -> f32 {
        f32::from_bits(buf[w as usize])
    }

    fn v3(buf: &[u32], w: u32) -> Vec3 {
        Vec3::new(f(buf, w), f(buf, w + 1), f(buf, w + 2))
    }

    /// Möller–Trumbore, two-sided (as `mesh_tri_t`): t, or -1.
    fn tri_t(p: [Vec3; 3], o: Vec3, d: Vec3) -> f32 {
        let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
        let pv = d.cross(e2);
        let det = e1.dot(pv);
        if det.abs() < 1e-12 {
            return -1.0;
        }
        let inv = 1.0 / det;
        let tv = o - p[0];
        let u = tv.dot(pv) * inv;
        if !(0.0..=1.0).contains(&u) {
            return -1.0;
        }
        let qv = tv.cross(e1);
        let v = d.dot(qv) * inv;
        if v < 0.0 || u + v > 1.0 {
            return -1.0;
        }
        e2.dot(qv) * inv
    }

    /// Row-major 3x4 at word `w`, applied to (`p`, `one`).
    fn apply(buf: &[u32], w: u32, p: Vec3, one: f32) -> Vec3 {
        Vec3::from_array(std::array::from_fn(|r| {
            v3(buf, w + 4 * r as u32).dot(p) + f(buf, w + 4 * r as u32 + 3) * one
        }))
    }

    /// The slab test of node word `w` against a ray, up to `max_t`.
    fn enters(buf: &[u32], w: u32, o: Vec3, inv: Vec3, max_t: f32) -> bool {
        let (t0, t1) = ((v3(buf, w) - o) * inv, (v3(buf, w + 3) - o) * inv);
        t0.min(t1).max_element().max(0.0) <= t0.max(t1).min_element().min(max_t)
    }

    fn safe_inv(d: Vec3) -> Vec3 {
        Vec3::select(d.abs().cmplt(Vec3::splat(1e-20)), Vec3::splat(1e-20), d).recip()
    }

    /// Walks a BVH whose nodes start at word `nodes`, from `root`; `leaf(first, count, best)`
    /// tests a leaf's items, lowering `best`.
    fn walk(
        buf: &[u32],
        nodes: u32,
        root: u32,
        o: Vec3,
        d: Vec3,
        best: &mut (f32, Option<u32>),
        leaf: &mut dyn FnMut(u32, u32, &mut (f32, Option<u32>)),
    ) {
        let inv = safe_inv(d);
        let mut stack = vec![root];
        while let Some(k) = stack.pop() {
            let w = nodes + NODE_WORDS as u32 * k;
            if !enters(buf, w, o, inv, best.0) {
                continue;
            }
            let (a, b) = (buf[w as usize + 6], buf[w as usize + 7]);
            if b & 1 << 31 != 0 {
                let second = b & 0x1fff_ffff;
                let (near, far) = if d[(b >> 29 & 3) as usize] < 0.0 {
                    (second, a)
                } else {
                    (a, second)
                };
                stack.extend([far, near]);
            } else {
                leaf(a, b, best);
            }
        }
    }

    /// The two-level walk as the WGSL will do it: the TLAS, then at each instance the ray in
    /// the shape's local space (direction not normalised, so t is shared) through its BLAS.
    /// The nearest (t, global triangle) with 1e-4 < t < max_t.
    fn two_level(buf: &[u32], o: Vec3, d: Vec3, max_t: f32) -> Option<(f32, u32)> {
        let h = &buf[..HEAD];
        let mut best = (max_t, None);
        if h[2] == 0 {
            return None;
        }
        walk(buf, h[3], 0, o, d, &mut best, &mut |a, n, best| {
            for item in a..a + n {
                let i = buf[(h[4] + item) as usize];
                let iw = h[1] + INST_WORDS as u32 * i;
                let (lo, ld) = (
                    apply(buf, iw + INST_INVERSE as u32, o, 1.0),
                    apply(buf, iw + INST_INVERSE as u32, d, 0.0),
                );
                let [root, first, base] =
                    [INST_ROOT, INST_FIRST_VERTEX, INST_BASE].map(|k| buf[iw as usize + k]);
                walk(buf, h[0], root, lo, ld, best, &mut |a, n, best| {
                    for t in a..a + n {
                        let v = HEAD as u32 + (first + 3 * t) * VERT_WORDS as u32;
                        let p = [0, 1, 2].map(|k| v3(buf, v + k * VERT_WORDS as u32));
                        let tt = tri_t(p, lo, ld);
                        if tt > 1e-4 && tt < best.0 {
                            *best = (tt, Some(base + t));
                        }
                    }
                });
            }
        });
        best.1.map(|t| (best.0, t))
    }

    /// The instance of a global triangle, by a binary search over the table's bases (as
    /// `mesh_surface` will find it).
    fn instance_of(buf: &[u32], tri: u32) -> u32 {
        let h = &buf[..HEAD];
        let (mut lo, mut hi) = (0, h[2]);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if buf[(h[1] + INST_WORDS as u32 * mid) as usize + INST_BASE] <= tri {
                lo = mid
            } else {
                hi = mid
            }
        }
        lo
    }

    /// A global triangle's corners: its instance (by the bases), the shape's vertices moved
    /// by the instance's matrix (as `mesh_surface` will decode a record).
    fn decode(buf: &[u32], tri: u32) -> [Vec3; 3] {
        let iw = buf[1] + INST_WORDS as u32 * instance_of(buf, tri);
        let (first, base) = (
            buf[iw as usize + INST_FIRST_VERTEX],
            buf[iw as usize + INST_BASE],
        );
        [0, 1, 2].map(|k| {
            apply(
                buf,
                iw,
                v3(
                    buf,
                    HEAD as u32 + (first + 3 * (tri - base) + k) * VERT_WORDS as u32,
                ),
                1.0,
            )
        })
    }

    /// Every placed triangle in camera-block-relative space, transformed in f64 from the
    /// meshes themselves (independent of the buffer), by global id, with their instance's
    /// table index; in the table's order (instances by mesh, then as given), each mesh's
    /// triangles as given.
    fn world_tris(
        meshes: &HashMap<u32, Arc<MeshData>>,
        instances: &[MeshInstance],
        origin: DVec3,
    ) -> Vec<[Vec3; 3]> {
        let mut sorted: Vec<&MeshInstance> = instances
            .iter()
            .filter(|i| meshes.get(&i.mesh).is_some_and(|m| !m.indices.is_empty()))
            .collect();
        sorted.sort_by_key(|i| i.mesh);
        let mut out = vec![];
        for i in sorted {
            let m = &meshes[&i.mesh];
            let x = DMat4::from_scale_rotation_translation(
                DVec3::splat(i.scale),
                i.rotation,
                i.position - origin,
            );
            out.extend(m.indices.as_chunks::<3>().0.iter().map(|t| {
                t.map(|k| {
                    x.transform_point3(Vec3::from_array(m.positions[k as usize]).as_dvec3())
                        .as_vec3()
                })
            }));
        }
        out
    }

    /// The two-level walk over the words finds what testing every placed triangle (moved
    /// in f64, not through the buffer) finds: the same triangle, the same t to f32 rounding;
    /// rays grazing an edge (where f32 rounding may decide) are skipped.
    #[test]
    fn two_level_walk_matches_brute_force() {
        let mut r = Rng(17);
        let (meshes, instances) = scene(&mut r);
        let origin = DVec3::new(1000.0, 64.0, -2000.0).floor();
        let shapes = Shapes::build(&meshes);
        let placed = shapes.place(&instances, origin);
        let buf = buffer(&shapes, &placed);
        let tris = world_tris(&meshes, &instances, origin);
        assert_eq!(placed.header.triangles as usize, tris.len());
        assert_eq!(
            placed.header.instance_count, 10,
            "the empty mesh's and the unknown mesh's instances are left out"
        );
        let (mut hits, mut grazes) = (0, 0);
        for _ in 0..3000 {
            let o = r.v(16.0);
            let d = (r.v(8.0) - o).normalize();
            let max_t = 5.0 + r.f(1.0).abs() * 30.0;
            let all: Vec<(f32, u32)> = tris
                .iter()
                .enumerate()
                .map(|(i, p)| (tri_t(*p, o, d), i as u32))
                .filter(|&(t, _)| t > 1e-4 && t < max_t)
                .collect();
            let brute = all.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0));
            // Grazing: a near-tie, or an edge within rounding of the ray.
            let graze = |p: [Vec3; 3]| {
                tri_t(p.map(|v| v + Vec3::splat(2e-4)), o, d) > 0.0
                    && tri_t(p.map(|v| v - Vec3::splat(2e-4)), o, d) < 0.0
                    || tri_t(p.map(|v| v - Vec3::splat(2e-4)), o, d) > 0.0
                        && tri_t(p.map(|v| v + Vec3::splat(2e-4)), o, d) < 0.0
            };
            if tris.iter().any(|p| graze(*p))
                || all
                    .iter()
                    .filter(|h| brute.is_some_and(|b| h.0 - b.0 < 1e-3))
                    .count()
                    > 1
            {
                grazes += 1;
                continue;
            }
            let got = two_level(&buf, o, d, max_t);
            match (got, brute) {
                (None, None) => {}
                (Some((t, i)), Some((bt, bi))) => {
                    // Ids name triangles in their BLAS's order: compare the triangle the id
                    // decodes to (its instance by the bases, moved by the table's matrix).
                    let p = decode(&buf, i);
                    assert!(
                        p.iter()
                            .zip(tris[bi as usize])
                            .all(|(a, b)| a.distance(b) < 1e-3),
                        "ray {o} {d}: triangle {p:?} vs {:?}",
                        tris[bi as usize]
                    );
                    assert!(
                        (t - bt).abs() < 1e-3 * bt.max(1.0),
                        "ray {o} {d}: t {t} vs {bt}"
                    );
                    hits += 1;
                }
                _ => panic!("ray {o} {d}: two-level {got:?}, brute force {brute:?}"),
            }
        }
        assert!(hits > 300, "{hits} rays hit");
        assert!(grazes < 300, "{grazes} rays skipped");
    }

    /// The instance found by an id's binary search holds that id, for every id; the draws
    /// cover each instance once, one per shape, with the shape's vertices; a new camera block
    /// changes only the instances' part.
    #[test]
    fn instances_draws_and_ids() {
        let mut r = Rng(5);
        let (meshes, instances) = scene(&mut r);
        let shapes = Shapes::build(&meshes);
        let placed = shapes.place(&instances, DVec3::new(990.0, 60.0, -2010.0));
        let buf = buffer(&shapes, &placed);
        let h = placed.header;
        for tri in 0..h.triangles {
            let w = (h.instances + INST_WORDS as u32 * instance_of(&buf, tri)) as usize;
            assert!(
                buf[w + INST_BASE] <= tri && tri < buf[w + INST_BASE] + buf[w + INST_TRIS],
                "triangle {tri}"
            );
        }
        let mut seen = vec![];
        for d in &placed.draws {
            for i in d.instances.clone() {
                let w = (h.instances + INST_WORDS as u32 * i) as usize;
                assert_eq!(
                    (buf[w + INST_FIRST_VERTEX], buf[w + INST_TRIS] * 3),
                    (d.first_vertex, d.vertices)
                );
                seen.push(i);
            }
        }
        assert_eq!(seen, (0..h.instance_count).collect::<Vec<_>>());
        assert_eq!(placed.draws.len(), 3, "one draw per shape placed");
        let moved = shapes.place(&instances, DVec3::new(1000.0, 64.0, -2000.0));
        assert_eq!(moved.header.blas, h.blas);
        assert_eq!(moved.header.instances, h.instances);
        assert_ne!(moved.words, placed.words);
    }

    /// Past the triangles a record can name (here a smaller limit), whole instances are left out and counted.
    #[test]
    fn too_many_triangles_drops_whole_instances() {
        let n = 30;
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let m = MeshData {
            id: 1,
            colors: vec![[0; 4]; 3],
            positions,
            indices: [0, 1, 2].repeat(n),
        };
        let shapes = Shapes::build(&HashMap::from([(1, Arc::new(m))]));
        let inst = |id| MeshInstance {
            id,
            mesh: 1,
            position: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            scale: 1.0,
        };
        let placed = shapes.place_within(
            &[inst(0), inst(1), inst(2), inst(3)],
            DVec3::ZERO,
            2 * n + n / 2,
        );
        assert_eq!(placed.header.instance_count, 2);
        assert_eq!(placed.header.triangles as usize, 2 * n);
        assert_eq!(placed.dropped, 2 * n);
    }
}
