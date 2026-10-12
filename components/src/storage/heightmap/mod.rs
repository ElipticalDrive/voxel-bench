//! Storage `heightmap` (the voxelspace back ends; from voxelspace): the top solid block and
//! its id per (x, z) column, in a ring around the camera. 2.5D: below its top a column
//! counts as solid, so there are no overhangs or caves.
//!
//! [`Heightmap`] builds the ring on the CPU from the loaded chunks (for every back end);
//! GPU back ends upload it ([`Heightmap::dirty_rows`]) and read it with `gpu.wgsl`
//! (`hm_at`, `hm_top`, the ray walk `hm_trace`), CPU back ends walk it with
//! [`Heightmap::trace`]. The walk serves shadow and bounce rays, so the storage slot means
//! what shading's rays walk, as on every other back end.

use std::collections::HashMap;

use glam::{IVec2, IVec3, Vec2, Vec3};
use mr_core::{BlockId, CHUNK_SIZE, ChunkData};

/// The GPU side: expects the back end to bind [`Heightmap::words`] as `hm: array<u32>` and
/// [`Heightmap::info`] as `hm_info: vec4<u32>` via [`gpu_wgsl`].
const GPU_WGSL: &str = include_str!("gpu.wgsl");

/// The GPU side with its bindings at `group`, `binding` (ring) and `binding + 1` (info).
pub fn gpu_wgsl(group: u32, binding: u32) -> String {
    format!(
        "@group({group}) @binding({binding}) var<storage, read> hm: array<u32>;\n\
         @group({group}) @binding({}) var<uniform> hm_info: vec4<u32>;\n{GPU_WGSL}",
        binding + 1
    )
}

/// A ring entry: low 16 bits = top y + 32768 (0 = empty column), high 16 bits = block id.
pub fn entry_top(e: u32) -> Option<i32> {
    (e & 0xffff != 0).then(|| (e & 0xffff) as i32 - 32768)
}

pub struct Heightmap {
    /// `size * size` entries, then the tile maxima: per 32x32 tile (a chunk column) the
    /// largest entry's top (`tiles * tiles` words, same encoding), so walks skip tiles
    /// their ray passes above.
    pub hm: Vec<u32>,
    pub size: i32,
    /// Tiles per ring side (`size / 32`).
    tiles: i32,
    /// The ring's reach from the camera in blocks: walks stop beyond it.
    reach: u32,
    /// The highest top in the ring (entry encoding; 0 = all empty).
    top: u32,
    /// Loaded chunk y positions per chunk column.
    columns: HashMap<IVec2, Vec<i32>>,
}

impl Heightmap {
    /// A ring covering `view_radius` chunks around the camera.
    pub fn new(view_radius: u32) -> Self {
        let size = (((2 * view_radius + 2) * CHUNK_SIZE as u32).next_power_of_two()) as i32;
        let tiles = size / CHUNK_SIZE;
        Self { hm: vec![0; (size * size + tiles * tiles) as usize], size, tiles, reach: (view_radius + 1) * CHUNK_SIZE as u32, top: 0, columns: HashMap::new() }
    }

    pub fn mask(&self) -> u32 {
        (self.size - 1) as u32
    }

    /// `hm_info` for the GPU: ring mask, reach (blocks), tile base (words), the highest top
    /// in the ring (entry encoding; 0 = all empty).
    pub fn info(&self) -> [u32; 4] {
        [self.mask(), self.reach, (self.size * self.size) as u32, self.top]
    }

    fn tile_index(&self, c: IVec2) -> usize {
        let m = self.tiles - 1;
        (self.size * self.size + (c.y & m) * self.tiles + (c.x & m)) as usize
    }

    pub fn columns(&self) -> usize {
        self.columns.len()
    }

    /// The entry of absolute column (x, z).
    #[inline]
    pub fn at(&self, x: i32, z: i32) -> u32 {
        let m = self.size - 1;
        self.hm[((z & m) * self.size + (x & m)) as usize]
    }

