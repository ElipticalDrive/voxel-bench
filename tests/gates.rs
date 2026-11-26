//! Gates run end to end on the CPU, on the three kinds of edge: the world's stream (in front
//! of the client), the hit records between visibility and shading (inside the executor) and
//! the frame behind the output (in the worker). What crosses is serialized, crosses a lossy
//! modelled network and is deserialized; the pass reports per gate what crossed (bytes,
//! packets lost, serialization time) and lives through what the lost bytes did to the data.
//! Lossless gates change no pixel. A gate on an edge no executor runs gates on yet is
//! refused, naming it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Writes a one-renderer plan of `edits` on `graph` and runs it: its success and log.
fn launch(dir: &Path, name: &str, graph: &str, edits: &str) -> (bool, String) {
    let d = dir.join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
generator = {{ type = "terrain", seed = 3 }}
camera = {{ type = "orbit", center = [0, 30, 0], radius = 40, height = 20, period = 600 }}
[run]
frames = 20
warmup = 0
resolution = [48, 32]
modes = ["unlit"]
view_radius = 2
[[renderer]]
graph = "{graph}"
edits = {edits}
[output]
dir = {:?}
save = {{ every = 0 }}
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
    for graph in ["raycast-cpu", "columns-cpu"] {
        let plain = run(&dir(), &format!("{graph}-plain"), graph, r#"[{ node = "shading", knobs = { placement = "deferred" } }]"#);
        assert!(plain["error"].is_null(), "{}", plain["error"]);
        let gated = run(&dir(), &format!("{graph}-gated"), graph, &format!("[{}]", inside("{}", "{}")));
        assert!(gated["error"].is_null(), "{}", gated["error"]);
        assert_eq!(gates(&gated).iter().map(|(k, (e, _))| format!("{k}:{e}")).collect::<Vec<_>>(), ["f:frame", "h:shading.hits"]);
        crossed(&gated, false);
        assert!(last_image(&dir(), &format!("{graph}-plain")) == last_image(&dir(), &format!("{graph}-gated")), "{graph}: a lossless gate changed the frame");
    }
}

/// What is refused, naming the gate, its edge and why: hits that never leave their pass
/// (shading inline), hits of an executor without their serialized form (the rasteriser), an
/// image into the post (compiled into its shading).
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
    assert!(log.contains("`g` on `shading.hits`") && log.contains("no serialized form"), "{log}");
    let post = r#"[
  { add = "post", role = "post", component = "clamp" },
  { add = "g", role = "gate", component = "net" },
  { node = "g", input = "in", from = "shading" },
  { node = "post", input = "image", from = "g" },
]"#;
    let (_, log) = launch(&dir(), "post", "raycast-cpu", post);
    assert!(log.contains("`g` on `post.image`") && log.contains("after the post"), "{log}");
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
