//! Procedural world generators. All are pure functions of position (plus seed), so any
//! chunk at any LOD can be produced independently and in parallel.

use std::sync::Arc;

use glam::{DVec3, IVec3};
use mr_core::chunk::cell_index;
use mr_core::{BlockId, CHUNK_SIZE, ChunkData, ChunkKey, DenseChunk};
use serde::{Deserialize, Serialize};

use crate::noise::{fbm2, hash3, perlin3, unit};
use crate::textures::blocks::*;

pub trait WorldGen: Send + Sync {
    fn block(&self, p: IVec3) -> BlockId;

    /// Range of LOD-0 chunk y coordinates `[min, max)` that may contain blocks.
    fn chunk_y_range(&self) -> (i32, i32);

    /// A sensible camera start position.
    fn spawn(&self) -> DVec3;

    /// Generates a chunk. LOD > 0 chunks point-sample one block per cell (the cell's
    /// centre-most block), which keeps thin features honest at a distance.
    fn chunk(&self, key: ChunkKey) -> ChunkData {
        sample_chunk(key, |p| self.block(p))
    }
}

/// Samples a chunk from a per-block function. At LOD > 0 a cell takes 8 samples (at the
/// quarter points); it is solid if any is, and takes the topmost solid sample's block,
/// since the top is what is usually seen (grass, not the dirt just beneath it).
pub fn sample_chunk(key: ChunkKey, f: impl Fn(IVec3) -> BlockId) -> ChunkData {
    let origin = key.origin();
    let step = 1 << key.lod;
    let offsets: Vec<IVec3> = if step == 1 {
        vec![IVec3::ZERO]
    } else {
        let (a, b) = (step / 4, step * 3 / 4);
        // Highest samples first so the first solid one is the topmost.
        [b, a].iter().flat_map(|&y| [a, b].into_iter().flat_map(move |z| [a, b].into_iter().map(move |x| IVec3::new(x, y, z)))).collect()
    };
    let mut d = DenseChunk::filled(AIR);
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let local = IVec3::new(x, y, z);
                let base = origin + local * step;
                d.0[cell_index(local)] = offsets.iter().map(|o| f(base + *o)).find(|b| b.is_solid()).unwrap_or(AIR);
            }
        }
    }
    ChunkData::Dense(Arc::new(d)).compact()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GenSpec {
    Empty,
    /// Infinite rolling terrain with mountains, caves and trees.
    Terrain {
        #[serde(default)]
        seed: u64,
        #[serde(default = "default_true")]
        caves: bool,
        #[serde(default = "default_true")]
        trees: bool,
    },
    /// Infinite flat plane of checkerboarded block types at y < 0.
    Flat {
        #[serde(default)]
        checker: bool,
    },
    /// Menger sponge of `3^level` blocks per side with its minimum corner at the origin.
    Menger { level: u32 },
    /// Closed room (white floor/ceiling, red/green side walls, blue back) with some
    /// blocks inside: the voxel Cornell box. Inner size `size` blocks.
    Room { size: i32 },
    /// Cube of side `size` centred at the origin, each block solid with probability
    /// `density`. Worst case for many renderers.
    Random { size: i32, density: f32, seed: u64 },
    /// Every block type on a row, plus debug-textured blocks, for checking textures/UVs.
    Showcase,
}

fn default_true() -> bool {
    true
}

impl GenSpec {
    pub fn build(&self) -> Arc<dyn WorldGen> {
        match self.clone() {
            GenSpec::Empty => Arc::new(EmptyGen),
            GenSpec::Terrain { seed, caves, trees } => Arc::new(TerrainGen { seed, caves, trees }),
            GenSpec::Flat { checker } => Arc::new(FlatGen { checker }),
            GenSpec::Menger { level } => Arc::new(MengerGen { size: 3i32.pow(level) }),
            GenSpec::Room { size } => Arc::new(RoomGen { size }),
            GenSpec::Random { size, density, seed } => Arc::new(RandomGen { size, density, seed }),
            GenSpec::Showcase => Arc::new(ShowcaseGen),
        }
    }
}

pub struct EmptyGen;

impl WorldGen for EmptyGen {
    fn block(&self, _: IVec3) -> BlockId {
        AIR
    }
    fn chunk_y_range(&self) -> (i32, i32) {
        (0, 0)
    }
    fn spawn(&self) -> DVec3 {
        DVec3::ZERO
    }
    fn chunk(&self, _: ChunkKey) -> ChunkData {
        ChunkData::Uniform(AIR)
    }
}

pub struct FlatGen {
    checker: bool,
}

