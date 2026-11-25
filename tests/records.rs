//! A run's records at scale, driving the built binary on a CPU pipeline at a tiny resolution:
//! the host's memory does not grow with the passes it runs (their frames live in the journal,
//! `report.json` is written from it at the end), and a disk that fills up stops the run loudly,
//! leaving it resumable, whether it fills during a pass or while the records are written.
//!
//! The full disk is a real one: a small tmpfs mounted in a user and mount namespace of the
//! test's own (`unshare -rm`, no root), filled by a file but for the room the case needs, the
//! file removed afterwards to free the space. Without unprivileged user namespaces (Ubuntu's AppArmor default; CI lifts it)
//! the test fails, naming that.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_multirenderer")
}

/// A plan of `passes` repetitions of one CPU pass of `frames` frames, writing into `out`.
fn plan(name: &str, passes: u32, frames: u32, out: &Path) -> String {
    format!(
        r#"name = "{name}"
[scene]
generator = {{ type = "flat" }}
camera = {{ type = "line", from = [0, 20, 0], to = [40, 20, 0] }}
[run]
frames = {frames}
warmup = 0
resolution = [32, 18]
view_radius = 1
parallel = false
repeats = {passes}
[[renderer]]
graph = "raycast-cpu"
[output]
dir = {out:?}
resume = true
"#
    )
}

fn dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn the_run(out: &Path) -> PathBuf {
    let mut v: Vec<_> = std::fs::read_dir(out).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    assert_eq!(v.len(), 1, "{v:?}");
    v.pop().unwrap()
}

/// Runs a plan to its end: the host's peak memory in MB (it prints it) and the report's size.
fn peak(d: &Path, passes: u32, frames: u32) -> (f64, u64) {
    let out = d.join(format!("out-{passes}"));
    let file = d.join(format!("plan-{passes}.toml"));
    std::fs::write(&file, plan("records", passes, frames, &out)).unwrap();
    let o = Command::new(bin()).arg("run").arg(&file).output().unwrap();
    let log = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{log}");
    let mb = log.lines().find_map(|l| l.strip_prefix("host peak memory: ")?.strip_suffix(" MB")?.parse().ok()).unwrap_or_else(|| panic!("no peak printed:\n{log}"));
    let report = the_run(&out).join("report.json");
    let r: serde_json::Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert_eq!(r.as_array().unwrap().len(), passes as usize);
    assert!(r.as_array().unwrap().iter().all(|p| p["per_frame"].as_array().unwrap().len() == frames as usize), "every frame of every pass in the report");
    (mb, std::fs::metadata(&report).unwrap().len())
}

#[test]
fn a_runs_memory_does_not_grow_with_its_passes() {
    let d = dir("records-memory");
    let frames = 600;
    let (small, small_bytes) = peak(&d, 6, frames);
    let (big, big_bytes) = peak(&d, 30, frames);
    let more_mb = (big_bytes - small_bytes) as f64 / 1048576.0;
    // Held in memory, the 24 more passes' frames took about twice their size in the report
    // (0.46 MB a pass of 600 frames against 0.21 MB on disk); now each is dropped once journaled.
    assert!(big - small < more_mb / 2.0, "the host's peak grew {:.1} MB ({small:.0} -> {big:.0}) for {more_mb:.1} MB more frames in the report", big - small);
}

