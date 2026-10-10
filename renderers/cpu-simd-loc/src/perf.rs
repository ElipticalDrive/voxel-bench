//! Minimal per-thread hardware counters via `perf_event_open` (user space only, so it works
//! with `perf_event_paranoid = 2`). Each pool thread opens two counter groups in the rayon
//! start handler; every render task reads them before and after, so only time spent
//! rendering is counted (not rayon's idle spinning). The two groups are multiplexed by
//! the kernel (one PMC is taken by the NMI watchdog); deltas are scaled by
//! enabled/running time.

use std::cell::RefCell;

const TYPE_HARDWARE: u32 = 0;
const TYPE_HW_CACHE: u32 = 3;
const TYPE_RAW: u32 = 4;

/// Groups of (name, type, config). The raw ones are AMD Zen 4/5 `LsDmndFillsFromSys`
/// (PMC 0x43): where *demand* data-cache fills were served from (L2; L3 or another
/// core's cache; DRAM).
pub const GROUPS: &[&[(&str, u32, u64)]] = &[
    &[("cycles", TYPE_HARDWARE, 0), ("instructions", TYPE_HARDWARE, 1), ("branch_misses", TYPE_HARDWARE, 5), ("l1d_miss", TYPE_HW_CACHE, 0x10000)],
    &[("cycles_b", TYPE_HARDWARE, 0), ("fill_l2", TYPE_RAW, 0x0143), ("fill_l3", TYPE_RAW, 0x1643), ("fill_dram", TYPE_RAW, 0x4843)],
];

pub fn names() -> Vec<&'static str> {
    GROUPS.iter().flat_map(|g| g.iter().map(|e| e.0)).collect()
}

struct State {
    groups: Vec<i32>,
    start: Vec<Vec<u64>>,
    sums: Vec<f64>,
    /// Per group: summed time enabled / running while rendering (multiplexing scale).
    times: Vec<[f64; 2]>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn open(kind: u32, config: u64, group: i32) -> i32 {
    // perf_event_attr (PERF_ATTR_SIZE_VER5 = 112 bytes): type, size, config, ..., read_format, flags.
    let mut attr = [0u64; 14];
    attr[0] = kind as u64 | (112u64 << 32);
    attr[1] = config;
    attr[4] = 1 | 2 | 8; // read_format: TOTAL_TIME_ENABLED | TOTAL_TIME_RUNNING | GROUP
    attr[5] = (1 << 5) | (1 << 6); // exclude_kernel, exclude_hv (enabled from the start)
    // SAFETY: attr is a valid, zero-padded perf_event_attr of the declared size.
    unsafe { libc::syscall(libc::SYS_perf_event_open, attr.as_ptr(), 0, -1, group, 0) as i32 }
}

/// Opens this thread's counters (called from the pool's start handler).
pub fn open_thread() {
    let mut groups = Vec::new();
    for g in GROUPS {
        let leader = open(g[0].1, g[0].2, -1);
        for e in &g[1..] {
            if leader >= 0 && open(e.1, e.2, leader) < 0 {
                tracing::warn!("perf event {} unavailable", e.0);
            }
        }
        groups.push(leader);
    }
    let n = names().len();
    STATE.with(|s| *s.borrow_mut() = Some(State { groups: groups.clone(), start: vec![Vec::new(); groups.len()], sums: vec![0.0; n], times: vec![[0.0; 2]; GROUPS.len()] }));
}

/// [time_enabled, time_running, values...] of a group.
fn read_group(fd: i32, n: usize) -> Vec<u64> {
    let mut v = vec![0u64; 3 + n];
    if fd >= 0 {
        // SAFETY: fd is ours; buffer holds nr, enabled, running and n values.
        let got = unsafe { libc::read(fd, v.as_mut_ptr() as *mut libc::c_void, v.len() * 8) };
        if got != (v.len() * 8) as isize {
            v.iter_mut().for_each(|x| *x = 0);
        }
    }
    v.remove(0);
    v
}

pub fn task_begin() {
    STATE.with(|s| {
        if let Some(s) = &mut *s.borrow_mut() {
            for (i, g) in GROUPS.iter().enumerate() {
                s.start[i] = read_group(s.groups[i], g.len());
            }
        }
    });
}

pub fn task_end() {
    STATE.with(|s| {
        if let Some(s) = &mut *s.borrow_mut() {
            let mut k = 0;
            for (i, g) in GROUPS.iter().enumerate() {
                let now = read_group(s.groups[i], g.len());
                s.times[i][0] += now[0].wrapping_sub(s.start[i][0]) as f64;
                s.times[i][1] += now[1].wrapping_sub(s.start[i][1]) as f64;
                for j in 0..g.len() {
                    s.sums[k] += now[2 + j].wrapping_sub(s.start[i][2 + j]) as f64;
                    k += 1;
                }
            }
        }
    });
}

/// Returns and clears this thread's sums, scaled for multiplexing (zeros if counters are off).
pub fn take_thread() -> Vec<f64> {
    STATE.with(|s| match &mut *s.borrow_mut() {
        Some(s) => {
            let mut out = std::mem::replace(&mut s.sums, vec![0.0; names().len()]);
            let mut k = 0;
            for (i, g) in GROUPS.iter().enumerate() {
                let [en, run] = std::mem::take(&mut s.times[i]);
                let scale = if run > 0.0 { en / run } else { 0.0 };
                for _ in 0..g.len() {
                    out[k] *= scale;
                    k += 1;
                }
            }
            out
        }
        None => vec![0.0; names().len()],
    })
}
