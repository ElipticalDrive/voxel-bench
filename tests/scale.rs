//! Worlds past a pipeline's size limits: the renderer fails naming the limit, at the smallest
//! size that shows it (the stopping sizes per pipeline: `scripts/scale.py`, docs/limits.md).
//! Needs a GPU (`--ignored`; `scripts/check.py gpu` runs them).

use std::path::Path;
use std::process::Command;

/// The error of the one lane of a flat world at `radius` with `renderer`, on a device clamped
/// to WebGPU's baseline sizes (`MR_GPU_LIMITS=sizes`); None when it rendered.
fn lane_error(dir: &Path, renderer: &str, radius: u32) -> Option<String> {
    lane_error_at(dir, renderer, radius, [64, 36])
}

/// [`lane_error`] at a resolution.
fn lane_error_at(dir: &Path, renderer: &str, radius: u32, [w, h]: [u32; 2]) -> Option<String> {
    let d = dir.join(format!("r{radius}-{w}x{h}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
frames = 1
warmup = 0
resolution = [{w}, {h}]
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

/// A run of `generator` at `radius` with `renderer` (one lit frame, saved), on the device as
/// it is or clamped (`MR_GPU_LIMITS`): the lane's error, the frame's PNG bytes and the log.
fn frame(dir: &Path, generator: &str, renderer: &str, radius: u32, limits: Option<&str>) -> (Option<String>, Vec<u8>, String) {
    let d = dir.join(format!("r{radius}-{}", limits.unwrap_or("device")));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let plan = format!(
        r#"[scene]
generator = {generator}
camera = {{ type = "line", from = [0, 60, 0], to = [40, 60, 0] }}
[run]
frames = 1
warmup = 0
resolution = [96, 54]
modes = ["lit"]
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
    let mut c = Command::new(env!("CARGO_BIN_EXE_multirenderer"));
    c.arg("run").arg(d.join("plan.toml")).env("RUST_LOG", "info");
    if let Some(l) = limits {
        c.env("MR_GPU_LIMITS", l);
    }
    let o = c.output().unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "the run failed:\n{log}");
    let report = walk(&d.join("out")).unwrap_or_else(|| panic!("no report:\n{log}"));
    let r: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    let png = find_png(report.parent().unwrap()).map(|p| std::fs::read(p).unwrap()).unwrap_or_default();
    // The worker's own log (where the executor says how it bound the data).
    let workers: String = std::fs::read_dir(report.parent().unwrap().join("logs")).into_iter().flatten().flatten().map(|e| std::fs::read_to_string(e.path()).unwrap_or_default()).collect();
    (r[0]["error"].as_str().map(str::to_string), png, log + &workers)
}

fn find_png(dir: &Path) -> Option<std::path::PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find_png(&p) {
                return Some(f);
            }
        } else if p.extension().is_some_and(|e| e == "png") && !p.to_string_lossy().contains("diff") {
            return Some(p);
        }
    }
    None
}

/// Data a device binds less of than a world needs is split over several bindings: a pool
/// storage's side data (`distance`'s fields, 32 KiB a chunk: past 4096 chunks at the
/// baseline's 128 MiB) and the splats' points (16 bytes a point: past 8 M). Each failed
/// naming `max_storage_buffer_binding_size` (lavapipe refused them so); now the clamped
/// device renders the same pixels as the device as it is, and its log shows the pages.
/// `distance` at the baseline (`sizes`, a flat world past 4096 chunks); the splats on
/// bindings of 16 MiB (`binding:16`: 1 M points a page; 8 M points take a large world).
#[test]
#[ignore]
fn side_data_and_points_past_the_binding_size_render_over_several_bindings() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scale-pages");
    let flat = r#"{ type = "flat" }"#;
    let distance = r#"graph = "raycast-gpu"
edits = [{ node = "storage", component = "distance" }]"#;
    let splat = r#"graph = "splat-gpu""#;
    for (name, generator, renderer, radius, limits, shows) in [("distance", flat, distance, 40, "sizes", "side data"), ("splat", r#"{ type = "lattice" }"#, splat, 10, "binding:16", "points")] {
        let d = dir.join(name);
        let (e, clamped, log) = frame(&d, generator, renderer, radius, Some(limits));
        assert_eq!(e, None, "{name} at radius {radius}, clamped:\n{log}");
        assert!(log.contains(&format!("{shows} over")), "{name}: no pages in the log:\n{log}");
        let (e, plain, log) = frame(&d, generator, renderer, radius, None);
        assert_eq!(e, None, "{name} at radius {radius}:\n{log}");
        assert!(!plain.is_empty() && clamped == plain, "{name}: the clamped device rendered other pixels");
    }
}


/// Frames are 2D textures: a side past `max_texture_dimension_2d` (8192 at the baseline)
/// failed with a bare "wgpu error: Validation Error". Now refused naming the limit.
#[test]
#[ignore]
fn a_frame_past_the_texture_limit_is_refused_naming_it() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scale-frame");
    let raycast = r#"graph = "raycast-gpu""#;
    assert_eq!(lane_error_at(&dir, raycast, 2, [8192, 8]), None);
    let e = lane_error_at(&dir, raycast, 2, [8193, 8]).expect("8193 wide rendered");
    assert!(e.contains("max_texture_dimension_2d") && e.contains("8192"), "{e}");
}
