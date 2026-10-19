//! Keys, flying the camera, and the key bindings tab.

use egui::{Key, Vec2};
use glam::Vec3;

use super::*;

impl App {
    // ---------------------------------------------------------------- input

    pub(super) fn keys(&mut self, ctx: &egui::Context) {
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

    pub(super) fn fly(&mut self, ctx: &egui::Context, look: Vec2) {
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
}

pub(super) fn help(ui: &mut egui::Ui) {
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
