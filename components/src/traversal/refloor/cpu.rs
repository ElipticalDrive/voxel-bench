//! CPU implementation of the `refloor` visibility component: the storage's probe returns the
//! largest empty aligned cube, the walk steps out of it and re-floors (packets of N rays in
//! lockstep).

use glam::{IVec3, Vec3};

use crate::storage::cpu::{Grid, Repr, ffloor};
use crate::traversal::cpu::Hit;

/// The kernel's declaration: `trace_packet` returns its lockstep iterations (the `steps` view).
pub struct Refloor;

impl crate::traversal::cpu::Kernel for Refloor {
    const NAME: &'static str = "refloor";
    const CAPS: &'static [crate::Cap] = &[crate::Cap::Steps];
}

/// Traces `N` rays in lockstep from the shared origin `o` (relative to `cam`).
/// Lanes with `d.x == NaN` are inactive. Each lane ignores the start cell (like the
/// reference) and stops at `tfar`.
#[inline(always)]
pub fn trace_packet<R: Repr, const N: usize>(grid: &Grid<R>, cam: IVec3, o: Vec3, dx: &[f32; N], dy: &[f32; N], dz: &[f32; N], max_t: f32, bmin: IVec3, bmax: IVec3, out: &mut [Option<Hit>; N]) -> u32 {
    let mut iterations = 0u32;
    let mut inv = [[0f32; N]; 3];
    let mut stp = [[0i32; N]; 3];
    let mut cell = [[0i32; N]; 3];
    let mut t = [0f32; N];
    let mut tfar = [0f32; N];
    let mut axis = [3u8; N];
    let mut active = [false; N];
    let start = o.floor().as_ivec3();
    let (bminf, bmaxf) = (bmin.as_vec3(), bmax.as_vec3());
    for l in 0..N {
        out[l] = None;
        if dx[l].is_nan() {
            continue;
        }
        let d = [dx[l], dy[l], dz[l]];
        let mut tlo = [0f32; 3];
        let mut thi = [0f32; 3];
        for a in 0..3 {
            let da = if d[a].abs() < 1e-20 { 1e-20 } else { d[a] };
            inv[a][l] = 1.0 / da;
            stp[a][l] = if da > 0.0 { 1 } else { -1 };
            let t0 = (bminf[a] - o[a]) * inv[a][l];
            let t1 = (bmaxf[a] - o[a]) * inv[a][l];
            tlo[a] = t0.min(t1);
            thi[a] = t0.max(t1);
        }
        let tnear = tlo[0].max(tlo[1]).max(tlo[2]).max(0.0);
        let tf = thi[0].min(thi[1]).min(thi[2]).min(max_t);
        if tnear > tf {
            continue;
        }
        for a in 0..3 {
            cell[a][l] = ffloor(o[a] + d[a] * tnear).clamp(bmin[a], bmax[a] - 1);
        }
        if tnear > 0.0 {
            axis[l] = if tlo[0] >= tlo[1] && tlo[0] >= tlo[2] { 0 } else if tlo[1] >= tlo[2] { 1 } else { 2 };
        }
        t[l] = tnear;
        tfar[l] = tf;
        active[l] = true;
    }
    let mut size = [1i32; N];
    // Per-lane cache of the current chunk (most steps stay inside it).
    let mut cached_chunk = [IVec3::splat(i32::MIN); N];
    let mut cached_idx = [0u32; N];
    for _ in 0..4096 {
        // Lookups (gathers): scalar per lane.
        let mut any = false;
        for l in 0..N {
            if !active[l] {
                continue;
            }
            let c = IVec3::new(cell[0][l], cell[1][l], cell[2][l]);
            let abs = c + cam;
            let ch = abs >> 5;
            if ch != cached_chunk[l] {
                cached_chunk[l] = ch;
                cached_idx[l] = grid.chunk(abs);
            }
            let (b, s) = grid.probe_in(cached_idx[l], abs);
            if b != 0 && c != start {
                out[l] = Some(Hit { t: t[l], cell: c, axis: axis[l] });
                active[l] = false;
                continue;
            }
            size[l] = s;
            any = true;
        }
        if !any {
            break;
        }
        iterations += 1;
        // Stepping: branch-free arithmetic over all lanes (vectorises).
        for l in 0..N {
            let s = size[l];
            let mut tb = [0f32; 3];
            let mut base = [0i32; 3];
            let mut bound = [0i32; 3];
            for a in 0..3 {
                let abs = cell[a][l] + cam[a];
                base[a] = (abs & -s) - cam[a];
                bound[a] = base[a] + if stp[a][l] > 0 { s } else { 0 };
                tb[a] = (bound[a] as f32 - o[a]) * inv[a][l];
            }
            let a = if tb[0] <= tb[1] && tb[0] <= tb[2] { 0 } else if tb[1] <= tb[2] { 1 } else { 2 };
            let tn = tb[a];
            let d = [dx[l], dy[l], dz[l]];
            for k in 0..3 {
                cell[k][l] = if k == a {
                    if stp[k][l] > 0 { bound[k] } else { base[k] - 1 }
                } else if s == 1 {
                    // Voxel step: only the crossing axis moves.
                    cell[k][l]
                } else {
                    ffloor(o[k] + d[k] * tn).clamp(base[k], base[k] + s - 1)
                };
            }
            t[l] = tn;
            axis[l] = a as u8;
            if tn > tfar[l] {
                active[l] = false;
            }
        }
    }
    iterations
}
