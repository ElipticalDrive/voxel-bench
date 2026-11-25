//! The host outlives any renderer: each injected failure (moment x way, `MR_FAULT_INJECTION`)
//! in each isolation, driving the built binary on a two-lane plan (the faulty lane beside a
//! healthy one). The run must end normally, the faulty lane's report say why, the healthy
//! lane run through; or, for the failures a thread cannot survive (abort, segfault), the
//! program end as documented.
//!
//! Not here: `knob` (a scripted run changes no knob live: the GUI's tests), `oom` in a thread
//! (it would exhaust the test machine's memory: the host shares it, that isolation's cost).
//! `oom` in a process runs under `prlimit --as` so the allocation is refused, not the machine
//! swapped.

use std::path::{Path, PathBuf};
use std::process::Command;

const MOMENTS: [&str; 5] = ["build", "first", "mid", "world", "shutdown"];
const WAYS: [&str; 5] = ["error", "panic", "abort", "segv", "hang"];

/// Software CI runners are slower: deadlines scale by `MR_TEST_SLOW`.
fn slow() -> f64 {
    std::env::var("MR_TEST_SLOW").ok().and_then(|v| v.parse::<f64>().ok()).filter(|f| *f >= 1.0).unwrap_or(1.0)
}

struct Outcome {
    case: String,
    status: std::process::ExitStatus,
    /// The faulty lane's (error, shutdown_error), the healthy lane's error; None without a report.
    faulty: Option<(Option<String>, Option<String>)>,
    healthy: Option<(Option<String>, Option<String>)>,
    log: String,
}

fn run(dir: &Path, isolation: &str, fault: &str, launcher: &[&str]) -> Outcome {
    let case = format!("{isolation} {fault}");
    let d = dir.join(format!("{isolation}-{}", fault.replace(':', "-")));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let launcher = launcher.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ");
    let s = slow();
    let plan = format!(
        r#"[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
frames = 15
warmup = 0
resolution = [64, 36]
view_radius = 1
isolation = "{isolation}"
launcher = [{launcher}]
watchdog = {{ build_s = {}, factor = 20, min_s = {} }}
[[renderer]]
graph = "raycast-cpu"
label = "faulty"
options = {{ fault = "{fault}" }}
[[renderer]]
graph = "raycast-cpu"
label = "healthy"
[output]
dir = {:?}
save = {{ every = 0 }}
"#,
        5.0 * s,
        2.0 * s,
        d.join("out").to_str().unwrap(),
    );
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_multirenderer"))
        .arg("run")
        .arg(d.join("plan.toml"))
        .env("MR_FAULT_INJECTION", "1")
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
    let report = find(&d.join("out"), "report.json").map(|p| serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).unwrap()).unwrap());
    let lane = |label: &str| report.as_ref().and_then(|r| r.as_array()?.iter().find(|l| l["label"] == label).cloned());
    let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|e| e.as_str()).map(String::from);
    Outcome {
        case,
        status: o.status,
        faulty: lane("faulty").map(|l| (text(&l, "error"), text(&l, "shutdown_error"))),
        healthy: lane("healthy").map(|l| (text(&l, "error"), text(&l, "shutdown_error"))),
        log,
    }
}

fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find(&p, name) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
    }
    None
}

/// What the faulty lane's report must say, per way (a substring of its error or shutdown error).
fn says(how: &str, isolation: &str) -> &'static str {
    match (how, isolation) {
        ("error", _) => "injected fault",
        ("panic", _) => "panicked: injected fault",
        ("abort", _) => "SIGABRT",
        ("segv", _) => "SIGSEGV",
        ("hang", "process") => "killed",
        ("hang", _) => "abandoned",
        ("oom", _) => "allocation refused",
        _ => unreachable!(),
    }
}

