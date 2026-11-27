//! Gates run end to end on the CPU, on the three kinds of edge: the world's stream (in front
//! of the client), the hit records between visibility and shading (inside the executor) and
//! the frame behind the output (in the worker). What crosses is serialized, crosses a lossy
//! modelled network and is deserialized; the pass reports per gate what crossed (bytes,
//! packets lost, serialization time) and lives through what the lost bytes did to the data.
//! The CPU rasteriser's faces (meshes and screen triangles) cross gates too.
//! Lossless gates change no pixel. A gate on an edge no executor runs gates on yet is
//! refused, naming it. The GPU's hit records through gates need a GPU (`--ignored`;
//! `scripts/check.py gpu` runs them).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Writes a one-renderer plan of `edits` on `graph` and runs it: its success and log.
fn launch(dir: &Path, name: &str, graph: &str, edits: &str) -> (bool, String) {
    launch_saving(dir, name, graph, edits, 0)
}

/// [`launch`], saving every `every`-th frame (0: the last).
fn launch_saving(dir: &Path, name: &str, graph: &str, edits: &str, every: u32) -> (bool, String) {
    launch_scene(dir, name, graph, edits, every, "unlit", r#"generator = { type = "terrain", seed = 3 }
camera = { type = "orbit", center = [0, 30, 0], radius = 40, height = 20, period = 600 }"#)
}

/// [`launch_saving`] in `scene` (the plan's scene table's lines), in pass mode `mode`.
fn launch_scene(dir: &Path, name: &str, graph: &str, edits: &str, every: u32, mode: &str, scene: &str) -> (bool, String) {
    let d = dir.join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
{scene}
[run]
frames = 20
warmup = 0
resolution = [48, 32]
modes = ["{mode}"]
view_radius = 2
[[renderer]]
graph = "{graph}"
edits = {edits}
[output]
dir = {:?}
save = {{ every = {every} }}
"#,
        d.join("out").to_str().unwrap(),
    );
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer")).arg("run").arg(d.join("plan.toml")).env("RUST_LOG", "warn").output().unwrap();
    (o.status.success(), String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout))
}

/// The report of a one-renderer run of `edits` on `graph` (a lane's error included).
fn run(dir: &Path, name: &str, graph: &str, edits: &str) -> serde_json::Value {
    let (ok, log) = launch(dir, name, graph, edits);
    assert!(ok, "the run failed:\n{log}");
    let report = find(&dir.join(name).join("out"), &|p| p.file_name().is_some_and(|n| n == "report.json")).unwrap_or_else(|| panic!("no report:\n{log}"));
    let r: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    r[0].clone()
}

fn find(dir: &Path, which: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find(&p, which) {
                return Some(f);
            }
        } else if which(&p) {
            return Some(p);
        }
    }
    None
}

/// The last frame a run saved (its file's bytes).
fn last_image(dir: &Path, name: &str) -> Vec<u8> {
    let png = find(&dir.join(name).join("out"), &|p| p.extension().is_some_and(|e| e == "png") && !p.to_string_lossy().contains("diff")).expect("a saved frame");
    std::fs::read(png).unwrap()
}

/// A pass's gate reports by gate id: its edge and figures.
fn gates(r: &serde_json::Value) -> BTreeMap<String, (String, serde_json::Value)> {
    let all = r["gates"].as_array().unwrap_or_else(|| panic!("no gates: {r}"));
    all.iter().map(|g| (g["gate"].as_str().unwrap().to_string(), (g["edge"].as_str().unwrap().to_string(), g["stats"].clone()))).collect()
}

fn dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("gates")
}

/// Two gates on the storage's changes: the second's losses hit the bytes.
const WORLD: &str = r#"
  { add = "near", role = "gate", component = "net" },
  { node = "near", input = "in", from = "world" },
  { node = "near", knobs = { latency_ms = 10, resend = true, loss = 0.1, seed = 1 } },
  { add = "far", role = "gate", component = "net" },
  { node = "far", input = "in", from = "near" },
  { node = "far", knobs = { latency_ms = 5, loss = 0.05, mtu = 512, seed = 2 } },
  { node = "storage", input = "changes", from = "far" },
"#;

/// Shading deferred (hit records cross from the visibility pass to the shading), a gate on
/// the hit records and one behind the output (the frame), `hits` and `frame` their knobs.
fn inside(hits: &str, frame: &str) -> String {
    format!(
        r#"
  {{ node = "shading", knobs = {{ placement = "deferred" }} }},
  {{ add = "h", role = "gate", component = "net" }},
  {{ node = "h", input = "in", from = "visibility" }},
  {{ node = "h", knobs = {hits} }},
  {{ node = "shading", input = "hits", from = "h" }},
  {{ add = "f", role = "gate", component = "net" }},
  {{ node = "f", input = "in", from = "shading" }},
  {{ node = "f", knobs = {frame} }},
"#
    )
}

/// Asserts every gate of `r` carried messages, bytes and (`lossy`) lost packets, and spent
/// time serializing.
fn crossed(r: &serde_json::Value, lossy: bool) {
    for (id, (_, g)) in gates(r) {
        let num = |k: &str| g[k].as_f64().unwrap_or_else(|| panic!("no {id}.{k}: {g}"));
        assert!(num("messages") > 0.0 && num("bytes") > 0.0 && num("packets") > 0.0, "{id}: {g}");
        assert!(!lossy || num("lost_packets") > 0.0, "{id}: {g}");
        assert!(num("encode_ms") > 0.0 && num("decode_ms") > 0.0, "{id}: {g}");
    }
}

