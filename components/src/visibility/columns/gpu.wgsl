// Visibility `columns` on the GPU (the root of gpu-voxelspace's program): Comanche-style
// column march. One invocation per screen column marches front-to-back over the heightmap
// storage, drawing spans with a y-buffer; shadow and bounce rays walk the same heightmap
// (`hm_trace`). Shading placement (PLAN step 12c): inline in the march, or deferred (the
// march writes records, the common pass shades).

struct Camera {
    forward: vec3<f32>, tan_half_fov: f32,
    right: vec3<f32>, aspect: f32,
    up: vec3<f32>, mode: u32,
    origin: vec3<f32>, sky: u32,
    width: u32, height: u32, _p0: u32, max_cells: u32,
}

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;
// Bindings 2 and 3: the heightmap storage (`hm`, `hm_info`; storage::heightmap); 4 and 5:
// the shading state (`sp`, `accum`; shading::state).

fn row_of(p: vec3<f32>) -> f32 {
    let depth = max(dot(p, cam.forward), 1e-4);
    let sy = dot(p, cam.up) / (depth * cam.tan_half_fov);
    return (1.0 - sy) * 0.5 * f32(cam.height);
}
fn column_ray(x: f32, row: f32) -> vec3<f32> {
    return camera_ray(cam.forward, cam.right, cam.up, cam.tan_half_fov, cam.aspect, vec2<f32>(x, row), vec2<f32>(f32(cam.width), f32(cam.height)));
}

// The heightmap slope at column `cell`: lit tops shade with it instead of the face normal.
fn slope_normal(cell: vec2<i32>) -> vec3<f32> {
    let hx0 = hm_top(hm_at(cell - vec2<i32>(1, 0)));
    let hx1 = hm_top(hm_at(cell + vec2<i32>(1, 0)));
    let hz0 = hm_top(hm_at(cell - vec2<i32>(0, 1)));
    let hz1 = hm_top(hm_at(cell + vec2<i32>(0, 1)));
    let gx = clamp(select(0.0, hx0, hx0 > -1e5) - select(0.0, hx1, hx1 > -1e5), -2.0, 2.0);
    let gz = clamp(select(0.0, hz0, hz0 > -1e5) - select(0.0, hz1, hz1 > -1e5), -2.0, 2.0);
    return normalize(vec3<f32>(gx * 0.5, 2.0, gz * 0.5));
}

// Every pixel the march covers is shaded inline or, with shader def `DEFERRED`, written as
// a hit record (`deferred.wgsl`). One body either way: a function between the march and
// the shading (a hook per pixel) cost 2.5% of unlit GPU time on NVIDIA (PLAN step 12c).
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= cam.width) { return; }
    let x = f32(gid.x) + 0.5;
    let o = cam.origin;
    let dc = column_ray(x, f32(cam.height) * 0.5);
    var h = vec2<f32>(dc.x, dc.z);
    if (length(h) < 1e-5) { h = vec2<f32>(cam.up.x, cam.up.z) * -sign(cam.forward.y); }
    if (length(h) < 1e-5) { h = vec2<f32>(1.0, 0.0); }
    h = normalize(h);
    var ybuf = i32(cam.height);
    var cell = vec2<i32>(floor(o.xz));
    let stp = vec2<i32>(select(-1, 1, h.x > 0.0), select(-1, 1, h.y > 0.0));
    let inv = 1.0 / max(abs(h), vec2<f32>(1e-9));
    var tmax = (select(vec2<f32>(cell), vec2<f32>(cell + 1), h > vec2<f32>(0.0)) - o.xz) * sign(h) * inv;
    tmax = max(tmax, vec2<f32>(0.0));
    var t0 = 0.0;
    var axis = 0u; // last stepped axis in xz (0 = x, 1 = z), invalid for the first cell
    var first = true;
    for (var i = 0u; i < cam.max_cells && ybuf > 0; i++) {
        let t1 = min(tmax.x, tmax.y);
        let e = hm_at(cell);
        let H = hm_top(e);
        if (H > -1e5) {
            let p0 = vec3<f32>(o.x + h.x * t0, H, o.z + h.y * t0);
            let p1 = vec3<f32>(o.x + h.x * t1, H, o.z + h.y * t1);
            let r = min(row_of(p0), row_of(p1));
            let rtop = max(i32(floor(r)), 0);
            let block = e >> 16u;
            for (var row = rtop; row < ybuf; row++) {
                let d = column_ray(x, f32(row) + 0.5);
                var face = 2u;
                var t = 0.0;
                var side = false;
                if (!first) {
                    let a = select(0u, 2u, axis == 1u);
                    let wall = f32(select(cell[axis], cell[axis] + 1, stp[axis] < 0));
                    if (abs(d[a]) > 1e-6) {
                        let tw = (wall - o[a]) / d[a];
                        if (tw > 0.0 && o.y + d.y * tw <= H) { side = true; t = tw; face = face_entered(a, d[a] > 0.0); }
                    }
                }
                if (!side) {
                    if (d.y < -1e-6) { t = (H - o.y) / d.y; } else { t = t0 / max(length(d.xz), 1e-4); }
                }
                let p = o + d * t;
                let local = clamp(p - vec3<f32>(f32(cell.x), H - 1.0, f32(cell.y)), vec3<f32>(0.0), vec3<f32>(0.999));
#ifdef DEFERRED
                records[u32(row) * cam.width + gid.x] = pack_hit(t, Surface(p, face_normal(face), local, block, face));
#else
                // Lit and unlit call the shading separately (a constant flag; see `raster`).
                var color: vec4<f32>;
                if (cam.mode == 1u) {
                    var n = face_normal(face);
                    if (!side) { n = slope_normal(cell); }
                    color = shade_surface(Surface(p, n, local, block, face), d, true, vec2<u32>(gid.x, u32(row)));
                } else {
                    color = shade_surface(Surface(p, face_normal(face), local, block, face), d, false, vec2<u32>(gid.x, u32(row)));
                }
                textureStore(out_tex, vec2<i32>(i32(gid.x), row), color);
#endif
            }
            ybuf = min(ybuf, rtop);
        }
        first = false;
        t0 = t1;
        if (tmax.x < tmax.y) { cell.x += stp.x; tmax.x += inv.x; axis = 0u; } else { cell.y += stp.y; tmax.y += inv.y; axis = 1u; }
    }
#ifdef DEFERRED
    for (var row = 0; row < ybuf; row++) { records[u32(row) * cam.width + gid.x] = vec2<u32>(0u); }
#else
    let sky = unpack4x8unorm(cam.sky);
    for (var row = 0; row < ybuf; row++) { textureStore(out_tex, vec2<i32>(i32(gid.x), row), sky); }
#endif
}