fn check(o: &Outcome, isolation: &str, at: &str, how: &str) -> Result<(), String> {
    let fail = |why: &str| Err(format!("{}: {why}\n--- log ---\n{}", o.case, o.log.lines().rev().take(25).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")));
    // A thread shares the host's process: an abort or a segfault ends it (the defined cost).
    if isolation == "thread" && matches!(how, "abort" | "segv") {
        return if o.status.success() { fail("the program survived what a thread cannot") } else { Ok(()) };
    }
    if !o.status.success() {
        return fail(&format!("the program ended {}", o.status));
    }
    let Some((error, shutdown)) = &o.faulty else { return fail("no report of the faulty lane") };
    if o.healthy != Some((None, None)) {
        return fail(&format!("the healthy lane: {:?}", o.healthy));
    }
    let (got, other) = if at == "shutdown" { (shutdown, error) } else { (error, shutdown) };
    if other.is_some() && at == "shutdown" {
        return fail(&format!("a shutdown fault failed the pass: {other:?}"));
    }
    match got {
        Some(e) if e.contains(says(how, isolation)) => Ok(()),
        _ => fail(&format!("expected the report to say `{}`, got error {error:?}, shutdown {shutdown:?}", says(how, isolation))),
    }
}

fn matrix(isolation: &str) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("survival");
    let mut cases: Vec<(String, String, Vec<&str>)> = MOMENTS.iter().flat_map(|at| WAYS.iter().map(move |how| (at.to_string(), how.to_string(), vec![]))).collect();
    if isolation == "process" && Command::new("prlimit").arg("--version").output().is_ok_and(|o| o.status.success()) {
        // 4 GiB of address space: the renderer's own needs fit, the blow-up is refused.
        cases.push(("mid".into(), "oom".into(), vec!["prlimit", "--as=4294967296"]));
    }
    // A few at a time: each is a whole run, and a crowded machine would trip the watchdog.
    let width = std::thread::available_parallelism().map_or(2, |n| (n.get() / 4).max(1));
    let mut failures = vec![];
    for chunk in cases.chunks(width) {
        let outcomes: Vec<_> = std::thread::scope(|s| {
            let dir = &dir;
            let hs: Vec<_> = chunk.iter().map(|(at, how, l)| s.spawn(move || (run(dir, isolation, &format!("{at}:{how}"), l), at, how))).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (o, at, how) in outcomes {
            if let Err(e) = check(&o, isolation, at, how) {
                failures.push(e);
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} cases failed:\n\n{}", failures.len(), cases.len(), failures.join("\n\n"));
}

#[test]
fn survives_every_fault_in_a_process() {
    matrix("process");
}

#[test]
fn survives_or_ends_as_documented_in_a_thread() {
    matrix("thread");
}

/// A sweep (as `scripts/sweep.py` runs it: passes one after another, `run.parallel = false`)
/// goes on past failing passes: each is recorded in `observations.jsonl` with its reason, the
/// passes after it run, and the catalogue lists it as failed.
#[test]
fn a_sweep_records_failed_passes_and_goes_on() {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("survival").join("sweep");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let s = slow();
    let faults = [("crash", "mid:segv", "SIGSEGV"), ("hangs", "first:hang", "killed"), ("errs", "build:error", "injected fault")];
    let mut plan = format!(
        r#"[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
frames = 15
warmup = 0
resolution = [64, 36]
view_radius = 1
parallel = false
watchdog = {{ build_s = {}, factor = 20, min_s = {} }}
[output]
dir = {:?}
save = {{ every = 0 }}
[[renderer]]
graph = "raycast-cpu"
label = "before"
"#,
        5.0 * s,
        2.0 * s,
        d.join("out").to_str().unwrap(),
    );
    for (label, fault, _) in faults {
        plan += &format!("[[renderer]]\ngraph = \"raycast-cpu\"\nlabel = \"{label}\"\noptions = {{ fault = \"{fault}\" }}\n");
    }
    plan += "[[renderer]]\ngraph = \"raycast-cpu\"\nlabel = \"after\"\n";
    std::fs::write(d.join("plan.toml"), plan).unwrap();
    let exe = env!("CARGO_BIN_EXE_multirenderer");
    let o = Command::new(exe).arg("run").arg(d.join("plan.toml")).env("MR_FAULT_INJECTION", "1").env("RUST_LOG", "warn").output().unwrap();
    let log = String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "the sweep ended {}:\n{log}", o.status);
    let obs = std::fs::read_to_string(find(&d.join("out"), "observations.jsonl").expect("observations.jsonl")).unwrap();
    let records: Vec<serde_json::Value> = obs.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let error = |label: &str| {
        let r = records.iter().find(|r| r.to_string().contains(&format!("\"{label}\""))).unwrap_or_else(|| panic!("no record of `{label}` in {obs}"));
        r["error"].as_str().map(String::from)
    };
    for label in ["before", "after"] {
        assert_eq!(error(label), None, "{label}");
    }
    for (label, _, says) in faults {
        let e = error(label).unwrap_or_else(|| panic!("`{label}` recorded as passed"));
        assert!(e.contains(says), "`{label}`: {e}");
    }
    let o = Command::new(exe).arg("catalogue").arg(d.join("out")).output().unwrap();
    assert!(o.status.success(), "catalogue: {}", String::from_utf8_lossy(&o.stderr));
    let cat = std::fs::read_to_string(d.join("out").join("catalogue.md")).unwrap();
    for (label, _, says) in faults {
        assert!(cat.lines().any(|l| l.contains(label) && l.contains("FAILED") && l.contains(says)), "the catalogue does not list `{label}` as failed:\n{cat}");
    }
}
