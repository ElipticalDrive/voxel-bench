//! Running full benchmarks from the GUI (as a child `multirenderer run`) and browsing
//! their reports.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use egui::Ui;
use serde_json::Value;

use crate::plan::Plan;

pub struct BenchRun {
    pub lines: Arc<Mutex<Vec<String>>>,
    pub report: Arc<Mutex<Option<PathBuf>>>,
    child: Child,
    pub finished: bool,
}

impl BenchRun {
    pub fn start(plan: &Plan, overrides: &[String]) -> anyhow::Result<Self> {
        let dir = PathBuf::from(plan.output.dir.clone().unwrap_or_else(|| "runs".into())).join(".gui");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.toml", crate::session::sanitize(&plan.name)));
        std::fs::write(&path, plan.to_toml()?)?;
        let mut child = Command::new(crate::worker::self_exe()?)
            .arg("run")
            .arg(&path)
            .args(overrides.iter().flat_map(|o| ["-s", o.as_str()]))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NO_COLOR", "1")
            .spawn()?;
        let lines = Arc::new(Mutex::new(vec![format!("$ multirenderer run {}", path.display())]));
        let report = Arc::new(Mutex::new(None));
        for stream in [Box::new(child.stdout.take().unwrap()) as Box<dyn std::io::Read + Send>, Box::new(child.stderr.take().unwrap())] {
            let (lines, report) = (lines.clone(), report.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let line = strip_ansi(&line);
                    if let Some(p) = line.strip_prefix("report: ") {
                        *report.lock().unwrap() = Some(PathBuf::from(p.trim()));
                    }
                    lines.lock().unwrap().push(line);
                }
            });
        }
        Ok(Self { lines, report, child, finished: false })
    }

    /// Returns true once the child process has exited.
    pub fn poll(&mut self) -> bool {
        if !self.finished
            && let Ok(Some(status)) = self.child.try_wait() {
            self.lines.lock().unwrap().push(format!("(benchmark exited: {status})"));
            self.finished = true;
        }
        self.finished
    }

    pub fn cancel(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for BenchRun {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
        }
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub struct Report {
    pub path: PathBuf,
    pub passes: Vec<Value>,
}

pub fn load_report(path: &Path) -> anyhow::Result<Report> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let passes = v.as_array().cloned().unwrap_or_default();
    Ok(Report { path: path.to_path_buf(), passes })
}

/// `runs/*/report.json`, newest first.
pub fn past_runs(base: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("report.json"))
        .filter(|p| p.exists())
        .collect();
    v.sort();
    v.reverse();
    v
}

fn f(v: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_f64()
}

/// One cell of the report table: its text and, where it has more to say, a hover.
pub struct Cell {
    pub text: String,
    pub hover: Option<String>,
}

fn cell(text: String) -> Cell {
    Cell { text, hover: None }
}

fn num(v: Option<f64>, d: usize) -> Option<Cell> {
    v.map(|x| cell(format!("{x:.d$}")))
}

/// A column of the report table after the label and mode: its header, the header's hover,
/// and a pass's cell (None: "-").
pub struct Column {
    pub name: &'static str,
    pub hover: &'static str,
    pub cell: fn(&Value) -> Option<Cell>,
}

