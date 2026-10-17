//! Visibility `columns` on the CPU (was cpu-voxelspace's): [`March`] walks one screen column
//! front to back over the [`Heightmap`], handing every row it covers to an [`Out`]: shaded
//! inline ([`Inline`]) or written as hit records ([`Records`]), which the common deferred
//! pass shades through [`Deferred`]. The executor parallelises over columns.

use glam::{IVec2, IVec3, Vec2, Vec3};
use mr_core::{Camera, Face};

use crate::shading::direct::cpu::Surface;
use crate::storage::heightmap::{Heightmap, entry_top};
use crate::visibility::record::Record;

const EMPTY: f32 = -1e6;

/// What a frame's column march reads: the heightmap and the camera.
pub struct March<'a> {
    hm: &'a Heightmap,
    cam: IVec3,
    /// The camera's position within its block (rays start here, camera-relative).
    pub o: Vec3,
    fwd: Vec3,
    right: Vec3,
    up: Vec3,
    thf: f32,
    aspect: f32,
    w: f32,
    /// Rows per column (the image height).
    pub h: usize,
    max_cells: usize,
}

/// Where the march's pixels go (PLAN step 12c): shaded inline, or hit records for the
/// common deferred pass.
pub trait Out {
    /// The march enters drawn column `cell` (camera-relative x, z).
    fn cell(&mut self, cell: IVec2);
    /// Row `row` shows surface `s` (with the face's normal) at `t` along its primary ray.
    fn hit(&mut self, row: usize, t: f32, s: Surface);
    /// Row `row` shows the sky.
    fn miss(&mut self, row: usize);
}

impl<'a> March<'a> {
    /// The march of a `w` x `h` image from `cam`, crossing at most `max_cells` columns.
    pub fn new(hm: &'a Heightmap, cam: &Camera, w: usize, h: usize, max_cells: usize) -> Self {
        let (cam_block, o) = cam.split_position();
        Self { hm, cam: cam_block, o, fwd: cam.forward(), right: cam.right(), up: cam.up(), thf: (cam.fov_y * 0.5).tan(), aspect: w as f32 / h as f32, w: w as f32, h, max_cells }
    }

    #[inline]
    fn hm_at(&self, c: IVec2) -> u32 {
        self.hm.at(c.x + self.cam.x, c.y + self.cam.z)
    }

    #[inline]
    fn top_of(&self, e: u32) -> f32 {
        entry_top(e).map_or(EMPTY, |y| (y + 1 - self.cam.y) as f32)
    }

    #[inline]
    fn row_of(&self, p: Vec3) -> f32 {
        let depth = p.dot(self.fwd).max(1e-4);
        let sy = p.dot(self.up) / (depth * self.thf);
        (1.0 - sy) * 0.5 * self.h as f32
    }

    #[inline]
    pub fn pixel_ray(&self, x: f32, row: f32) -> Vec3 {
        let sx = (2.0 * x / self.w - 1.0) * self.thf * self.aspect;
        let sy = (1.0 - 2.0 * row / self.h as f32) * self.thf;
        (self.fwd + sx * self.right + sy * self.up).normalize()
    }

    /// The heightmap slope at column `cell`: lit tops shade with it instead of the face
    /// normal.
    #[inline]
    fn slope(&self, cell: IVec2) -> Vec3 {
        let g = |c: IVec2| {
            let v = self.top_of(self.hm_at(c));
            if v > -1e5 { v } else { 0.0 }
        };
        let gx = (g(cell - IVec2::X) - g(cell + IVec2::X)).clamp(-2.0, 2.0);
        let gz = (g(cell - IVec2::Y) - g(cell + IVec2::Y)).clamp(-2.0, 2.0);
        Vec3::new(gx * 0.5, 2.0, gz * 0.5).normalize()
    }

    /// Texture coordinates of point `p` relative to column `cell`'s top block.
    #[inline]
    fn local(&self, p: Vec3, cell: IVec2, top: f32) -> Vec3 {
        (p - Vec3::new(cell.x as f32, top - 1.0, cell.y as f32)).clamp(Vec3::ZERO, Vec3::splat(0.999))
    }

