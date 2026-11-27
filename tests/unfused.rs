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
