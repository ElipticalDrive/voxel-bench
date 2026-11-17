//! Procedurally generated 16x16 block textures, so the project needs no asset files.

use mr_core::{BlockClass, BlockId, BlockRegistry, BlockType, TEX_SIZE, Texture};

use crate::noise::{hash3, unit};

/// Well-known block ids of the default registry.
pub mod blocks {
    use mr_core::BlockId;
    pub const AIR: BlockId = BlockId(0);
    pub const STONE: BlockId = BlockId(1);
    pub const DIRT: BlockId = BlockId(2);
    pub const GRASS: BlockId = BlockId(3);
    pub const SAND: BlockId = BlockId(4);
    pub const LOG: BlockId = BlockId(5);
    pub const LEAVES: BlockId = BlockId(6);
    pub const PLANKS: BlockId = BlockId(7);
    pub const BRICKS: BlockId = BlockId(8);
    pub const SNOW: BlockId = BlockId(9);
    pub const GRAVEL: BlockId = BlockId(10);
    pub const COBBLE: BlockId = BlockId(11);
    pub const WHITE: BlockId = BlockId(12);
    pub const RED: BlockId = BlockId(13);
    pub const GREEN: BlockId = BlockId(14);
    pub const BLUE: BlockId = BlockId(15);
    pub const DEBUG: BlockId = BlockId(16);
    pub const ORE: BlockId = BlockId(17);
    pub const GLOW: BlockId = BlockId(18);
    /// See-through blocks (after every opaque one, so existing worlds keep their ids).
    pub const GLASS: BlockId = BlockId(19);
    pub const GLASS_RED: BlockId = BlockId(20);
    pub const GLASS_GREEN: BlockId = BlockId(21);
    pub const GLASS_BLUE: BlockId = BlockId(22);
    pub const GRATE: BlockId = BlockId(23);
}

type Rgb = [f32; 3];

struct Builder {
    reg: BlockRegistry,
}

impl Builder {
    fn tex(&mut self, name: &str, f: impl Fn(usize, usize, u64) -> Rgb) -> u16 {
        self.tex_alpha(name, |x, y, h| (f(x, y, h), 1.0))
    }

    /// A texture with coverage: `f` gives colour and alpha; all-opaque keeps `alpha` empty.
    fn tex_alpha(&mut self, name: &str, f: impl Fn(usize, usize, u64) -> (Rgb, f32)) -> u16 {
        let seed = name.bytes().fold(0u64, |a, b| a.wrapping_mul(131).wrapping_add(b as u64));
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        let (texels, mut alpha): (Vec<[u8; 4]>, Vec<u8>) = (0..TEX_SIZE * TEX_SIZE)
            .map(|i| {
                let (x, y) = (i % TEX_SIZE, i / TEX_SIZE);
                let (c, a) = f(x, y, hash3(seed, x as i32, y as i32, 0));
                ([q(c[0]), q(c[1]), q(c[2]), 255], q(a))
            })
            .unzip();
        if alpha.iter().all(|&a| a == 255) {
            alpha.clear();
        }
        self.reg.textures.push(Texture { name: name.into(), texels, alpha });
        (self.reg.textures.len() - 1) as u16
    }

    fn block(&mut self, name: &str, faces: [u16; 6]) -> BlockId {
        self.reg.blocks.push(BlockType { name: name.into(), faces, class: BlockClass::Opaque });
        BlockId((self.reg.blocks.len() - 1) as u16)
    }

    fn cube(&mut self, name: &str, t: u16) -> BlockId {
        self.block(name, [t; 6])
    }

    fn see_through(&mut self, name: &str, t: u16, class: BlockClass) -> BlockId {
        let b = self.cube(name, t);
        self.reg.blocks[b.0 as usize].class = class;
        b
    }
}

fn speckle(base: Rgb, amount: f32) -> impl Fn(usize, usize, u64) -> Rgb {
    move |_, _, h| {
        let k = 1.0 + (unit(h) - 0.5) * 2.0 * amount;
        [base[0] * k, base[1] * k, base[2] * k]
    }
}

