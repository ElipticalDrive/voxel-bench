//! Interactive front-end: edit/save/load plans, run a live session with renderers side by
//! side, fly the camera, poke the world, watch metrics, snapshot-compare against the
//! reference, and launch full benchmarks.

mod bench;
mod editor;
mod live;

use std::path::{Path, PathBuf};

use egui::{Color32, Key, PointerButton, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use glam::Vec3;
use mr_core::{BlockId, Camera, PassMode, RendererInfo};
use mr_world::textures::default_registry;

use crate::plan::Plan;
use live::{CameraControl, Cmd, Control, LaneStatus, LiveHandle, Sample};
use crate::tune::{self, Algorithm};
use std::collections::{HashMap, HashSet};

const DEFAULT_PLAN: &str = r#"
name = "playground"

[scene]
generator = { type = "terrain", seed = 7 }
camera = { type = "orbit", center = [0, 70, 0], radius = 120, height = 50, period = 1200 }
lights = [{ type = "scatter", count = 150, seed = 1, center = [0, 60, 0], extent = [150, 60, 150], on_surface = true }]
events = [{ type = "animate_lights", amplitude = 1.5, period = 90 }]

[run]
resolution = [1280, 720]
view_radius = 10
streaming = { budget = 512 }

[[renderer]]
name = "gpu-dda"

[[renderer]]
name = "gpu-raster"
"#;

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
            path_text: "plans/playground.toml".into(),
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
                self.path_text = p.display().to_string();
                self.plan_path = Some(p);
                self.dirty = false;
            }
            Err(e) => self.message = format!("save failed: {e:#}"),
        }
    }

    fn start_live(&mut self, ctx: &egui::Context) {
        self.live = None; // stops the previous session (joins its thread)
        self.textures.clear();
        self.compare_tex = None;
        self.focus = None;
        self.follow_path = true;
        self.scene_frame = 0.0;
        let c = ctx.clone();
        self.live = Some(LiveHandle::start(self.plan.clone(), self.control(), move || c.request_repaint()));
        self.message = "live session starting".into();
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

    // ---------------------------------------------------------------- input

    fn keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (pressed, mods) = ctx.input(|i| {
            let keys = [
                Key::L, Key::F, Key::P, Key::R, Key::G, Key::C, Key::E, Key::Tab, Key::Escape, Key::F5, Key::OpenBracket, Key::CloseBracket, Key::S,
                Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::H, Key::F11, Key::Z, Key::M, Key::O,
            ];
            (keys.into_iter().filter(|k| i.key_pressed(*k)).collect::<Vec<_>>(), i.modifiers)
        });
        for k in pressed {
            match k {
                Key::S if mods.command => {
                    if let Some(p) = self.plan_path.clone() {
                        self.save(p);
                    } else {
                        self.save(PathBuf::from(&self.path_text));
                    }
                }
                Key::F5 => self.start_live(ctx),
                Key::L => self.mode.lighting = if self.mode.lighting == mr_core::RenderMode::Unlit { mr_core::RenderMode::Lit } else { mr_core::RenderMode::Unlit },
                Key::N => self.mode.meshes = !self.mode.meshes,
                Key::F => {
                    if self.follow_path {
                        self.go_free();
                    } else {
                        self.follow_path = true;
                    }
                }
                Key::P => self.playing = !self.playing,
                Key::R => {
                    self.scene_frame = 0.0;
                    self.follow_path = true;
                }
                Key::OpenBracket | Key::CloseBracket => {
                    let step = if mods.shift { 100.0 } else { 10.0 };
                    self.scene_frame = (self.scene_frame + if k == Key::OpenBracket { -step } else { step }).max(0.0);
                }
                Key::G => {
                    if let Some(c) = self.live_camera() {
                        self.send(Cmd::AddLight { position: c.position, color: Vec3::from_array(self.light_color), intensity: self.light_intensity, radius: self.light_radius });
                    }
                }
                Key::C => {
                    self.send(Cmd::Compare);
                    self.right_tab = RightTab::Compare;
                }
                Key::E => self.edit_mode = !self.edit_mode,
                Key::H => self.right_tab = RightTab::Help,
                Key::Tab => {
                    let n = self.textures.len().max(1);
                    self.focus = Some(self.focus.map(|f| (f + 1) % n).unwrap_or(0));
                }
                Key::Escape if self.fullscreen => self.set_fullscreen(ctx, false),
                Key::Escape => self.focus = None,
                Key::F11 => self.set_fullscreen(ctx, !self.fullscreen),
                Key::Z => self.pixel_exact = !self.pixel_exact,
                Key::O => self.overlays = !self.overlays,
                Key::M => self.match_resolution(ctx),
                k => {
                    let i = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9].iter().position(|x| *x == k);
                    if let Some(i) = i {
                        self.focus = if self.focus == Some(i) { None } else { Some(i) };
                    }
                }
            }
        }
    }

    fn fly(&mut self, ctx: &egui::Context, look: Vec2) {
        let typing = ctx.egui_wants_keyboard_input();
        let (dt, held, mods) = ctx.input(|i| {
            let keys = [Key::W, Key::A, Key::S, Key::D, Key::Space, Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown];
            (i.stable_dt.min(0.1), keys.map(|k| !typing && i.key_down(k)), i.modifiers)
        });
        let [w, a, s, d, up, al, ar, au, ad] = held;
        let s = s && !mods.command;
        let down = !typing && mods.shift;
        let mut look = look * 0.004;
        let arrow = 1.8 * dt;
        look.x += (ar as i32 - al as i32) as f32 * arrow;
        look.y += (ad as i32 - au as i32) as f32 * arrow;
        let moving = w || a || s || d || up || down;
        if !(moving || look != Vec2::ZERO) {
            return;
        }
        self.go_free();
        self.cam.yaw += look.x;
        self.cam.pitch = (self.cam.pitch - look.y).clamp(-1.55, 1.55);
        let f = self.cam.forward();
        let fwd = Vec3::new(f.x, 0.0, f.z).normalize_or_zero();
        let right = self.cam.right();
        let mut m = Vec3::ZERO;
        for (on, dir) in [(w, fwd), (s, -fwd), (d, right), (a, -right), (up, Vec3::Y), (down, -Vec3::Y)] {
            if on {
                m += dir;
            }
        }
        let boost = if mods.ctrl { 5.0 } else { 1.0 };
        self.cam.position += (m.normalize_or_zero() * self.speed * boost * dt).as_dvec3();
        self.cam.fov_y = self.plan.scene.fov_deg.to_radians();
    }

    // ---------------------------------------------------------------- panels

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button("📂 Open", |ui| {
                ui.weak("Plans in ./plans");
                for p in plan_files() {
                    if ui.button(p.display().to_string()).on_hover_text("Load this plan (unsaved changes to the current one are lost)").clicked() {
                        self.load(&p);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("New (playground template)").on_hover_text("Start from a small built-in plan with a few renderers").clicked() {
                    self.plan = Plan::parse(DEFAULT_PLAN, &[]).unwrap();
                    self.option_text = option_texts(&self.plan);
                    self.plan_path = None;
                    self.dirty = true;
                    ui.close();
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.path_text).desired_width(240.0)).on_hover_text("Plan file path used by Load path and Save");
            if ui.button("Load path").on_hover_text("Load the plan at the path to the left").clicked() {
                self.load(&PathBuf::from(self.path_text.clone()));
            }
            if ui.button(if self.dirty { "💾 Save*" } else { "💾 Save" }).on_hover_text("Save the plan as TOML to the path field (Ctrl+S). * = unsaved changes").clicked() {
                self.save(PathBuf::from(self.path_text.clone()));
            }
            ui.separator();
            let live = self.live.is_some();
            if ui.button(if live { "↻ Restart live (F5)" } else { "▶ Start live (F5)" })
                .on_hover_text("Start every renderer in the plan side by side, each in its own process, driven by the camera here")
                .clicked()
            {
                self.start_live(ui.ctx());
            }
            if live && ui.button("■ Stop").on_hover_text("Stop the live session and its renderer processes").clicked() {
                self.live = None;
                self.textures.clear();
            }
            ui.separator();
            let running = self.bench.as_ref().is_some_and(|b| !b.finished);
            if !running && ui.button("⏱ Run benchmark").on_hover_text("Runs the full plan in a separate process; results appear in the Benchmark tab").clicked() {
                match bench::BenchRun::start(&self.plan, &[]) {
                    Ok(b) => {
                        self.bench = Some(b);
                        self.right_tab = RightTab::Benchmark;
                    }
                    Err(e) => self.message = format!("benchmark failed to start: {e:#}"),
                }
            }
            if running {
                ui.spinner();
                if ui.button("Cancel benchmark").on_hover_text("Kill the benchmark process; finished passes stay in the Benchmark tab").clicked() {
                    if let Some(b) = &mut self.bench {
                        b.cancel();
                    }
                }
            }
            ui.separator();
            ui.weak(&self.message);
        });
        ui.horizontal_wrapped(|ui| {
            // Two latching toggles (IDEAS §13): lighting and meshes are independent.
            let mut lit = self.mode.lighting == mr_core::RenderMode::Lit;
            if ui.toggle_value(&mut lit, "💡 Lit").on_hover_text("Sun, sky and point lights with shadows; off = textures only (L toggles)").changed() {
                self.mode.lighting = if lit { mr_core::RenderMode::Lit } else { mr_core::RenderMode::Unlit };
            }
            ui.toggle_value(&mut self.mode.meshes, "🔺 Meshes").on_hover_text("Draw the scene's triangle meshes with the voxel world (renderers with the `meshes` tag; others show voxels only). N toggles");
            ui.separator();
            if ui.checkbox(&mut self.follow_path, "follow path (F)").on_hover_text("Camera follows the plan's scripted path. Any movement key switches to free fly.").changed() && !self.follow_path {
                self.follow_path = true;
                self.go_free();
            }
            if ui.button(if self.playing { "⏸" } else { "▶" }).on_hover_text("Play/pause the scene clock: path position and scripted events (P)").clicked() {
                self.playing = !self.playing;
            }
            let frames = self.plan.run.frames.max(1) as f64;
            ui.add(egui::Slider::new(&mut self.scene_frame, 0.0..=frames).text("scene frame").step_by(1.0)).on_hover_text("Position on the scripted path. [ and ] step 10 frames (Shift: 100), R rewinds.");
            ui.checkbox(&mut self.events_on, "events").on_hover_text("Apply the plan's scripted events (block edits, lights) as the scene clock passes them");
            ui.separator();
            ui.add(egui::DragValue::new(&mut self.speed).speed(0.5).range(0.5..=2000.0).prefix("speed ").suffix(" b/s")).on_hover_text("Free-fly speed in blocks per second (mouse wheel over the view; hold Ctrl for 5×)");
            ui.add(egui::DragValue::new(&mut self.fps_cap).speed(1.0).range(0.0..=10000.0).prefix("fps cap ")).on_hover_text("Frame-rate limit per renderer; 0 = uncapped. Capping frees GPU time for the others.");
            ui.add(egui::DragValue::new(&mut self.probe_interval).speed(0.1).range(0.0..=60.0).prefix("probe every ").suffix(" s"))
                .on_hover_text("Score every renderer against the reference this often (0 = only on C / when a tuner asks)");
            ui.add(egui::DragValue::new(&mut self.display_hz).speed(1.0).range(1.0..=240.0).prefix("display ").suffix(" Hz"))
                .on_hover_text("How often frames are read back for display. Other frames render without read-back.");
            ui.separator();
            ui.toggle_value(&mut self.edit_mode, "✏ edit (E)").on_hover_text("Left click: remove block. Ctrl+left click: place block.\nEdits are sent to every renderer like scripted ones.");
            if self.edit_mode {
                egui::ComboBox::from_id_salt("place")
                    .selected_text(&self.block_names[self.place_block])
                    .show_ui(ui, |ui| {
                        for (i, n) in self.block_names.iter().enumerate() {
                            ui.selectable_value(&mut self.place_block, i, n);
                        }
                    })
                    .response
                    .on_hover_text("Block placed by Ctrl+left click");
            }
            ui.menu_button("💡 lights", |ui| {
                ui.weak("Point lights you drop are added to the world for every renderer.");
                ui.horizontal(|ui| {
                    ui.label("colour");
                    ui.color_edit_button_rgb(&mut self.light_color).on_hover_text("Colour of the next dropped light");
                });
                ui.add(egui::Slider::new(&mut self.light_intensity, 1.0..=500.0).logarithmic(true).text("intensity")).on_hover_text("Brightness of the next dropped light");
                ui.add(egui::Slider::new(&mut self.light_radius, 2.0..=100.0).text("radius")).on_hover_text("Distance in blocks at which the light's contribution reaches zero");
                if ui.button("Drop light at camera (G)").on_hover_text("Place a point light where the camera is (visible in Lit mode)").clicked() {
                    if let Some(c) = self.live_camera() {
                        self.send(Cmd::AddLight { position: c.position, color: Vec3::from_array(self.light_color), intensity: self.light_intensity, radius: self.light_radius });
                    }
                }
                if ui.button("Remove dropped lights").on_hover_text("Remove lights placed with G; the plan's own lights stay").clicked() {
                    self.send(Cmd::ClearUserLights);
                }
            });
            ui.separator();
            if ui.button("⛶ Fullscreen (F11)").on_hover_text("Show only the renderer views, fullscreen (F11 or Esc to leave)").clicked() {
                self.set_fullscreen(ui.ctx(), true);
            }
            ui.toggle_value(&mut self.pixel_exact, "1:1 (Z)").on_hover_text("One frame pixel per physical screen pixel (no scaling; larger frames are cropped)");
            if let Some([w, h]) = self.tile_px {
                if ui.button(format!("match {w}×{h} (M)")).on_hover_text("Set the render resolution to this tile's exact pixel size and restart live").clicked() {
                    self.match_resolution(ui.ctx());
                }
            }
            ui.toggle_value(&mut self.overlays, "overlays (O)").on_hover_text("Renderer names, fps and the status line over the views");
            if ui.button("⚖ Compare (C)").on_hover_text("Render this exact view on every renderer and the reference, then score them").clicked() {
                self.send(Cmd::Compare);
                self.right_tab = RightTab::Compare;
            }
        });
    }

    fn left(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Workflow");
            ui.selectable_value(&mut self.left_tab, LeftTab::Form, "Form").on_hover_text("Edit the plan with forms");
            if ui.selectable_value(&mut self.left_tab, LeftTab::Toml, "TOML").on_hover_text("Edit the plan as raw TOML (what Save writes)").clicked() {
                self.toml_text = self.plan.to_toml().unwrap_or_default();
                self.toml_error = None;
            }
        });
        ui.separator();
        match self.left_tab {
            LeftTab::Form => {
                let camera = self.live_camera();
                let mut restart = None;
                let live = self.live.is_some();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut ectx = editor::EditorCtx {
                        infos: &self.infos,
                        block_names: &self.block_names,
                        camera,
                        option_text: &mut self.option_text,
                        restart_lane: &mut restart,
                        live,
                    };
                    if editor::plan_editor(ui, &mut self.plan, &mut ectx) {
                        self.dirty = true;
                    }
                    ui.add_space(8.0);
                    if live {
                        ui.weak("World, path and run-setting changes take effect when you restart the live session (F5). Use ↻ on a renderer to restart just that one.");
                    }
                });
                if let Some(i) = restart {
                    let n = self.live.as_ref().map(|l| l.shared.lock().unwrap().lanes.len()).unwrap_or(0);
                    let spec = self.plan.renderers[i].clone();
                    self.send(if i < n { Cmd::Restart { lane: i, spec } } else { Cmd::AddLane(spec) });
                }
            }
            LeftTab::Toml => {
                ui.horizontal(|ui| {
                    if ui.button("Apply").on_hover_text("Parse this TOML and replace the plan with it").clicked() {
                        match Plan::parse(&self.toml_text, &[]) {
                            Ok(p) => {
                                self.plan = p;
                                self.option_text = option_texts(&self.plan);
                                self.toml_error = None;
                                self.dirty = true;
                            }
                            Err(e) => self.toml_error = Some(format!("{e:#}")),
                        }
                    }
                    if ui.button("Revert").on_hover_text("Discard text edits; show the current plan again").clicked() {
                        self.toml_text = self.plan.to_toml().unwrap_or_default();
                        self.toml_error = None;
                    }
                });
                if let Some(e) = &self.toml_error {
                    ui.colored_label(Color32::LIGHT_RED, e);
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.toml_text).code_editor().desired_width(f32::INFINITY).desired_rows(40)).on_hover_text("Edit, then Apply. Field reference: README.md, section Plans.");
                });
            }
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(18));

        // Mouse look + wheel speed.
        let mut look = Vec2::ZERO;
        if resp.dragged_by(PointerButton::Secondary) || (!self.edit_mode && resp.dragged_by(PointerButton::Primary)) || resp.dragged_by(PointerButton::Middle) {
            look = resp.drag_delta();
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.speed = (self.speed * 1.1f32.powf(scroll / 40.0)).clamp(0.5, 2000.0);
            }
        }
        self.fly(ui.ctx(), look);

        let Some(live) = &self.live else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Press F5 (or ▶ Start live) to start the renderers in this plan.\nH for key bindings.",
                egui::FontId::proportional(18.0),
                Color32::GRAY,
            );
            return;
        };

        // Snapshot lane state.
        let lanes: Vec<(String, LaneStatus, Option<live::LiveFrame>, f64, Option<f64>, bool)> = {
            let sh = live.shared.lock().unwrap();
            sh.lanes
                .iter()
                .map(|l| {
                    let (fps, gpu) = recent(&l.samples, 1.0);
                    // Lanes whose renderer can't draw meshes say so while mesh mode is on.
                    let no_meshes = self.mode.meshes && !self.infos.iter().any(|i| i.name == l.spec.name && i.tags.iter().any(|t| t == "meshes"));
                    (l.spec.label().to_string(), l.status.clone(), l.frame.clone(), fps, gpu, no_meshes)
                })
                .collect()
        };
        self.textures.resize_with(lanes.len(), || None);
        let visible: Vec<usize> = match self.focus {
            Some(f) if f < lanes.len() => vec![f],
            _ => (0..lanes.len()).collect(),
        };
        let n = visible.len().max(1);
        let cols = (n as f32).sqrt().ceil() as usize;
        let rows = n.div_ceil(cols);
        let tile = Vec2::new(rect.width() / cols as f32, rect.height() / rows as f32);
        let mut pick: Option<(Rect, u32, u32)> = None;
        let hover = resp.hover_pos();
        let ppp = ui.ctx().pixels_per_point();
        let snap = |p: Pos2| Pos2::new((p.x * ppp).round() / ppp, (p.y * ppp).round() / ppp);
        for (k, &i) in visible.iter().enumerate() {
            let (label, status, frame, fps, gpu, no_meshes) = &lanes[i];
            let mut t = Rect::from_min_size(rect.min + Vec2::new((k % cols) as f32 * tile.x, (k / cols) as f32 * tile.y), tile);
            if visible.len() > 1 {
                t = t.shrink(2.0);
                painter.rect_stroke(t, 2.0, egui::Stroke::new(1.0, Color32::from_gray(50)), egui::StrokeKind::Inside);
            }
            t = Rect::from_min_max(snap(t.min), snap(t.max));
            if k == 0 {
                self.tile_px = Some([(t.width() * ppp).round() as u32, (t.height() * ppp).round() as u32]);
            }
            if let Some(f) = frame {
                let entry = &mut self.textures[i];
                if entry.as_ref().is_none_or(|e| e.seq != f.seq) {
                    let img = egui::ColorImage::from_rgba_unmultiplied([f.width as usize, f.height as usize], &f.pixels);
                    match entry {
                        Some(e) => {
                            e.tex.set(img, TextureOptions::NEAREST);
                            e.seq = f.seq;
                        }
                        None => *entry = Some(LaneTex { tex: ui.ctx().load_texture(format!("lane{i}"), img, TextureOptions::NEAREST), seq: f.seq }),
                    }
                }
                let size = if self.pixel_exact {
                    // One frame pixel per physical screen pixel; larger frames are cropped.
                    Vec2::new(f.width as f32, f.height as f32) / ppp
                } else {
                    let aspect = f.width as f32 / f.height as f32;
                    if t.width() / t.height() > aspect { Vec2::new(t.height() * aspect, t.height()) } else { Vec2::new(t.width(), t.width() / aspect) }
                };
                let min = snap(t.center() - size / 2.0);
                let img_rect = Rect::from_min_size(min, size);
                if let Some(e) = &self.textures[i] {
                    painter.with_clip_rect(t).image(e.tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                if hover.is_some_and(|h| img_rect.contains(h)) {
                    pick = Some((img_rect, f.width, f.height));
                }
            }
            if !self.overlays {
                if resp.double_clicked() && hover.is_some_and(|h| t.contains(h)) {
                    self.focus = if self.focus.is_some() { None } else { Some(i) };
                }
                continue;
            }
            let mut text = format!("{} {}  {:.0} fps", i + 1, label, fps);
            if let Some(g) = gpu {
                text += &format!("  gpu {g:.2} ms");
            }
            if *no_meshes {
                text += "  (no meshes)";
            }
            match status {
                LaneStatus::Starting => text += "  — starting…",
                LaneStatus::Failed(e) => text += &format!("  — FAILED: {}", e.lines().next().unwrap_or("")),
                LaneStatus::Running => {}
            }
            let galley = painter.layout_no_wrap(text, egui::FontId::monospace(13.0), Color32::WHITE);
            let bg = Rect::from_min_size(t.min + Vec2::splat(4.0), galley.size() + Vec2::splat(8.0));
            painter.rect_filled(bg, 3.0, Color32::from_black_alpha(170));
            painter.galley(bg.min + Vec2::splat(4.0), galley, Color32::WHITE);
            if resp.double_clicked() && hover.is_some_and(|h| t.contains(h)) {
                self.focus = if self.focus.is_some() { None } else { Some(i) };
            }
        }

        // HUD
        if self.overlays {
            let hud = format!(
                "{} · {} · {} · speed {:.0}{}{}{}",
                self.mode.name(),
                if self.follow_path { format!("path @ {:.0}", self.scene_frame) } else { "free fly".into() },
                if self.playing { "▶" } else { "⏸" },
                self.speed,
                if self.edit_mode { " · EDIT" } else { "" },
                if self.pixel_exact { " · 1:1" } else { "" },
                if self.fullscreen { " · F11/Esc exits fullscreen · O hides overlays" } else { "" }
            );
            painter.text(rect.left_bottom() + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM, hud, egui::FontId::monospace(13.0), Color32::LIGHT_GRAY);
        }
        if self.edit_mode {
            if let Some(h) = hover {
                painter.circle_stroke(h, 6.0, egui::Stroke::new(1.5, Color32::YELLOW));
            }
        }

        // Block editing.
        if self.edit_mode && resp.clicked_by(PointerButton::Primary) {
            if let (Some((r, w, h)), Some(p), Some(cam)) = (pick, resp.interact_pointer_pos(), self.live_camera()) {
                let x = (p.x - r.min.x) / r.width() * w as f32;
                let y = (p.y - r.min.y) / r.height() * h as f32;
                let dir = cam.ray_dir_at(x, y, w, h);
                let ctrl = ui.input(|i| i.modifiers.ctrl);
                let place = ctrl.then_some(BlockId(self.place_block as u16 + 1));
                self.send(Cmd::Edit { origin: cam.position, dir, place });
            }
        }
    }

    fn right(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.right_tab, RightTab::Metrics, "📈 Metrics").on_hover_text("Live statistics, knobs and auto-tuners per renderer");
            ui.selectable_value(&mut self.right_tab, RightTab::Compare, "⚖ Compare").on_hover_text("Image comparison of the current view against the reference (C)");
            ui.selectable_value(&mut self.right_tab, RightTab::Benchmark, "⏱ Benchmark").on_hover_text("Results of the last benchmark run");
            ui.selectable_value(&mut self.right_tab, RightTab::Help, "❓ Keys").on_hover_text("Key bindings (H)");
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("right").show(ui, |ui| match self.right_tab {
            RightTab::Metrics => self.metrics(ui),
            RightTab::Compare => self.compare(ui),
            RightTab::Benchmark => self.benchmark(ui),
            RightTab::Help => help(ui),
        });
    }

    fn metrics(&mut self, ui: &mut egui::Ui) {
        let Some(live) = &self.live else {
            ui.weak("No live session.");
            return;
        };
        let shared = live.shared.clone();
        let mut actions: Vec<LaneAction> = vec![];
        let (series, quality): (Vec<(String, Vec<Sample>)>, Vec<(String, Vec<[f64; 2]>)>) = {
            let sh = shared.lock().unwrap();
            ui.label(format!("{} · {} lights · {} chunks cached on host", sh.status, sh.lights, sh.cached_chunks))
                .on_hover_text("Chunks are generated once on the host and streamed to each renderer around its camera");
            for (i, l) in sh.lanes.iter().enumerate() {
                ui.push_id(("lane", i), |ui| {
                    ui.group(|ui| self.lane_panel(ui, i, l, &mut actions));
                });
            }
            (
                sh.lanes.iter().map(|l| (l.spec.label().to_string(), window(&l.samples, self.plot_window).to_vec())).collect(),
                sh.lanes
                    .iter()
                    .map(|l| (l.spec.label().to_string(), l.quality.iter().map(|q| [q.t, if self.plot_ms_ssim { q.ms_ssim_db } else { q.psnr }]).collect()))
                    .collect(),
            )
        };
        for a in actions {
            match a {
                LaneAction::Focus(i) => self.focus = if self.focus == Some(i) { None } else { Some(i) },
                LaneAction::Restart(i) => {
                    if let Some(spec) = self.plan.renderers.get(i).cloned() {
                        self.send(Cmd::Restart { lane: i, spec });
                    }
                }
                LaneAction::Remove(i) => {
                    self.send(Cmd::RemoveLane(i));
                    self.textures.clear();
                    self.focus = None;
                }
                LaneAction::Set(lane, name, value) => self.send(Cmd::SetParam { lane, name, value }),
                LaneAction::Tune(lane, config) => self.send(Cmd::Tune { lane, config }),
                LaneAction::SaveToPlan(i, spec) => {
                    if let Some(r) = self.plan.renderers.get_mut(i) {
                        *r = spec;
                        self.option_text = option_texts(&self.plan);
                        self.dirty = true;
                        self.message = format!("knobs of renderer {} copied into the plan", i + 1);
                    }
                }
            }
        }
        ui.add(egui::Slider::new(&mut self.plot_window, 2.0..=120.0).text("plot window (s)")).on_hover_text("Seconds of history shown in the plots below");
        let t_end = series.iter().filter_map(|(_, s)| s.last().map(|x| x.t)).fold(0.0, f64::max);
        for (title, f) in [
            ("frame interval (ms)", (|s: &Sample| Some(s.interval_ms)) as fn(&Sample) -> Option<f32>),
            ("GPU time (ms)", |s: &Sample| s.gpu_ms),
            ("CPU time (ms)", |s: &Sample| Some(s.cpu_ms)),
        ] {
            ui.label(title).on_hover_text(match title {
                "frame interval (ms)" => "Time between consecutive completed frames (1000 / fps). Spikes are hitches.",
                "GPU time (ms)" => "GPU execution time per frame from timestamp queries (GPU renderers only)",
                _ => "Time the renderer process spent on the CPU per frame (submission, or the whole frame for CPU renderers)",
            });
            egui_plot::Plot::new(title)
                .height(130.0)
                .legend(egui_plot::Legend::default())
                .include_y(0.0)
                .allow_scroll(false)
                .allow_drag(false)
                .show(ui, |pu| {
                    for (i, (name, s)) in series.iter().enumerate() {
                        let pts: Vec<[f64; 2]> = s.iter().filter_map(|x| f(x).map(|v| [x.t - t_end, v as f64])).collect();
                        pu.line(egui_plot::Line::new(name.clone(), pts).color(lane_color(i)));
                    }
                });
        }
        if quality.iter().any(|(_, q)| !q.is_empty()) {
            ui.horizontal(|ui| {
                ui.label("quality vs reference (dB, probes):").on_hover_text("Each probe renders the current view on the reference renderer and scores every lane against it. Higher is better.");
                ui.selectable_value(&mut self.plot_ms_ssim, false, "PSNR").on_hover_text("Pixel-exact error (≥40 dB looks identical); harsh on resolution changes");
                ui.selectable_value(&mut self.plot_ms_ssim, true, "MS-SSIM").on_hover_text("Structural similarity over 5 scales; 20 dB = 0.99, 30 dB = 0.999");
            });
            egui_plot::Plot::new("quality").height(130.0).legend(egui_plot::Legend::default()).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                for (i, (name, q)) in quality.iter().enumerate() {
                    let pts: Vec<[f64; 2]> = q.iter().filter(|p| p[0] >= t_end - self.plot_window).map(|p| [p[0] - t_end, p[1]]).collect();
                    pu.line(egui_plot::Line::new(name.clone(), pts).color(lane_color(i)));
                    pu.points(egui_plot::Points::new(name.clone(), q.iter().filter(|p| p[0] >= t_end - self.plot_window).map(|p| [p[0] - t_end, p[1]]).collect::<Vec<_>>()).color(lane_color(i)).radius(2.5));
                }
            });
        } else {
            ui.weak("Quality plot: set a probe interval in the toolbar (or press C) to score lanes against the reference.");
        }
    }

    fn lane_panel(&mut self, ui: &mut egui::Ui, i: usize, l: &live::LaneView, actions: &mut Vec<LaneAction>) {
        ui.horizontal(|ui| {
            ui.colored_label(lane_color(i), format!("■ {} {}", i + 1, l.spec.label()));
            ui.weak(format!("{}×{}", l.resolution[0], l.resolution[1])).on_hover_text("Current render resolution (after host.render_scale)");
            if ui.small_button("focus").on_hover_text("Show only this renderer in the view (number keys, or double-click a view)").clicked() {
                actions.push(LaneAction::Focus(i));
            }
            if ui.small_button("↻").on_hover_text("Hot-swap to this renderer's spec from the plan editor; the old one keeps rendering until the new one is ready").clicked() {
                actions.push(LaneAction::Restart(i));
            }
            if ui.small_button("💾").on_hover_text("Copy this renderer's current knob values into the plan (then Save to keep them)").clicked() {
                actions.push(LaneAction::SaveToPlan(i, l.spec.clone()));
            }
            if ui.small_button("✖").on_hover_text("Remove from the live session (the plan is unchanged)").clicked() {
                actions.push(LaneAction::Remove(i));
            }
        });
        if let Some(sw) = &l.swap {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("hot swap: {sw} (still rendering the old one)"));
            });
        }
        match &l.status {
            LaneStatus::Failed(e) => {
                ui.colored_label(Color32::LIGHT_RED, e);
                return;
            }
            LaneStatus::Starting => {
                ui.weak("starting…");
                return;
            }
            LaneStatus::Running => {}
        }
        let w = window(&l.samples, 2.0);
        let (fps, _) = recent(&l.samples, 1.0);
        let stat = |f: &dyn Fn(&Sample) -> Option<f32>| {
            let mut v: Vec<f32> = w.iter().filter_map(f).collect();
            if v.is_empty() {
                return "-".to_string();
            }
            v.sort_by(f32::total_cmp);
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            format!("{mean:.2} / {:.2}", v[((v.len() - 1) as f32 * 0.99) as usize])
        };
        egui::CollapsingHeader::new(format!("stats — {fps:.0} fps{}", l.quality.back().map(|q| format!(" · {:.1} dB PSNR · {:.1} dB MS-SSIM", q.psnr, q.ms_ssim_db)).unwrap_or_default()))
            .id_salt(("stats", i))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                    for (name, f) in [
                        ("frame interval ms (mean/p99)", &(|s: &Sample| Some(s.interval_ms).filter(|x| *x > 0.0)) as &dyn Fn(&Sample) -> Option<f32>),
                        ("latency ms", &|s: &Sample| Some(s.latency_ms)),
                        ("gpu ms", &|s: &Sample| s.gpu_ms),
                        ("cpu ms", &|s: &Sample| Some(s.cpu_ms)),
                        ("update ms", &|s: &Sample| Some(s.update_ms)),
                    ] {
                        ui.label(name).on_hover_text(match name {
                            "frame interval ms (mean/p99)" => "Time between completed frames over the last 2 s: mean and 99th percentile",
                            "latency ms" => "Host-measured time from sending a frame request to the frame arriving (includes queueing)",
                            "gpu ms" => "GPU execution time from timestamp queries",
                            "cpu ms" => "Renderer-process CPU time per frame",
                            _ => "Time spent applying world updates (chunk uploads, edits, lights) per frame",
                        });
                        ui.monospace(stat(f));
                        ui.end_row();
                    }
                    if let Some(q) = l.quality.back() {
                        ui.label("quality (last probe)").on_hover_text("Agreement with the reference renderer at the last probe; bad = pixels off by more than 16/255");
                        ui.monospace(format!("PSNR {:.1} · SSIM {:.1} · MS-SSIM {:.1} dB · {:.2}% bad", q.psnr, q.ssim_db, q.ms_ssim_db, q.bad_pct));
                        ui.end_row();
                    }
                    ui.label("memory").on_hover_text("RSS: the renderer process's resident memory. GPU: buffers and textures it reports allocating.");
                    ui.monospace(format!(
                        "RSS {} MB · GPU {} MB",
                        l.rss_mb.map(|x| format!("{x:.0}")).unwrap_or("-".into()),
                        l.gpu_mb.map(|x| format!("{x:.0}")).unwrap_or("-".into())
                    ));
                    ui.end_row();
                    ui.label("streaming").on_hover_text("Chunks sent to this renderer so far, and chunks queued for it (limited per frame)");
                    ui.monospace(format!("{} chunks sent · {} pending", l.chunks_sent, l.pending_chunks));
                    ui.end_row();
                    for (k, v) in &l.counters {
                        ui.label(k).on_hover_text("Renderer-specific counter");
                        ui.monospace(if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.3}") });
                        ui.end_row();
                    }
                });
            });
        egui::CollapsingHeader::new("knobs").id_salt(("knobs", i)).show(ui, |ui| {
            ui.weak("↻ = changing it hot-swaps the renderer · 🔒 = the tuner may not touch it");
            egui::Grid::new("knobs").num_columns(3).striped(true).show(ui, |ui| {
                for (spec, value) in &l.params {
                    let label = if spec.live { spec.name.clone() } else { format!("{} ↻", spec.name) };
                    ui.label(label).on_hover_text(&spec.description);
                    let key = (i, spec.name.clone());
                    let shown = self.knob_edits.get(&key).cloned().unwrap_or(value.clone());
                    let continuous = spec.live && spec.name != "host.render_scale";
                    if let Some((v, commit)) = knob_widget(ui, spec, &shown) {
                        if continuous || commit {
                            self.knob_edits.remove(&key);
                            actions.push(LaneAction::Set(i, spec.name.clone(), v));
                        } else {
                            self.knob_edits.insert(key.clone(), v);
                        }
                    }
                    let mut locked = self.locks.contains(&key);
                    if ui.checkbox(&mut locked, "🔒").on_hover_text("Lock: the auto-tuner may not change this knob (applies when the tuner is (re)started)").changed() {
                        if locked {
                            self.locks.insert(key);
                        } else {
                            self.locks.remove(&key);
                        }
                    }
                    ui.end_row();
                }
            });
        });
        let running = l.tuner.is_some();
        egui::CollapsingHeader::new(if running { "auto-tune (running)" } else { "auto-tune" }).id_salt(("tune", i)).default_open(running).show(ui, |ui| {
            let t = self.tune_ui.entry(i).or_default();
            ui.horizontal(|ui| {
                ui.label("goal").on_hover_text("What the tuner optimises. Locked knobs (🔒 in the knob list above) are never touched.");
                ui.selectable_value(&mut t.goal, 0, "max quality @ fps").on_hover_text("Hold at least the target frame rate; spend whatever headroom remains on image quality");
                ui.selectable_value(&mut t.goal, 1, "max fps @ quality").on_hover_text("Keep quality at or above a floor (dB vs the reference); go as fast as possible");
                ui.selectable_value(&mut t.goal, 2, "max fps").on_hover_text("As fast as possible; quality is ignored (no reference probes needed)");
            });
            match t.goal {
                0 => {
                    ui.add(egui::DragValue::new(&mut t.fps).range(1.0..=10000.0).prefix("target ").suffix(" fps")).on_hover_text("Frame rate to hold. Measured as throughput over each window; 2 % below still counts as met.");
                }
                1 => {
                    ui.add(egui::DragValue::new(&mut t.psnr).range(5.0..=99.0).prefix("floor ").suffix(" dB")).on_hover_text("Minimum quality vs the reference renderer, in dB of the chosen metric.\nMS-SSIM: 20 dB = 0.99, 30 dB = 0.999. PSNR: ≥40 dB is visually near-identical.");
                }
                _ => {}
            }
            if t.goal != 2 {
                ui.horizontal(|ui| {
                    ui.label("quality metric").on_hover_text("How quality is measured: the current view is rendered by the reference renderer and compared");
                    ui.selectable_value(&mut t.metric, tune::QualityMetric::MsSsim, "MS-SSIM").on_hover_text("Multi-scale structural similarity: degrades gracefully with resolution (recommended)");
                    ui.selectable_value(&mut t.metric, tune::QualityMetric::Psnr, "PSNR").on_hover_text("Pixel-exact: any resolution change on pixel-scale detail costs ~30 dB at once");
                });
            }
            ui.horizontal(|ui| {
                ui.label("algorithm").on_hover_text("How the next configuration to try is chosen");
                ui.selectable_value(&mut t.algorithm, Algorithm::LocalSearch, "local search").on_hover_text("Randomised hill climbing: changes one unlocked knob at a time, keeps it if the goal improves");
                ui.selectable_value(&mut t.algorithm, Algorithm::ResolutionController, "resolution controller").on_hover_text("Dynamic resolution: steers host.render_scale continuously to hold the fps target (needs the fps goal)");
                ui.selectable_value(&mut t.algorithm, Algorithm::Bayesian, "bayesian").on_hover_text("Learns a model of fps and quality over all unlocked knobs and picks the most promising point; changes several at once");
            });
            ui.checkbox(&mut t.allow_restart, "may change ↻ knobs (hot swaps)").on_hover_text("Knobs marked ↻ need the renderer to restart. The replacement loads while the old one keeps rendering, but each trial costs a load.");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut t.settle).range(0.05..=10.0).speed(0.05).prefix("settle ").suffix(" s")).on_hover_text("Seconds ignored after each change (pipelines warm up, streaming settles)");
                ui.add(egui::DragValue::new(&mut t.window).range(0.1..=30.0).speed(0.05).prefix("measure ").suffix(" s")).on_hover_text("Seconds of frames measured per trial. Longer = less noise, slower search.");
                ui.add(egui::DragValue::new(&mut t.patience).range(0..=200).prefix("patience ")).on_hover_text("Trials without improvement before the tuner settles on the best configuration and only watches it.\nIt resumes exploring if the goal stops being met. 0 = explore forever.");
            });
            let t = t.clone();
            let locked: std::collections::BTreeSet<String> = self.locks.iter().filter(|(lane, _)| *lane == i).map(|(_, n)| n.clone()).collect();
            let config = tune::TunerConfig {
                goal: match t.goal {
                    0 => tune::Goal::MaxQualityAtFps { fps: t.fps },
                    1 => tune::Goal::MaxFpsAtQuality { psnr: t.psnr },
                    _ => tune::Goal::MaxFps,
                },
                algorithm: t.algorithm,
                metric: t.metric,
                locked,
                allow_restart: t.allow_restart,
                settle_s: t.settle,
                window_s: t.window,
                patience: t.patience,
            };
            ui.horizontal(|ui| {
                if ui.button(if running { "↻ restart tuner" } else { "▶ start tuner" }).on_hover_text("Start with the goal, locks and settings above (restarting first restores the best configuration found)").clicked() {
                    actions.push(LaneAction::Tune(i, Some(config.clone())));
                }
                if running && ui.button("■ stop").on_hover_text("Stop and apply the best configuration found").clicked() {
                    actions.push(LaneAction::Tune(i, None));
                }
            });
            if config.goal.needs_quality() {
                ui.weak("This goal needs quality: the tuner requests reference probes itself (the reference renderer uses CPU time).");
            }
            if let Some(tv) = &l.tuner {
                let metric = tv.config.metric.name();
                ui.label(format!("{:?} via {:?}", tv.config.goal, tv.config.algorithm));
                ui.horizontal(|ui| {
                    if tv.holding {
                        ui.colored_label(Color32::LIGHT_GREEN, "● holding").on_hover_text("Converged: the best configuration is applied and re-measured periodically");
                    } else {
                        ui.colored_label(Color32::GOLD, "● exploring").on_hover_text("Trying configurations");
                    }
                    ui.label(format!("{} · {} trials, {} accepted", tv.status, tv.trials, tv.accepted));
                });
                if let Some(best) = &tv.best {
                    ui.label(format!("best: {best}")).on_hover_text("Best measured result; stopping the tuner applies it");
                    if tv.best_diff.is_empty() {
                        ui.weak("   (the starting configuration)");
                    }
                    for (k, from, to) in &tv.best_diff {
                        ui.monospace(format!("   {k}: {from} → {to}"));
                    }
                }
                let pts: Vec<[f64; 2]> = tv.history.iter().map(|h| [h.t, h.score]).collect();
                let pick = |kind: tune::PointKind| -> Vec<[f64; 2]> { tv.history.iter().filter(|h| h.kind == kind).map(|h| [h.t, h.score]).collect() };
                ui.weak("score per evaluation").on_hover_text("Higher is better. Constrained goals score above 100 only when the constraint is met.\nGreen: accepted · red: rejected · blue: check while holding");
                egui_plot::Plot::new(("tuner", i)).height(110.0).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                    pu.line(egui_plot::Line::new("score", pts).color(lane_color(i)));
                    pu.points(egui_plot::Points::new("accepted", pick(tune::PointKind::Accepted)).radius(3.0).color(Color32::LIGHT_GREEN));
                    pu.points(egui_plot::Points::new("rejected", pick(tune::PointKind::Rejected)).radius(2.0).color(Color32::from_rgb(220, 90, 90)));
                    pu.points(egui_plot::Points::new("hold", pick(tune::PointKind::Hold)).radius(2.5).color(Color32::LIGHT_BLUE));
                });
                let fps: Vec<[f64; 2]> = tv.history.iter().map(|h| [h.t, h.fps]).collect();
                let q: Vec<[f64; 2]> = tv.history.iter().filter_map(|h| h.psnr.map(|p| [h.t, p])).collect();
                ui.weak(format!("measured fps and {metric} per evaluation"));
                egui_plot::Plot::new(("tuner-fq", i)).height(90.0).legend(egui_plot::Legend::default()).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                    pu.line(egui_plot::Line::new("fps", fps).color(Color32::LIGHT_BLUE));
                    if !q.is_empty() {
                        pu.line(egui_plot::Line::new(format!("{metric} dB"), q).color(Color32::GOLD));
                    }
                });
                // Knob trajectories, normalised to each knob's range.
                let names: Vec<String> = tv.history.iter().rev().find(|h| !h.knobs.is_empty()).map(|h| h.knobs.iter().map(|(k, _)| k.clone()).collect()).unwrap_or_default();
                if !names.is_empty() {
                    ui.weak("knob values (0 = minimum, 1 = maximum of each knob's range)").on_hover_text("Each line is one tunable knob over time; hover a line for its name");
                    egui_plot::Plot::new(("tuner-knobs", i)).height(110.0).include_y(0.0).include_y(1.0).legend(egui_plot::Legend::default()).allow_scroll(false).allow_drag(false).show(ui, |pu| {
                        for (n, name) in names.iter().enumerate() {
                            let line: Vec<[f64; 2]> = tv.history.iter().filter_map(|h| h.knobs.iter().find(|(k, _)| k == name).map(|(_, v)| [h.t, *v])).collect();
                            let hue = n as f32 / names.len() as f32;
                            pu.line(egui_plot::Line::new(name.trim_start_matches("host."), line).color(egui::ecolor::Hsva::new(hue, 0.6, 0.95, 1.0)));
                        }
                    });
                }
                egui::ScrollArea::vertical().id_salt(("tlog", i)).max_height(120.0).stick_to_bottom(true).show(ui, |ui| {
                    for line in &tv.log {
                        ui.monospace(line);
                    }
                });
            }
        });
        if !l.events.is_empty() {
            egui::CollapsingHeader::new(format!("events ({})", l.events.len())).id_salt(("ev", i)).show(ui, |ui| {
                for (t, e) in l.events.iter().rev().take(20) {
                    ui.monospace(format!("{t:7.1}s  {e}"));
                }
            });
        }
    }

    fn compare(&mut self, ui: &mut egui::Ui) {
        let Some(live) = &self.live else {
            ui.weak("Start a live session, frame a view, then press C.");
            return;
        };
        let sh = live.shared.lock().unwrap();
        if sh.comparing {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("rendering comparison (the reference is slow)…");
            });
        }
        let Some(r) = &sh.compare else {
            ui.weak("Press C (or ⚖ Compare) to render the current view on every renderer and the reference.");
            return;
        };
        if self.compare_tex.as_ref().is_none_or(|c| c.seq != r.seq) {
            let size = [r.width as usize, r.height as usize];
            let mk = |name: String, px: &[u8]| ui.ctx().load_texture(name, egui::ColorImage::from_rgba_unmultiplied(size, px), TextureOptions::NEAREST);
            self.compare_tex = Some(CompareTex {
                seq: r.seq,
                reference: mk("cmp-ref".into(), &r.reference),
                lanes: r.lanes.iter().enumerate().map(|(i, l)| (mk(format!("cmp{i}"), &l.image), mk(format!("cmpd{i}"), &l.diff))).collect(),
                show: 0,
                diff: true,
            });
        }
        let p = r.camera.position;
        ui.label(format!("{} mode · camera ({:.1}, {:.1}, {:.1}) · reference: {}", r.mode.name(), p.x, p.y, p.z, r.reference_label));
        egui::Grid::new("cmp").striped(true).show(ui, |ui| {
            ui.strong("renderer");
            ui.strong("PSNR dB").on_hover_text("Peak signal-to-noise ratio vs the reference; higher is better, ≥40 dB looks identical, 99 = exact");
            ui.strong("MS-SSIM dB").on_hover_text("Multi-scale structural similarity as dB (20 = 0.99, 30 = 0.999); tolerant of resampling");
            ui.strong("MAE").on_hover_text("Mean absolute error per channel, 0–1");
            ui.strong("bad %").on_hover_text("Pixels whose largest channel differs by more than 16/255");
            ui.end_row();
            for l in &r.lanes {
                ui.label(&l.label);
                ui.monospace(format!("{:.1}", l.quality.psnr.min(99.0)));
                ui.monospace(format!("{:.1}", l.quality.ms_ssim_db));
                ui.monospace(format!("{:.4}", l.quality.mae));
                ui.monospace(format!("{:.3}", l.quality.bad_pct));
                ui.end_row();
            }
        });
        let labels: Vec<String> = r.lanes.iter().map(|l| l.label.clone()).collect();
        drop(sh);
        let Some(ct) = &mut self.compare_tex else { return };
        ui.horizontal_wrapped(|ui| {
            for (i, l) in labels.iter().enumerate() {
                ui.selectable_value(&mut ct.show, i, l).on_hover_text("Show this renderer's frame");
            }
            ui.selectable_value(&mut ct.show, usize::MAX, "reference").on_hover_text("Show the reference renderer's frame");
            ui.checkbox(&mut ct.diff, "show diff").on_hover_text("Grey image with differing pixels in red (brighter = larger difference)");
        });
        let tex = match ct.lanes.get(ct.show) {
            Some((img, diff)) => {
                if ct.diff {
                    diff
                } else {
                    img
                }
            }
            None => &ct.reference,
        };
        let w = ui.available_width();
        let s = tex.size_vec2();
        ui.image((tex.id(), Vec2::new(w, w * s.y / s.x)));
        ui.weak("Diff: grey = match, red = differs (brighter = larger).");
    }

    fn benchmark(&mut self, ui: &mut egui::Ui) {
        if self.live.is_some() && self.bench.as_ref().is_some_and(|b| !b.finished) {
            ui.colored_label(Color32::YELLOW, "The live session is sharing the GPU with this benchmark; stop it for clean numbers.");
        }
        if let Some(b) = &mut self.bench {
            if b.poll() {
                if let Some(p) = b.report.lock().unwrap().take() {
                    match bench::load_report(&p) {
                        Ok(r) => self.report = Some(r),
                        Err(e) => self.message = format!("report: {e:#}"),
                    }
                }
            }
            egui::CollapsingHeader::new("benchmark output").default_open(!b.finished).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("bench-out").max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
                    for l in b.lines.lock().unwrap().iter() {
                        ui.monospace(l);
                    }
                });
            });
        }
        let base = PathBuf::from(self.plan.output.dir.clone().unwrap_or_else(|| "runs".into()));
        ui.menu_button("📂 Load a previous report", |ui| {
            ui.weak("Benchmark runs found in the plan's output directory, newest first");
            for p in bench::past_runs(&base).into_iter().take(40) {
                let name = p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                    match bench::load_report(&p) {
                        Ok(r) => self.report = Some(r),
                        Err(e) => self.message = format!("report: {e:#}"),
                    }
                    ui.close();
                }
            }
        });
        if let Some(r) = &self.report {
            bench::report_ui(ui, r);
        } else {
            ui.weak("Run a benchmark (⏱) or load a previous report.");
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
            if let Some(l) = &self.live {
                *l.control.lock().unwrap() = self.control();
                ctx.request_repaint();
            }
            return;
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

        if let Some(l) = &self.live {
            *l.control.lock().unwrap() = self.control();
            ctx.request_repaint();
        }
        if self.bench.as_ref().is_some_and(|b| !b.finished) {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

enum LaneAction {
    Focus(usize),
    Restart(usize),
    Remove(usize),
    SaveToPlan(usize, crate::plan::RendererSpec),
    Set(usize, String, mr_core::ParamValue),
    Tune(usize, Option<tune::TunerConfig>),
}

#[derive(Clone)]
struct TuneUi {
    goal: u8,
    fps: f64,
    psnr: f64,
    metric: tune::QualityMetric,
    algorithm: Algorithm,
    allow_restart: bool,
    settle: f64,
    window: f64,
    patience: u32,
}

impl Default for TuneUi {
    fn default() -> Self {
        let d = tune::TunerConfig::default();
        Self { goal: 0, fps: 144.0, psnr: 25.0, metric: tune::QualityMetric::MsSsim, algorithm: Algorithm::LocalSearch, allow_restart: false, settle: d.settle_s, window: d.window_s, patience: d.patience }
    }
}

/// Edits a knob value. Returns the new value and whether the edit is complete (e.g. a
/// drag finished), for knobs that should only be applied once.
fn knob_widget(ui: &mut egui::Ui, spec: &mr_core::ParamSpec, value: &mr_core::ParamValue) -> Option<(mr_core::ParamValue, bool)> {
    use mr_core::{ParamKind, ParamValue};
    match &spec.kind {
        ParamKind::Bool => {
            let mut b = value.as_bool().unwrap_or(false);
            ui.checkbox(&mut b, "").on_hover_text(&spec.description).changed().then(|| (ParamValue::Bool(b), true))
        }
        ParamKind::Int { min, max } => {
            let mut v = value.as_i64().unwrap_or(*min);
            let speed = ((*max - *min) as f64 / 300.0).max(0.05);
            let r = ui.add(egui::DragValue::new(&mut v).range(*min..=*max).speed(speed)).on_hover_text(format!("{}\nRange {min}–{max}. Drag or double-click to type.", spec.description));
            let commit = r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus());
            (r.changed() || commit).then(|| (ParamValue::Int(v), commit))
        }
        ParamKind::Float { min, max, log } => {
            let mut v = value.as_f64().unwrap_or(*min);
            let r = ui.add(egui::Slider::new(&mut v, *min..=*max).logarithmic(*log).max_decimals(3)).on_hover_text(&spec.description);
            let commit = r.drag_stopped() || (r.changed() && !r.dragged());
            (r.changed() || commit).then(|| (ParamValue::Float(v), commit))
        }
        ParamKind::Choice(opts) => {
            let cur = value.as_str().unwrap_or("").to_string();
            let mut out = None;
            egui::ComboBox::from_id_salt(&spec.name)
                .selected_text(&cur)
                .show_ui(ui, |ui| {
                    for o in opts {
                        if ui.selectable_label(*o == cur, o).clicked() && *o != cur {
                            out = Some((ParamValue::Choice(o.clone()), true));
                        }
                    }
                })
                .response
                .on_hover_text(&spec.description);
            out
        }
    }
}

fn plan_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir("plans").into_iter().flatten().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect();
    v.sort();
    v
}

