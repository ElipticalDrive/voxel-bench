//! The compact renderer editor (IDEAS §12), shared by the plan editor and the live session:
//! one row for the back end with its own knobs, then one row per stage (slot) with the
//! chosen component and that component's knobs right next to it. Every component of a
//! slot is listed; the ones that cannot run here are greyed out, and hovering them says
//! why (what they conflict with).

use egui::{ComboBox, Ui};
use mr_components::{BACKENDS, Composition, Slot, components};
use mr_core::{ParamSpec, ParamValue};

/// What the user changed in the grid.
pub enum StageEdit {
    Backend(String),
    Slot(Slot, String),
    /// A knob; `commit` is false while a drag is still in progress.
    Knob { name: String, value: ParamValue, commit: bool },
}

/// Whether `name` is a slot-selection parameter (shown as the stage's component picker).
fn is_slot_param(name: &str) -> bool {
    Slot::ALL.iter().any(|s| s.name() == name)
}

/// Draws the grid for composition `comp`. `knobs` are the back end's parameters with their
/// current values; `restart_marks` labels knobs that need a restart with ↻ (live session);
/// `after_knob` may add a widget after each knob (e.g. the tuner lock).
pub fn stage_grid(ui: &mut Ui, comp: &Composition, knobs: &[(ParamSpec, ParamValue)], restart_marks: bool, after_knob: &mut dyn FnMut(&mut Ui, &ParamSpec)) -> Vec<StageEdit> {
    let mut edits = vec![];
    let mut knob_row = |ui: &mut Ui, edits: &mut Vec<StageEdit>, filter: &dyn Fn(&ParamSpec) -> bool| {
        for (spec, value) in knobs.iter().filter(|(s, _)| !is_slot_param(&s.name) && filter(s)) {
            let label = if restart_marks && !spec.live { format!("{} ↻", spec.name) } else { spec.name.clone() };
            // Keep label, value and extras on one line: wrap before them if they won't fit.
            if ui.available_size_before_wrap().x < knob_width(ui, &label, spec) {
                ui.end_row();
            }
            ui.add(egui::Label::new(egui::RichText::new(label).weak()).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(&spec.description);
            if let Some((v, commit)) = knob_widget(ui, spec, value) {
                edits.push(StageEdit::Knob { name: spec.name.clone(), value: v, commit });
            }
            after_knob(ui, spec);
            ui.add_space(6.0);
        }
    };
    // One wrapping row per stage (a grid would not wrap and would widen the panel); the
    // fixed-width stage label keeps the pickers aligned.
    let label = |ui: &mut Ui, text: &str, strong: bool, tip: &str| {
        let t = if strong { egui::RichText::new(text).strong() } else { egui::RichText::new(text) };
        ui.add_sized([96.0, 18.0], egui::Label::new(t).truncate()).on_hover_text(tip);
    };
    // Back end, with the knobs that belong to it rather than to a component.
    ui.horizontal_wrapped(|ui| {
        label(ui, "back end", true, "Where the renderer runs; it decides which stages exist");
        ComboBox::from_id_salt("backend")
            .selected_text(&comp.backend)
            .show_ui(ui, |ui| {
                for b in BACKENDS {
                    let resp = ui.add_enabled(b.renderer.is_some(), egui::Button::selectable(comp.backend == b.name, b.name));
                    if b.renderer.is_some() {
                        if resp.on_hover_text(b.description).clicked() && comp.backend != b.name {
                            edits.push(StageEdit::Backend(b.name.into()));
                        }
                    } else {
                        resp.on_disabled_hover_text(format!("{}\n\nNot available yet", b.description));
                    }
                }
            })
            .response
            .on_hover_text(mr_components::backend(&comp.backend).map_or("", |b| b.description));
        knob_row(ui, &mut edits, &|s| s.owner.is_none());
    });
    // One row per stage: the component, then its knobs.
    for slot in comp.slots() {
        let current = comp.get(slot).to_string();
        ui.horizontal_wrapped(|ui| {
            label(ui, slot.label(), false, "A stage of the renderer: pick the component for it");
            ComboBox::from_id_salt(slot.name())
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    for c in components(slot) {
                        let why = mr_components::conflict(comp, slot, c.name);
                        let resp = ui.add_enabled(why.is_none(), egui::Button::selectable(current == c.name, c.name));
                        match why {
                            None => {
                                if resp.on_hover_text(c.description).clicked() && current != c.name {
                                    edits.push(StageEdit::Slot(slot, c.name.into()));
                                }
                            }
                            Some(why) => {
                                resp.on_disabled_hover_text(format!("{}\n\nConflicts: {why}", c.description));
                            }
                        }
                    }
                })
                .response
                .on_hover_text(components(slot).find(|c| c.name == current).map_or("", |c| c.description));
            knob_row(ui, &mut edits, &|s| s.owned_by(slot.name(), &current));
        });
    }
    if let Some(note) = mr_components::caps::adapter_note(comp) {
        ui.weak(format!("ⓘ {note}"));
    }
    edits
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

/// After a back-end switch: keeps only the options the new back end knows (`params`).
pub fn prune_options(options: &mut toml::Table, params: &[ParamSpec]) {
    options.retain(|k, _| params.iter().any(|p| &p.name == k));
}
