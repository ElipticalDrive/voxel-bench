//! The central view: lane images, picking and editing the world.

use egui::{Color32, PointerButton, Pos2, Rect, Sense, TextureOptions, Vec2};
use mr_core::BlockId;

use super::live::LaneStatus;

use super::metrics::{EVENT_SECS, recent};
use super::*;

impl App {
    pub(super) fn viewport(&mut self, ui: &mut egui::Ui) {
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
        type LaneSnap = (String, LaneStatus, Option<live::LiveFrame>, f64, Option<f64>, bool, Option<mr_core::Legend>, Vec<(f64, String)>);
        let lanes: Vec<LaneSnap> = {
            let sh = live.shared.lock().unwrap();
            let now = sh.start.map_or(0.0, |s| s.elapsed().as_secs_f64());
            sh.lanes
                .iter()
                .map(|l| {
                    let (fps, gpu) = recent(&l.samples, 1.0);
                    // Lanes whose renderer can't draw meshes say so while mesh mode is on.
                    let no_meshes = self.mode.meshes && !self.infos.iter().any(|i| i.name == l.spec.name && l.spec.supports(i, self.mode));
                    // The events of the last seconds, newest first, with their age.
                    let recent: Vec<(f64, String)> = l.events.iter().rev().map(|(t, e)| (now - t, e.clone())).take_while(|(age, _)| *age < EVENT_SECS).take(3).collect();
                    (l.spec.label().to_string(), l.status.clone(), l.frame.clone(), fps, gpu, no_meshes, l.legend.clone(), recent)
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
            let (label, status, frame, fps, gpu, no_meshes, legend, recent) = &lanes[i];
            let mut reading = None;
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
                if let Some(h) = hover.filter(|&h| img_rect.contains(h) && t.contains(h)) {
                    pick = Some((img_rect, f.width, f.height));
                    // The diagnostic view's value under the cursor.
                    if let Some(lg) = legend {
                        let x = ((h.x - img_rect.min.x) / img_rect.width() * f.width as f32) as u32;
                        let y = ((h.y - img_rect.min.y) / img_rect.height() * f.height as f32) as u32;
                        reading = legend::read(lg, &f.pixels, f.width, x.min(f.width - 1), y.min(f.height - 1));
                        if let (Some(r), true) = (reading, self.overlays) {
                            let galley = painter.layout_no_wrap(legend::describe(lg, r), egui::FontId::monospace(12.0), Color32::WHITE);
                            let bg = Rect::from_min_size(h + Vec2::new(14.0, 10.0), galley.size() + Vec2::splat(6.0));
                            painter.rect_filled(bg, 3.0, Color32::from_black_alpha(200));
                            painter.galley(bg.min + Vec2::splat(3.0), galley, Color32::WHITE);
                        }
                    }
                }
            }
            if !self.overlays {
                if resp.double_clicked() && hover.is_some_and(|h| t.contains(h)) {
                    self.focus = if self.focus.is_some() { None } else { Some(i) };
                }
                continue;
            }
            if let Some(lg) = legend {
                legend::draw(&painter.with_clip_rect(t), t, lg, reading);
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
            // Clipped to the tile: narrow tiles would overlap their neighbours.
            let painter = painter.with_clip_rect(t);
            let bg = Rect::from_min_size(t.min + Vec2::splat(4.0), galley.size() + Vec2::splat(8.0));
            painter.rect_filled(bg, 3.0, Color32::from_black_alpha(170));
            painter.galley(bg.min + Vec2::splat(4.0), galley, Color32::WHITE);
            // The lane's latest events, fading out.
            let mut y = bg.max.y + 3.0;
            for (age, e) in recent {
                let a = (((EVENT_SECS - age) / 1.5).min(1.0) * 255.0) as u8;
                let bad = e.starts_with("failed") || e.starts_with("refused") || e.starts_with("hot swap failed");
                let col = if bad { Color32::from_rgba_unmultiplied(255, 140, 120, a) } else { Color32::from_rgba_unmultiplied(200, 230, 255, a) };
                let galley = painter.layout(e.clone(), egui::FontId::proportional(12.0), col, (t.width() - 24.0).max(80.0));
                let bg = Rect::from_min_size(Pos2::new(t.min.x + 4.0, y), galley.size() + Vec2::splat(6.0));
                painter.rect_filled(bg, 3.0, Color32::from_black_alpha((a as f32 * 0.6) as u8));
                painter.galley(bg.min + Vec2::splat(3.0), galley, col);
                y = bg.max.y + 2.0;
            }
            if !recent.is_empty() {
                ui.ctx().request_repaint();
            }
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
}
