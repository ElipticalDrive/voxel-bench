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
        let mut child = Command::new(std::env::current_exe()?)
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
        if !self.finished {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.lines.lock().unwrap().push(format!("(benchmark exited: {status})"));
                self.finished = true;
            }
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

pub fn report_ui(ui: &mut Ui, r: &Report) {
    ui.horizontal(|ui| {
        ui.label(r.path.display().to_string());
        if ui.small_button("open folder").on_hover_text("Open the run directory (report.json, CSVs, saved frames) in the file manager").clicked() {
            if let Some(d) = r.path.parent() {
                let _ = Command::new("xdg-open").arg(d).spawn();
            }
        }
    });
    let cols = ["renderer", "mode", "fps", "frame ms", "p99 ms", "gpu ms", "cpu ms", "PSNR", "bad %", "init ms", "RSS MB", "GPU MB"];
    egui::ScrollArea::horizontal().id_salt("report-table").show(ui, |ui| {
        egui::Grid::new("report").striped(true).show(ui, |ui| {
            for c in cols {
                ui.strong(c).on_hover_text(match c {
                    "renderer" => "Renderer label from the plan",
                    "mode" => "Unlit (textures only) or lit (sun, sky, point lights, shadows)",
                    "fps" => "Frames completed per second of wall time over the pass",
                    "frame ms" => "Mean time between completed frames",
                    "p99 ms" => "99th-percentile time between completed frames (hitches)",
                    "gpu ms" => "Mean GPU execution time per frame (timestamp queries)",
                    "cpu ms" => "Mean renderer-process CPU time per frame",
                    "PSNR" => "Mean PSNR vs the reference renderer in dB (higher is better, 99 = exact)",
                    "bad %" => "Mean share of pixels off by more than 16/255 vs the reference",
                    "init ms" => "Renderer start-up time (device, pipelines)",
                    "RSS MB" => "Peak resident memory of the renderer process",
                    _ => "GPU memory the renderer reports allocating",
                });
            }
            ui.end_row();
            for p in &r.passes {
                let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("-").to_string();
                let n = |v: Option<f64>, d: usize| v.map(|x| format!("{x:.d$}")).unwrap_or_else(|| "-".into());
                ui.label(s("label"));
                ui.label(s("mode"));
                if let Some(e) = p.get("error").and_then(|e| e.as_str()) {
                    ui.colored_label(egui::Color32::LIGHT_RED, "FAILED").on_hover_text(e);
                    ui.end_row();
                    continue;
                }
                ui.label(n(f(p, &["fps"]), 1));
                ui.label(n(f(p, &["frame_interval_ms", "mean"]), 2));
                ui.label(n(f(p, &["frame_interval_ms", "p99"]), 2));
                ui.label(n(f(p, &["gpu_ms", "mean"]), 2));
                ui.label(n(f(p, &["cpu_ms", "mean"]), 2));
                ui.label(n(f(p, &["quality", "psnr", "mean"]), 1));
                ui.label(n(f(p, &["quality", "bad_pct", "mean"]), 2));
                ui.label(n(f(p, &["init_ms"]), 0));
                ui.label(n(f(p, &["peak_rss_mb"]), 0));
                ui.label(n(f(p, &["memory", "gpu_bytes"]).filter(|x| *x > 0.0).map(|x| x / 1048576.0), 0));
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
