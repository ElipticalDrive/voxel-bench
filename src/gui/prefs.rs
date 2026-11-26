//! The GUI's preferences: a small window (⚙ in the toolbar), kept between sessions in the
//! configuration directory (`prefs.toml`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use egui::{FontId, TextStyle};

/// What the preferences window sets.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(default)]
pub(super) struct Prefs {
    /// Text size relative to the rest of the interface (high pixel density screens, poor
    /// eyesight), on top of `ui_scale`.
    pub text_scale: f32,
    /// The whole interface's scale, on top of the display's (Ctrl + / Ctrl - / Ctrl 0 set it
    /// too).
    pub ui_scale: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { text_scale: 1.0, ui_scale: 1.0 }
    }
}

/// The scales offered.
const SCALES: std::ops::RangeInclusive<f32> = 0.5..=3.0;

/// The preferences, what of them the context has, and the window.
#[derive(Default)]
pub(super) struct PrefsState {
    pub prefs: Prefs,
    /// What [`Self::apply`] last gave the context.
    applied: Option<Prefs>,
    /// The text styles' sizes before any text scale.
    base: Option<BTreeMap<TextStyle, FontId>>,
    pub open: bool,
}

impl PrefsState {
    fn file() -> Option<PathBuf> {
        Some(super::config_dir()?.join("prefs.toml"))
    }

    /// The last session's preferences, or the defaults.
    pub fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        let prefs = Self::file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        Self { prefs, ..Default::default() }
    }

    fn save(&self) {
        if cfg!(test) {
            return;
        }
        if let (Some(f), Ok(text)) = (Self::file(), toml::to_string(&self.prefs)) {
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(f, text);
        }
    }

    /// Gives the context the preferences when they changed (each frame; nothing to do
    /// otherwise). A zoom set by the keyboard is taken into them and kept.
    pub fn apply(&mut self, ctx: &egui::Context) {
        if let Some(a) = self.applied
            && a == self.prefs
            && (ctx.zoom_factor() - a.ui_scale).abs() > 1e-3
        {
            self.prefs.ui_scale = ctx.zoom_factor();
            self.applied = Some(self.prefs);
            self.save();
            return;
        }
        if self.applied == Some(self.prefs) {
            return;
        }
        let base = self.base.get_or_insert_with(|| ctx.global_style().text_styles.clone()).clone();
        let scale = self.prefs.text_scale;
        ctx.all_styles_mut(|s| {
            for (style, font) in s.text_styles.iter_mut() {
                if let Some(b) = base.get(style) {
                    font.size = b.size * scale;
                }
            }
        });
        ctx.set_zoom_factor(self.prefs.ui_scale);
        self.applied = Some(self.prefs);
    }

    /// The preferences window, when open; a change is applied and saved.
    pub fn window(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        let before = self.prefs;
        let p = &mut self.prefs;
        egui::Window::new("Preferences").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("prefs-rows").num_columns(2).spacing([6.0, 2.0]).show(ui, |ui| {
                for (name, v, tip) in [
                    ("text scale", &mut p.text_scale, "Text size relative to the rest of the interface"),
                    ("interface scale", &mut p.ui_scale, "The whole interface's size, text included (also Ctrl + / Ctrl - / Ctrl 0)"),
                ] {
                    super::knobs::form_row(ui, name, tip, |ui| {
                        super::knobs::fit_slider(ui);
                        ui.add(egui::Slider::new(v, SCALES).step_by(0.05).update_while_editing(false)).on_hover_text(tip);
                    });
                }
            });
            if ui.button("Reset").on_hover_text("Both scales back to 1").clicked() {
                *p = Prefs::default();
            }
        });
        self.open = open;
        if self.prefs != before {
            self.apply(ctx);
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text scale multiplies every text style's size from where it started, and setting
    /// it back gives the sizes back; the preferences read back from their stored form.
    #[test]
    fn text_scale_scales_the_text_styles() {
        let ctx = egui::Context::default();
        let body = |ctx: &egui::Context| ctx.global_style().text_styles[&TextStyle::Body].size;
        let start = body(&ctx);
        let mut s = PrefsState::default();
        s.prefs.text_scale = 2.0;
        s.apply(&ctx);
        assert_eq!(body(&ctx), start * 2.0);
        s.prefs.text_scale = 1.5;
        s.apply(&ctx);
        assert_eq!(body(&ctx), start * 1.5, "scaled from the start, not from the last scale");
        s.prefs.text_scale = 1.0;
        s.apply(&ctx);
        assert_eq!(body(&ctx), start);
        let p = Prefs { text_scale: 1.25, ui_scale: 0.75 };
        assert_eq!(toml::from_str::<Prefs>(&toml::to_string(&p).unwrap()).unwrap(), p);
        assert_eq!(toml::from_str::<Prefs>("").unwrap(), Prefs::default(), "a missing preference is its default");
    }
}