/// Samples within the last `secs` seconds of the newest one.
fn window(s: &std::collections::VecDeque<Sample>, secs: f64) -> Vec<Sample> {
    let Some(last) = s.back() else { return vec![] };
    s.iter().rev().take_while(|x| x.t >= last.t - secs).copied().collect::<Vec<_>>().into_iter().rev().collect()
}

/// (fps, mean gpu ms) over the last `secs` seconds.
fn recent(s: &std::collections::VecDeque<Sample>, secs: f64) -> (f64, Option<f64>) {
    let w = window(s, secs);
    if w.len() < 2 {
        return (0.0, None);
    }
    let span = (w.last().unwrap().t - w[0].t).max(1e-6);
    let g: Vec<f64> = w.iter().filter_map(|x| x.gpu_ms.map(f64::from)).collect();
    ((w.len() - 1) as f64 / span, (!g.is_empty()).then(|| g.iter().sum::<f64>() / g.len() as f64))
}

fn lane_color(i: usize) -> Color32 {
    const C: [Color32; 6] = [
        Color32::from_rgb(90, 170, 255),
        Color32::from_rgb(255, 140, 70),
        Color32::from_rgb(110, 210, 120),
        Color32::from_rgb(220, 110, 220),
        Color32::from_rgb(240, 210, 80),
        Color32::from_rgb(120, 220, 220),
    ];
    C[i % C.len()]
}

