//! A run that dies part-way resumes where it stopped (`output.resume`): driving the built
//! binary on a three-pass plan whose second pass takes the whole program down (an injected
//! abort in a thread-isolated worker, `MR_FAULT_INJECTION`). The first run dies in it; the
//! second, resumed, keeps the first pass and dies in it again; the third records it as failed
//! with the reason, runs the third pass and finishes: every pass in the report, the first one
//! run once.
//!
//! Not here: a full disk (needs a small filesystem mounted for the test). A failed journal write
//! ends the run like any death, and resuming continues it the same way.

use std::path::Path;
use std::process::Command;

fn run(plan: &Path) -> (std::process::ExitStatus, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer"))
        .arg("run")
        .arg(plan)
        .env("MR_FAULT_INJECTION", "1")
        .env("RUST_LOG", "info")
        .output()
        .unwrap();
    (
        o.status,
        String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout),
    )
}

fn runs(out: &Path) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(out)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn a_run_that_dies_resumes_where_it_stopped() {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("resume");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let out = d.join("out");
    let renderer = |label: &str, fault: Option<&str>| {
        let opts = fault
            .map(|f| format!("options = {{ fault = \"{f}\" }}\n"))
            .unwrap_or_default();
        format!("[[renderer]]\ngraph = \"raycast-cpu\"\nlabel = \"{label}\"\n{opts}")
    };
    let plan = format!(
        r#"name = "resume"
[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
frames = 8
warmup = 0
resolution = [64, 36]
view_radius = 1
isolation = "thread"
parallel = false
{}{}{}[output]
dir = {:?}
resume = true
"#,
        renderer("one", None),
        renderer("two", Some("build:abort")),
        renderer("three", None),
        out.to_str().unwrap(),
    );
    let plan_file = d.join("plan.toml");
    std::fs::write(&plan_file, plan).unwrap();

    let (status, log) = run(&plan_file);
    assert!(
        !status.success(),
        "the abort in a thread should end the program:\n{log}"
    );
    let dirs = runs(&out);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    let run_dir = &dirs[0];
    assert!(run_dir.join("passes.jsonl").exists() && !run_dir.join("report.json").exists());
    let journal = std::fs::read_to_string(run_dir.join("passes.jsonl")).unwrap();
    let first: serde_json::Value = journal
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find_map(|e| e.get("report").filter(|r| r["label"] == "one").cloned())
        .expect("the first pass journaled");

    let (status, log) = run(&plan_file);
    assert!(
        !status.success(),
        "resumed, the second pass dies once more:\n{log}"
    );
    assert!(log.contains("(resumed)"), "{log}");
    assert_eq!(runs(&out).len(), 1, "resumed in the same directory");

    let (status, log) = run(&plan_file);
    assert!(
        status.success(),
        "the third run gives the second pass up and finishes:\n{log}"
    );
    assert_eq!(runs(&out).len(), 1);
    assert!(
        !run_dir.join("passes.jsonl").exists(),
        "a finished run's journal is removed"
    );
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("report.json")).unwrap())
            .unwrap();
    let lane = |l: &str| {
        report
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["label"] == l)
            .cloned()
            .collect::<Vec<_>>()
    };
    let (one, two, three) = (lane("one"), lane("two"), lane("three"));
    assert_eq!((one.len(), two.len(), three.len()), (1, 1, 1), "{report:#}");
    assert_eq!(
        one[0]["wall_s"], first["wall_s"],
        "the first pass stands from the first run"
    );
    assert!(one[0]["error"].is_null() && three[0]["error"].is_null());
    assert!(
        two[0]["error"]
            .as_str()
            .unwrap()
            .contains("died during this pass each of the 2 times"),
        "{}",
        two[0]
    );
    let observations = std::fs::read_to_string(run_dir.join("observations.jsonl")).unwrap();
    assert_eq!(observations.lines().count(), 3, "{observations}");
}