/// The columns `passes` fill: every pass has the first ones; the rest (the like score,
/// convergence, world updates, deadlines, streaming) show only when some pass that ran
/// reports them.
pub fn columns(passes: &[Value]) -> Vec<Column> {
    let ok: Vec<&Value> = passes.iter().filter(|p| p.get("error").is_none_or(Value::is_null)).collect();
    let always = [
        Column { name: "fps", hover: "Frames completed per second of wall time over the pass", cell: |p| num(f(p, &["fps"]), 1) },
        Column { name: "frame ms", hover: "Mean time between completed frames", cell: |p| num(f(p, &["frame_interval_ms", "mean"]), 2) },
        Column { name: "p99 ms", hover: "99th-percentile time between completed frames (hitches)", cell: |p| num(f(p, &["frame_interval_ms", "p99"]), 2) },
        Column { name: "gpu ms", hover: "Mean GPU execution time per frame (timestamp queries)", cell: |p| num(f(p, &["gpu_ms", "mean"]), 2) },
        Column { name: "cpu ms", hover: "Mean renderer-process CPU time per frame", cell: |p| num(f(p, &["cpu_ms", "mean"]), 2) },
        Column { name: "latency ms", hover: "Mean time from a frame's request to its completion", cell: |p| num(f(p, &["latency_ms", "mean"]), 2) },
        Column { name: "PSNR", hover: "Mean PSNR vs the reference renderer in dB (higher is better, 99 = exact)", cell: |p| num(f(p, &["quality", "psnr", "mean"]), 1) },
        Column {
            name: "settled dB",
            hover: "PSNR of the frames rendered with the camera held still long enough to settle (accumulating pipelines: converged); the score the catalogue's floor uses where present",
            cell: |p| num(f(p, &["quality_settled", "psnr", "mean"]), 1),
        },
        Column { name: "bad %", hover: "Mean share of pixels off by more than 16/255 vs the reference", cell: |p| num(f(p, &["quality", "bad_pct", "mean"]), 2) },
        Column { name: "init ms", hover: "Renderer start-up time (device, pipelines)", cell: |p| num(f(p, &["init_ms"]), 0) },
        Column {
            name: "shaders ms",
            hover: "The part of init ms spent on shaders: composing and validating modules, and creating pipelines (the driver's compile)",
            cell: |p| {
                let s = [f(p, &["shaders", "module_ms"]), f(p, &["shaders", "pipeline_ms"])];
                let n = |v: Option<f64>| v.map_or("-".into(), |x| format!("{x:.0}"));
                (!s.iter().all(Option::is_none)).then(|| Cell { text: format!("{} + {}", n(s[0]), n(s[1])), hover: Some("modules + pipelines".into()) })
            },
        },
        Column { name: "RSS MB", hover: "Peak resident memory of the renderer process", cell: |p| num(f(p, &["peak_rss_mb"]), 0) },
        Column { name: "GPU MB", hover: "GPU memory the renderer counts holding", cell: |p| num(f(p, &["memory", "gpu_bytes"]).filter(|x| *x > 0.0).map(|x| x / 1048576.0), 0) },
        Column {
            name: "device MB",
            hover: "Measured: the most its GPU devices' allocators held (rounding and transient memory included)",
            cell: |p| num(f(p, &["memory", "gpu_device_bytes"]).filter(|x| *x > 0.0).map(|x| x / 1048576.0), 0),
        },
    ];
    let optional = [
        Column {
            name: "like dB",
            hover: "PSNR against a reference given what the pipeline was given (its LOD region, `like`): a lossy pipeline scores low in PSNR and high here, a wrong one low in both",
            cell: |p| {
                let v = f(p, &["quality_like", "psnr", "mean"])?;
                Some(Cell { text: format!("{v:.1}"), hover: p.get("like").and_then(Value::as_str).map(|l| format!("like {l}")) })
            },
        },
        Column {
            name: "spp",
            hover: "Paths per pixel its settled frames accumulated (pipelines that accumulate lit frames, held still to reach compare.converge)",
            cell: |p| num(f(p, &["converged_spp"]), 0),
        },
        Column { name: "update ms", hover: "Mean time applying world updates per frame (chunk uploads, edits, lights); per byte arrived on hover", cell: update_cell },
        Column {
            name: "missed",
            hover: "Frames not done by their deadline in a paced pass (`run.pacing` at a fixed fps)",
            cell: |p| num(f(p, &["missed_deadlines"]), 0),
        },
        Column {
            name: "wire MB",
            hover: "Bytes sent over its link in the pass, resends and warm-up included; arrived bytes and chunks on hover. Shown for passes over a link that is not free, with holes, or complete only after their first frame",
            cell: |p| {
                stream(p)?;
                let mb = |k| f(p, &[k]).unwrap_or(0.0) / 1048576.0;
                Some(Cell { text: format!("{:.1}", mb("wire_bytes")), hover: Some(format!("{:.1} MB arrived, {:.0} chunks sent", mb("arrived_bytes"), f(p, &["chunks_sent"]).unwrap_or(0.0))) })
            },
        },
        Column {
            name: "holes",
            hover: "Chunks of its region missing when a frame was requested (the server's backlog, on the link, not generated yet): mean / max per frame; frames with any on hover",
            cell: |p| {
                stream(p)?;
                let (mean, max) = (f(p, &["holes", "mean"]).unwrap_or(0.0), f(p, &["holes", "max"]).unwrap_or(0.0));
                Some(Cell { text: format!("{mean:.1} / {max:.0}"), hover: Some(format!("{:.0} frames with holes", f(p, &["frames_with_holes"]).unwrap_or(0.0))) })
            },
        },
        Column {
            name: "first complete",
            hover: "The first frame requested with nothing of its region missing: its number, and the wall time from the pass's start (initial load included)",
            cell: |p| {
                stream(p)?;
                Some(match p.get("first_complete").filter(|c| !c.is_null()) {
                    Some(c) => Cell {
                        text: format!("#{:.0} {:.0} ms", f(c, &["frame"]).unwrap_or(0.0), f(c, &["ms"]).unwrap_or(0.0)),
                        hover: Some(format!("at {:.2} s on the run's clock", f(c, &["clock_s"]).unwrap_or(0.0))),
                    },
                    None => cell("never".into()),
                })
            },
        },
    ];
    let mut cols: Vec<Column> = always.into_iter().collect();
    cols.extend(optional.into_iter().filter(|c| ok.iter().any(|p| (c.cell)(p).is_some())));
    cols
}