fn help(ui: &mut egui::Ui) {
    ui.label("Hover over any control, column heading or stat for an explanation.");
    ui.add_space(4.0);
    egui::Grid::new("keys").striped(true).show(ui, |ui| {
        for (k, d) in [
            ("F5", "start / restart the live session with the current plan"),
            ("W A S D", "move (switches from path to free fly)"),
            ("Space / Shift", "up / down"),
            ("Ctrl (held)", "5× speed"),
            ("Right-drag / Left-drag / arrows", "look (left-drag only outside edit mode)"),
            ("Mouse wheel", "fly speed"),
            ("L", "toggle lit / unlit"),
            ("N", "toggle meshes (mesh mode)"),
            ("F", "follow the plan's camera path ↔ free fly"),
            ("P", "play / pause the scene clock (path + scripted events)"),
            ("[ / ]", "scrub scene clock −/+10 frames (Shift: 100)"),
            ("R", "reset scene clock and return to the path"),
            ("E", "edit mode: click removes a block, Ctrl+click places one"),
            ("G", "drop a point light at the camera"),
            ("C", "compare this exact view against the reference"),
            ("1–9 / Tab / Esc", "focus a renderer / cycle / show all"),
            ("F11", "fullscreen viewport (Esc or F11 to leave)"),
            ("Z", "1:1 pixels: one frame pixel per screen pixel (cropped, never scaled)"),
            ("M", "match render resolution to the tile's pixel size and restart live"),
            ("O", "show / hide overlays (labels, HUD)"),
            ("Double-click", "focus / unfocus a tile"),
            ("Ctrl+S", "save the plan"),
        ] {
            ui.monospace(k);
            ui.label(d);
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    ui.weak(
        "Live lanes run independently: each renders the latest camera as fast as it can (or up to the fps cap). \
         Frames are read back only at the display rate, so the metrics reflect the renderer rather than the GUI. \
         Renderer option changes apply with ↻; world/path/run changes apply on F5.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, app: &mut App, keys: &[Key], t: &mut f64) {
        *t += 1.0 / 60.0;
        let mut events = vec![];
        for &key in keys {
            for pressed in [true, false] {
                events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
            }
        }
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1700.0, 1000.0))),
            time: Some(*t),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| app.draw(ui));
        out.textures_delta.clear(); // no renderer in tests
    }

    /// Draws the real GUI headlessly through a live session (needs a GPU; `--ignored`).
    #[test]
    #[ignore]
    fn gui_headless_smoke() {
        let ctx = egui::Context::default();
        let mut app = App::new(&ctx, None);
        app.plan = Plan::parse(
            r#"
            [scene]
            generator = { type = "showcase" }
            camera = { type = "orbit", center = [20, 2, 0], radius = 26, height = 12, period = 120 }
            [run]
            resolution = [320, 180]
            view_radius = 3
            isolation = "thread"
            [[renderer]]
            name = "gpu-dda"
            [[renderer]]
            name = "gpu-raster"
            "#,
            &[],
        )
        .unwrap();
        app.option_text = option_texts(&app.plan);
        let mut t = 0.0;
        for tab in [RightTab::Metrics, RightTab::Compare, RightTab::Benchmark, RightTab::Help] {
            app.right_tab = tab;
            frame(&ctx, &mut app, &[], &mut t);
        }
        frame(&ctx, &mut app, &[Key::F5], &mut t);
        assert!(app.live.is_some());
        let start = std::time::Instant::now();
        while app.textures.iter().flatten().count() < 2 {
            if start.elapsed().as_secs() >= 60 {
                let msg = {
                    let sh = app.live.as_ref().unwrap().shared.lock().unwrap();
                    let lanes: Vec<_> = sh.lanes.iter().map(|l| (l.spec.name.clone(), l.status.clone(), l.frames, l.frame.is_some())).collect();
                    format!("no frames displayed; status {}; lanes {lanes:?}; log:\n{}", sh.status, sh.log.join("\n"))
                };
                panic!("{msg}");
            }
            frame(&ctx, &mut app, &[], &mut t);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        app.right_tab = RightTab::Metrics;
        for keys in [&[Key::L][..], &[Key::F], &[Key::W], &[Key::E], &[Key::Num1], &[Key::Escape], &[Key::C]] {
            for _ in 0..5 {
                frame(&ctx, &mut app, keys, &mut t);
            }
        }
        assert!(!app.follow_path, "W should switch to free fly");
        assert_eq!(app.mode, PassMode::LIT);
        let start = std::time::Instant::now();
        while app.live.as_ref().unwrap().shared.lock().unwrap().compare.is_none() {
            assert!(start.elapsed().as_secs() < 60, "compare never finished");
            frame(&ctx, &mut app, &[], &mut t);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        frame(&ctx, &mut app, &[], &mut t);
        assert!(app.compare_tex.is_some(), "compare tab should have uploaded images");

        // Tuner + periodic probes: the Metrics tab must draw knobs, tuner and quality plots.
        app.right_tab = RightTab::Metrics;
        app.probe_interval = 0.3;
        app.send(Cmd::Tune { lane: 0, config: Some(tune::TunerConfig { goal: tune::Goal::MaxFps, settle_s: 0.1, window_s: 0.3, ..Default::default() }) });
        app.send(Cmd::SetParam { lane: 1, name: "depth_prepass".into(), value: mr_core::ParamValue::Bool(false) });
        let start = std::time::Instant::now();
        loop {
            frame(&ctx, &mut app, &[], &mut t);
            let done = {
                let sh = app.live.as_ref().unwrap().shared.lock().unwrap();
                sh.lanes[0].tuner.as_ref().is_some_and(|t| t.trials >= 1) && sh.lanes.iter().all(|l| l.quality.len() >= 2)
            };
            if done {
                break;
            }
            assert!(start.elapsed().as_secs() < 60, "tuner/probes did not progress");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        app.probe_interval = 0.0;
        frame(&ctx, &mut app, &[], &mut t);
        app.left_tab = LeftTab::Toml;
        frame(&ctx, &mut app, &[], &mut t);

        // Fullscreen + 1:1 + match resolution: the viewport becomes the whole screen and
        // the render resolution follows it exactly.
        frame(&ctx, &mut app, &[Key::F11], &mut t);
        assert!(app.fullscreen);
        frame(&ctx, &mut app, &[Key::Num1], &mut t);
        frame(&ctx, &mut app, &[], &mut t);
        assert_eq!(app.tile_px, Some([1700, 1000]));
        frame(&ctx, &mut app, &[Key::M], &mut t);
        assert_eq!(app.plan.run.resolution, [1700, 1000]);
        assert!(app.pixel_exact);
        let start = std::time::Instant::now();
        while app.textures.iter().flatten().count() < 1 {
            assert!(start.elapsed().as_secs() < 60, "no frames after resolution change");
            frame(&ctx, &mut app, &[], &mut t);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let tex = app.textures.iter().flatten().next().unwrap();
        assert_eq!(tex.tex.size(), [1700, 1000]);
        frame(&ctx, &mut app, &[Key::O, Key::Z], &mut t);
        frame(&ctx, &mut app, &[Key::Escape], &mut t);
        assert!(!app.fullscreen);
        frame(&ctx, &mut app, &[], &mut t);
        app.live = None;
    }
}
