//! Interactive front-end: edit/save/load plans, run a live session with renderers side by
//! side, fly the camera, poke the world, watch metrics, snapshot-compare against the
//! reference, and launch full benchmarks.

mod bench;
mod editor;
mod graph;
mod input;
mod knobs;
mod legend;
mod live;
mod metrics;
mod panels;
mod schemes;
mod viewport;

use std::path::{Path, PathBuf};

use egui::TextureHandle;
use mr_core::{Camera, PassMode, RendererInfo};
use mr_world::textures::default_registry;

use crate::plan::Plan;
use live::{CameraControl, Cmd, Control, LiveHandle};
use metrics::TuneUi;
use std::collections::{HashMap, HashSet};

/// The plain starting point (New); the showy set-ups are `plans/demo-*.toml` (🎬 Demos).
const DEFAULT_PLAN: &str = include_str!("../../plans/playground.toml");

const SCENE_FPS: f64 = 60.0;

pub fn run(initial: Option<PathBuf>) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1700.0, 1000.0]).with_title("MultiRenderer"),
        ..Default::default()
    };
    eframe::run_native("MultiRenderer", options, Box::new(move |cc| Ok(Box::new(App::new(&cc.egui_ctx, initial)))))
        .map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(PartialEq, Clone, Copy)]
enum LeftTab {
    Form,
    Toml,
}

#[derive(PartialEq, Clone, Copy)]
enum RightTab {
    Metrics,
    Compare,
    Benchmark,
    Schemes,
    Help,
}

struct LaneTex {
    tex: TextureHandle,
    seq: u64,
}

struct CompareTex {
    seq: u64,
    reference: TextureHandle,
    lanes: Vec<(TextureHandle, TextureHandle)>,
    show: usize,
    diff: bool,
}

struct App {
    plan: Plan,
    plan_path: Option<PathBuf>,
    path_text: String,
    dirty: bool,
    left_tab: LeftTab,
    toml_text: String,
    toml_error: Option<String>,
    option_text: Vec<String>,
    infos: Vec<RendererInfo>,
    block_names: Vec<String>,

    live: Option<LiveHandle>,
    follow_path: bool,
    cam: Camera,
    mode: PassMode,
    scene_frame: f64,
    playing: bool,
    events_on: bool,
    speed: f32,
    fps_cap: f64,
    display_hz: f64,
    probe_interval: f64,
    /// Lanes shown in an HDR window of their own (bit `i`: lane `i`).
    windows: u64,
    /// Those windows' paper white: the frames' 1.0 relative to the display's SDR white.
    paper_white: f32,
    edit_mode: bool,
    /// Viewport only, OS fullscreen (F11).
    fullscreen: bool,
    /// Draw frames at exactly one frame pixel per screen pixel (Z).
    pixel_exact: bool,
    /// Labels and HUD drawn over the frames (O).
    overlays: bool,
    /// Physical pixel size of the first visible tile, for "match resolution".
    tile_px: Option<[u32; 2]>,
    place_block: usize,
    focus: Option<usize>,
    textures: Vec<Option<LaneTex>>,
    compare_tex: Option<CompareTex>,
    plot_window: f64,
    light_color: [f32; 3],
    light_intensity: f32,
    light_radius: f32,

    right_tab: RightTab,
    /// In-progress knob edits not yet applied (restart-only knobs apply on release).
    knob_edits: HashMap<(usize, String), mr_core::ParamValue>,
    /// Knobs the tuner may not touch, per lane.
    locks: HashSet<(usize, String)>,
    tune_ui: HashMap<usize, TuneUi>,
    plot_ms_ssim: bool,
    bench: Option<bench::BenchRun>,
    report: Option<bench::Report>,
    message: String,
    /// The pipeline editor window, if open.
    graph_win: Option<graph::Window>,
    trials: graph::Trials,
    schemes: schemes::Browser,
}

/// A vertical scroll area that stays where the user left it when its content shrinks for a
/// while (a restarting lane shows only "starting…"; a reloaded plan rebuilds its forms): the
/// offset the user chose comes back once the content is tall enough again.
pub fn steady_scroll<R>(ui: &mut egui::Ui, salt: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    steady_scroll_area(ui, salt, add).inner
}

fn steady_scroll_area<R>(ui: &mut egui::Ui, salt: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let id = ui.id().with(("steady-scroll", salt));
    let (mut want, last) = ui.data(|d| d.get_temp::<(f32, f32)>(id)).unwrap_or_default();
    let out = egui::ScrollArea::vertical().id_salt(salt).show(ui, add);
    let off = out.state.offset.y;
    let max = (out.content_size.y - out.inner_rect.height()).max(0.0);
    // Pulled down to the bottom of shorter content: not the user's move.
    let clamped = off >= max - 0.5 && off < last;
    if (off - last).abs() > 0.5 && !clamped {
        want = off;
    } else if off < want - 0.5 && max > off + 0.5 {
        // Taller again: back to the user's offset (from the next frame).
        let mut st = out.state.clone();
        st.offset.y = want.min(max);
        st.store(ui.ctx(), out.id);
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| d.insert_temp(id, (want, off)));
    out
}