#[test]
fn the_world_stream_through_lossy_gates() {
    let r = run(&dir(), "lossy", "raycast-cpu", &format!("[{WORLD}]"));
    assert!(r["error"].is_null(), "{}", r["error"]);
    assert!(r["label"].as_str().unwrap().contains("gate"), "{}", r["label"]);
    let gs = gates(&r);
    assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["far:client.changes", "near:client.changes"]);
    crossed(&r, true);
    assert!(r["frames"].as_u64().unwrap() > 0);
}

/// The three kinds of edge at once, lossy: the world's stream, the hit records, the frame.
/// Every gate reports what crossed it on its edge; the pass lives through the losses.
#[test]
fn world_hits_and_frame_through_lossy_gates() {
    let edits = format!("[{WORLD}{}]", inside("{ latency_ms = 20, loss = 0.05, mtu = 512, seed = 3 }", "{ latency_ms = 30, jitter_ms = 10, loss = 0.1, seed = 4 }"));
    let r = run(&dir(), "three", "raycast-cpu", &edits);
    assert!(r["error"].is_null(), "{}", r["error"]);
    let gs = gates(&r);
    assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["f:frame", "far:client.changes", "h:shading.hits", "near:client.changes"]);
    crossed(&r, true);
    // One message a frame on the hit records, each the 48 x 32 records of 8 bytes and more.
    let h = &gs["h"].1;
    let messages = h["messages"].as_f64().unwrap();
    assert!(messages >= r["frames"].as_f64().unwrap(), "{h}");
    assert!(h["bytes"].as_f64().unwrap() >= messages * 48.0 * 32.0 * 8.0, "{h}");
}

