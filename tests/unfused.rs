//! Graphs the CPU executor runs as passes no region fused before (G45's first unfused
//! shapes): a drawer reading the geometry's faces straight (it transforms them itself), a
//! culling over another's culled set, a face source's lookup storage kept as a pass, cubes
//! drawn as their faces, a shading without a shadow site, the column march's shadow rays over
//! a heightmap of their own. Each runs; where it computes the same image as a shipped graph,
//! its frames are that graph's, pixel for pixel.

use std::path::{Path, PathBuf};
use std::process::Command;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("unfused")
}

/// Runs a one-renderer plan of `edits` on `graph` with the camera orbiting, saving every
/// frame: its success and log.
fn launch(name: &str, graph: &str, edits: &str) -> (bool, String) {
    let d = dir().join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
generator = {{ type = "terrain", seed = 3 }}
camera = {{ type = "orbit", center = [0, 30, 0], radius = 40, height = 20, period = 600 }}
[run]
frames = 4
warmup = 0
resolution = [48, 32]
modes = ["unlit", "lit"]
view_radius = 2
[[renderer]]
graph = "{graph}"
edits = [{edits}]
[output]
dir = {:?}
save = {{ every = 1 }}
compare = {{ every = 1 }}
"#,
        d.join("out").to_str().unwrap(),
    );
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer")).arg("run").arg(d.join("plan.toml")).env("RUST_LOG", "warn").output().unwrap();
    (o.status.success(), String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout))
}

/// The frames a run saved, per pass mode (by file name, in frame order).
fn images(name: &str, mode: &str) -> Vec<Vec<u8>> {
    fn all(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                all(&p, out);
            } else if p.extension().is_some_and(|e| e == "png") && !p.to_string_lossy().contains("diff") {
                out.push(p);
            }
        }
    }
    let mut paths = vec![];
    all(&dir().join(name).join("out"), &mut paths);
    paths.retain(|p| p.to_string_lossy().contains(&format!("-{mode}/")));
    paths.sort();
    paths.iter().map(|p| std::fs::read(p).unwrap()).collect()
}

/// Runs `graph` with `edits` (it must run) and, for each mode in `same`, compares its frames
/// with the plain graph's (`plain_edits` on it).
fn check(graph: &str, plain_edits: &str, edits: &str, same: &[&str]) {
    // Directories by the case (the tests run in parallel, each its own plain run).
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (graph, plain_edits, edits).hash(&mut h);
    let case = h.finish();
    let plain = format!("{graph}-{case:x}-plain");
    let (ok, log) = launch(&plain, graph, plain_edits);
    assert!(ok, "{graph} [{plain_edits}]: {log}");
    let name = format!("{graph}-{case:x}");
    let (ok, log) = launch(&name, graph, edits);
    assert!(ok, "{graph} [{edits}] does not run: {log}");
    for mode in ["unlit", "lit"] {
        let (p, u) = (images(&plain, mode), images(&name, mode));
        assert!(!u.is_empty() && p.len() == u.len(), "{graph} [{edits}] {mode}: {} and {} frames saved", p.len(), u.len());
        if same.contains(&mode) {
            assert!(p == u, "{graph} [{edits}] {mode}: the frames differ from the plain graph's");
        } else {
            // Where the image is not the same (no shadow rays, lit), it shows.
            assert!(p != u, "{graph} [{edits}] {mode}: the frames are the plain graph's");
        }
    }
    // The frames show surfaces (lit and unlit tell them apart; the sky alone would not).
    assert!(images(&name, "unlit")[0] != images(&name, "lit")[0], "{graph} [{edits}]: unlit and lit frames are the same (no surface drawn?)");
}

const BOTH: &[&str] = &["unlit", "lit"];
const DEFERRED: &str = r#"{ node = "shading", knobs = { placement = "deferred" } }"#;

#[test]
fn raster_drawer_reads_the_geometry_straight() {
    let straight = r#"{ node = "draw", input = "faces", from = "geometry" }, { remove = "visibility" }"#;
    check("raster-cpu", "", straight, BOTH);
    check("raster-cpu", DEFERRED, &format!("{straight}, {DEFERRED}"), BOTH);
}