/// Knob value as a plan option value.
pub fn param_to_toml(v: &mr_core::ParamValue) -> toml::Value {
    match v {
        mr_core::ParamValue::Bool(b) => toml::Value::Boolean(*b),
        mr_core::ParamValue::Int(i) => toml::Value::Integer(*i),
        mr_core::ParamValue::Float(f) => toml::Value::Float(*f),
        mr_core::ParamValue::Choice(s) => toml::Value::String(s.clone()),
    }
}

fn option_texts(plan: &Plan) -> Vec<String> {
    plan.renderers.iter().map(|r| toml::to_string(&r.extra_options()).unwrap_or_default()).collect()
}

impl App {
    fn new(ctx: &egui::Context, initial: Option<PathBuf>) -> Self {
        ctx.set_theme(egui::Theme::Dark);
        let reg = default_registry();
        let block_names: Vec<String> = reg.blocks.iter().skip(1).map(|b| b.name.clone()).collect();
        let mut app = Self {
            plan: Plan::parse(DEFAULT_PLAN, &[]).expect("default plan"),
            plan_path: None,
            path_text: "plans/untitled.toml".into(),
            dirty: false,
            left_tab: LeftTab::Form,
            toml_text: String::new(),
            toml_error: None,
            option_text: vec![],
            infos: mr_worker::infos(),
            block_names,
            live: None,
            follow_path: true,
            cam: Camera::default(),
            mode: PassMode::UNLIT,
            scene_frame: 0.0,
            playing: true,
            events_on: true,
            speed: 20.0,
            fps_cap: 0.0,
            display_hz: 60.0,
            probe_interval: 0.0,
            windows: 0,
            paper_white: 1.0,
            edit_mode: false,
            fullscreen: false,
            pixel_exact: false,
            overlays: true,
            tile_px: None,
            place_block: 7,
            focus: None,
            textures: vec![],
            compare_tex: None,
            plot_window: 10.0,
            light_color: [1.0, 0.8, 0.5],
            light_intensity: 30.0,
            light_radius: 20.0,
            right_tab: RightTab::Metrics,
            knob_edits: HashMap::new(),
            locks: HashSet::new(),
            tune_ui: HashMap::new(),
            plot_ms_ssim: false,
            bench: None,
            report: None,
            message: String::new(),
            graph_win: None,
            trials: Default::default(),
            schemes: Default::default(),
        };
        app.option_text = option_texts(&app.plan);
        if let Some(p) = initial {
            app.load(&p);
        }
        app
    }

    fn load(&mut self, p: &Path) {
        match Plan::load(p, &[]) {
            Ok(plan) => {
                self.mode = plan.run.modes.first().copied().unwrap_or(PassMode::UNLIT);
                self.plan = plan;
                self.option_text = option_texts(&self.plan);
                self.plan_path = Some(p.to_path_buf());
                self.path_text = p.display().to_string();
                self.dirty = false;
                self.toml_text = self.plan.to_toml().unwrap_or_default();
                self.message = format!("loaded {}", p.display());
                remember_plan(p);
            }
            Err(e) => self.message = format!("load failed: {e:#}"),
        }
    }

    fn save(&mut self, p: PathBuf) {
        let r = self.plan.to_toml().and_then(|t| {
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            Ok(std::fs::write(&p, t)?)
        });
        match r {
            Ok(()) => {
                self.message = format!("saved {}", p.display());
                remember_plan(&p);
                self.path_text = p.display().to_string();
                self.plan_path = Some(p);
                self.dirty = false;
            }
            Err(e) => self.message = format!("save failed: {e:#}"),
        }
    }

    fn start_live(&mut self, ctx: &egui::Context) {
        // A restart keeps the knob values changed in the running session.
        let carry = self.live.as_ref().map_or(vec![], |l| live::Carry::of(&l.shared.lock().unwrap().lanes));
        self.live = None; // stops the previous session (joins its thread)
        self.textures.clear();
        self.compare_tex = None;
        self.focus = None;
        self.follow_path = true;
        self.scene_frame = 0.0;
        let c = ctx.clone();
        self.live = Some(LiveHandle::start(self.plan.clone(), carry, self.control(), move || c.request_repaint()));
        self.message = "live session starting".into();
    }

