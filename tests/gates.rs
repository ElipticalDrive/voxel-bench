//! Gates on the world's stream, run end to end on the CPU: the world's changes are serialized,
//! cross a lossy modelled network and are deserialized in front of the client; the pass
//! reports what crossed (bytes, packets lost, serialization time) and lives through what
//! the lost bytes did to the data. A gate on an edge no executor runs gates on yet is
//! refused, naming it.

use std::path::Path;
use std::process::Command;

/// The report of a one-renderer run of `edits` on `raycast-cpu` (a lane's error included).
fn run(dir: &Path, name: &str, edits: &str) -> serde_json::Value {
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
graph = "raycast-cpu"
edits = {edits}
[output]
dir = {:?}
save = {{ every = 0 }}
"#,
        d.join("out").to_str().unwrap(),
    );
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer")).arg("run").arg(d.join("plan.toml")).env("RUST_LOG", "warn").output().unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "the run failed:\n{log}");
    let report = walk(&d.join("out")).unwrap_or_else(|| panic!("no report:\n{log}"));
    let r: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    r[0].clone()
}

fn walk(dir: &Path) -> Option<std::path::PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = walk(&p) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|n| n == "report.json") {
            return Some(p);
        }
    }
    None
}

/// Two gates on the storage's changes: the second's losses hit the bytes.
const GATED: &str = r#"[
  { add = "near", role = "gate", component = "net" },
  { node = "near", input = "in", from = "world" },
  { node = "near", knobs = { latency_ms = 10, resend = true, loss = 0.1, seed = 1 } },
  { add = "far", role = "gate", component = "net" },
  { node = "far", input = "in", from = "near" },
  { node = "far", knobs = { latency_ms = 5, loss = 0.05, mtu = 512, seed = 2 } },
  { node = "storage", input = "changes", from = "far" },
]"#;

#[test]
fn the_world_stream_through_lossy_gates() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gates");
    let r = run(&dir, "lossy", GATED);
    assert!(r["error"].is_null(), "{}", r["error"]);
    assert!(r["label"].as_str().unwrap().contains("gate"), "{}", r["label"]);
    let g = &r["gate"];
    let num = |k: &str| g[k].as_f64().unwrap_or_else(|| panic!("no gate.{k}: {g}"));
    assert!(num("messages") > 0.0 && num("bytes") > 0.0 && num("packets") > 0.0, "{g}");
    assert!(num("lost_packets") > 0.0, "{g}");
    assert!(num("encode_ms") > 0.0 && num("decode_ms") > 0.0, "{g}");
    assert!(r["frames"].as_u64().unwrap() > 0);
}

#[test]
fn a_gate_on_hits_is_refused_naming_it() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gates");
    let edits = r#"[
  { add = "g", role = "gate", component = "net" },
  { node = "g", input = "in", from = "visibility" },
  { node = "shading", input = "hits", from = "g" },
]"#;
    let d = dir.join("hits");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!("[scene]\ngenerator = {{ type = \"flat\" }}\n[run]\nframes = 1\n[[renderer]]\ngraph = \"raycast-cpu\"\nedits = {edits}\n[output]\ndir = {:?}\n", d.join("out").to_str().unwrap());
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer")).arg("run").arg(d.join("plan.toml")).env("RUST_LOG", "warn").output().unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
    assert!(log.contains("`g` on `shading.hits`") && log.contains("not on hits edges"), "{log}");
}