/// Lossless gates with no delay change no pixel: the serialized forms read back what was
/// written, on the hit records (the ray caster's and the column marcher's) and the frame.
#[test]
fn lossless_gates_change_no_pixel() {
    for graph in ["raycast-cpu", "columns-cpu", "raster-cpu"] {
        let plain = run(&dir(), &format!("{graph}-plain"), graph, r#"[{ node = "shading", knobs = { placement = "deferred" } }]"#);
        assert!(plain["error"].is_null(), "{}", plain["error"]);
        let source = if graph == "raster-cpu" { "draw" } else { "visibility" };
        let gated = run(&dir(), &format!("{graph}-gated"), graph, &format!("[{}]", inside("{}", "{}").replace(r#"from = "visibility""#, &format!(r#"from = "{source}""#))));
        assert!(gated["error"].is_null(), "{}", gated["error"]);
        assert_eq!(gates(&gated).iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["f:frame", "h:shading.hits"]);
        crossed(&gated, false);
        assert!(last_image(&dir(), &format!("{graph}-plain")) == last_image(&dir(), &format!("{graph}-gated")), "{graph}: a lossless gate changed the frame");
    }
}

/// Every frame a run saved, in frame order (the files' bytes).
fn images(dir: &Path, name: &str) -> Vec<Vec<u8>> {
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
    all(&dir.join(name).join("out"), &mut paths);
    paths.sort();
    paths.iter().map(|p| std::fs::read(p).unwrap()).collect()
}

/// Hit records late behind a gate are shaded with the camera they were cast with: with the
/// camera moving, a lossless gate delaying the records by a few frames shows exactly an
/// earlier frame of the ungated run, on each CPU drawer writing records.
#[test]
fn late_hits_shaded_with_their_own_camera() {
    for graph in ["raycast-cpu", "columns-cpu", "raster-cpu"] {
        let (plain, gated) = (format!("{graph}-moving-plain"), format!("{graph}-moving-late"));
        let (ok, log) = launch_saving(&dir(), &plain, graph, r#"[{ node = "shading", knobs = { placement = "deferred" } }]"#, 1);
        assert!(ok, "{log}");
        let source = if graph == "raster-cpu" { "draw" } else { "visibility" };
        let edits = format!("[{}]", inside("{ latency_ms = 40 }", "{}").replace(r#"from = "visibility""#, &format!(r#"from = "{source}""#)));
        let (ok, log) = launch_saving(&dir(), &gated, graph, &edits, 1);
        assert!(ok, "{log}");
        let (plain, gated) = (images(&dir(), &plain), images(&dir(), &gated));
        let last = gated.last().expect("saved frames");
        assert!(plain.len() > 4 && plain.last() != plain.get(plain.len() - 4), "{graph}: the camera does not move visibly");
        assert!(last != plain.last().unwrap(), "{graph}: the late records show the current frame");
        assert!(plain[plain.len() - 5..].contains(last), "{graph}: the late records do not show an earlier frame as it was");
    }
}

/// A gate on each of the CPU rasteriser's faces edges: the geometry's meshes into the
/// visibility (`m`) and its screen triangles into the drawer (`t`), `meshes` and `tris`
/// their knobs.
fn on_faces(meshes: &str, tris: &str) -> String {
    format!(
        r#"[
  {{ add = "m", role = "gate", component = "net" }},
  {{ node = "m", input = "in", from = "geometry" }},
  {{ node = "m", knobs = {meshes} }},
  {{ node = "visibility", input = "faces", from = "m" }},
  {{ add = "t", role = "gate", component = "net" }},
  {{ node = "t", input = "in", from = "visibility" }},
  {{ node = "t", knobs = {tris} }},
  {{ node = "draw", input = "faces", from = "t" }},
]"#
    )
}

/// The rasteriser's faces cross gates: lossless, no pixel changes; lossy, the pass lives on
/// and each gate reports what crossed. Meshes late behind a gate are drawn from this frame's
/// camera (they are the same from every camera): once they have arrived, the frame is the
/// ungated one's.
#[test]
fn raster_faces_cross_gates() {
    let plain = run(&dir(), "faces-plain", "raster-cpu", "[]");
    assert!(plain["error"].is_null(), "{}", plain["error"]);
    let gated = run(&dir(), "faces-gated", "raster-cpu", &on_faces("{}", "{}"));
    assert!(gated["error"].is_null(), "{}", gated["error"]);
    assert_eq!(gates(&gated).iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["m:visibility.faces", "t:draw.faces"]);
    crossed(&gated, false);
    assert!(last_image(&dir(), "faces-plain") == last_image(&dir(), "faces-gated"), "a lossless gate on the faces changed the frame");
    // The meshes with resend (each is many packets: one lost unresent and it does not
    // decode), the triangles without.
    let lossy = run(&dir(), "faces-lossy", "raster-cpu", &on_faces("{ latency_ms = 10, loss = 0.1, resend = true, mtu = 512, seed = 5 }", "{ latency_ms = 20, loss = 0.2, mtu = 512, seed = 6 }"));
    assert!(lossy["error"].is_null(), "{}", lossy["error"]);
    crossed(&lossy, true);
    let t = &gates(&lossy)["t"].1;
    assert!(t["bytes"].as_f64().unwrap() > t["messages"].as_f64().unwrap() * 1000.0, "triangles crossed: {t}");
    let late = run(&dir(), "faces-late", "raster-cpu", &on_faces("{ latency_ms = 40 }", "{}"));
    assert!(late["error"].is_null(), "{}", late["error"]);
    assert!(last_image(&dir(), "faces-plain") == last_image(&dir(), "faces-late"), "late meshes were drawn from another frame's camera");
}

/// What is refused, naming the gate, its edge and why: hits that never leave their pass
/// (shading inline, on the CPU and the GPU), hits of an executor without their serialized
/// form (the GPU column marcher), a drawer writing no records, an image into the post
/// (compiled into its shading).
#[test]
fn gates_refused_naming_them() {
    let on_hits = |from: &str| {
        format!(
            r#"[
  {{ add = "g", role = "gate", component = "net" }},
  {{ node = "g", input = "in", from = "{from}" }},
  {{ node = "shading", input = "hits", from = "g" }},
]"#
        )
    };
    let (_, log) = launch(&dir(), "inline", "raycast-cpu", &on_hits("visibility"));
    assert!(log.contains("`g` on `shading.hits`") && log.contains("placement = inline"), "{log}");
    let (_, log) = launch(&dir(), "raster", "raster-cpu", &on_hits("draw"));
    assert!(log.contains("`g` on `shading.hits`") && log.contains("placement = inline"), "{log}");
    // Refused by `accept`, before any GPU is asked for.
    let (_, log) = launch(&dir(), "raycast-gpu-inline", "raycast-gpu", &on_hits("visibility"));
    assert!(log.contains("`g` on `shading.hits`") && log.contains("placement = inline"), "{log}");
    let (_, log) = launch(&dir(), "raster-gpu-forward", "raster-gpu", &on_hits("draw"));
    assert!(log.contains("`g` on `shading.hits`") && log.contains("no hit records"), "{log}");
    let (_, log) = launch(&dir(), "columns-gpu-inline", "columns-gpu", &on_hits("visibility"));
    assert!(log.contains("`g` on `shading.hits`") && log.contains("placement = inline"), "{log}");
    let (_, log) = launch(&dir(), "raster-gpu-surfaces", "raster-gpu", &format!("[{}]", gate_on("meshes", "draw", "meshes", "{}")));
    assert!(log.contains("`g` on `draw.meshes`") && log.contains("GPU ray casting"), "{log}");
    // A second ray query over the geometry (the shadow rays') reading it without the gate.
    let two = format!(
        r#"[{} {{ add = "k2", role = "traversal", component = "rayquery" }}, {{ node = "k2", input = "geometry", from = "geometry" }}, {{ node = "shading", input = "shadow", from = "k2" }},]"#,
        gate_on("geometry", "kernel", "geometry", "{}")
    );
    let (_, log) = launch(&dir(), "rayquery-gpu-geometry-mixed", "rayquery-gpu", &two);
    assert!(log.contains("`g` on `kernel.geometry`") && log.contains("`k2`.geometry has other gates"), "{log}");
    let post = r#"[
  { add = "post", role = "post", component = "clamp" },
  { add = "g", role = "gate", component = "net" },
  { node = "g", input = "in", from = "shading" },
  { node = "post", input = "image", from = "g" },
]"#;
    let (_, log) = launch(&dir(), "post", "raycast-cpu", post);
    assert!(log.contains("`g` on `post.image`") && log.contains("after the post"), "{log}");
}

/// On a GPU: a gate on the hit records the shading reads, shading `deferred` or a drawer
/// writing records (`gbuffer`, no meshes node), or splats.
fn gpu_hits(graph: &str, hits: &str) -> String {
    let source = match graph {
        "raster-gpu" => r#"{ node = "draw", component = "gbuffer" }, { remove = "meshes" },"#,
        "splat-gpu" => "",
        _ => r#"{ node = "shading", knobs = { placement = "deferred" } }, { remove = "meshes" },"#,
    };
    let from = if graph == "raster-gpu" { "draw" } else { "visibility" };
    format!(
        r#"[{source}
  {{ add = "h", role = "gate", component = "net" }},
  {{ node = "h", input = "in", from = "{from}" }},
  {{ node = "h", knobs = {hits} }},
  {{ node = "shading", input = "hits", from = "h" }},
]"#
    )
}

/// On the GPU the hit records cross a lossy gate end to end: read back from the pass that
/// writes them, serialized through the gate, written to the shading's own copy of the edge
/// (the shading split off on the same GPU, its walks over its own storage). The pass lives
/// through the losses and reports the gate. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_hits_through_a_lossy_gate() {
    for graph in ["raycast-gpu", "raster-gpu", "splat-gpu"] {
        let r = run(&dir(), &format!("{graph}-lossy"), graph, &gpu_hits(graph, "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, seed = 5 }"));
        assert!(r["error"].is_null(), "{graph}: {}", r["error"]);
        let gs = gates(&r);
        assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["h:shading.hits"], "{graph}");
        crossed(&r, true);
        let h = &gs["h"].1;
        let messages = h["messages"].as_f64().unwrap();
        assert!(messages >= r["frames"].as_f64().unwrap(), "{graph}: {h}");
        assert!(h["bytes"].as_f64().unwrap() >= messages * 48.0 * 32.0 * 8.0, "{graph}: {h}");
    }
}

/// A lossless gate with no delay on the GPU's hit records changes no pixel: the records read
/// back and written again are the ones the shading would have read. Needs a GPU.
#[test]
#[ignore]
fn gpu_lossless_hits_gate_changes_no_pixel() {
    let plain = run(&dir(), "raycast-gpu-plain", "raycast-gpu", r#"[{ node = "shading", knobs = { placement = "deferred" } }, { remove = "meshes" }]"#);
    assert!(plain["error"].is_null(), "{}", plain["error"]);
    let gated = run(&dir(), "raycast-gpu-gated", "raycast-gpu", &gpu_hits("raycast-gpu", "{}"));
    assert!(gated["error"].is_null(), "{}", gated["error"]);
    crossed(&gated, false);
    assert!(last_image(&dir(), "raycast-gpu-plain") == last_image(&dir(), "raycast-gpu-gated"), "a lossless gate on the GPU's hit records changed the frame");
}

/// The world's side gated at its three places: a slow lossy network into the client
/// (resending; what the fixed server-to-client link was), one on the generated chunks (the
/// generator on the client's cpu) and one on the client's changes into the renderer. Each
/// reports on its edge; chunks inside the gates are holes the pass counts, and the bytes sent
/// are the first gate's line.
#[test]
fn the_world_side_gated_at_each_place() {
    let net = r#"
  { add = "net", role = "gate", component = "net" },
  { node = "net", input = "in", from = "world" },
  { node = "net", knobs = { latency_ms = 150, jitter_ms = 80, bandwidth_mbps = 5, loss = 0.05, reorder = true, resend = true, seed = 7 } },
  { node = "storage", input = "changes", from = "net" },
"#;
    let r = run(&dir(), "into-client", "raycast-cpu", &format!("[{net}]"));
    assert!(r["error"].is_null(), "{}", r["error"]);
    assert_eq!(gates(&r).iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["net:client.changes"]);
    crossed(&r, true);
    assert!(r["world_gated"].as_bool() == Some(true) && r["frames_with_holes"].as_u64().unwrap() > 0, "{r}");
    assert_eq!(r["wire_bytes"].as_f64(), gates(&r)["net"].1["bytes"].as_f64());
    let local = r#"
  { add = "client", role = "client", component = "pick" },
  { add = "generator", role = "generator", component = "procedural" },
  { node = "generator", device = "cpu" },
  { add = "gen", role = "gate", component = "net" },
  { node = "gen", input = "in", from = "generator" },
  { node = "gen", knobs = { latency_ms = 20, resend = true, loss = 0.1, seed = 8 } },
  { node = "client", input = "chunks", from = "gen" },
  { add = "out", role = "gate", component = "net" },
  { node = "out", input = "in", from = "client" },
  { node = "out", knobs = { latency_ms = 10 } },
  { node = "storage", input = "changes", from = "out" },
"#;
    let r = run(&dir(), "client-side", "raycast-cpu", &format!("[{local}]"));
    assert!(r["error"].is_null(), "{}", r["error"]);
    assert_eq!(gates(&r).iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["gen:client.chunks", "out:changes"]);
    crossed(&r, false);
    assert!(r["frames_with_holes"].as_u64().unwrap() > 0, "{r}");
    // A gate on the generated chunks with the generator on the server is refused, naming it.
    let server = r#"[
  { add = "client", role = "client", component = "pick" },
  { add = "generator", role = "generator", component = "procedural" },
  { add = "gen", role = "gate", component = "net" },
  { node = "gen", input = "in", from = "generator" },
  { node = "client", input = "chunks", from = "gen" },
]"#;
    let (_, log) = launch(&dir(), "server-chunks", "raycast-cpu", server);
    assert!(log.contains("`gen` on `client.chunks`") && log.contains("on the server"), "{log}");
}

/// A gate on the CPU discoverer's faces (`draw.faces`, a previous-frame edge: discovery runs
/// a frame ahead of the drawer), `knobs` its knobs.
fn on_discovered(knobs: &str) -> String {
    format!(
        r#"[
  {{ add = "d", role = "gate", component = "net" }},
  {{ node = "d", input = "in", from = "visibility" }},
  {{ node = "d", knobs = {knobs} }},
  {{ node = "draw", input = "faces", from = "d" }},
]"#
    )
}

