//! CPU implementation of the `incremental` visibility component (IDEAS §1): hierarchical DDA
//! with per-level exit times, moving between levels incrementally.

use glam::{IVec3, Vec3};

use crate::storage::cpu::{Entry, Grid, Repr, ffloor};
use crate::traversal::cpu::{Hit, Tally};

/// Next level above `level` in the level mask `mask` (5 = chunk is always present).
#[inline(always)]
pub fn level_above(mask: u32, level: u32) -> u32 {
    (mask >> (level + 1)).trailing_zeros() + level + 1
}

/// Next level below `level` in `mask` (0 is always present).
#[inline(always)]
pub fn level_below(mask: u32, level: u32) -> u32 {
    31 - (mask & ((1 << level) - 1)).leading_zeros()
}

/// Incremental-level hierarchical DDA (IDEAS §1, the user's intended traversal). The ray
/// keeps a cell `c` at the current level (absolute coordinates `>> level`) with per-axis
/// `tmax` (exit times) and `tdelta` (cell size in t). Every move is incremental, with no
/// re-flooring:
/// * **step** at the current level: classic DDA, one add;
/// * **ascend** one level: `tmax` gains `tdelta` on the axes where the cell was the first
///   child in the ray's direction, and `tdelta` doubles;
/// * **descend** one level: `tdelta` halves, the midplane exit is `tmax - tdelta`, and the
///   child is the second one along an axis exactly when the entry time has passed it.
///
/// Ascents are tried only after a step crosses the next level's boundary (otherwise the
/// parent is the one already known to be occupied). Levels a representation can't test
/// (`!R::LEVELS`) are skipped over in one go. Returns the hit and the occupancy tests made.
#[inline(always)]
pub fn trace_incremental<R: Repr, const EXACT: bool>(grid: &Grid<R>, levels: u32, cam: IVec3, o: Vec3, d: Vec3, max_t: f32, bmin: IVec3, bmax: IVec3) -> (Option<Hit>, Tally) {
    const TOP: u32 = 5;
    let mask = R::LEVELS & levels | 1 | 1 << TOP;
    let (bminf, bmaxf) = (bmin.as_vec3(), bmax.as_vec3());
    let mut inv = [0f32; 3];
    let mut stp = [0i32; 3];
    let mut tlo = [0f32; 3];
    let mut thi = [0f32; 3];
    for a in 0..3 {
        let da = if d[a].abs() < 1e-20 { 1e-20 } else { d[a] };
        inv[a] = 1.0 / da;
        stp[a] = if da > 0.0 { 1 } else { -1 };
        let t0 = (bminf[a] - o[a]) * inv[a];
        let t1 = (bmaxf[a] - o[a]) * inv[a];
        tlo[a] = t0.min(t1);
        thi[a] = t0.max(t1);
    }
    let mut t = tlo[0].max(tlo[1]).max(tlo[2]).max(0.0);
    let tfar = thi[0].min(thi[1]).min(thi[2]).min(max_t);
    if t > tfar {
        return (None, Tally::default());
    }
    let mut axis = 3u8;
    if t > 0.0 {
        axis = if tlo[0] >= tlo[1] && tlo[0] >= tlo[2] { 0 } else if tlo[1] >= tlo[2] { 1 } else { 2 };
    }
    let start = o.floor().as_ivec3() + cam;
    let mut c = [0i32; 3];
    let mut tmax = [0f32; 3];
    let mut tdelta = [0f32; 3];
    for a in 0..3 {
        let rel = ffloor(o[a] + d[a] * t).clamp(bmin[a], bmax[a] - 1);
        c[a] = rel + cam[a];
        tmax[a] = ((rel + (stp[a] > 0) as i32) as f32 - o[a]) * inv[a];
        tdelta[a] = inv[a].abs();
    }
    let mut level = 0u32;
    let mut may_ascend = true;
    let mut tally = Tally::default();
    // Exit time along axis `b` of the level-`level` cell `cb`, from integers (no drift).
    let exit = |b: usize, cb: i32, level: u32| ((((cb + (stp[b] > 0) as i32) << level) - cam[b]) as f32 - o[b]) * inv[b];
    let mut cached_chunk = IVec3::splat(i32::MIN);
    let mut cached_idx = 0u32;
    let mut occupied = |level: u32, c: [i32; 3]| -> bool {
        let shift = (TOP - level) as i32;
        let cv = IVec3::from_array(c);
        let ch = cv >> shift;
        if ch != cached_chunk {
            cached_chunk = ch;
            cached_idx = grid.chunk_at(ch);
        }
        match cached_idx {
            0 => false,
            idx => match &grid.entries[idx as usize - 1] {
                Entry::Uniform(_) => true,
                Entry::Dense(_) if level == TOP => true,
                Entry::Dense(r) => r.occupied(level, cv & ((1 << shift) - 1)),
            },
        }
    };
    for _ in 0..4096 {
        if occupied(level, c) {
            if level > 0 {
                // Descend to the next level the representation can test.
                let below = level_below(mask, level);
                while level > below {
                    for b in 0..3 {
                        tdelta[b] *= 0.5;
                        // Exit of the first child in the ray's direction (the midplane).
                        let tmid = if EXACT { exit(b, c[b] * 2 + (stp[b] < 0) as i32, level - 1) } else { tmax[b] - tdelta[b] };
                        let second = t >= tmid;
                        if !second {
                            tmax[b] = tmid;
                        }
                        c[b] = c[b] * 2 + (second ^ (stp[b] < 0)) as i32;
                    }
                    level -= 1;
                }
                tally.down += 1;
                may_ascend = false;
                continue;
            }
            if c != start.to_array() {
                return (Some(Hit { t, cell: IVec3::from_array(c) - cam, axis }), tally);
            }
        } else {
            // Climb while the parent is empty, then step at the highest empty level.
            while may_ascend && level < TOP {
                let above = level_above(mask, level);
                let k = (above - level) as i32;
                tally.up += 1;
                if occupied(above, [c[0] >> k, c[1] >> k, c[2] >> k]) {
                    break;
                }
                while level < above {
                    for b in 0..3 {
                        let first = (c[b] & 1 == 0) == (stp[b] > 0);
                        if first {
                            tmax[b] += tdelta[b];
                        }
                        tdelta[b] *= 2.0;
                        c[b] >>= 1;
                    }
                    level += 1;
                }
                if EXACT {
                    for b in 0..3 {
                        tmax[b] = exit(b, c[b], level);
                    }
                }
            }
        }
        // Step at the current level.
        let a = if tmax[0] <= tmax[1] && tmax[0] <= tmax[2] { 0 } else if tmax[1] <= tmax[2] { 1 } else { 2 };
        t = tmax[a];
        if t > tfar {
            return (None, tally);
        }
        tally.steps += 1;
        let old = c[a];
        c[a] += stp[a];
        tmax[a] = if EXACT { exit(a, c[a], level) } else { tmax[a] + tdelta[a] };
        axis = a as u8;
        let k = (level_above(mask, level).min(TOP + 1) - level) as i32;
        may_ascend = level < TOP && (old >> k) != (c[a] >> k);
    }
    (None, tally)
}