fn mul(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// The default block set. Ids match [`blocks`].
pub fn default_registry() -> BlockRegistry {
    let mut b = Builder { reg: BlockRegistry::default() };
    let missing = b.tex("missing", |x, y, _| if (x / 4 + y / 4) % 2 == 0 { [1.0, 0.0, 1.0] } else { [0.0; 3] });
    b.block("air", [missing; 6]);

    let stone = b.tex("stone", speckle([0.50, 0.50, 0.52], 0.15));
    let dirt = b.tex("dirt", speckle([0.47, 0.33, 0.22], 0.2));
    let grass_top = b.tex("grass_top", speckle([0.35, 0.62, 0.25], 0.2));
    let grass_side = b.tex("grass_side", |_, y, h| {
        let k = 1.0 + (unit(h) - 0.5) * 0.35;
        let edge = 3 + (h % 3) as usize;
        if y < edge { mul([0.35, 0.62, 0.25], k) } else { mul([0.47, 0.33, 0.22], k) }
    });
    let sand = b.tex("sand", speckle([0.86, 0.80, 0.58], 0.08));
    let log_top = b.tex("log_top", |x, y, h| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        let ring = if (d as i32) % 3 == 0 { 0.8 } else { 1.0 };
        if d > 7.0 { [0.35, 0.25, 0.15] } else { mul([0.72, 0.58, 0.38], ring * (0.95 + unit(h) * 0.1)) }
    });
    let bark = b.tex("bark", |x, _, h| mul([0.40, 0.29, 0.17], if x % 4 == 0 { 0.7 } else { 0.9 + unit(h) * 0.2 }));
    let leaves = b.tex("leaves", |_, _, h| if unit(h) < 0.15 { [0.10, 0.25, 0.08] } else { mul([0.22, 0.52, 0.18], 0.8 + unit(h >> 8) * 0.4) });
    let planks = b.tex("planks", |x, y, h| {
        let seam = y % 4 == 3 || (x + (y / 4) * 5) % 16 == 0;
        mul([0.70, 0.54, 0.32], if seam { 0.65 } else { 0.92 + unit(h) * 0.12 })
    });
    let bricks = b.tex("bricks", |x, y, h| {
        let offset = if (y / 4) % 2 == 0 { 0 } else { 4 };
        let mortar = y % 4 == 3 || (x + offset) % 8 == 7;
        if mortar { [0.75, 0.73, 0.70] } else { mul([0.62, 0.25, 0.20], 0.9 + unit(h) * 0.2) }
    });
    let snow = b.tex("snow", speckle([0.95, 0.96, 0.98], 0.03));
    let gravel = b.tex("gravel", |_, _, h| mul([0.55, 0.52, 0.50], 0.6 + unit(h) * 0.6));
    let cobble = b.tex("cobble", |x, y, h| {
        let cell = hash3(7, (x / 5) as i32, (y / 5) as i32, 0);
        let edge = x % 5 == 0 || y % 5 == 0;
        mul([0.48, 0.48, 0.48], if edge { 0.6 } else { 0.8 + unit(cell) * 0.3 + unit(h) * 0.1 })
    });
    let white = b.tex("white", speckle([0.85, 0.85, 0.85], 0.03));
    let red = b.tex("red", speckle([0.80, 0.12, 0.10], 0.03));
    let green = b.tex("green", speckle([0.15, 0.70, 0.15], 0.03));
    let blue = b.tex("blue", speckle([0.15, 0.25, 0.85], 0.03));
    // Debug texture per face: u ramps red, v ramps green, blue identifies the face, and a
    // white marker sits in the top-left (u=0, v=0) corner. Makes UV/orientation bugs obvious.
    let mut debug = [0u16; 6];
    for (i, d) in debug.iter_mut().enumerate() {
        *d = b.tex(&format!("debug_{i}"), move |x, y, _| {
            if x < 3 && y < 3 {
                return [1.0; 3];
            }
            [x as f32 / 15.0, y as f32 / 15.0, i as f32 / 5.0]
        });
    }
    let ore = b.tex("ore", |_, _, h| if unit(h) < 0.12 { [0.9, 0.75, 0.2] } else { mul([0.50, 0.50, 0.52], 0.85 + unit(h >> 8) * 0.3) });
    let glow = b.tex("glow", |x, y, h| {
        let c = (x as i32 - 8).abs().max((y as i32 - 8).abs()) as f32 / 8.0;
        mul([1.0, 0.85, 0.45], 1.0 - c * 0.3 + unit(h) * 0.05)
    });

    b.cube("stone", stone);
    b.cube("dirt", dirt);
    b.block("grass", [grass_side, grass_side, grass_top, dirt, grass_side, grass_side]);
    b.cube("sand", sand);
    b.block("log", [bark, bark, log_top, log_top, bark, bark]);
    b.cube("leaves", leaves);
    b.cube("planks", planks);
    b.cube("bricks", bricks);
    b.cube("snow", snow);
    b.cube("gravel", gravel);
    b.cube("cobble", cobble);
    b.cube("white", white);
    b.cube("red", red);
    b.cube("green", green);
    b.cube("blue", blue);
    b.block("debug", debug);
    b.cube("ore", ore);
    b.cube("glow", glow);

    // Glass: a nearly clear pane with a denser frame; stained panes of even alpha; a grate of
    // bars and holes (cutout).
    let glass = b.tex_alpha("glass", |x, y, h| {
        let frame = x == 0 || y == 0 || x == TEX_SIZE - 1 || y == TEX_SIZE - 1;
        let streak = (x + y) % 7 == 0;
        if frame { ([0.80, 0.86, 0.90], 0.85) } else { (mul([0.85, 0.93, 0.97], 0.97 + unit(h) * 0.06), if streak { 0.35 } else { 0.18 }) }
    });
    let stained = |c: Rgb| move |_: usize, _: usize, h: u64| (mul(c, 0.95 + unit(h) * 0.1), 0.55);
    let glass_red = b.tex_alpha("glass_red", stained([0.85, 0.15, 0.12]));
    let glass_green = b.tex_alpha("glass_green", stained([0.15, 0.75, 0.20]));
    let glass_blue = b.tex_alpha("glass_blue", stained([0.15, 0.30, 0.90]));
    let grate = b.tex_alpha("grate", |x, y, h| {
        let bar = x % 4 == 0 || y % 4 == 0;
        (mul([0.42, 0.44, 0.47], 0.85 + unit(h) * 0.3), if bar { 1.0 } else { 0.0 })
    });
    b.see_through("glass", glass, BlockClass::Translucent);
    b.see_through("glass_red", glass_red, BlockClass::Translucent);
    b.see_through("glass_green", glass_green, BlockClass::Translucent);
    b.see_through("glass_blue", glass_blue, BlockClass::Translucent);
    b.see_through("grate", grate, BlockClass::Cutout);
    b.reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match() {
        let r = default_registry();
        assert_eq!(r.find("stone"), Some(blocks::STONE));
        assert_eq!(r.find("debug"), Some(blocks::DEBUG));
        assert_eq!(r.find("glow"), Some(blocks::GLOW));
        assert_eq!(r.find("glass"), Some(blocks::GLASS));
        assert_eq!(r.find("grate"), Some(blocks::GRATE));
    }

    /// Opaque blocks keep alpha 255 and no coverage; see-through ones have coverage, the
    /// cutout one only 0 or 255.
    #[test]
    fn classes_and_coverage() {
        let r = default_registry();
        for (i, b) in r.blocks.iter().enumerate().skip(1) {
            let id = BlockId(i as u16);
            for f in b.faces {
                let t = &r.textures[f as usize];
                assert!(t.texels.iter().all(|p| p[3] == 255), "{}", t.name);
                assert_eq!(t.alpha.is_empty(), b.class == BlockClass::Opaque, "{}", t.name);
                if b.class == BlockClass::Cutout {
                    assert!(t.alpha.iter().all(|&a| a == 0 || a == 255), "{}", t.name);
                }
            }
            assert_eq!(r.see_through(id), id.0 >= blocks::GLASS.0);
        }
        assert!(!r.see_through(blocks::AIR));
    }
}
