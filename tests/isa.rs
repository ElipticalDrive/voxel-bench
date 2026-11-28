//! The CPU ray caster's hot loop is pixel-equal whichever instruction set it is dispatched to
//! (`isa` = scalar, avx2, avx512): a CPU without AVX-512 (or without AVX2) renders exactly what
//! this one does. Every storage x walk x shading (refloor's packets of 1 and 8), on terrain and
//! the lattice, unlit and lit, rendered once per instruction set in one run and compared frame
//! by frame. An instruction set this CPU lacks falls back to scalar (still compared, so the test
//! runs anywhere; the log says which loop ran). The other CPU regions have no dispatch: the
//! generic build targets baseline x86-64, and a pipeline compiled for a CPU is checked pixel for
//! pixel against it before use (`src/pipelines.rs`; `MR_TARGET_CPU` compiles for another CPU).

#![cfg(target_arch = "x86_64")]

use std::path::Path;
use std::process::Command;

const WORLDS: [(&str, &str); 2] = [
    ("terrain", r#"generator = { type = "terrain", seed = 7 }
camera = { type = "flythrough", points = [[0, 110, 0], [260, 140, 120]] }
lights = [{ type = "scatter", count = 20, seed = 1, center = [130, 80, 60], extent = [200, 60, 120], on_surface = true, intensity = 25.0, radius = 18.0 }]"#),
    ("lattice", r#"generator = { type = "lattice" }
camera = { type = "spin", period = 300, pitch_deg = -10 }"#),
];

/// The instruction sets, in the order their lanes are listed: the run names a lane's repeats
/// `<label>`, `<label>__2`, `<label>__3`.
const ISAS: [&str; 3] = ["scalar", "avx2", "avx512"];

/// Every storage x walk x shading, and refloor in packets of 8, with the hot loop on `isa`.
fn renderers(isa: &str) -> String {
    let vary = r#"vary = [{ node = "storage", component = "*" }, { node = "kernel", component = "*" }, { node = "shading", component = "*" }]"#;
    format!(
        r#"{{ graph = "raycast-cpu", options = {{ isa = "{isa}" }}, {vary} }}, {{ graph = "raycast-cpu", options = {{ isa = "{isa}" }}, edits = [{{ node = "kernel", knobs = {{ packet = "8" }} }}], vary = [{{ node = "storage", component = "*" }}] }}"#
    )
}

fn files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.ends_with("-diff.png")).collect();
    v.sort();
    v
}

#[test]
fn every_instruction_set_renders_the_same_pixels() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("isa");
    for (world, scene) in WORLDS {
        let out = dir.join(world);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let plan = format!(
            "name = \"isa\"\n[scene]\n{scene}\n[run]\nframes = 1\nwarmup = 0\nresolution = [64, 36]\nmodes = [\"unlit\", \"lit\"]\nview_radius = 3\nisolation = \"thread\"\n\
             [output]\ndir = {:?}\nsave = {{ every = 1 }}\n",
            out.join("run").to_str().unwrap()
        );
        std::fs::write(out.join("plan.toml"), plan).unwrap();
        let list = ISAS.map(renderers).join(", ");
        let o = Command::new(env!("CARGO_BIN_EXE_multirenderer")).arg("run").arg(out.join("plan.toml")).args(["-s", &format!("renderer=[{list}]")]).env("RUST_LOG", "info").output().unwrap();
        let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
        assert!(o.status.success(), "{world} failed:\n{log}");
        // Each loop this CPU has ran (not a silent fallback to scalar).
        let has = [true, std::arch::is_x86_feature_detected!("avx2"), std::arch::is_x86_feature_detected!("avx512f") && std::arch::is_x86_feature_detected!("avx512bw")];
        for (name, has) in ["Scalar", "Avx2", "Avx512"].into_iter().zip(has) {
            assert!(!has || log.contains(&format!("{name} hot loop")), "{world}: no lane ran the {name} loop");
        }
        let run = std::fs::read_dir(out.join("run")).unwrap().flatten().map(|e| e.path()).find(|p| p.join("frames").is_dir()).unwrap_or_else(|| panic!("no frames:\n{log}"));
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(run.join("report.json")).unwrap()).unwrap();
        let failed: Vec<String> = report.as_array().unwrap().iter().filter_map(|o| o["error"].as_str().map(|e| format!("{} {}: {e}", o["label"], o["mode"]))).collect();
        assert!(failed.is_empty(), "{world}: passes failed: {failed:?}");
        let frames = run.join("frames");
        let all = files(&frames);
        let repeat = |n: &str| n.rsplit_once('-').is_some_and(|(l, _)| l.rsplit_once("__").is_some_and(|(_, k)| k.parse::<u32>().is_ok()));
        // A lane outside raycast-cpu's ISA dispatch (the heightfield kernel runs in the column
        // march's region, which has no `isa`) is one renderer whatever the option: it runs once.
        let dispatched = |n: &str| !n.contains("heightfield");
        let lanes: Vec<&String> = all.iter().filter(|n| !repeat(n) && dispatched(n)).collect();
        assert!(lanes.len() >= 20, "{world}: only {} lanes x modes: {all:?}", lanes.len());
        let once = all.iter().filter(|n| !dispatched(n)).count();
        assert_eq!(all.len() - once, 3 * lanes.len(), "{world}: every lane once per instruction set: {all:?}");
        let mut differ = vec![];
        for lane in &lanes {
            // `<label>-<mode>`: the repeats insert `__N` before the mode.
            let (label, mode) = lane.rsplit_once('-').unwrap();
            let base = frames.join(lane);
            for (n, isa) in [(2, ISAS[1]), (3, ISAS[2])] {
                let other = frames.join(format!("{label}__{n}-{mode}"));
                for f in files(&base) {
                    let a = image::open(base.join(&f)).unwrap().to_rgb8();
                    let b = image::open(other.join(&f)).unwrap_or_else(|e| panic!("{world} {isa} {}: {e}", other.display())).to_rgb8();
                    let px = a.pixels().zip(b.pixels()).filter(|(x, y)| x != y).count();
                    if px > 0 || a.dimensions() != b.dimensions() {
                        differ.push(format!("{lane} on {isa}, {f}: {px} pixels"));
                    }
                }
            }
        }
        assert!(differ.is_empty(), "{world}: {} frames differ from scalar: {}", differ.len(), differ.join("; "));
    }
}