impl WorldGen for FlatGen {
    fn block(&self, p: IVec3) -> BlockId {
        if p.y >= 0 {
            AIR
        } else if self.checker {
            BlockId(1 + ((p.x.rem_euclid(17) + p.z.rem_euclid(17)) % 17) as u16)
        } else if p.y == -1 {
            GRASS
        } else {
            DIRT
        }
    }
    fn chunk_y_range(&self) -> (i32, i32) {
        (-2, 0)
    }
    fn spawn(&self) -> DVec3 {
        DVec3::new(0.5, 10.0, 0.5)
    }
    fn chunk(&self, key: ChunkKey) -> ChunkData {
        let o = key.origin();
        if o.y >= 0 {
            ChunkData::Uniform(AIR)
        } else if o.y + key.extent() <= -2 && !self.checker {
            ChunkData::Uniform(DIRT)
        } else {
            sample_chunk(key, |p| self.block(p))
        }
    }
}

pub struct MengerGen {
    size: i32,
}

impl WorldGen for MengerGen {
    fn block(&self, p: IVec3) -> BlockId {
        if p.cmplt(IVec3::ZERO).any() || p.cmpge(IVec3::splat(self.size)).any() {
            return AIR;
        }
        let (mut x, mut y, mut z) = (p.x, p.y, p.z);
        while x > 0 || y > 0 || z > 0 {
            let ones = (x % 3 == 1) as u8 + (y % 3 == 1) as u8 + (z % 3 == 1) as u8;
            if ones >= 2 {
                return AIR;
            }
            x /= 3;
            y /= 3;
            z /= 3;
        }
        [STONE, BRICKS, COBBLE, PLANKS][((p.x / 9 + p.y / 9 + p.z / 9) % 4) as usize]
    }
    fn chunk_y_range(&self) -> (i32, i32) {
        (0, (self.size + CHUNK_SIZE - 1) / CHUNK_SIZE)
    }
    fn spawn(&self) -> DVec3 {
        let s = self.size as f64;
        DVec3::new(s * 1.6, s * 1.2, s * 1.9)
    }
    fn chunk(&self, key: ChunkKey) -> ChunkData {
        let o = key.origin();
        if o.cmpge(IVec3::splat(self.size)).any() || (o + key.extent()).cmple(IVec3::ZERO).any() {
            return ChunkData::Uniform(AIR);
        }
        sample_chunk(key, |p| self.block(p))
    }
}

pub struct RoomGen {
    size: i32,
}

impl WorldGen for RoomGen {
    fn block(&self, p: IVec3) -> BlockId {
        let s = self.size;
        let inside = |v: i32| (0..s).contains(&v);
        let shell = |v: i32| (-1..=s).contains(&v);
        if !(shell(p.x) && shell(p.y) && shell(p.z)) {
            return AIR;
        }
        if inside(p.x) && inside(p.y) && inside(p.z) {
            // A tall pillar, a short box and a floating debug block.
            let (a, b) = (s / 4, s / 2);
            let pillar = (a..a + s / 5).contains(&p.x) && (a..a + s / 5).contains(&p.z) && p.y < s * 3 / 5;
            let boxb = (b + 1..b + 1 + s / 4).contains(&p.x) && (b..b + s / 4).contains(&p.z) && p.y < s / 4;
            let float = p == IVec3::new(s / 2, s * 3 / 4, s / 4);
            return if pillar { WHITE } else if boxb { PLANKS } else if float { DEBUG } else { AIR };
        }
        if p.x == -1 {
            RED
        } else if p.x == s {
            GREEN
        } else if p.z == -1 {
            BLUE
        } else if p.z == s {
            // Front wall with an opening so the camera can look in.
            let opening = (s / 4..s - s / 4).contains(&p.x) && (s / 4..s - s / 4).contains(&p.y);
            if opening { AIR } else { WHITE }
        } else {
            WHITE
        }
    }
    fn chunk_y_range(&self) -> (i32, i32) {
        (-1, (self.size + CHUNK_SIZE) / CHUNK_SIZE + 1)
    }
    fn spawn(&self) -> DVec3 {
        let s = self.size as f64;
        DVec3::new(s * 0.5, s * 0.5, s * 1.6)
    }
}

pub struct RandomGen {
    size: i32,
    density: f32,
    seed: u64,
}

impl WorldGen for RandomGen {
    fn block(&self, p: IVec3) -> BlockId {
        let h = self.size / 2;
        if p.cmplt(IVec3::splat(-h)).any() || p.cmpge(IVec3::splat(self.size - h)).any() {
            return AIR;
        }
        let r = hash3(self.seed, p.x, p.y, p.z);
        if unit(r) < self.density { BlockId(1 + ((r >> 8) % 11) as u16) } else { AIR }
    }
    fn chunk_y_range(&self) -> (i32, i32) {
        let h = self.size / 2;
        ((-h).div_euclid(CHUNK_SIZE), (self.size - h + CHUNK_SIZE - 1).div_euclid(CHUNK_SIZE))
    }
    fn spawn(&self) -> DVec3 {
        DVec3::new(0.0, 0.0, self.size as f64)
    }
}

