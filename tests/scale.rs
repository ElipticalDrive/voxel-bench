//! Worlds past a pipeline's size limits: the renderer fails naming the limit, at the smallest
//! size that shows it (the stopping sizes per pipeline: `scripts/scale.py`, docs/limits.md).
//! Needs a GPU (`--ignored`; `scripts/check.py gpu` runs them).

use std::path::Path;
use std::process::Command;

/// The error of the one lane of a flat world at `radius` with `renderer`, on a device clamped
/// to WebGPU's baseline sizes (`MR_GPU_LIMITS=sizes`); None when it rendered.
fn lane_error(dir: &Path, renderer: &str, radius: u32) -> Option<String> {
    let d = dir.join(format!("r{radius}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
frames = 1
warmup = 0
resolution = [64, 36]
modes = ["unlit"]
view_radius = {radius}
[[renderer]]
{renderer}
[output]
dir = {:?}
save = {{ every = 0 }}
"#,
        d.join("out").to_str().unwrap(),
    );
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer")).arg("run").arg(d.join("plan.toml")).env("MR_GPU_LIMITS", "sizes").env("RUST_LOG", "warn").output().unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "the run failed:\n{log}");
    let report = walk(&d.join("out")).unwrap_or_else(|| panic!("no report:\n{log}"));
    let r: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    r[0]["error"].as_str().map(str::to_string)
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

/// The clipmap holds the view's chunks in one 3D texture per field, `2 radius + 1` chunks
/// wide: past `max_texture_dimension_3d` (2048 texels at the baseline: radius 31) chunks
/// evicted each other from their slots unreported (frames missing them). Now refused.
#[test]
#[ignore]
fn clipmap_past_its_texture_is_refused_naming_the_limit() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scale-clipmap");
    let clipmap = r#"graph = "raycast-gpu"
edits = [{ node = "storage", component = "clipmap" }]"#;
    assert_eq!(lane_error(&dir, clipmap, 31), None);
    let e = lane_error(&dir, clipmap, 32).expect("radius 32 rendered");
    assert!(e.contains("max_texture_dimension_3d") && e.contains("radius of 31 at most"), "{e}");
}
