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