pub struct ShowcaseGen;

impl WorldGen for ShowcaseGen {
    fn block(&self, p: IVec3) -> BlockId {
        if p.y == -1 {
            return if (p.x + p.z).rem_euclid(2) == 0 { WHITE } else { GRAVEL };
        }
        if p.y == 1 && p.z == 0 && p.x >= 0 && p.x % 2 == 0 && p.x / 2 < 18 {
            return BlockId(1 + (p.x / 2) as u16);
        }
        // A 3x3x3 debug cube at the end of the row, and one floating debug block.
        if (40..43).contains(&p.x) && (1..4).contains(&p.y) && (-1..2).contains(&p.z) {
            return DEBUG;
        }
        if p == IVec3::new(18, 5, 0) {
            return DEBUG;
        }
        AIR
    }
    fn chunk_y_range(&self) -> (i32, i32) {
        (-1, 1)
    }
    fn spawn(&self) -> DVec3 {
        DVec3::new(18.0, 6.0, 16.0)
    }
}

/// The huge-world generator: infinite in x/z, heights roughly -40..200.
pub struct TerrainGen {
    seed: u64,
    caves: bool,
    trees: bool,
}

const SEA: i32 = 12;

impl TerrainGen {
    pub fn height(&self, x: i32, z: i32) -> i32 {
        let (fx, fz) = (x as f32, z as f32);
        let base = fbm2(self.seed, fx / 400.0, fz / 400.0, 5) * 40.0;
        let m = fbm2(self.seed ^ 0xabc, fx / 900.0, fz / 900.0, 3);
        let mountain = (m - 0.1).max(0.0).powf(1.5) * 450.0;
        let detail = fbm2(self.seed ^ 0x77, fx / 40.0, fz / 40.0, 3) * 4.0;
        (20.0 + base + mountain + detail) as i32
    }

    fn cave(&self, p: IVec3, surface: i32) -> bool {
        if !self.caves || p.y > surface - 1 || p.y < -60 {
            return false;
        }
        let s = 1.0 / 28.0;
        let a = perlin3(self.seed ^ 0xcafe, p.x as f32 * s, p.y as f32 * s * 1.6, p.z as f32 * s);
        let b = perlin3(self.seed ^ 0xbeef, p.x as f32 * s, p.y as f32 * s * 1.6, p.z as f32 * s);
        // Intersection of two noise "sheets" gives long spaghetti tunnels.
        a.abs() < 0.07 && b.abs() < 0.07
    }

    fn column(&self, p: IVec3, h: i32) -> BlockId {
        if p.y > h {
            return AIR;
        }
        if self.cave(p, h) {
            return AIR;
        }
        let depth = h - p.y;
        if h > 150 {
            return if depth == 0 { SNOW } else { STONE };
        }
        if h <= SEA + 1 {
            return if depth < 4 { SAND } else { STONE };
        }
        match depth {
            0 => GRASS,
            1..=3 => DIRT,
            4..=20 if unit(hash3(self.seed, p.x, p.y, p.z)) < 0.01 => ORE,
            _ => STONE,
        }
    }

    /// Tree rooted at column (x, z), if any: returns trunk height.
    fn tree_at(&self, x: i32, z: i32, h: i32) -> Option<i32> {
        if !self.trees || h <= SEA + 1 || h > 110 {
            return None;
        }
        let r = hash3(self.seed ^ 0x7733, x, 0, z);
        (unit(r) < 0.012).then(|| 4 + (r >> 32) as i32 % 3)
    }

    fn tree_block(&self, p: IVec3, heights: &dyn Fn(i32, i32) -> i32) -> Option<BlockId> {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let (tx, tz) = (p.x + dx, p.z + dz);
                let h = heights(tx, tz);
                let Some(trunk) = self.tree_at(tx, tz, h) else { continue };
                let top = h + trunk;
                if dx == 0 && dz == 0 && p.y > h && p.y <= top {
                    return Some(LOG);
                }
                let dy = p.y - top;
                if (-2..=1).contains(&dy) {
                    let r = if dy >= 0 { 1 } else { 2 };
                    if dx.abs() <= r && dz.abs() <= r && !(dx.abs() == 2 && dz.abs() == 2) {
                        return Some(LEAVES);
                    }
                }
            }
        }
        None
    }
}

impl WorldGen for TerrainGen {
    fn block(&self, p: IVec3) -> BlockId {
        let h = self.height(p.x, p.z);
        let b = self.column(p, h);
        if b.is_air() && p.y > h {
            if let Some(t) = self.tree_block(p, &|x, z| self.height(x, z)) {
                return t;
            }
        }
        b
    }

