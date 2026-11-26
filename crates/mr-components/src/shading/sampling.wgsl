// Sampling for the stateful shadings (`pathtrace`, `ao`; put before their source by
// `shading::gpu_wgsl`): a per-pixel PCG sequence, cosine-weighted directions, and the
// signature of a pixel's primary surface that their accumulation (`accum`) keys on.

var<private> rng_state: u32;

fn rng_pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn rng_rand() -> f32 {
    rng_state = rng_pcg(rng_state);
    return f32(rng_state >> 8u) * (1.0 / 16777216.0);
}

fn rng_cosine_dir(n: vec3<f32>) -> vec3<f32> {
    let u1 = rng_rand();
    let u2 = rng_rand();
    let r = sqrt(u1);
    let phi = 6.2831853 * u2;
    let a = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.5);
    let t = normalize(cross(n, a));
    let b = cross(n, t);
    return normalize(t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt(max(0.0, 1.0 - u1)));
}

const ACC_COUNT_MASK: u32 = 0xfffu;
const ACC_SIG_MASK: u32 = 0xfffff000u;

// The primary surface of a pixel, hashed: the cell behind the point and the normal.
fn surface_sig(p: vec3<f32>, n: vec3<f32>) -> u32 {
    let c = bitcast<vec3<u32>>(vec3<i32>(floor(p - n * 0.5)));
    let q = bitcast<vec3<u32>>(vec3<i32>(round(n * 8.0)));
    return rng_pcg(c.x ^ rng_pcg(c.y ^ rng_pcg(c.z ^ rng_pcg(q.x ^ (q.y << 8u) ^ (q.z << 16u))))) & ACC_SIG_MASK;
}