/// Runs `script` (sh) in a user and mount namespace of its own.
fn in_namespace(script: &str) -> std::process::Output {
    let probe = Command::new("unshare").args(["-rm", "true"]).output();
    match probe {
        Ok(o) if o.status.success() => {}
        Ok(o) => panic!(
            "unprivileged user namespaces are needed for the full-disk test (`unshare -rm` failed: {}); on Ubuntu: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => panic!("`unshare` (util-linux) is needed for the full-disk test: {e}"),
    }
    Command::new("unshare").args(["-rm", "sh", "-c", script]).output().unwrap()
}

/// Kilobytes, rounded up to whole pages (tmpfs's unit).
fn pages(bytes: u64) -> u64 {
    bytes.div_ceil(4096) * 4
}

#[test]
fn a_full_disk_stops_the_run_loudly_and_it_resumes_once_there_is_room() {
    let d = dir("records-full-disk");
    let (passes, frames) = (8, 600);
    // The size of the records on a disk with room, to size the small disks by.
    let (_, report_bytes) = peak(&d, passes, frames);
    // Two disks: one that fills during the passes (the journal outgrows it), one that holds the
    // journal but not the records written from it at the end (journal, observations, report).
    for (case, kb) in [("pass", pages(report_bytes * 2 / 5)), ("records", pages(report_bytes * 3 / 2))] {
        let mnt = d.join(case);
        std::fs::create_dir_all(&mnt).unwrap();
        let file = d.join(format!("plan-{case}.toml"));
        std::fs::write(&file, plan(case, passes, frames, &mnt)).unwrap();
        let (first, second, copy) = (d.join(format!("{case}-1.log")), d.join(format!("{case}-2.log")), d.join(format!("{case}-copy")));
        // Mounted (a remount to resize it is refused in a user namespace, so it is filled but
        // for {kb} kB), run (the disk fills), a copy of what it left, then the filler removed,
        // run again (resumed), and a copy of the end: the mount goes with the namespace.
        let filler = 64 * 1024 - kb;
        let script = format!(
            "set -u; mount -t tmpfs -o size=64m,uid=0,gid=0 tmpfs {mnt:?} || exit 90; mkdir {mnt:?}/fill; dd if=/dev/zero of={mnt:?}/fill/x bs=1k count={filler} 2>/dev/null || exit 91; {bin:?} run {file:?} >{first:?} 2>&1; echo $? >>{first:?}; mkdir -p {copy:?}/1 {copy:?}/2; rm -r {mnt:?}/fill; cp -a {mnt:?}/. {copy:?}/1/; {bin:?} run {file:?} >{second:?} 2>&1; echo $? >>{second:?}; cp -a {mnt:?}/. {copy:?}/2/",
            bin = bin()
        );
        let o = in_namespace(&script);
        assert!(o.status.success(), "[{case}] the namespace script: {o:?}");
        let (log1, log2) = (std::fs::read_to_string(&first).unwrap(), std::fs::read_to_string(&second).unwrap());
        let status = |log: &str| log.lines().last().unwrap().trim().parse::<i32>().unwrap();
        // The first run stops, saying why and what is left; nothing half written stays.
        assert_ne!(status(&log1), 0, "[{case}] a {kb} kB disk should stop the run:\n{log1}");
        assert!(log1.contains("No space left on device"), "[{case}] the cause named:\n{log1}");
        assert!(log1.contains(if case == "pass" { "journal" } else { "the run stays unfinished" }), "[{case}]:\n{log1}");
        let left = the_run(&copy.join("1"));
        assert!(left.join("passes.jsonl").exists() && !left.join("report.json").exists(), "[{case}] unfinished: a journal, no report");
        let names: Vec<String> = std::fs::read_dir(&left).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(!names.iter().any(|n| n.ends_with(".part") || n.ends_with(".new")), "[{case}] parts removed: {names:?}");
        let journal = std::fs::read_to_string(left.join("passes.jsonl")).unwrap();
        let finished = journal.lines().filter(|l| l.starts_with("{\"done\"")).count();
        if case == "pass" {
            assert!((1..passes as usize).contains(&finished), "[{case}] the disk should fill part-way: {finished} of {passes} passes journaled");
        } else {
            assert_eq!(finished, passes as usize, "[{case}] every pass journaled, the records not written");
        }
        // Resumed with room: the passes it finished stand, the rest run, every record written.
        assert_eq!(status(&log2), 0, "[{case}] resumed with room:\n{log2}");
        assert!(log2.contains("(resumed)") && log2.contains(&format!("resumed: {finished} reports")), "[{case}]:\n{log2}");
        let done = the_run(&copy.join("2"));
        assert_eq!(done, copy.join("2").join(left.file_name().unwrap()), "[{case}] the same run");
        assert!(!done.join("passes.jsonl").exists());
        let r: serde_json::Value = serde_json::from_slice(&std::fs::read(done.join("report.json")).unwrap()).unwrap();
        let mut repeats: Vec<u64> = r.as_array().unwrap().iter().map(|p| p["repeat"].as_u64().unwrap()).collect();
        repeats.sort();
        assert_eq!(repeats, (0..passes as u64).collect::<Vec<_>>(), "[{case}] every pass once");
        assert!(r.as_array().unwrap().iter().all(|p| p["error"].is_null() && p["per_frame"].as_array().unwrap().len() == frames as usize));
        assert_eq!(std::fs::read_to_string(done.join("observations.jsonl")).unwrap().lines().count(), passes as usize);
    }
}
