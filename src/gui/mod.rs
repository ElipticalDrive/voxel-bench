//! Interactive front-end: edit/save/load plans, run a live session with renderers side by
//! side, fly the camera, poke the world, watch metrics, snapshot-compare against the
//! reference, and launch full benchmarks.

mod bench;
mod dock;
mod editor;
pub(crate) mod graph;
mod gpu;
mod input;
mod knobs;
mod legend;
mod live;
mod metrics;
mod panels;
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
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1700.0, 1000.0]).with_title("MultiRenderer"),
        ..Default::default()
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    if display::x11_only()? {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            b.with_x11();
        }));
    }
    // egui-wgpu panics when it draws on a lost device: that panic is the loss, handed over
    // like one the GUI saw first (see `gpu`); any other panic stays one.
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            "MultiRenderer",
            options,
            Box::new(move |cc| {
                let mut app = App::new(&cc.egui_ctx, initial);
                app.gpu = Some(gpu::GuiGpu::new(cc.wgpu_render_state.as_ref(), &cc.egui_ctx));
                if let Some((why, original)) = gpu::restarted() {
                    app.message = format!("restarted: the previous GUI's GPU device was {why}; its live session stopped");
                    if let Some(o) = original {
                        let name = o.as_ref().map_or("the untitled plan".to_string(), |o| o.display().to_string());
                        app.message += &format!("; unsaved changes of {name} recovered (not yet saved)");
                        if let Some(o) = &o {
                            app.path_text = o.display().to_string();
                        }
                        app.plan_path = o;
                        app.dirty = true;
                    }
                    eprintln!("GUI {}", app.message);
                }
                Ok(Box::new(app))
            }),
        )
    }));
    match run {
        Ok(r) => r.map_err(|e| anyhow::anyhow!("{e}")),
        Err(panic) => match gpu::lost() {
            Some(why) => {
                eprintln!("the GUI's GPU device was {why} and drawing on it failed: handing over to a fresh GUI");
                gpu::hand_over(&why)
            }
            None => std::panic::resume_unwind(panic),
        },
    }
}

/// The graphical session on Linux, checked before the window opens so a missing one says why.
#[cfg(all(unix, not(target_os = "macos")))]
mod display {
    use std::path::PathBuf;

    fn set(var: &str) -> Option<String> {
        std::env::var(var).ok().filter(|v| !v.is_empty())
    }

    fn wsl() -> bool {
        std::fs::read_to_string("/proc/sys/kernel/osrelease").is_ok_and(|r| r.to_lowercase().contains("microsoft"))
    }

    /// Whether to open the window through X11 alone: a Wayland display is named but its socket
    /// is not there (WSLg with systemd, a stale variable) while an X display is. An error when
    /// there is no display at all.
    pub fn x11_only() -> anyhow::Result<bool> {
        let wayland = set("WAYLAND_DISPLAY").map(|d| {
            let p = PathBuf::from(&d);
            if p.is_absolute() { p } else { PathBuf::from(set("XDG_RUNTIME_DIR").unwrap_or_default()).join(p) }
        });
        let wayland_up = set("WAYLAND_SOCKET").is_some() || wayland.as_ref().is_some_and(|p| p.exists());
        let x11 = set("DISPLAY").is_some();
        if wayland_up {
            return Ok(false);
        }
        if x11 {
            if let Some(p) = wayland {
                eprintln!("WAYLAND_DISPLAY names {}, which does not exist; using X11 (DISPLAY)", p.display());
            }
            return Ok(true);
        }
        let mut msg = String::from("no graphical session: neither a Wayland display (WAYLAND_DISPLAY) nor an X display (DISPLAY) is available, so the GUI cannot open. Benchmarks without a window still work (`multirenderer run <plan>`).");
        if wsl() {
            msg.push_str(" This is WSL: WSLg provides the session (`wsl --update` in Windows, then `wsl --shutdown` and reopen); running MultiRenderer on Windows itself uses the GPU directly and is the better choice.");
        }
        anyhow::bail!(msg)
    }
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
    /// Where each pane is: in a dock or a window of its own.
    docks: dock::Docks,
    toml_text: String,
    /// The pass the TOML pane was last drawn in: shown again after a gap, it re-reads the plan.
    toml_pass: Option<u64>,
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
    /// The canvas (the viewport) has the keyboard: the last click landed on it. Ctrl+S is
    /// then fast reverse flight, elsewhere it saves the plan.
    canvas_focus: bool,
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

    /// In-progress knob edits not yet applied (restart-only knobs apply on release).
    knob_edits: HashMap<(usize, String), mr_core::ParamValue>,
    /// Knobs the tuner may not touch, per lane.
    locks: HashSet<(usize, String)>,
    tune_ui: HashMap<usize, TuneUi>,
    plot_ms_ssim: bool,
    bench: Option<bench::BenchRun>,
    report: Option<bench::Report>,
    message: String,
    /// The pipeline editor, if open (shown by the Pipeline pane).
    graph_win: Option<graph::Window>,
    trials: graph::Trials,
    /// The GUI's own GPU device, watched for errors and loss (none in headless tests).
    gpu: Option<gpu::GuiGpu>,
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
        let mut st = out.state;
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
    plan.renderers.iter().map(|r| toml::to_string(&r.options).unwrap_or_default()).collect()
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
            docks: dock::Docks::load(),
            toml_pass: None,
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
            canvas_focus: false,
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
            knob_edits: HashMap::new(),
            locks: HashSet::new(),
            tune_ui: HashMap::new(),
            plot_ms_ssim: false,
            bench: None,
            report: None,
            message: String::new(),
            graph_win: None,
            trials: Default::default(),
            gpu: None,
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

    /// The GUI's GPU device is lost: stop the session, hand over to a fresh GUI (see [`gpu`])
    /// and exit before drawing again (egui-wgpu panics on a lost device).
    fn device_lost(&mut self, why: &str) -> ! {
        eprintln!("the GUI's GPU device was {why}: stopping the live session and handing over to a fresh GUI");
        self.live = None;
        if let Err(e) = gpu::hand_over(why) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        std::process::exit(0);
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
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Fullscreen(on));
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
        let unsaved = self.dirty.then_some(&self.plan);
        match self.gpu.as_mut().and_then(|g| g.poll(&ctx, &self.plan_path, unsaved, &mut self.message)) {
            Some(gpu::Event::Lost(why)) => self.device_lost(&why),
            Some(gpu::Event::StartLive) => self.start_live(&ctx),
            Some(gpu::Event::MarkDirty) => self.dirty = true,
            None => {}
        }
        self.keys(&ctx);
        // A click anywhere takes the keyboard from the canvas; the viewport gives it back when
        // the click was on it.
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.canvas_focus = false;
        }
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

        if !self.fullscreen {
            egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
            self.docks(ui);
        }
        match self.middle_pane() {
            Some(p) => egui::CentralPanel::default().show(ui, |ui| self.middle(ui, p)),
            None => egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.viewport(ui)),
        };
        self.finish_docks(&ctx);
        self.pane_windows(&ctx);
        if self.fullscreen {
            self.sync_live(&ctx);
            return;
        }

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

/// The GUI's directory in the user's configuration directory.
fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("multirenderer"))
}

/// The list of recently loaded or saved plans.
fn recent_file() -> Option<PathBuf> {
    Some(config_dir()?.join("recent-plans.txt"))
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
