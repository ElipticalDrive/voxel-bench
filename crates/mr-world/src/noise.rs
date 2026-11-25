//! Small deterministic hash and gradient noise, so generated worlds are bit-identical
//! across runs and machines.

#[inline]
pub fn hash(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

#[inline]
pub fn hash3(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    hash(seed ^ hash((x as u32 as u64) | ((y as u32 as u64) << 32)) ^ hash(z as u32 as u64 ^ 0x9e3779b97f4a7c15))
}

/// Uniform float in `[0, 1)` from a hash.
#[inline]
pub fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn grad2(h: u64, x: f32, y: f32) -> f32 {
    let a = unit(h) * std::f32::consts::TAU;
    a.cos() * x + a.sin() * y
}

#[inline]
fn grad3(h: u64, x: f32, y: f32, z: f32) -> f32 {
    match h % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// 2D Perlin noise, roughly in `[-1, 1]`.
pub fn perlin2(seed: u64, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (ix, iy) = (x0 as i32, y0 as i32);
    let g = |dx: i32, dy: i32| grad2(hash3(seed, ix + dx, iy + dy, 0), fx - dx as f32, fy - dy as f32);
    let (u, v) = (fade(fx), fade(fy));
    let a = g(0, 0) + u * (g(1, 0) - g(0, 0));
    let b = g(0, 1) + u * (g(1, 1) - g(0, 1));
    (a + v * (b - a)) * std::f32::consts::SQRT_2
}

/// 3D Perlin noise, roughly in `[-1, 1]`.
pub fn perlin3(seed: u64, x: f32, y: f32, z: f32) -> f32 {
    let (x0, y0, z0) = (x.floor(), y.floor(), z.floor());
    let (fx, fy, fz) = (x - x0, y - y0, z - z0);
    let (ix, iy, iz) = (x0 as i32, y0 as i32, z0 as i32);
    let g = |dx: i32, dy: i32, dz: i32| {
        grad3(hash3(seed, ix + dx, iy + dy, iz + dz), fx - dx as f32, fy - dy as f32, fz - dz as f32)
    };
    let (u, v, w) = (fade(fx), fade(fy), fade(fz));
    let l = |a: f32, b: f32, t: f32| a + t * (b - a);
    let x00 = l(g(0, 0, 0), g(1, 0, 0), u);
    let x10 = l(g(0, 1, 0), g(1, 1, 0), u);
    let x01 = l(g(0, 0, 1), g(1, 0, 1), u);
    let x11 = l(g(0, 1, 1), g(1, 1, 1), u);
    l(l(x00, x10, v), l(x01, x11, v), w)
}

pub fn fbm2(seed: u64, x: f32, y: f32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * perlin2(seed.wrapping_add(o as u64 * 7919), x * freq, y * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}