#[test]
fn raster_culling_over_a_culled_set() {
    let twice = r#"{ add = "cull2", role = "visibility", component = "faces" }, { node = "cull2", input = "faces", from = "visibility" }, { node = "draw", input = "faces", from = "cull2" }"#;
    check("raster-cpu", "", twice, BOTH);
}

#[test]
fn raster_lookup_storage_kept_as_a_pass() {
    // The shadow rays' storage as the lookup, then a storage of its own (voxels, heights).
    check("raster-cpu", "", r#"{ node = "visibility", input = "storage", from = "storage" }"#, BOTH);
    for c in ["bits", "heightmap"] {
        check("raster-cpu", "", &format!(r#"{{ node = "visibility", input = "storage", from = {{ role = "storage", component = "{c}" }} }}, {{ node = "storage-2", input = "changes", from = "world" }}"#), BOTH);
    }
}

#[test]
fn raster_draws_cubes_as_their_faces() {
    // Every face of a surface block: the depth test leaves the exposed ones, as greedy's.
    check("raster-cpu", "", r#"{ node = "geometry", component = "cubes" }"#, BOTH);
}

#[test]
fn shading_without_a_shadow_site() {
    // Unlit frames need no shadow rays: the same.
    let raster = r#"{ node = "shading", unbind = "shadow" }, { remove = "kernel" }, { remove = "storage" }"#;
    check("raster-cpu", "", raster, &["unlit"]);
    check("raster-cpu", DEFERRED, &format!("{raster}, {DEFERRED}"), &["unlit"]);
    let columns = r#"{ node = "shading", unbind = "shadow" }, { remove = "kernel" }"#;
    check("columns-cpu", "", columns, &["unlit"]);
    check("columns-cpu", DEFERRED, &format!("{columns}, {DEFERRED}"), &["unlit"]);
}

#[test]
fn columns_shadow_rays_over_a_heightmap_of_their_own() {
    let own = r#"{ node = "kernel", input = "storage", from = { role = "storage", component = "heightmap" } }, { node = "storage-2", input = "changes", from = "world" }"#;
    check("columns-cpu", "", own, BOTH);
    check("columns-cpu", DEFERRED, &format!("{own}, {DEFERRED}"), BOTH);
}

#[test]
fn raster_draws_boxes_and_points_as_their_faces() {
    // The exposed block faces each stands for: naive's quads (culled), the same image.
    let plain = r#"{ node = "geometry", component = "naive" }"#;
    let straight = r#"{ node = "draw", input = "faces", from = "geometry" }, { remove = "visibility" }"#;
    for c in ["boxes", "points"] {
        check("raster-cpu", plain, &format!(r#"{{ node = "geometry", component = "{c}" }}"#), BOTH);
        check("raster-cpu", &format!("{plain}, {straight}"), &format!(r#"{{ node = "geometry", component = "{c}" }}, {straight}"#), BOTH);
    }
}

/// Run `name`'s score against the reference, over its passes (unlit, lit): the lowest median
/// PSNR and the highest median share of bad pixels (%), over the frames (the median: a run
/// with an edge timed `prev` draws a blank first frame, the hold's value before any).
fn score(name: &str) -> (f64, f64) {
    fn find(dir: &Path) -> Option<PathBuf> {
        std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find_map(|p| if p.is_dir() { find(&p) } else { (p.file_name()? == "report.json").then_some(p) })
    }
    let p = find(&dir().join(name).join("out")).expect("a report");
    let passes = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).unwrap()).unwrap();
    let (mut psnr, mut bad) = (f64::MAX, 0.0f64);
    for r in passes.as_array().unwrap().iter().filter(|r| !r["label"].as_str().unwrap_or("").ends_with("(ref)")) {
        assert!(r["error"].is_null(), "{name}: {}", r["error"]);
        let q = &r["quality"];
        psnr = psnr.min(q["psnr"]["p50"].as_f64().unwrap_or_else(|| panic!("{name}: no quality score in {r}")));
        bad = bad.max(q["bad_pct"]["p50"].as_f64().unwrap_or_else(|| panic!("{name}: no bad pixels in {q}")));
    }
    (psnr, bad)
}

/// CPU discovery (`quadtree`, `cluster`) drawn by the CPU rasteriser: in the drawer's frame
/// (faces `now`), a frame late (`prev`), through a culling node, deferred. Against the
/// reference, in the drawer's frame it scores as raster-cpu's meshed faces do (measured
/// 2026-11-27: both 99 dB, 0 % bad on this world: discovery found every visible face); a frame
/// late it draws the faces found from the last frame's camera from this one's, so what came
/// into view since is missing (measured: 19-21 dB, 3-5 % bad at the orbit's 0.4 blocks a frame).
#[test]
fn raster_draws_cpu_discovery() {
    let (ok, log) = launch("discovery-meshed", "raster-cpu", "");
    assert!(ok, "{log}");
    let (meshed, meshed_bad) = score("discovery-meshed");
    for c in ["quadtree", "cluster"] {
        let found = format!(r#"{{ add = "disc", role = "visibility", component = "{c}" }}, {{ node = "disc", input = "exposure", from = "storage" }}, {{ node = "disc", input = "rays", from = "kernel" }}"#);
        let drawn = r#"{ remove = "visibility" }, { remove = "geometry" }"#;
        // Each case's allowance against the meshed score: dB below it, points of bad pixels
        // above it.
        let cases = [
            ("now", format!(r#"{found}, {{ node = "draw", input = "faces", from = "disc" }}, {drawn}"#), (1.0, 0.5)),
            ("prev", format!(r#"{found}, {{ node = "draw", input = "faces", from = "disc" }}, {{ node = "draw", input = "faces", when = "prev" }}, {drawn}"#), (90.0, 8.0)),
            ("culled", format!(r#"{found}, {{ node = "visibility", input = "faces", from = "disc" }}, {{ remove = "geometry" }}"#), (1.0, 0.5)),
            ("deferred", format!(r#"{found}, {{ node = "draw", input = "faces", from = "disc" }}, {drawn}, {DEFERRED}"#), (1.0, 0.5)),
        ];
        for (how, edits, (db, pts)) in cases {
            let name = format!("discovery-{c}-{how}");
            let (ok, log) = launch(&name, "raster-cpu", &edits);
            assert!(ok, "{c} {how}: {log}");
            let (p, bad) = score(&name);
            println!("{c} {how}: {p:.1} dB, {bad:.2} % bad (meshed {meshed:.1} dB, {meshed_bad:.2} %)");
            assert!(p > meshed - db && bad < meshed_bad + pts, "{c} {how}: {p:.1} dB, {bad:.2} % bad; meshed {meshed:.1} dB, {meshed_bad:.2} %");
            assert!(images(&name, "unlit")[0] != images(&name, "lit")[0], "{c} {how}: no surface drawn");
        }
    }
}

/// The ray caster over a heightmap's march (`heightfield` as its primary kernel), run by the
/// columns region: one ray per pixel through the same heightmap. Against the reference it
/// scores at least as columns-cpu's march does, within `CAST_DB` and `CAST_BAD` points
/// (measured 2026-11-27, the worse of unlit and lit at 48x32: the caster 26.2 dB, 5.6 % bad;
/// the column march 17.4 dB, 34.6 %; why the march scores so low here is not looked into).
#[test]
fn raycast_over_a_heightfield() {
    let (ok, log) = launch("heightfield-columns", "columns-cpu", "");
    assert!(ok, "{log}");
    let (columns, columns_bad) = score("heightfield-columns");
    let cast = r#"{ add = "caster", role = "visibility", component = "raycast" }, { node = "caster", input = "primary", from = "kernel" }, { node = "shading", input = "hits", from = "caster" }, { remove = "visibility" }"#;
    for (how, edits) in [("inline", cast.to_string()), ("deferred", format!("{cast}, {DEFERRED}"))] {
        let name = format!("heightfield-{how}");
        let (ok, log) = launch(&name, "columns-cpu", &edits);
        assert!(ok, "{how}: {log}");
        let (p, bad) = score(&name);
        println!("raycast {how}: {p:.1} dB, {bad:.2} % bad (columns {columns:.1} dB, {columns_bad:.2} %)");
        assert!(p > columns - CAST_DB && bad < columns_bad + CAST_BAD, "{how}: {p:.1} dB, {bad:.2} % bad; columns {columns:.1} dB, {columns_bad:.2} %");
        assert!(images(&name, "unlit")[0] != images(&name, "lit")[0], "{how}: no surface drawn");
    }
}

const CAST_DB: f64 = 1.0;
const CAST_BAD: f64 = 1.0;
