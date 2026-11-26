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
    /// The table's text, made once on first draw (a report does not change once loaded).
    table: std::sync::OnceLock<Table>,
}

impl Report {
    pub fn new(path: PathBuf, passes: Vec<Value>) -> Self {
        Self { path, passes, table: Default::default() }
    }
}

/// The report table as drawn: its columns, every row's text, and each column's width (set at
/// the first draw, when the fonts are known). Made once per report, so a frame costs only the
/// rows in view, however many passes the report holds.
struct Table {
    cols: Vec<Column>,
    rows: Vec<Row>,
    widths: std::sync::OnceLock<Vec<f32>>,
}

struct Row {
    label: String,
    /// What else the pass says of itself (left out, a fault while stopping, failed lanes beside).
    notes: Option<String>,
    mode: String,
    /// The error of a failed pass (its cells are not shown).
    error: Option<String>,
    cells: Vec<Option<Cell>>,
}

impl Table {
    fn of(passes: &[Value]) -> Self {
        let cols = columns(passes);
        let rows = passes
            .iter()
            .map(|p| {
                let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("-").to_string();
                let list = |k: &str| p.get(k).and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).filter(|x| !x.is_empty());
                let notes: Vec<String> = [
                    list("not_supported").map(|x| format!("left out: {x}")),
                    p.get("shutdown_error").and_then(|x| x.as_str()).map(|x| format!("while stopping: {x}")),
                    list("beside_failed").map(|x| format!("shared the machine for part of the pass with failed {x}")),
                ]
                .into_iter()
                .flatten()
                .collect();
                let error = p.get("error").and_then(|e| e.as_str()).map(String::from);
                let cells = if error.is_some() { Vec::new() } else { cols.iter().map(|c| c.cell(p)).collect() };
                Row { label: if notes.is_empty() { s("label") } else { format!("{} ⚠", s("label")) }, notes: (!notes.is_empty()).then(|| notes.join("\n")), mode: s("mode"), error, cells }
            })
            .collect();
        Self { cols, rows, widths: Default::default() }
    }

    /// Whether `row` matches the filter `f` (lower case words, each in its label or mode).
    fn shows(row: &Row, f: &str) -> bool {
        crate::gui::graph::shows(&format!("{} {}", row.label, row.mode), f)
    }
}

pub fn load_report(path: &Path) -> anyhow::Result<Report> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let passes = v.as_array().cloned().unwrap_or_default();
    Ok(Report::new(path.to_path_buf(), passes))
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

/// A column of the report table after the label and mode: its header, the header's hover,
/// and a pass's cell (None: "-"): one metric (`crate::metrics`), or a view over several
/// (`covers` names them, so they are not shown again on their own).
pub struct Column {
    pub name: &'static str,
    pub hover: &'static str,
    pub cell: fn(&Value) -> Option<Cell>,
    pub metric: Option<&'static crate::metrics::Metric>,
    pub covers: &'static [&'static str],
}

impl Column {
    pub fn cell(&self, p: &Value) -> Option<Cell> {
        match self.metric {
            Some(m) => m.of(p).map(|x| cell(m.show(x))),
            None => (self.cell)(p),
        }
    }

    /// The column of metric `name` (a registry entry: headed by its name, its description on hover).
    fn of(name: &str) -> Self {
        let m = crate::metrics::get(name).unwrap_or_else(|| panic!("no metric `{name}`"));
        Column { name: m.name, hover: m.description, cell: |_| None, metric: Some(m), covers: &[] }
    }
}