/// The CPU discoverer's face set crosses a gate on its way to the GPU that draws it, on the
/// previous-frame edge: lossless, no pixel changes; lossy (no resend: a message with a lost
/// packet arrives with records zeroed, or does not decode), the pass lives on and the gate
/// reports the whole set crossing each frame; within the frame (`now`) the same gate runs.
/// Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn discovered_faces_cross_a_gate() {
    let plain = run(&dir(), "discover-plain", "discover-cpu", "[]");
    assert!(plain["error"].is_null(), "{}", plain["error"]);
    let gated = run(&dir(), "discover-gated", "discover-cpu", &on_discovered("{}"));
    assert!(gated["error"].is_null(), "{}", gated["error"]);
    assert_eq!(gates(&gated).iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["d:draw.faces"]);
    crossed(&gated, false);
    assert!(last_image(&dir(), "discover-plain") == last_image(&dir(), "discover-gated"), "a lossless gate on the discovered faces changed the frame");
    let lossy = run(&dir(), "discover-lossy", "discover-cpu", &on_discovered("{ latency_ms = 30, jitter_ms = 10, loss = 0.02, seed = 9 }"));
    assert!(lossy["error"].is_null(), "{}", lossy["error"]);
    crossed(&lossy, true);
    let d = &gates(&lossy)["d"].1;
    let messages = d["messages"].as_f64().unwrap();
    assert!(messages >= lossy["frames"].as_f64().unwrap(), "one face set a frame: {d}");
    assert!(d["bytes"].as_f64().unwrap() > messages * 1000.0, "the face set crossed: {d}");
    let now = run(&dir(), "discover-now", "discover-cpu", &on_discovered("{}").replacen('[', r#"[{ node = "draw", input = "faces", when = "now" }, "#, 1));
    assert!(now["error"].is_null(), "{}", now["error"]);
    crossed(&now, false);
}

/// See-through blocks behind a CPU gate on the hit records: the ray caster's records cross
/// with their layers (eight records a pixel), and a lossless gate changes no pixel of the
/// composited frame.
#[test]
fn transparency_layers_cross_a_cpu_gate() {
    let glass = r#"generator = { type = "glass" }
camera = { type = "static", position = [34, 36, 66], target = [23, 2, 16] }"#;
    let report = |name: &str| -> serde_json::Value {
        let p = find(&dir().join(name).join("out"), &|p| p.file_name().is_some_and(|n| n == "report.json")).expect("a report");
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).unwrap()).unwrap()[0].clone()
    };
    let (ok, log) = launch_scene(&dir(), "glass-plain", "raycast-cpu", r#"[{ node = "shading", knobs = { placement = "deferred" } }]"#, 0, "unlit", glass);
    assert!(ok, "{log}");
    let (ok, log) = launch_scene(&dir(), "glass-gated", "raycast-cpu", &format!("[{}]", inside("{}", "{}")), 0, "unlit", glass);
    assert!(ok, "{log}");
    let (plain, gated) = (report("glass-plain"), report("glass-gated"));
    for r in [&plain, &gated] {
        assert!(r["error"].is_null(), "{}", r["error"]);
        assert!(!r["not_supported"].to_string().contains("ransparency"), "transparency left out: {}", r["not_supported"]);
    }
    let h = &gates(&gated)["h"].1;
    let messages = h["messages"].as_f64().unwrap();
    assert!(h["bytes"].as_f64().unwrap() >= messages * 48.0 * 32.0 * 8.0 * 8.0, "the layers crossed: {h}");
    assert!(last_image(&dir(), "glass-plain") == last_image(&dir(), "glass-gated"), "a lossless gate changed the composited layers");
}