    fn chunk_y_range(&self) -> (i32, i32) {
        (-2, 10)
    }

    fn spawn(&self) -> DVec3 {
        DVec3::new(0.5, self.height(0, 0) as f64 + 30.0, 0.5)
    }

    fn chunk(&self, key: ChunkKey) -> ChunkData {
        let (y0, y1) = self.chunk_y_range();
        let o = key.origin();
        let ext = key.extent();
        if o.y >= y1 * CHUNK_SIZE || o.y + ext <= y0 * CHUNK_SIZE {
            return ChunkData::Uniform(AIR);
        }
        let step = 1 << key.lod;
        let half = step / 2;
        // Column heights with a 2-block margin for trees (LOD 0 only).
        let m = if key.lod == 0 { 2 } else { 0 };
        let w = (CHUNK_SIZE + 2 * m) as usize;
        let mut heights = vec![0i32; w * w];
        let (mut hmin, mut hmax) = (i32::MAX, i32::MIN);
        for z in 0..w as i32 {
            for x in 0..w as i32 {
                let h = self.height(o.x + (x - m) * step + half, o.z + (z - m) * step + half);
                heights[z as usize * w + x as usize] = h;
                hmin = hmin.min(h);
                hmax = hmax.max(h);
            }
        }
        if o.y > hmax + 8 {
            return ChunkData::Uniform(AIR);
        }
        let caves_possible = self.caves && o.y < hmax && o.y + ext > -60;
        if o.y + ext <= hmin - 21 && !caves_possible {
            return ChunkData::Uniform(STONE);
        }
        let hat = |x: i32, z: i32| heights[(z + m) as usize * w + (x + m) as usize];
        let mut d = DenseChunk::filled(AIR);
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let h = hat(x, z);
                for y in 0..CHUNK_SIZE {
                    let mut b = if key.lod == 0 {
                        let p = o + IVec3::new(x, y, z);
                        let mut b = self.column(p, h);
                        if b.is_air() && p.y > h && p.y <= hmax + 9 {
                            let lookup = |wx: i32, wz: i32| hat(wx - o.x, wz - o.z);
                            if let Some(t) = self.tree_block(p, &lookup) {
                                b = t;
                            }
                        }
                        b
                    } else {
                        // Coarse cell spanning [y0, y0 + step): solid if the surface reaches
                        // into it, showing the topmost block it contains (the surface block
                        // for the cell holding the surface).
                        let y0 = o.y + y * step;
                        if h < y0 {
                            AIR
                        } else {
                            let top = (y0 + step - 1).min(h);
                            let p = IVec3::new(o.x + x * step + half, top, o.z + z * step + half);
                            let b = self.column(p, h);
                            if b.is_air() { STONE } else { b }
                        }
                    };
                    // Trees at LOD: sample the canopy at a few points of the cell.
                    if key.lod > 0 && b.is_air() && self.trees {
                        let y0 = o.y + y * step;
                        if y0 <= hmax + 9 && y0 + step > hmin {
                            let q = (step / 4).max(1);
                            for off in [IVec3::new(half, half, half), IVec3::new(q, half, q), IVec3::new(step - q, half, step - q), IVec3::new(q, half, step - q), IVec3::new(step - q, half, q)] {
                                let p = IVec3::new(o.x + x * step, y0, o.z + z * step) + off;
                                if p.y > self.height(p.x, p.z) {
                                    if let Some(t) = self.tree_block(p, &|wx, wz| self.height(wx, wz)) {
                                        b = t;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    d.0[cell_index(IVec3::new(x, y, z))] = b;
                }
            }
        }
        ChunkData::Dense(Arc::new(d)).compact()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_fast_path_matches_block() {
        let g = TerrainGen { seed: 3, caves: true, trees: true };
        for pos in [IVec3::new(0, 0, 0), IVec3::new(-3, 1, 5), IVec3::new(10, -1, -7)] {
            let key = ChunkKey::lod0(pos);
            let c = g.chunk(key);
            for i in (0..mr_core::CHUNK_VOLUME).step_by(97) {
                let l = mr_core::chunk::cell_of_index(i);
                assert_eq!(c.get(l), g.block(key.origin() + l), "chunk {pos} cell {l}");
            }
        }
    }

    #[test]
    fn menger_holes() {
        let g = MengerGen { size: 27 };
        assert!(g.block(IVec3::new(0, 0, 0)).is_solid());
        assert!(g.block(IVec3::new(13, 13, 0)).is_air());
        assert!(g.block(IVec3::new(4, 4, 0)).is_air());
    }
}