    /// After world updates: rebuilds the chunk columns of the `changed` LOD-0 chunk
    /// positions from `chunk` (the chunks now held); returns the rebuilt chunk columns.
    pub fn apply<'a>(&mut self, changed: impl IntoIterator<Item = IVec3>, chunk: impl Fn(IVec3) -> Option<&'a ChunkData>) -> Vec<IVec2> {
        let mut tiles = vec![];
        for pos in changed {
            let c = IVec2::new(pos.x, pos.z);
            let ys = self.columns.entry(c).or_default();
            match (chunk(pos).is_some(), ys.iter().position(|&y| y == pos.y)) {
                (true, None) => ys.push(pos.y),
                (false, Some(i)) => {
                    ys.swap_remove(i);
                }
                _ => {}
            }
            if ys.is_empty() {
                self.columns.remove(&c);
            }
            if !tiles.contains(&c) {
                tiles.push(c);
            }
        }
        for &c in &tiles {
            self.rebuild_tile(c, &chunk);
        }
        if !tiles.is_empty() {
            let base = (self.size * self.size) as usize;
            self.top = self.hm[base..].iter().map(|e| e & 0xffff).max().unwrap_or(0);
        }
        tiles
    }

    /// Rebuilds the 32x32 entries of chunk column `c`.
    fn rebuild_tile<'a>(&mut self, c: IVec2, chunk: &impl Fn(IVec3) -> Option<&'a ChunkData>) {
        let mut ys = self.columns.get(&c).cloned().unwrap_or_default();
        ys.sort_unstable_by(|a, b| b.cmp(a));
        let mask = self.size - 1;
        let mut top = 0u32;
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let mut entry = 0u32;
                'found: for &cy in &ys {
                    if let Some(ch) = chunk(IVec3::new(c.x, cy, c.y)) {
                        for ly in (0..CHUNK_SIZE).rev() {
                            let b = ch.get(IVec3::new(lx, ly, lz));
                            if b != BlockId::AIR {
                                let y = cy * CHUNK_SIZE + ly;
                                entry = ((y + 32768).clamp(1, 65535) as u32) | ((b.0 as u32) << 16);
                                break 'found;
                            }
                        }
                    }
                }
                let wx = c.x * CHUNK_SIZE + lx;
                let wz = c.y * CHUNK_SIZE + lz;
                self.hm[((wz & mask) * self.size + (wx & mask)) as usize] = entry;
                top = top.max(entry & 0xffff);
            }
        }
        let t = self.tile_index(c);
        self.hm[t] = top;
    }

    /// The word ranges (start, length) chunk column `c` occupies (its rows and its tile
    /// maximum), for uploads.
    pub fn dirty_rows(&self, c: IVec2) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mask = self.size - 1;
        (0..CHUNK_SIZE)
            .map(move |lz| {
                let wz = c.y * CHUNK_SIZE + lz;
                (((wz & mask) * self.size + ((c.x * CHUNK_SIZE) & mask)) as usize, CHUNK_SIZE as usize)
            })
            .chain(std::iter::once((self.tile_index(c), 1)))
    }

    /// The first column a ray from camera-relative `o` along `d` hits within `max_t`:
    /// (t, camera-relative cell, axis of the face entered, block). As `hm_trace` in
    /// `gpu.wgsl` (see there).
    pub fn trace(&self, cam: IVec3, o: Vec3, d: Vec3, max_t: f32) -> Option<(f32, IVec3, usize, u16)> {
        let rel_top = |e: u32| -> f32 { if e & 0xffff == 0 { -1e6 } else { ((e & 0xffff) as i32 - 32768 + 1 - cam.y) as f32 } };
        let global = rel_top(self.top);
        let dxz = Vec2::new(d.x, d.z);
        let oxz = Vec2::new(o.x, o.z);
        let hlen = dxz.length();
        let stp = IVec2::new(if dxz.x > 0.0 { 1 } else { -1 }, if dxz.y > 0.0 { 1 } else { -1 });
        let inv = Vec2::ONE / dxz.abs().max(Vec2::splat(1e-9));
        let bound = |cell: IVec2| -> Vec2 {
            let mut t = Vec2::ZERO;
            for a in 0..2 {
                t[a] = if dxz[a].abs() < 1e-9 { f32::INFINITY } else { ((if dxz[a] > 0.0 { cell[a] + 1 } else { cell[a] }) as f32 - oxz[a]) * dxz[a].signum() * inv[a] };
            }
            t
        };
        let mut cell = IVec2::new(o.x.floor() as i32, o.z.floor() as i32);
        let mut tmax = bound(cell);
        let mut t0 = 0.0f32;
        let mut axis = 0usize;
        let mut first = true;
        for _ in 0..4 * self.reach + 64 {
            if t0 > max_t || t0 * hlen > self.reach as f32 {
                return None;
            }
            let y0 = o.y + d.y * t0;
            if d.y >= 0.0 && y0 >= global {
                return None;
            }
            // Skip the tile (chunk column) if the ray stays above its highest column.
            let abs = cell + IVec2::new(cam.x, cam.z);
            let tile_top = rel_top(self.hm[self.tile_index(IVec2::new(abs.x >> 5, abs.y >> 5))]);
            let tile_min = IVec2::new((abs.x >> 5) * 32 - cam.x, (abs.y >> 5) * 32 - cam.z);
            let tb = bound(tile_min + IVec2::new(if stp.x > 0 { 31 } else { 0 }, if stp.y > 0 { 31 } else { 0 }));
            let t_exit = tb.x.min(tb.y);
            if y0.min(o.y + d.y * t_exit) >= tile_top {
                first = false;
                // Into the next tile, in integers so that every skip advances.
                axis = if tb.x < tb.y { 0 } else { 1 };
                t0 = t_exit;
                let p = oxz + dxz * t0;
                let mut next = IVec2::new(p.x.floor() as i32, p.y.floor() as i32).clamp(tile_min, tile_min + IVec2::splat(31));
                next[axis] = if stp[axis] > 0 { tile_min[axis] + 32 } else { tile_min[axis] - 1 };
                cell = next;
                tmax = bound(cell);
                continue;
            }
            let t1 = tmax.x.min(tmax.y);
            let e = self.at(abs.x, abs.y);
            let hh = rel_top(e);
            if !first && y0 < hh {
                let y = (y0.floor() as i32).min(hh as i32 - 1);
                return Some((t0, IVec3::new(cell.x, y, cell.y), if axis == 0 { 0 } else { 2 }, (e >> 16) as u16));
            }
            if d.y < 0.0 && (!first || o.y >= hh) {
                let tt = (hh - o.y) / d.y;
                if tt >= t0 && tt < t1 {
                    return (tt <= max_t).then(|| (tt, IVec3::new(cell.x, hh as i32 - 1, cell.y), 1, (e >> 16) as u16));
                }
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
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat solid ground (tops at y = 31) around a 64-high tower in chunk column (1, 0).
    fn world() -> Heightmap {
        let mut hm = Heightmap::new(4);
        let ground = ChunkData::Uniform(BlockId(1));
        let tower = ChunkData::Uniform(BlockId(2));
        let mut held = vec![];
        for x in -4..=4 {
            for z in -4..=4 {
                held.push(IVec3::new(x, 0, z));
            }
        }
        held.push(IVec3::new(1, 1, 0));
        let lookup = |p: IVec3| -> Option<&ChunkData> {
            if p == IVec3::new(1, 1, 0) { Some(&tower) } else if p.y == 0 && p.x.abs() <= 4 && p.z.abs() <= 4 { Some(&ground) } else { None }
        };
        hm.apply(held.clone(), lookup);
        hm
    }

    #[test]
    fn walks_hit_tops_and_sides_and_skip_tiles() {
        let hm = world();
        let cam = IVec3::ZERO;
        // Straight down from above the ground: the top at y = 32.
        let (t, cell, axis, block) = hm.trace(cam, Vec3::new(-10.5, 40.0, 5.5), Vec3::NEG_Y, 1e7).unwrap();
        assert!((t - 8.0).abs() < 1e-4 && cell == IVec3::new(-11, 31, 5) && axis == 1 && block == 1);
        // Horizontally towards the tower (x from 32): its side, block 2.
        let (t, cell, axis, block) = hm.trace(cam, Vec3::new(10.5, 50.0, 10.5), Vec3::X, 1e7).unwrap();
        assert!((t - 21.5).abs() < 1e-4 && cell.x == 32 && axis == 0 && block == 2, "{t} {cell} {axis} {block}");
        // Upwards and slanted over the ground: nothing (stops above the highest top).
        assert!(hm.trace(cam, Vec3::new(-50.0, 33.0, -50.0), Vec3::new(1.0, 0.2, 0.3).normalize(), 1e7).is_none());
        // Shallow descent across many tiles: lands on the ground, past the skipped tiles.
        let d = Vec3::new(1.0, -0.02, 0.0).normalize();
        let (t, cell, axis, _) = hm.trace(cam, Vec3::new(-120.5, 33.0, -100.5), d, 1e7).unwrap();
        assert!(axis == 1 && cell.y == 31 && (t * d.x - 50.0).abs() < 1.0, "{t} {cell}");
        // Short shadow ray (max_t) stops before the ground.
        assert!(hm.trace(cam, Vec3::new(0.5, 40.0, 0.5), Vec3::NEG_Y, 5.0).is_none());
    }
}