    /// Marches screen column `gx` front to back, handing its rows to `out`.
    pub fn column<O: Out>(&self, gx: usize, out: &mut O) {
        let (o, h) = (self.o, self.h);
        let x = gx as f32 + 0.5;
        let dc = self.pixel_ray(x, h as f32 * 0.5);
        let mut hd = Vec2::new(dc.x, dc.z);
        if hd.length() < 1e-5 {
            hd = Vec2::new(self.up.x, self.up.z) * -self.fwd.y.signum();
        }
        if hd.length() < 1e-5 {
            hd = Vec2::X;
        }
        let hd = hd.normalize();
        let mut ybuf = h as i32;
        let mut cell = IVec2::new(o.x.floor() as i32, o.z.floor() as i32);
        let stp = IVec2::new(if hd.x > 0.0 { 1 } else { -1 }, if hd.y > 0.0 { 1 } else { -1 });
        let inv = Vec2::ONE / hd.abs().max(Vec2::splat(1e-9));
        let oxz = Vec2::new(o.x, o.z);
        let edge = Vec2::new(
            if hd.x > 0.0 { (cell.x + 1) as f32 } else { cell.x as f32 },
            if hd.y > 0.0 { (cell.y + 1) as f32 } else { cell.y as f32 },
        );
        let mut tmax = ((edge - oxz) * Vec2::new(hd.x.signum(), hd.y.signum()) * inv).max(Vec2::ZERO);
        let mut t0 = 0.0f32;
        let mut axis = 0usize;
        let mut first = true;
        let mut i = 0;
        while i < self.max_cells && ybuf > 0 {
            i += 1;
            let t1 = tmax.x.min(tmax.y);
            let e = self.hm_at(cell);
            let hh = self.top_of(e);
            if hh > -1e5 {
                let p0 = Vec3::new(o.x + hd.x * t0, hh, o.z + hd.y * t0);
                let p1 = Vec3::new(o.x + hd.x * t1, hh, o.z + hd.y * t1);
                let r = self.row_of(p0).min(self.row_of(p1));
                let rtop = (r.floor() as i32).max(0);
                let block = (e >> 16) as u16;
                out.cell(cell);
                for row in rtop..ybuf {
                    let d = self.pixel_ray(x, row as f32 + 0.5);
                    let mut face = Face::PosY;
                    let mut t = 0.0;
                    let mut side = false;
                    if !first {
                        let a = if axis == 1 { 2 } else { 0 };
                        let wall = if stp[axis] < 0 { cell[axis] + 1 } else { cell[axis] } as f32;
                        if d[a].abs() > 1e-6 {
                            let tw = (wall - o[a]) / d[a];
                            if tw > 0.0 && o.y + d.y * tw <= hh {
                                side = true;
                                t = tw;
                                face = Face::from_index(a * 2 + (d[a] > 0.0) as usize);
                            }
                        }
                    }
                    if !side {
                        t = if d.y < -1e-6 { (hh - o.y) / d.y } else { t0 / Vec2::new(d.x, d.z).length().max(1e-4) };
                    }
                    let p = o + d * t;
                    out.hit(row as usize, t, Surface { p, n: face.normal().as_vec3(), local: self.local(p, cell, hh), block, face });
                }
                ybuf = ybuf.min(rtop);
            }
            first = false;
            t0 = t1;
            if tmax.x < tmax.y {
                cell.x += stp.x;
                tmax.x += inv.x;
                axis = 0;
            } else {
                cell.y += stp.y;
                tmax.y += inv.y;
                axis = 1;
            }
        }
        for row in 0..ybuf.max(0) as usize {
            out.miss(row);
        }
    }
}

/// `placement = inline`: rows shaded as the march covers them (into a column of RGBA8).
pub struct Inline<'a, S> {
    col: &'a mut [u8],
    m: &'a March<'a>,
    shade: &'a S,
    lit: bool,
    /// The current column's top normal (lit: the heightmap slope).
    top_n: Vec3,
    sky: [u8; 4],
}

impl<'a, S> Inline<'a, S> {
    /// Shades into `col` (a column of RGBA8) with `shade`; `lit` tops take the heightmap
    /// slope as their normal.
    pub fn new(col: &'a mut [u8], m: &'a March<'a>, shade: &'a S, lit: bool, sky: [u8; 4]) -> Self {
        Self { col, m, shade, lit, top_n: Vec3::Y, sky }
    }
}

impl<S: Fn(&Surface) -> [u8; 4]> Out for Inline<'_, S> {
    #[inline]
    fn cell(&mut self, cell: IVec2) {
        self.top_n = if self.lit { self.m.slope(cell) } else { Vec3::Y };
    }
    #[inline]
    fn hit(&mut self, row: usize, _: f32, mut s: Surface) {
        if s.face == Face::PosY {
            s.n = self.top_n;
        }
        self.col[row * 4..row * 4 + 4].copy_from_slice(&(self.shade)(&s));
    }
    #[inline]
    fn miss(&mut self, row: usize) {
        self.col[row * 4..row * 4 + 4].copy_from_slice(&self.sky);
    }
}

/// `placement = deferred`: rows written as hit records (a column of them).
pub struct Records<'a> {
    pub col: &'a mut [Record],
}

impl Out for Records<'_> {
    #[inline]
    fn cell(&mut self, _: IVec2) {}
    #[inline]
    fn hit(&mut self, row: usize, t: f32, s: Surface) {
        self.col[row] = Record::hit(t, s.block, s.face, (s.p - s.local).round().as_ivec3());
    }
    #[inline]
    fn miss(&mut self, row: usize) {
        self.col[row] = Record::SKY;
    }
}

/// `placement = deferred`: the frame's column-major records, shaded by the common deferred
/// pass. The decoded surface gets the march's own details back from the record's cell (as
/// the GPU's `view_surface`): texture coordinates relative to the column's top block, the
/// heightmap slope on lit tops.
pub struct Deferred<'a, S> {
    pub recs: &'a [Record],
    pub m: &'a March<'a>,
    pub shade: &'a S,
    pub lit: bool,
    pub sky: [u8; 4],
}

impl<S: Fn(&Surface) -> [u8; 4] + Sync> crate::shading::deferred::Records for Deferred<'_, S> {
    type Record = Record;
    #[inline]
    fn record_at(&self, x: u32, y: u32) -> Record {
        self.recs[x as usize * self.m.h + y as usize]
    }
    #[inline]
    fn hit(r: Record) -> bool {
        r.is_hit()
    }
    fn shade(&self, r: Record, x: u32, y: u32) -> [u8; 4] {
        let d = self.m.pixel_ray(x as f32 + 0.5, y as f32 + 0.5);
        let mut s = r.surface(self.m.o, d);
        let c = (s.p - s.local).round();
        let cell = IVec2::new(c.x as i32, c.z as i32);
        s.local = self.m.local(s.p, cell, self.m.top_of(self.m.hm_at(cell)));
        if self.lit && s.face == Face::PosY {
            s.n = self.m.slope(cell);
        }
        (self.shade)(&s)
    }
    #[inline]
    fn miss(&self, _: u32, _: u32) -> [u8; 4] {
        self.sky
    }
}