/// On the GPU, in the glass world, the ray caster's layered records (eight a pixel) cross a
/// gate on the hits whole: the shading past it composites them. A lossless gate changes no
/// pixel of the composited frame; a lossy one renders on and reports the gate. Needs a GPU.
#[test]
#[ignore]
fn transparency_layers_cross_a_gpu_gate() {
    let glass = r#"generator = { type = "glass" }
camera = { type = "static", position = [34, 36, 66], target = [23, 2, 16] }"#;
    let report = |name: &str| -> serde_json::Value {
        let p = find(&dir().join(name).join("out"), &|p| p.file_name().is_some_and(|n| n == "report.json")).expect("a report");
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).unwrap()).unwrap()[0].clone()
    };
    let runs = [
        ("glass-gpu-plain", r#"[{ node = "shading", knobs = { placement = "deferred" } }, { remove = "meshes" }]"#.to_string()),
        ("glass-gpu-gated", gpu_hits("raycast-gpu", "{}")),
        ("glass-gpu-lossy", gpu_hits("raycast-gpu", "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, reorder = true, seed = 5 }")),
    ];
    for (name, edits) in &runs {
        let (ok, log) = launch_scene(&dir(), name, "raycast-gpu", edits, 0, glass);
        assert!(ok, "{name}: {log}");
        let r = report(name);
        assert!(r["error"].is_null(), "{name}: {}", r["error"]);
        assert!(!r["not_supported"].to_string().contains("ransparency"), "{name}: transparency left out: {}", r["not_supported"]);
        if *name != "glass-gpu-plain" {
            crossed(&r, *name == "glass-gpu-lossy");
            let h = &gates(&r)["h"].1;
            // Once the glass has arrived every frame's records cross with their layers.
            assert!(h["bytes"].as_f64().unwrap() >= 10.0 * 48.0 * 32.0 * 8.0 * 8.0, "{name}: the layers crossed: {h}");
        }
    }
    assert!(last_image(&dir(), "glass-gpu-plain") == last_image(&dir(), "glass-gpu-gated"), "a lossless gate changed the composited layers");
}

