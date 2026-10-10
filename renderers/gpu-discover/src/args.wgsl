// gpu-discover: single-thread passes turning GPU counters into indirect dispatch/draw args.

struct Level { level: u32, size: u32, in_off: u32, out_off: u32, parity: u32, cap: u32, list_cap: u32, _p: u32 }

@group(0) @binding(0) var<uniform> lv: Level;
@group(0) @binding(1) var<storage, read> st: array<u32>;
@group(0) @binding(2) var<storage, read_write> args: array<u32>;
@group(0) @binding(3) var<storage, read> lcount: array<u32>;

fn put_wg(at: u32, n: u32) {
    let g = (n + 255u) / 256u;
    args[at] = min(g, 32768u);
    args[at + 1u] = max(1u, (g + 32767u) / 32768u);
    args[at + 2u] = 1u;
}

// Args for level `lv.level`: rays at [L*8], classify at [L*8+4].
@compute @workgroup_size(1)
fn level_args() {
    let l = lv.level;
    put_wg(l * 8u, min(st[8u + l], lv.cap));
    put_wg(l * 8u + 4u, min(st[l], lv.cap));
}

// Draw args for this frame's face list and next frame's survivor dispatch.
@compute @workgroup_size(1)
fn finalize() {
    let n = min(lcount[lv.parity], lv.list_cap);
    put_wg(64u + lv.parity * 4u, n);
    let d = 72u + lv.parity * 4u;
    args[d] = 4u;
    args[d + 1u] = n;
    args[d + 2u] = 0u;
    args[d + 3u] = 0u;
}

// Footprint jobs of level `lv.level`: one workgroup per job, at [80 + L*4].
@compute @workgroup_size(1)
fn job_args() {
    let n = min(st[48u + lv.level], 262144u);
    let at = 80u + lv.level * 4u;
    args[at] = min(n, 32768u);
    args[at + 1u] = max(1u, (n + 32767u) / 32768u);
    args[at + 2u] = 1u;
}

// Draw args for the merged quad list (count in lcount[2]), at [108].
@compute @workgroup_size(1)
fn merge_args() {
    args[108] = 4u;
    args[109] = min(lcount[2], lv.list_cap);
    args[110] = 0u;
    args[111] = 0u;
}