/// The columns `passes` fill: every pass has the first ones; the rest (the like score,
/// convergence, world updates, deadlines, streaming) show only when some pass that ran
/// reports them.
pub fn columns(passes: &[Value]) -> Vec<Column> {
    let ok: Vec<&Value> = passes.iter().filter(|p| p.get("error").is_none_or(Value::is_null)).collect();
    let always = [
        Column::of("fps"),
        Column::of("frame_ms"),
        Column::of("frame_p99_ms"),
        Column::of("gpu_ms"),
        Column::of("cpu_ms"),
        Column::of("latency_ms"),
        Column::of("psnr"),
        Column::of("psnr_settled"),
        Column::of("bad_pct"),
        Column::of("init_ms"),
        Column {
            name: "shaders ms",
            hover: "The part of init ms spent on shaders: composing and validating modules, and creating pipelines (the driver's compile)",
            cell: |p| {
                let s = [f(p, &["shaders", "module_ms"]), f(p, &["shaders", "pipeline_ms"])];
                let n = |v: Option<f64>| v.map_or("-".into(), |x| format!("{x:.0}"));
                (!s.iter().all(Option::is_none)).then(|| Cell { text: format!("{} + {}", n(s[0]), n(s[1])), hover: Some("modules + pipelines".into()) })
            },
            metric: None,
            covers: &["shader_module_ms", "shader_pipeline_ms"],
        },
        Column::of("peak_rss_mb"),
        Column::of("gpu_mb"),
        Column::of("gpu_device_mb"),
    ];
    let optional = [
        Column {
            name: "like dB",
            hover: "PSNR against a reference given what the pipeline was given (its LOD region, `like`): a lossy pipeline scores low in PSNR and high here, a wrong one low in both",
            cell: |p| {
                let v = f(p, &["quality_like", "psnr", "mean"])?;
                Some(Cell { text: format!("{v:.1}"), hover: p.get("like").and_then(Value::as_str).map(|l| format!("like {l}")) })
            },
            metric: None,
            covers: &["psnr_like"],
        },
        Column::of("converged_spp"),
        Column { name: "update ms", hover: "Mean time applying world updates per frame (chunk uploads, edits, lights); per byte arrived on hover", cell: update_cell, metric: None, covers: &["update_ms", "update_ns_per_byte"] },
        Column::of("missed_deadlines"),
        Column {
            name: "wire MB",
            hover: "Bytes sent over its link in the pass, resends and warm-up included; arrived bytes and chunks on hover. Shown for passes over a link that is not free, with holes, or complete only after their first frame",
            cell: |p| {
                stream(p)?;
                let mb = |k| f(p, &[k]).unwrap_or(0.0) / 1048576.0;
                Some(Cell { text: format!("{:.1}", mb("wire_bytes")), hover: Some(format!("{:.1} MB arrived, {:.0} chunks sent", mb("arrived_bytes"), f(p, &["chunks_sent"]).unwrap_or(0.0))) })
            },
            metric: None,
            covers: &["wire_bytes", "arrived_bytes"],
        },
        Column {
            name: "holes",
            hover: "Chunks of its region missing when a frame was requested (the server's backlog, on the link, not generated yet): mean / max per frame; frames with any on hover",
            cell: |p| {
                stream(p)?;
                let (mean, max) = (f(p, &["holes", "mean"]).unwrap_or(0.0), f(p, &["holes", "max"]).unwrap_or(0.0));
                Some(Cell { text: format!("{mean:.1} / {max:.0}"), hover: Some(format!("{:.0} frames with holes", f(p, &["frames_with_holes"]).unwrap_or(0.0))) })
            },
            metric: None,
            covers: &["holes", "holes_max", "frames_with_holes"],
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
            metric: None,
            covers: &["first_complete_frame", "first_complete_ms"],
        },
    ];
    // Every other metric a pass that ran measured, in the registry's order (one a view
    // covers stays in its view, shown or not).
    let shown: Vec<&str> = always.iter().chain(&optional).flat_map(|c| c.metric.map(|m| m.name).into_iter().chain(c.covers.iter().copied())).collect();
    let mut cols: Vec<Column> = always.into_iter().collect();
    cols.extend(optional.into_iter().filter(|c| ok.iter().any(|p| c.cell(p).is_some())));
    let rest: Vec<Column> = crate::metrics::METRICS.iter().filter(|m| !shown.contains(&m.name)).map(|m| Column::of(m.name)).collect();
    cols.extend(rest.into_iter().filter(|c| ok.iter().any(|p| c.cell(p).is_some())));
    cols
}