/// A gate `g` (knobs `knobs`) taking `reader`.`port` from `from`.
fn gate_on(from: &str, reader: &str, port: &str, knobs: &str) -> String {
    format!(r#"{{ add = "g", role = "gate", component = "net" }}, {{ node = "g", input = "in", from = "{from}" }}, {{ node = "g", knobs = {knobs} }}, {{ node = "{reader}", input = "{port}", from = "g" }},"#)
}

/// Edges whose data a GPU executor holds between two of its passes: the GPU column
/// marcher's hit records (shading deferred), the beam prepass's ray starts into the walk,
/// hole detection's tiles into GPU discovery and the discovered faces into the drawer. Each:
/// the graph, the edits before the edge, the edge's source, reader and port.
fn between_passes() -> Vec<(&'static str, &'static str, &'static str, &'static str, &'static str)> {
    vec![
        ("columns-gpu", r#"{ node = "shading", knobs = { placement = "deferred" } },"#, "visibility", "shading", "hits"),
        ("raycast-gpu", r#"{ add = "prepass", role = "prepass", component = "beam" }, { node = "prepass", input = "storage", from = "storage" },"#, "prepass", "visibility", "start"),
        (
            "discover-gpu",
            r#"{ add = "holes", role = "holes", component = "coverage" },
  { node = "draw", input = "faces", when = { iterate = 2 } },
  { node = "holes", input = "covered", from = "draw" },
  { node = "holes", input = "sky", from = "draw" },
  { node = "holes", input = "sky", when = "prev" },"#,
            "holes",
            "visibility",
            "where",
        ),
        ("discover-gpu", "", "visibility", "draw", "faces"),
    ]
}

/// On a GPU, what crosses between two passes goes through a lossy gate end to end: read back
/// before the reader's pass, serialized through the gate, the last that arrived written in
/// place (what the producer keeps across frames restored after). The pass lives through the
/// losses and reports the gate on its edge. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_edges_between_passes_through_lossy_gates() {
    for (graph, before, from, reader, port) in between_passes() {
        let edge = format!("{reader}.{port}");
        let edits = format!("[{before} {}]", gate_on(from, reader, port, "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, seed = 5 }"));
        let r = run(&dir(), &format!("{graph}-{reader}-{port}-lossy"), graph, &edits);
        assert!(r["error"].is_null(), "{edge}: {}", r["error"]);
        let gs = gates(&r);
        assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), [format!("g:{edge}")], "{graph}");
        crossed(&r, true);
        let g = &gs["g"].1;
        assert!(g["messages"].as_f64().unwrap() >= r["frames"].as_f64().unwrap(), "{edge}: {g}");
    }
}

/// What crosses a viewed edge between passes on a GPU (the column marcher's hit records) late
/// behind a gate is used with the camera of its frame: with the camera moving, a lossless
/// gate delaying the records shows exactly an earlier frame of the ungated run. Needs a GPU.
#[test]
#[ignore]
fn gpu_late_records_between_passes_with_their_own_camera() {
    let (graph, before, from, reader, port) = between_passes()[0];
    let (plain, gated) = (format!("{graph}-moving-plain"), format!("{graph}-moving-late"));
    let (ok, log) = launch_saving(&dir(), &plain, graph, &format!("[{before}]"), 1);
    assert!(ok, "{log}");
    let (ok, log) = launch_saving(&dir(), &gated, graph, &format!("[{before} {}]", gate_on(from, reader, port, "{ latency_ms = 40 }")), 1);
    assert!(ok, "{log}");
    let (plain, gated) = (images(&dir(), &plain), images(&dir(), &gated));
    let last = gated.last().expect("saved frames");
    assert!(plain.len() > 4 && plain.last() != plain.get(plain.len() - 4), "the camera does not move visibly");
    assert!(last != plain.last().unwrap(), "the late records show the current frame");
    assert!(plain[plain.len() - 5..].contains(last), "the late records do not show an earlier frame as it was");
}