/// Some(()) when the pass's streaming is worth showing: over a link that is not free, with
/// holes, or complete only after its first recorded frame (or never).
fn stream(p: &Value) -> Option<()> {
    let link = p.get("link").is_some_and(|l| !l.is_null());
    let holes = f(p, &["frames_with_holes"]).unwrap_or(0.0) > 0.0;
    let first = p.get("per_frame").and_then(|v| v.get(0)).and_then(|v| f(v, &["frame"]));
    let late = match p.get("first_complete").filter(|c| !c.is_null()) {
        Some(c) => first.is_some_and(|first| f(c, &["frame"]).unwrap_or(0.0) > first),
        None => true,
    };
    (link || holes || late).then_some(())
}

fn update_cell(p: &Value) -> Option<Cell> {
    let mean = f(p, &["update_ms", "mean"]).filter(|x| *x > 0.0)?;
    Some(Cell { text: format!("{mean:.2}"), hover: f(p, &["update_ns_per_byte"]).map(|x| format!("{x:.2} ns per byte arrived")) })
}

pub fn report_ui(ui: &mut Ui, r: &Report) {
    ui.horizontal(|ui| {
        ui.label(r.path.display().to_string());
        if ui.small_button("open folder").on_hover_text("Open the run directory (report.json, CSVs, saved frames) in the file manager").clicked()
            && let Some(d) = r.path.parent() {
            let _ = Command::new(if cfg!(windows) { "explorer" } else { "xdg-open" }).arg(d).spawn();
        }
    });
    let cols = columns(&r.passes);
    egui::ScrollArea::horizontal().id_salt("report-table").show(ui, |ui| {
        egui::Grid::new("report").striped(true).show(ui, |ui| {
            ui.strong("renderer").on_hover_text("Renderer label from the plan");
            ui.strong("mode").on_hover_text("Unlit (textures only) or lit (sun, sky, point lights, shadows)");
            for c in &cols {
                ui.strong(c.name).on_hover_text(c.hover);
            }
            ui.end_row();
            for p in &r.passes {
                let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("-").to_string();
                // What else the pass says of itself: what it left out, a fault while stopping,
                // lanes that failed beside it.
                let list = |k: &str| p.get(k).and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).filter(|x| !x.is_empty());
                let notes: Vec<String> = [
                    list("not_supported").map(|x| format!("left out: {x}")),
                    p.get("shutdown_error").and_then(|x| x.as_str()).map(|x| format!("while stopping: {x}")),
                    list("beside_failed").map(|x| format!("shared the machine for part of the pass with failed {x}")),
                ]
                .into_iter()
                .flatten()
                .collect();
                if notes.is_empty() {
                    ui.label(s("label"));
                } else {
                    ui.label(format!("{} ⚠", s("label"))).on_hover_text(notes.join("\n"));
                }
                ui.label(s("mode"));
                if let Some(e) = p.get("error").and_then(|e| e.as_str()) {
                    ui.colored_label(egui::Color32::LIGHT_RED, "FAILED").on_hover_text(e);
                    ui.end_row();
                    continue;
                }
                for c in &cols {
                    match (c.cell)(p) {
                        Some(Cell { text, hover: Some(h) }) => ui.label(text).on_hover_text(h),
                        Some(Cell { text, hover: None }) => ui.label(text),
                        None => ui.label("-"),
                    };
                }
                ui.end_row();
            }
        });
    });
    // fps per pass (log scale would hide differences; plain bars, reference passes excluded).
    let bars: Vec<egui_plot::Bar> = r
        .passes
        .iter()
        .filter(|p| !p.get("label").and_then(|l| l.as_str()).unwrap_or("").ends_with("(ref)"))
        .enumerate()
        .map(|(i, p)| {
            let name = format!("{} {}", p.get("label").and_then(|x| x.as_str()).unwrap_or("?"), p.get("mode").and_then(|x| x.as_str()).unwrap_or(""));
            egui_plot::Bar::new(i as f64, f(p, &["fps"]).unwrap_or(0.0)).name(name).width(0.7)
        })
        .collect();
    egui_plot::Plot::new(("fps-bars", r.path.display().to_string()))
        .height(180.0)
        .y_axis_label("fps")
        .allow_scroll(false)
        .show(ui, |pu| pu.bar_chart(egui_plot::BarChart::new("fps", bars)));
}