/// Some(()) when the pass's streaming is worth showing: through gates on the world's side
/// (older reports: over a link), with holes, or complete only after its first recorded frame
/// (or never).
fn stream(p: &Value) -> Option<()> {
    let link = p.get("world_gated").and_then(Value::as_bool).unwrap_or(false) || p.get("link").is_some_and(|l| !l.is_null());
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
    let t = r.table.get_or_init(|| Table::of(&r.passes));
    let font = egui::TextStyle::Body.resolve(ui.style());
    let gap = ui.spacing().item_spacing.x;
    // Each column as wide as its widest text, measured once.
    let widths = t.widths.get_or_init(|| {
        let w = |text: &str, ui: &Ui| ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), egui::Color32::WHITE).size().x);
        let mut ws: Vec<f32> = ["renderer", "mode"].iter().chain(t.cols.iter().map(|c| &c.name)).map(|h| w(h, ui)).collect();
        for row in &t.rows {
            ws[0] = ws[0].max(w(&row.label, ui));
            ws[1] = ws[1].max(w(&row.mode, ui));
            if row.error.is_some() {
                ws[2] = ws[2].max(w("FAILED", ui));
            }
            for (i, c) in row.cells.iter().enumerate() {
                ws[i + 2] = ws[i + 2].max(w(c.as_ref().map_or("-", |c| &c.text), ui));
            }
        }
        ws
    });
    let filter_id = ui.id().with(("report-filter", &r.path));
    let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
    let want = filter.to_lowercase();
    let shown: Vec<usize> = (0..t.rows.len()).filter(|&i| Table::shows(&t.rows[i], &want)).collect();
    ui.horizontal(|ui| {
        ui.label("filter");
        if ui.add(egui::TextEdit::singleline(&mut filter).hint_text("words in the label or mode").desired_width(220.0)).on_hover_text("Show only the passes whose label or mode contains every word typed (any case)").changed() {
            ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
        }
        ui.weak(format!("{} of {} passes", shown.len(), t.rows.len()));
    });
    let row_h = ui.text_style_height(&egui::TextStyle::Body).max(ui.spacing().interact_size.y);
    // The rows get the height the chart (below) leaves; in a pane that grows without bound
    // (an outer scroll area), as many rows as the chart is tall, times four.
    const CHART_H: f32 = 180.0;
    let left = ui.available_height() - CHART_H - 3.0 * row_h;
    let body_h = if left.is_finite() { left.max(row_h * 4.0) } else { CHART_H * 4.0 };
    let cell = |ui: &mut Ui, w: f32, text: egui::WidgetText| ui.add_sized([w, row_h], egui::Label::new(text).truncate());
    let in_view = egui::ScrollArea::horizontal().id_salt("report-table").show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.horizontal(|ui| {
            cell(ui, widths[0], egui::RichText::new("renderer").strong().into()).on_hover_text("Renderer label from the plan");
            cell(ui, widths[1], egui::RichText::new("mode").strong().into()).on_hover_text("Unlit (textures only) or lit (sun, sky, point lights, shadows)");
            for (c, w) in t.cols.iter().zip(&widths[2..]) {
                cell(ui, *w, egui::RichText::new(c.name).strong().into()).on_hover_text(c.hover);
            }
        });
        // Only the rows in view are drawn.
        egui::ScrollArea::vertical().id_salt("report-rows").max_height(body_h).show_rows(ui, row_h, shown.len(), |ui, range| {
            for (i, row) in shown[range.clone()].iter().map(|&k| &t.rows[k]).enumerate() {
                let stripe = (range.start + i) % 2 == 1;
                ui.horizontal(|ui| {
                    if stripe {
                        let full = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(widths.iter().sum::<f32>() + gap * widths.len() as f32, row_h));
                        ui.painter().rect_filled(full, 0.0, ui.visuals().faint_bg_color);
                    }
                    let l = cell(ui, widths[0], row.label.clone().into());
                    if let Some(n) = &row.notes {
                        l.on_hover_text(n);
                    }
                    cell(ui, widths[1], row.mode.clone().into());
                    if let Some(e) = &row.error {
                        cell(ui, widths[2], egui::RichText::new("FAILED").color(egui::Color32::LIGHT_RED).into()).on_hover_text(e);
                        return;
                    }
                    for (c, w) in row.cells.iter().zip(&widths[2..]) {
                        match c {
                            Some(Cell { text, hover: Some(h) }) => drop(cell(ui, *w, text.clone().into()).on_hover_text(h)),
                            Some(Cell { text, hover: None }) => drop(cell(ui, *w, text.clone().into())),
                            None => drop(cell(ui, *w, "-".into())),
                        }
                    }
                });
            }
            range
        })
        .inner
    })
    .inner;
    // fps of the passes in view (log scale would hide differences; plain bars, reference
    // passes excluded): scrolling the table scrolls the chart.
    let bars: Vec<egui_plot::Bar> = shown[in_view]
        .iter()
        .map(|&k| &r.passes[k])
        .filter(|p| !p.get("label").and_then(|l| l.as_str()).unwrap_or("").ends_with("(ref)"))
        .enumerate()
        .map(|(i, p)| {
            let name = format!("{} {}", p.get("label").and_then(|x| x.as_str()).unwrap_or("?"), p.get("mode").and_then(|x| x.as_str()).unwrap_or(""));
            egui_plot::Bar::new(i as f64, f(p, &["fps"]).unwrap_or(0.0)).name(name).width(0.7)
        })
        .collect();
    egui_plot::Plot::new(("fps-bars", r.path.display().to_string()))
        .height(CHART_H)
        .y_axis_label("fps")
        .allow_scroll(false)
        .show(ui, |pu| pu.bar_chart(egui_plot::BarChart::new("fps", bars)));
}