/// A lossless gate with no delay between two passes on a GPU changes no pixel under a moving
/// camera: what is read back and written again is what the reader would have read. Needs a
/// GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_lossless_gates_between_passes_change_no_pixel() {
    for (graph, before, from, reader, port) in between_passes() {
        let name = format!("{graph}-{reader}-{port}");
        let edge = format!("{reader}.{port}");
        let plain = run(&dir(), &format!("{name}-plain"), graph, &format!(r#"[{before} {{ node = "{reader}", input = "{port}", from = "{from}" }}]"#));
        assert!(plain["error"].is_null(), "{edge}: {}", plain["error"]);
        let gated = run(&dir(), &format!("{name}-gated"), graph, &format!("[{before} {}]", gate_on(from, reader, port, "{}")));
        assert!(gated["error"].is_null(), "{edge}: {}", gated["error"]);
        crossed(&gated, false);
        assert!(last_image(&dir(), &format!("{name}-plain")) == last_image(&dir(), &format!("{name}-gated")), "{edge}: a lossless gate changed the frame");
    }
}

/// A room with moving meshes in view (a spinning torus, orbiting spheres), seen from a fixed
/// camera.
const MESH_ROOM: &str = r#"generator = { type = "room", size = 48 }
camera = { type = "static", position = [24, 24, 85], target = [24, 20, 20] }
meshes = [
    { shape = "torus", position = [30, 30, 30], scale = 7, detail = 24, animate = [{ type = "spin", axis = [1, 1, 0], period = 16 }] },
    { shape = "sphere", position = [14, 14, 14], scale = 3, detail = 16, count = 3, seed = 1, animate = [{ type = "orbit", center = [14.5, 0, 14.5], radius = 12, period = 20 }] },
]"#;

/// A run of `edits` on GPU ray casting in [`MESH_ROOM`] in pass mode `mode`: its report.
fn mesh_room(name: &str, mode: &str, edits: &str) -> serde_json::Value {
    let (ok, log) = launch_scene(&dir(), name, "raycast-gpu", edits, 0, mode, MESH_ROOM);
    assert!(ok, "the run failed:\n{log}");
    let report = find(&dir().join(name).join("out"), &|p| p.file_name().is_some_and(|n| n == "report.json")).unwrap_or_else(|| panic!("no report:\n{log}"));
    let r: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    assert!(r[0]["error"].is_null(), "{name}: {}", r[0]["error"]);
    r[0].clone()
}

/// GPU ray casting's mesh surfaces (the records the meshes' pass rasterises, which the rays
/// stop at) cross gates between the two passes: lossy, the pass lives on and reports the gate,
/// one frame's records a message; lossless, no pixel changes, with the meshes in the frame
/// (it differs from the pass without them), inline and deferred. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_mesh_surfaces_through_gates() {
    let r = mesh_room("surfaces-lossy", "unlit+meshes", &format!("[{}]", gate_on("meshes", "visibility", "meshes", "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, seed = 5 }")));
    let gs = gates(&r);
    assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["g:visibility.meshes"]);
    crossed(&r, true);
    let g = &gs["g"].1;
    assert!(g["messages"].as_f64().unwrap() >= r["frames"].as_f64().unwrap(), "{g}");
    assert!(g["bytes"].as_f64().unwrap() >= g["messages"].as_f64().unwrap() * 48.0 * 32.0 * 8.0, "{g}");
    for (placement, name) in [("inline", "surfaces"), ("deferred", "surfaces-deferred")] {
        let set = format!(r#"{{ node = "shading", knobs = {{ placement = "{placement}" }} }},"#);
        mesh_room(&format!("{name}-none"), "unlit", &format!("[{set}]"));
        mesh_room(&format!("{name}-plain"), "unlit+meshes", &format!("[{set}]"));
        let gated = mesh_room(&format!("{name}-gated"), "unlit+meshes", &format!("[{set} {}]", gate_on("meshes", "visibility", "meshes", "{}")));
        crossed(&gated, false);
        let img = |n: &str| last_image(&dir(), &format!("{name}-{n}"));
        assert!(img("plain") != img("none"), "{placement}: no mesh in the frame");
        assert!(img("plain") == img("gated"), "{placement}: a lossless gate on the mesh surfaces changed the frame");
    }
}

/// On the GPU rasteriser, the geometry's faces cross a lossy gate chunk by chunk at their
/// upload: the pass lives through the losses and reports the gate, one message per chunk
/// meshed or dropped. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_geometry_faces_through_a_lossy_gate() {
    let edits = format!("[{}]", gate_on("geometry", "visibility", "faces", "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, reorder = true, seed = 5 }"));
    let r = run(&dir(), "raster-gpu-geometry-lossy", "raster-gpu", &edits);
    assert!(r["error"].is_null(), "{}", r["error"]);
    let gs = gates(&r);
    assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["g:visibility.faces"]);
    crossed(&r, true);
}

/// A lossless gate with no delay on the GPU rasteriser's geometry changes no pixel: every
/// chunk's faces arrive in the frame they were meshed in. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_lossless_geometry_gate_changes_no_pixel() {
    let plain = run(&dir(), "raster-gpu-geometry-plain", "raster-gpu", "[]");
    assert!(plain["error"].is_null(), "{}", plain["error"]);
    let gated = run(&dir(), "raster-gpu-geometry-gated", "raster-gpu", &format!("[{}]", gate_on("geometry", "visibility", "faces", "{}")));
    assert!(gated["error"].is_null(), "{}", gated["error"]);
    crossed(&gated, false);
    assert!(last_image(&dir(), "raster-gpu-geometry-plain") == last_image(&dir(), "raster-gpu-geometry-gated"), "a lossless gate changed the frame");
}

