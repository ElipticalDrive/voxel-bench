//! Edges timed `prev` in the CPU executor: the reader reads its producer's value of the last
//! frame (with that frame's camera where the value is seen from one), whatever the edge, and
//! the world's changes reach a node reading them `prev` a frame late. With the camera
//! moving, a frame of a graph whose hits (or screen triangles) arrive a frame late is the
//! frame before of the same graph without, pixel for pixel.

use std::path::{Path, PathBuf};
use std::process::Command;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("late")
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
frames = 12
warmup = 0
resolution = [48, 32]
modes = ["unlit", "lit"]
view_radius = 2
[[renderer]]
graph = "{graph}"
edits = {edits}
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

/// Each late frame is the plain run's frame before.
fn one_frame_behind(graph: &str, edit: &str) {
    let (plain, late) = (format!("{graph}-plain"), format!("{graph}-{}", edit.len()));
    let (ok, log) = launch(&plain, graph, r#"[{ node = "shading", knobs = { placement = "deferred" } }]"#);
    assert!(ok, "{log}");
    let (ok, log) = launch(&late, graph, &format!("[{edit}]"));
    assert!(ok, "{graph} with {edit}: {log}");
    for mode in ["unlit", "lit"] {
        let (p, l) = (images(&plain, mode), images(&late, mode));
        assert!(p.len() > 4 && p.len() == l.len(), "{graph} {mode}: {} and {} frames saved", p.len(), l.len());
        assert!(p[p.len() - 1] != p[p.len() - 2], "{graph} {mode}: the camera does not move visibly");
        for k in 2..p.len() {
            assert!(l[k] == p[k - 1], "{graph} {mode} with {edit}: frame {k} is not the plain frame {}", k - 1);
        }
    }
}

#[test]
fn hits_a_frame_late_show_the_frame_before() {
    // Placement inline: the shading reads its hits late, so it runs in a pass of its own.
    for graph in ["raycast-cpu", "columns-cpu"] {
        one_frame_behind(graph, r#"{ node = "shading", input = "hits", when = "prev" }"#);
    }
    one_frame_behind("raster-cpu", r#"{ node = "shading", input = "hits", when = "prev" }"#);
}

#[test]
fn screen_triangles_a_frame_late_show_the_frame_before() {
    one_frame_behind("raster-cpu", r#"{ node = "draw", input = "faces", when = "prev" }, { node = "shading", knobs = { placement = "deferred" } }"#);
}

/// Each late frame of `graph` with `edit` is the frame before of `graph` with `plain` (the
/// same graph as the late one's, timed now).
fn behind(graph: &str, plain: &str, edit: &str) {
    behind_in(graph, plain, edit, &["unlit", "lit"]);
}

fn behind_in(graph: &str, plain: &str, edit: &str, modes: &[&str]) {
    let (p_name, l_name) = (format!("{graph}-plain-{}", plain.len()), format!("{graph}-late-{}", edit.len()));
    let (ok, log) = launch(&p_name, graph, &format!("[{plain}]"));
    assert!(ok, "{graph} with {plain}: {log}");
    let sep = if plain.is_empty() { "" } else { ", " };
    let (ok, log) = launch(&l_name, graph, &format!("[{plain}{sep}{edit}]"));
    assert!(ok, "{graph} with {edit}: {log}");
    for &mode in modes {
        let (p, l) = (images(&p_name, mode), images(&l_name, mode));
        assert!(p.len() > 4 && p.len() == l.len(), "{graph} {mode}: {} and {} frames saved", p.len(), l.len());
        assert!(p[p.len() - 1] != p[p.len() - 2], "{graph} {mode}: the camera does not move visibly");
        for k in 2..p.len() {
            assert!(l[k] == p[k - 1], "{graph} {mode} with {edit}: frame {k} is not the plain frame {}", k - 1);
        }
    }
}

/// On one GPU the hit records a frame late are kept on the device between the pass writing
/// them and the shading's, shaded with the camera they were made with: each frame is the
/// frame before (rays write records whatever the placement once their hits are late).
#[test]
#[ignore = "needs a GPU"]
fn gpu_hits_a_frame_late_show_the_frame_before() {
    let hits = r#"{ node = "shading", input = "hits", when = "prev" }"#;
    let deferred = r#"{ node = "shading", knobs = { placement = "deferred" } }"#;
    for graph in ["raycast-gpu", "columns-gpu"] {
        behind(graph, deferred, hits);
    }
    // The ray queries' acceleration structure stays placed for this frame's camera block,
    // so lit frames' shadow rays from last frame's records are off by the block the camera
    // crossed (a known gap): unlit frames only.
    behind_in("rayquery-gpu", deferred, hits, &["unlit"]);
    // Drawers writing records (a forward drawer shades in its own pass: refused, named).
    for graph in ["raster-gpu", "discover-gpu"] {
        behind(graph, r#"{ node = "draw", component = "gbuffer" }"#, hits);
    }
    for graph in ["splat-gpu", "slices-gpu"] {
        behind(graph, "", hits);
    }
}

/// Other edges a GPU executor holds between passes, timed `prev`: the rays reading last
/// frame's mesh surfaces (with this frame's camera), the drawer drawing last frame's face set.
/// They run and draw the world.
#[test]
#[ignore = "needs a GPU"]
fn gpu_held_edges_a_frame_late_run() {
    for (graph, edit) in [
        ("raycast-gpu", r#"{ node = "visibility", input = "meshes", when = "prev" }"#),
        ("discover-gpu", r#"{ node = "draw", input = "faces", when = "prev" }"#),
    ] {
        let name = format!("{graph}-held-{}", edit.len());
        let (ok, log) = launch(&name, graph, &format!("[{edit}]"));
        assert!(ok, "{graph} with {edit}: {log}");
        for mode in ["unlit", "lit"] {
            let l = images(&name, mode);
            assert!(l.len() > 4, "{graph} {mode} with {edit}: {} frames saved", l.len());
            assert!(l[l.len() - 1] != l[0], "{graph} {mode} with {edit}: nothing drawn changes");
        }
    }
}

/// The world's changes a frame late on a GPU (every node reading them alike: one copy of the
/// world serves them): once the world has stopped changing, the frames are the plain run's.
#[test]
#[ignore = "needs a GPU"]
fn gpu_world_changes_a_frame_late_catch_up() {
    let late = |n: &str| format!(r#"{{ node = "{n}", input = "changes", when = "prev" }}"#);
    for (graph, nodes) in [
        ("raycast-gpu", &["storage"][..]),
        ("raycast-gpu", &["meshes"][..]),
        ("columns-gpu", &["storage"][..]),
        ("slices-gpu", &["storage"][..]),
        ("rayquery-gpu", &["geometry"][..]),
        ("splat-gpu", &["geometry", "storage"][..]),
        ("raster-gpu", &["geometry", "storage"][..]),
        ("discover-gpu", &["storage"][..]),
    ] {
        let edit = nodes.iter().map(|n| late(n)).collect::<Vec<_>>().join(", ");
        let (plain, l) = (format!("{graph}-plain-world"), format!("{graph}-world-{}", edit.len()));
        let (ok, log) = launch(&plain, graph, "[]");
        assert!(ok, "{log}");
        let (ok, log) = launch(&l, graph, &format!("[{edit}]"));
        assert!(ok, "{graph} with {edit}: {log}");
        for mode in ["unlit", "lit"] {
            let (p, q) = (images(&plain, mode), images(&l, mode));
            assert!(!p.is_empty() && p.last() == q.last(), "{graph} {mode} with {edit}: the last frame differs");
        }
    }
}

/// The world's meshes a frame late are culled and drawn from this frame's camera (they are
/// the same from every camera), and the storage's changes a frame late reach it: once the
/// world has stopped changing, the frames are the plain run's.
#[test]
fn world_side_values_a_frame_late_catch_up() {
    for (graph, edit) in [
        ("raster-cpu", r#"{ node = "visibility", input = "faces", when = "prev" }"#),
        ("raster-cpu", r#"{ node = "geometry", input = "changes", when = "prev" }"#),
        ("raster-cpu", r#"{ node = "storage", input = "changes", when = "prev" }"#),
        ("raycast-cpu", r#"{ node = "storage", input = "changes", when = "prev" }"#),
        ("columns-cpu", r#"{ node = "storage", input = "changes", when = "prev" }"#),
    ] {
        let (plain, late) = (format!("{graph}-plain-world"), format!("{graph}-world-{}", edit.len()));
        let (ok, log) = launch(&plain, graph, "[]");
        assert!(ok, "{log}");
        let (ok, log) = launch(&late, graph, &format!("[{edit}]"));
        assert!(ok, "{graph} with {edit}: {log}");
        for mode in ["unlit", "lit"] {
            let (p, l) = (images(&plain, mode), images(&late, mode));
            assert!(p.last() == l.last(), "{graph} {mode} with {edit}: the last frame differs");
        }
    }
}
