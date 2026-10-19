//! Knob widgets shared by the renderer summaries and the pipeline editor.

use egui::{ComboBox, Ui};
use mr_core::{ParamSpec, ParamValue};

/// A knob's description, and whether it needs a restart and is specialisable.
pub fn knob_tip(spec: &ParamSpec) -> String {
    let mut t = spec.description.clone();
    if !spec.live {
        t += "\n\n↻ Takes effect on a restart.";
    }
    if spec.baked {
        t += "\n\nSpecialisable: a pipeline compiled for its value bakes it in as a constant (another value is another compiled pipeline).";
    }
    t
}

/// Width of the value column: every knob's widget fits it, so the names line up.
pub const VALUE_W: f32 = 180.0;

/// A cell `VALUE_W` wide for a value's widget(s), left-aligned.
pub fn value_cell<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.set_min_width(VALUE_W);
        add(ui)
    })
    .inner
}

/// Sizes the next slider (with its value box) to fill a value cell.
pub fn fit_slider(ui: &mut Ui) {
    ui.spacing_mut().slider_width = VALUE_W - ui.spacing().interact_size.x - 2.0 * ui.spacing().item_spacing.x;
}

/// A form row in a two-column grid: the value's widgets in a value cell, then the name.
pub fn form_row<R>(ui: &mut Ui, name: &str, tip: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let r = value_cell(ui, add);
    ui.add(egui::Label::new(egui::RichText::new(name).weak()).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(tip);
    ui.end_row();
    r
}

/// Knobs as a grid: the tuner's lock (a column only when `lock` is given), the value (a
/// constant width) and the name; `marks` labels restart-only knobs with ↻. Returns
/// (index into `knobs`, value, commit).
pub fn knob_grid<'l>(ui: &mut Ui, salt: impl std::hash::Hash + std::fmt::Debug, knobs: &[(&ParamSpec, ParamValue)], marks: bool, mut lock: Option<&mut (dyn FnMut(&mut Ui, &ParamSpec) + 'l)>) -> Vec<(usize, ParamValue, bool)> {
    let mut out = vec![];
    if knobs.is_empty() {
        return out;
    }
    egui::Grid::new(salt).num_columns(if lock.is_some() { 3 } else { 2 }).spacing([6.0, 2.0]).show(ui, |ui| {
        for (i, (spec, value)) in knobs.iter().enumerate() {
            if let Some(lock) = lock.as_deref_mut() {
                ui.horizontal(|ui| lock(ui, spec));
            }
            if let Some((v, commit)) = value_cell(ui, |ui| knob_widget(ui, spec, value)) {
                out.push((i, v, commit));
            }
            let label = if marks && !spec.live { format!("{} ↻", spec.name) } else { spec.name.clone() };
            ui.add(egui::Label::new(egui::RichText::new(label).weak()).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(knob_tip(spec));
            ui.end_row();
        }
    });
    out
}

/// Edits a knob value. Returns the new value and whether the edit is complete (e.g. a
/// drag finished), for knobs that should only be applied once.
pub fn knob_widget(ui: &mut Ui, spec: &ParamSpec, value: &ParamValue) -> Option<(ParamValue, bool)> {
    use mr_core::ParamKind;
    match &spec.kind {
        ParamKind::Bool => {
            let mut b = value.as_bool().unwrap_or(false);
            ui.checkbox(&mut b, "").on_hover_text(&spec.description).changed().then(|| (ParamValue::Bool(b), true))
        }
        ParamKind::Int { min, max } => {
            let mut v = value.as_i64().unwrap_or(*min);
            let speed = ((*max - *min) as f64 / 300.0).max(0.05);
            let r = ui.add(egui::DragValue::new(&mut v).range(*min..=*max).speed(speed).update_while_editing(false)).on_hover_text(format!("{}\nRange {min}–{max}. Drag, or double-click to type and Enter to apply.", spec.description));
            let commit = r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus());
            (r.changed() || commit).then(|| (ParamValue::Int(v), commit))
        }
        ParamKind::Float { min, max, log } => {
            let mut v = value.as_f64().unwrap_or(*min);
            fit_slider(ui);
            let r = ui.add(egui::Slider::new(&mut v, *min..=*max).logarithmic(*log).max_decimals(3).update_while_editing(false)).on_hover_text(&spec.description);
            let commit = r.drag_stopped() || (r.changed() && !r.dragged());
            (r.changed() || commit).then(|| (ParamValue::Float(v), commit))
        }
        ParamKind::Choice(opts) => {
            let cur = value.as_str().unwrap_or("").to_string();
            let mut out = None;
            ComboBox::from_id_salt(&spec.name)
                .width(VALUE_W - ui.spacing().item_spacing.x)
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

/// A plan's option value for knob `spec` (its default when unset or invalid).
pub fn option_value(spec: &ParamSpec, options: &toml::Table) -> ParamValue {
    options.get(&spec.name).and_then(|v| serde_json::to_value(v).ok()).and_then(|j| spec.from_json(&j)).unwrap_or_else(|| spec.default.clone())
}
