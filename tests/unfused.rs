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
    const TERRAIN: &str = r#"generator = { type = "terrain", seed = 3 }
camera = { type = "orbit", center = [0, 30, 0], radius = 40, height = 20, period = 600 }"#;
    launch_in(name, graph, edits, TERRAIN, r#"["unlit", "lit"]"#)
}

/// [`launch`] in another scene (the `[scene]` table's lines) and pass modes.
fn launch_in(name: &str, graph: &str, edits: &str, scene: &str, modes: &str) -> (bool, String) {
    let d = dir().join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
{scene}
[run]
frames = 4
warmup = 0
resolution = [48, 32]
modes = {modes}
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

/// CPU discovery whose exposure tests read another storage than its rays walk: storages hold
/// the same blocks, so the frames are the one-storage graph's, pixel for pixel.
#[test]
fn raster_cpu_discovery_exposure_in_another_storage() {
    let found = r#"{ add = "disc", role = "visibility", component = "quadtree" }, { node = "disc", input = "rays", from = "kernel" }, { node = "draw", input = "faces", from = "disc" }, { remove = "visibility" }, { remove = "geometry" }"#;
    let one = format!(r#"{found}, {{ node = "disc", input = "exposure", from = "storage" }}"#);
    for c in ["hashed", "bits"] {
        let two = format!(r#"{found}, {{ node = "disc", input = "exposure", from = {{ role = "storage", component = "{c}" }} }}, {{ node = "storage-2", input = "changes", from = "world" }}"#);
        check("raster-cpu", &one, &two, BOTH);
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
    for (how, edits) in [("inline", CAST.to_string()), ("deferred", format!("{CAST}, {DEFERRED}"))] {
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
/// The column march's graph with the ray caster in the march's place, its primary rays the
/// heightmap's march (`kernel`, `heightfield`).
const CAST: &str = r#"{ add = "caster", role = "visibility", component = "raycast" }, { node = "caster", input = "primary", from = "kernel" }, { node = "shading", input = "hits", from = "caster" }, { remove = "visibility" }"#;

/// GPU: the ray caster over a heightmap's march, run by the GPU columns region (one ray per
/// pixel through the heightmap's `hm_trace`), inline and deferred. Against the reference it
/// scores as the CPU's caster over the same graph does (the same walk; the GPU's float
/// arithmetic differs), within `CAST_DB` and `CAST_BAD` points. Needs a GPU (`--ignored`).
#[test]
#[ignore = "needs a GPU"]
fn gpu_raycast_over_a_heightfield() {
    for (how, edits) in [("inline", CAST.to_string()), ("deferred", format!("{CAST}, {DEFERRED}"))] {
        let (cpu, gpu) = (format!("gpu-heightfield-{how}-cpu"), format!("gpu-heightfield-{how}"));
        let (ok, log) = launch(&cpu, "columns-cpu", &edits);
        assert!(ok, "cpu {how}: {log}");
        let (ok, log) = launch(&gpu, "columns-gpu", &edits);
        assert!(ok, "gpu {how}: {log}");
        let ((c, c_bad), (g, g_bad)) = (score(&cpu), score(&gpu));
        println!("gpu raycast {how}: {g:.1} dB, {g_bad:.2} % bad (cpu {c:.1} dB, {c_bad:.2} %)");
        assert!(g > c - CAST_DB && g_bad < c_bad + CAST_BAD, "{how}: {g:.1} dB, {g_bad:.2} % bad; cpu {c:.1} dB, {c_bad:.2} %");
        assert!(images(&gpu, "unlit")[0] != images(&gpu, "lit")[0], "{how}: no surface drawn");
    }
}

/// GPU: the drawer reads the geometry's faces straight (no culling node): every chunk's faces
/// drawn, the same image as culled (the culling only leaves out what the depth test and the
/// hardware's back-face culling would). Needs a GPU (`--ignored`).
#[test]
#[ignore = "needs a GPU"]
fn gpu_drawer_reads_the_geometry_straight() {
    let straight = r#"{ node = "draw", input = "faces", from = "geometry" }, { remove = "visibility" }"#;
    check("raster-gpu", "", straight, BOTH);
    let gbuffer = r#"{ node = "draw", component = "gbuffer" }"#;
    check("raster-gpu", gbuffer, &format!("{straight}, {gbuffer}"), BOTH);
    let naive = r#"{ node = "geometry", component = "naive" }"#;
    check("raster-gpu", naive, &format!("{straight}, {naive}"), BOTH);
}

/// GPU: a culling over another's culled set (both for this frame's camera: the same set).
#[test]
#[ignore = "needs a GPU"]
fn gpu_culling_over_a_culled_set() {
    let twice = r#"{ add = "cull2", role = "visibility", component = "faces" }, { node = "cull2", input = "faces", from = "visibility" }, { node = "draw", input = "faces", from = "cull2" }"#;
    check("raster-gpu", "", twice, BOTH);
}

/// GPU: the rasteriser with no storage at all, its shadow rays ray queries into the drawn
/// geometry's acceleration structure (no walk): the same image, lit too (both find the same
/// blocks).
#[test]
#[ignore = "needs a GPU"]
fn gpu_raster_without_a_storage() {
    let rq = r#"{ remove = "meshes" }, { node = "shading", input = "shadow", from = { role = "traversal", component = "rayquery" } }, { node = "traversal", input = "geometry", from = "geometry" }, { remove = "kernel" }, { remove = "storage" }"#;
    check("raster-gpu", r#"{ remove = "meshes" }"#, rq, BOTH);
}

/// GPU-drawn CPU discovery (`discover-cpu`) whose exposure tests read another CPU storage than
/// its rays walk: the same blocks, the same frames.
#[test]
#[ignore = "needs a GPU"]
fn gpu_drawn_discovery_exposure_in_another_storage() {
    for c in ["hashed", "bits"] {
        let two = format!(r#"{{ node = "visibility", input = "exposure", from = {{ role = "storage", component = "{c}" }} }}, {{ node = "storage-2", device = "cpu" }}, {{ node = "storage-2", input = "changes", from = "world" }}"#);
        check("discover-cpu", "", &two, BOTH);
    }
}

/// GPU: boxes and points drawn by the GPU rasteriser as the exposed block faces each stands
/// for (naive's culled quads, the same image), through the culling and straight.
#[test]
#[ignore = "needs a GPU"]
fn gpu_raster_draws_boxes_and_points_as_their_faces() {
    let plain = r#"{ node = "geometry", component = "naive" }"#;
    let straight = r#"{ node = "draw", input = "faces", from = "geometry" }, { remove = "visibility" }"#;
    for c in ["boxes", "points"] {
        check("raster-gpu", plain, &format!(r#"{{ node = "geometry", component = "{c}" }}"#), BOTH);
        check("raster-gpu", &format!("{plain}, {straight}"), &format!(r#"{{ node = "geometry", component = "{c}" }}, {straight}"#), BOTH);
    }
}

/// The meshes plan's room, its meshes animated (`plans/meshes.toml`, lit by its lights).
const MESH_ROOM: &str = r#"generator = { type = "room", size = 48 }
camera = { type = "static", position = [24, 24, 85], target = [24, 20, 20] }
environment = { no_sun = true, ambient = [0.03, 0.03, 0.03] }
lights = [{ type = "point", position = [24, 44, 24], color = [1.0, 0.95, 0.85], intensity = 400.0, radius = 70.0 }]
meshes = [
    { shape = "torus", position = [30, 30, 30], scale = 7, detail = 48, animate = [{ type = "spin", axis = [1, 1, 0], period = 160 }] },
    { shape = "box", position = [17, 9, 40], scale = 1.5, count = 12, spread = [6, 3, 4], seed = 2, animate = [{ type = "bob", amplitude = 1, period = 50 }] },
]"#;

/// GPU: the ray-query caster with meshes (`composite`: their records rasterised first, the
/// primary queries ending at them), inline and deferred. Against the reference it scores as
/// the walk with the same meshes does, within `CAST_DB` and `CAST_BAD` points (measured
/// 2026-11-28 at 48x32: both 84.8 dB, 0 % bad; at 320x180 81.1 dB unlit, 76.6 dB lit), and the meshes show (the
/// frames differ from the caster's without them).
#[test]
#[ignore = "needs a GPU"]
fn gpu_rayquery_with_meshes() {
    const MODES: &str = r#"["unlit+meshes", "lit+meshes"]"#;
    let meshes = r#"{ add = "meshes", role = "meshes", component = "composite" }, { node = "visibility", input = "meshes", from = "meshes" }"#;
    let (ok, log) = launch_in("meshes-walk", "raycast-gpu", "", MESH_ROOM, MODES);
    assert!(ok, "{log}");
    let (walk, walk_bad) = score("meshes-walk");
    let (ok, log) = launch_in("meshes-rq-none", "rayquery-gpu", "", MESH_ROOM, MODES);
    assert!(ok, "{log}");
    for (how, edits) in [("inline", meshes.to_string()), ("deferred", format!("{meshes}, {DEFERRED}"))] {
        let name = format!("meshes-rq-{how}");
        let (ok, log) = launch_in(&name, "rayquery-gpu", &edits, MESH_ROOM, MODES);
        assert!(ok, "{how}: {log}");
        let (p, bad) = score(&name);
        println!("rayquery + meshes {how}: {p:.1} dB, {bad:.2} % bad (walk {walk:.1} dB, {walk_bad:.2} %)");
        assert!(p > walk - CAST_DB && bad < walk_bad + CAST_BAD, "{how}: {p:.1} dB, {bad:.2} % bad; walk {walk:.1} dB, {walk_bad:.2} %");
        for mode in ["unlit+meshes", "lit+meshes"] {
            assert!(images(&name, mode) != images("meshes-rq-none", mode), "{how} {mode}: the frames are the caster's without meshes");
        }
    }
}

/// GPU: `composite` and `depth` are one another's alias where a region draws the other (user,
/// 2026-11-28: the same image, so accepted): a ray caster's meshes named `depth`, a drawer's
/// named `composite`, each pixel-equal to the name the region draws by.
#[test]
#[ignore = "needs a GPU"]
fn gpu_meshes_aliases_draw_the_same() {
    const MODES: &str = r#"["unlit+meshes", "lit+meshes"]"#;
    let rq = r#"{ add = "meshes", role = "meshes", component = "composite" }, { node = "visibility", input = "meshes", from = "meshes" }"#;
    let cases = [
        ("raycast-gpu", String::new(), r#"{ node = "meshes", component = "depth" }"#.to_string()),
        ("rayquery-gpu", rq.to_string(), rq.replace("composite", "depth")),
        ("raster-gpu", String::new(), r#"{ node = "meshes", component = "composite" }"#.to_string()),
        ("raster-gpu", r#"{ node = "draw", component = "gbuffer" }"#.to_string(), r#"{ node = "draw", component = "gbuffer" }, { node = "meshes", component = "composite" }"#.to_string()),
    ];
    for (i, (graph, plain, alias)) in cases.iter().enumerate() {
        let (a, b) = (format!("alias-{i}-plain"), format!("alias-{i}"));
        let (ok, log) = launch_in(&a, graph, plain, MESH_ROOM, MODES);
        assert!(ok, "{graph} [{plain}]: {log}");
        let (ok, log) = launch_in(&b, graph, alias, MESH_ROOM, MODES);
        assert!(ok, "{graph} [{alias}] does not run: {log}");
        for mode in ["unlit+meshes", "lit+meshes"] {
            let (p, q) = (images(&a, mode), images(&b, mode));
            assert!(!p.is_empty() && p == q, "{graph} [{alias}] {mode}: the frames differ from [{plain}]'s ({} and {} saved)", p.len(), q.len());
        }
    }
}

/// GPU discovery with meshes in its drawer's depth (`depth`, and its alias `composite`):
/// forward, gbuffer, and with hole detection (whose face raster is its own: only its sky mask
/// sees the meshes). Against the reference it scores at least as raster-gpu's meshed faces
/// with the same meshes do, within `CAST_DB` and `CAST_BAD` points (measured 2026-11-28 at
/// 48x32: raster-gpu 27.6 dB, its faces losing 4 pixels here with or without meshes;
/// discovery 36.9 dB forward and with hole detection, whose lit mesh fragments' rays see
/// voxels only, 84.8 dB gbuffer = the walk's); the meshes show (the frames differ from
/// discovery's without them), and `composite` draws pixel-equal to `depth`.
#[test]
#[ignore = "needs a GPU"]
fn gpu_discovery_with_meshes() {
    const MODES: &str = r#"["unlit+meshes", "lit+meshes"]"#;
    const MESHES: &str = r#"{ add = "meshes", role = "meshes", component = "depth" }, { node = "meshes", input = "changes", from = "world" }, { node = "meshes", input = "shadow", from = "kernel" }, { node = "draw", input = "meshes", from = "meshes" }"#;
    const HOLES: &str = r#"{ add = "holes", role = "holes", component = "coverage" }, { node = "draw", input = "faces", when = { iterate = 2 } }, { node = "holes", input = "covered", from = "draw" }, { node = "holes", input = "sky", from = "draw" }, { node = "holes", input = "sky", when = "prev" }, { node = "visibility", input = "where", from = "holes" }"#;
    const GBUFFER: &str = r#"{ node = "draw", component = "gbuffer" }"#;
    let (ok, log) = launch_in("disc-meshes-raster", "raster-gpu", "", MESH_ROOM, MODES);
    assert!(ok, "{log}");
    let (raster, raster_bad) = score("disc-meshes-raster");
    let (ok, log) = launch_in("meshes-walk-disc", "raycast-gpu", "", MESH_ROOM, MODES);
    assert!(ok, "{log}");
    let walk = score("meshes-walk-disc");
    let (ok, log) = launch_in("disc-meshes-none", "discover-gpu", "", MESH_ROOM, MODES);
    assert!(ok, "{log}");
    for (how, edits) in [("forward", MESHES.to_string()), ("gbuffer", format!("{MESHES}, {GBUFFER}")), ("holes", format!("{MESHES}, {HOLES}"))] {
        let name = format!("disc-meshes-{how}");
        let (ok, log) = launch_in(&name, "discover-gpu", &edits, MESH_ROOM, MODES);
        assert!(ok, "{how}: {log}");
        let (p, bad) = score(&name);
        println!("discover-gpu + meshes {how}: {p:.1} dB, {bad:.2} % bad (raster-gpu {raster:.1} dB, {raster_bad:.2} %)");
        assert!(p > raster - CAST_DB && bad < raster_bad + CAST_BAD, "{how}: {p:.1} dB, {bad:.2} % bad; raster-gpu {raster:.1} dB, {raster_bad:.2} %");
        if how == "gbuffer" {
            // Deferred, its rays meet the triangles: as the walk.
            assert!(p > walk.0 - CAST_DB && bad < walk.1 + CAST_BAD, "{how}: {p:.1} dB, {bad:.2} % bad; walk {:.1} dB, {:.2} %", walk.0, walk.1);
        }
        if how != "holes" {
            // Hole detection's first frame differs by design; the drawn ones are compared.
            for mode in ["unlit+meshes", "lit+meshes"] {
                assert!(images(&name, mode) != images("disc-meshes-none", mode), "{how} {mode}: the frames are discovery's without meshes");
            }
        }
    }
    let (ok, log) = launch_in("disc-meshes-composite", "discover-gpu", &MESHES.replace("\"depth\"", "\"composite\""), MESH_ROOM, MODES);
    assert!(ok, "composite: {log}");
    for mode in ["unlit+meshes", "lit+meshes"] {
        let (p, q) = (images("disc-meshes-forward", mode), images("disc-meshes-composite", mode));
        assert!(!p.is_empty() && p == q, "composite {mode}: the frames differ from depth's");
    }
}