/// On splats, the geometry's points cross a lossy gate chunk by chunk between their rebuild
/// and the upload: the pass lives through the losses and reports the gate. Needs a GPU
/// (`--ignored`).
#[test]
#[ignore]
fn gpu_splat_points_through_a_lossy_gate() {
    let edits = format!("[{}]", gate_on("geometry", "visibility", "points", "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, reorder = true, seed = 5 }"));
    let r = run(&dir(), "splat-gpu-points-lossy", "splat-gpu", &edits);
    assert!(r["error"].is_null(), "{}", r["error"]);
    let gs = gates(&r);
    assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["g:visibility.points"]);
    crossed(&r, true);
}

/// A lossless gate with no delay on the splats' points changes no pixel: every chunk's
/// points arrive in the frame they were rebuilt in. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_lossless_points_gate_changes_no_pixel() {
    let plain = run(&dir(), "splat-gpu-points-plain", "splat-gpu", "[]");
    assert!(plain["error"].is_null(), "{}", plain["error"]);
    let gated = run(&dir(), "splat-gpu-points-gated", "splat-gpu", &format!("[{}]", gate_on("geometry", "visibility", "points", "{}")));
    assert!(gated["error"].is_null(), "{}", gated["error"]);
    crossed(&gated, false);
    assert!(last_image(&dir(), "splat-gpu-points-plain") == last_image(&dir(), "splat-gpu-points-gated"), "a lossless gate changed the frame");
}

/// On ray queries, the geometry's primitives cross a lossy gate chunk by chunk between the
/// meshing and the BLAS build: the pass lives through the losses and reports the gate. Needs
/// a GPU with ray queries (`--ignored`).
#[test]
#[ignore]
fn gpu_rayquery_geometry_through_a_lossy_gate() {
    for geometry in ["greedy", "boxes"] {
        let edits = format!(
            r#"[{{ node = "geometry", component = "{geometry}" }}, {}]"#,
            gate_on("geometry", "kernel", "geometry", "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, reorder = true, seed = 5 }")
        );
        let r = run(&dir(), &format!("rayquery-gpu-{geometry}-lossy"), "rayquery-gpu", &edits);
        assert!(r["error"].is_null(), "{geometry}: {}", r["error"]);
        let gs = gates(&r);
        assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["g:kernel.geometry"]);
        crossed(&r, true);
    }
}

/// A lossless gate with no delay on ray queries' geometry changes no pixel: every chunk's
/// primitives arrive in the frame they were built in. Needs a GPU with ray queries
/// (`--ignored`).
#[test]
#[ignore]
fn gpu_lossless_rayquery_geometry_gate_changes_no_pixel() {
    for geometry in ["greedy", "boxes"] {
        let pick = format!(r#"{{ node = "geometry", component = "{geometry}" }},"#);
        let plain = run(&dir(), &format!("rayquery-gpu-{geometry}-plain"), "rayquery-gpu", &format!("[{pick}]"));
        assert!(plain["error"].is_null(), "{geometry}: {}", plain["error"]);
        let gated = run(&dir(), &format!("rayquery-gpu-{geometry}-gated"), "rayquery-gpu", &format!("[{pick} {}]", gate_on("geometry", "kernel", "geometry", "{}")));
        assert!(gated["error"].is_null(), "{geometry}: {}", gated["error"]);
        crossed(&gated, false);
        assert!(last_image(&dir(), &format!("rayquery-gpu-{geometry}-plain")) == last_image(&dir(), &format!("rayquery-gpu-{geometry}-gated")), "{geometry}: a lossless gate changed the frame");
    }
}

/// On the GPU rasteriser, the culled faces (each frame's indirect draws) cross a lossy gate
/// at their upload: the pass lives through the losses and reports the gate, one message a
/// frame. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_culled_faces_through_a_lossy_gate() {
    let edits = format!("[{}]", gate_on("visibility", "draw", "faces", "{ latency_ms = 20, jitter_ms = 5, loss = 0.05, mtu = 512, reorder = true, seed = 5 }"));
    let r = run(&dir(), "raster-gpu-culled-lossy", "raster-gpu", &edits);
    assert!(r["error"].is_null(), "{}", r["error"]);
    let gs = gates(&r);
    assert_eq!(gs.iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["g:draw.faces"]);
    crossed(&r, true);
}

/// A lossless gate with no delay on the GPU rasteriser's culled faces changes no pixel:
/// each frame draws the list culled for it. Needs a GPU (`--ignored`).
#[test]
#[ignore]
fn gpu_lossless_culled_faces_gate_changes_no_pixel() {
    let plain = run(&dir(), "raster-gpu-culled-plain", "raster-gpu", "[]");
    assert!(plain["error"].is_null(), "{}", plain["error"]);
    let gated = run(&dir(), "raster-gpu-culled-gated", "raster-gpu", &format!("[{}]", gate_on("visibility", "draw", "faces", "{}")));
    assert!(gated["error"].is_null(), "{}", gated["error"]);
    crossed(&gated, false);
    assert!(last_image(&dir(), "raster-gpu-culled-plain") == last_image(&dir(), "raster-gpu-culled-gated"), "a lossless gate changed the frame");
}
