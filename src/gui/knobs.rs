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

/// Roughly how wide a knob's label, widget and extras are (for wrapping them as a unit).
fn knob_width(ui: &Ui, label: &str, spec: &ParamSpec) -> f32 {
    use mr_core::ParamKind;
    let text = ui.painter().layout_no_wrap(label.to_string(), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::WHITE).size().x;
    let widget = match spec.kind {
        ParamKind::Bool => 24.0,
        ParamKind::Int { .. } => 56.0,
        ParamKind::Float { .. } => ui.spacing().slider_width + 60.0,
        ParamKind::Choice(_) => ui.spacing().combo_width + 24.0,
    };
    text + widget + 40.0
}

/// Knobs as label + widget pairs, wrapped as units; `marks` labels restart-only knobs with ↻;
/// `after` may add a widget after each (the tuner's lock). Returns (name, value, commit).
pub fn knob_row(ui: &mut Ui, knobs: &[(&ParamSpec, ParamValue)], marks: bool, after: &mut dyn FnMut(&mut Ui, &ParamSpec)) -> Vec<(String, ParamValue, bool)> {
    let mut out = vec![];
    for (spec, value) in knobs {
        let label = if marks && !spec.live { format!("{} ↻", spec.name) } else { spec.name.clone() };
        if ui.available_size_before_wrap().x < knob_width(ui, &label, spec) {
            ui.end_row();
        }
        ui.add(egui::Label::new(egui::RichText::new(label).weak()).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(knob_tip(spec));
        if let Some((v, commit)) = knob_widget(ui, spec, value) {
            out.push((spec.name.clone(), v, commit));
        }
        after(ui, spec);
        ui.add_space(6.0);
    }
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
            ComboBox::from_id_salt(&spec.name)
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