    /// Takes over what the session's windows changed, then hands it the controls.
    fn sync_live(&mut self, ctx: &egui::Context) {
        let Some(l) = &self.live else { return };
        {
            let mut sh = l.shared.lock().unwrap();
            for i in sh.closed_windows.drain(..) {
                self.windows &= !(1u64 << i.min(63));
            }
            if let Some(g) = sh.paper_white.take() {
                self.paper_white = g;
            }
        }
        *l.control.lock().unwrap() = self.control();
        ctx.request_repaint();
    }

    fn control(&self) -> Control {
        Control {
            camera: if self.follow_path { CameraControl::Path } else { CameraControl::Free(self.cam) },
            mode: self.mode,
            scene_frame: self.scene_frame as u32,
            events: self.events_on,
            fps_cap: self.fps_cap,
            display_hz: self.display_hz,
            probe_interval: self.probe_interval,
            windows: self.windows,
            paper_white: self.paper_white,
        }
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        self.fullscreen = on;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
    }

    /// Sets the render resolution to the current tile's physical pixel size and restarts
    /// the live session, so frames map 1:1 onto the screen.
    fn match_resolution(&mut self, ctx: &egui::Context) {
        let Some(px) = self.tile_px else { return };
        if self.plan.run.resolution != px {
            self.plan.run.resolution = px;
            self.dirty = true;
            self.message = format!("resolution set to {}×{}", px[0], px[1]);
        }
        self.pixel_exact = true;
        self.start_live(ctx);
    }

    fn live_camera(&self) -> Option<Camera> {
        self.live.as_ref().and_then(|l| l.shared.lock().unwrap().camera)
    }

    fn go_free(&mut self) {
        if self.follow_path {
            if let Some(c) = self.live_camera() {
                self.cam = c;
            }
            self.follow_path = false;
        }
    }

    fn send(&self, c: Cmd) {
        if let Some(l) = &self.live {
            l.send(c);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl App {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.keys(&ctx);
        if self.playing && self.live.is_some() {
            let dt = ctx.input(|i| i.stable_dt.min(0.1)) as f64;
            self.scene_frame += dt * SCENE_FPS;
            let frames = self.plan.run.frames.max(1) as f64;
            if self.scene_frame > frames {
                self.scene_frame = 0.0;
            }
        }
        let title = format!("MultiRenderer — {}{}", self.plan_path.as_ref().map(|p| p.display().to_string()).unwrap_or("(unsaved)".into()), if self.dirty { " *" } else { "" });
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        if self.fullscreen {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.viewport(ui));
            self.sync_live(&ctx);
            return;
        }
        if let Some(p) = schemes::take_request(&ctx) {
            self.schemes.open = Some(p);
            self.right_tab = RightTab::Schemes;
        }
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("log").resizable(true).default_size(110.0).show(ui, |ui| {
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                if let Some(l) = &self.live {
                    for line in l.shared.lock().unwrap().log.iter() {
                        ui.monospace(line);
                    }
                }
            });
        });
        egui::Panel::left("editor").resizable(true).default_size(400.0).show(ui, |ui| self.left(ui));
        egui::Panel::right("side").resizable(true).default_size(430.0).show(ui, |ui| self.right(ui));
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.viewport(ui));
        self.pipeline_window(&ctx);

        self.sync_live(&ctx);
        if self.bench.as_ref().is_some_and(|b| !b.finished) {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

fn plan_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir("plans").into_iter().flatten().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect();
    v.sort();
    v
}

/// The list of recently loaded or saved plans, in the user's configuration directory.
fn recent_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("multirenderer").join("recent-plans.txt"))
}

/// The recent plans that still exist, newest first (shown relative to the working
/// directory when under it).
fn recent_plans() -> Vec<PathBuf> {
    let text = recent_file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    let cwd = std::env::current_dir().unwrap_or_default();
    text.lines().map(PathBuf::from).filter(|p| p.is_file()).map(|p| p.strip_prefix(&cwd).map(Path::to_path_buf).unwrap_or(p)).collect()
}

fn remember_plan(p: &Path) {
    let (Some(f), Ok(p)) = (recent_file(), std::fs::canonicalize(p)) else { return };
    if cfg!(test) {
        return;
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut list: Vec<PathBuf> = vec![p.clone()];
    list.extend(recent_plans().into_iter().map(|q| cwd.join(q)).filter(|q| *q != p).take(7));
    let text: String = list.iter().map(|q| format!("{}\n", q.display())).collect();
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(f, text);
}

fn is_demo(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("demo-"))
}

/// A plan file's leading comment (what it shows), joined into one paragraph.
fn lead_comment(p: &Path) -> String {
    let text = std::fs::read_to_string(p).unwrap_or_default();
    let lines: Vec<&str> = text.lines().map_while(|l| l.strip_prefix('#')).map(str::trim).collect();
    if lines.is_empty() { p.display().to_string() } else { lines.join(" ") }
}

#[cfg(test)]
mod tests;
