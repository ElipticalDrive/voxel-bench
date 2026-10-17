//! The compact renderer editor (IDEAS §12, PLAN step 9d), shared by the plan editor and
//! the live session: one row for the back end with its own knobs, then the pipeline as a
//! tree from its output. Each input port expands into the node bound to it, with the
//! node's component and that component's knobs right next to it; a node reached a second
//! way is shown as shared. Ray-casting sites off the lead (shadow and bounce rays, mesh
//! shadows, exposure tests) pick their binding: shared with another site, or a kernel (and
//! storage) of their own. Every choice is listed; those that cannot run here are greyed
//! out, and hovering them says why.

use std::collections::{BTreeMap, HashMap};

use egui::{ComboBox, Ui};
use mr_components::graph::{Data, Graph, Role, inputs};
use mr_components::{BACKENDS, Composition, Slot, components};
use mr_core::{ParamSpec, ParamValue};

use crate::plan::RendererSpec;

/// What the user changed in the grid.
pub enum StageEdit {
    Backend(String),
    Slot(Slot, String),
    /// The sites bound otherwise (the spec's whole `sites`, already checked to run).
    Sites(BTreeMap<String, String>),
    /// A knob; `commit` is false while a drag is still in progress.
    Knob { name: String, value: ParamValue, commit: bool },
}

/// Whether `name` is a slot-selection parameter (shown as the stage's component picker).
fn is_slot_param(name: &str) -> bool {
    Slot::ALL.iter().any(|s| s.name() == name)
}

/// Indentation per tree level.
const INDENT: f32 = 12.0;

/// Draws the tree for composed renderer `spec`. `knobs` are the back end's parameters with
/// their current values; `restart_marks` labels knobs that need a restart with ↻ (live
/// session); `after_knob` may add a widget after each knob (e.g. the tuner lock).
pub fn stage_grid(ui: &mut Ui, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], restart_marks: bool, after_knob: &mut dyn FnMut(&mut Ui, &ParamSpec)) -> Vec<StageEdit> {
    let Some(comp) = spec.compose.as_ref() else { return vec![] };
    let mut t = Tree { spec, comp, knobs, restart_marks, after_knob, edits: vec![], seen: HashMap::new(), shown: vec![], graph: None, values: BTreeMap::new(), lead: None };
    // Back end, with the knobs that belong to it rather than to a component.
    ui.horizontal_wrapped(|ui| {
        label(ui, 0, "back end", true, "Where the renderer runs; it decides which stages exist");
        ComboBox::from_id_salt("backend")
            .selected_text(&comp.backend)
            .show_ui(ui, |ui| {
                for b in BACKENDS {
                    let resp = ui.add_enabled(b.renderer.is_some(), egui::Button::selectable(comp.backend == b.name, b.name));
                    if b.renderer.is_some() {
                        if resp.on_hover_text(b.description).clicked() && comp.backend != b.name {
                            t.edits.push(StageEdit::Backend(b.name.into()));
                        }
                    } else {
                        resp.on_disabled_hover_text(format!("{}\n\nNot available yet", b.description));
                    }
                }
            })
            .response
            .on_hover_text(mr_components::backend(&comp.backend).map_or("", |b| b.description));
        t.knob_row(ui, &|s| s.owner.is_none());
    });
    match spec.pipeline().map(|g| g.check()) {
        Some(Ok(ck)) => {
            t.lead = ck.graph.lead_site();
            t.values = ck.graph.site_values().into_iter().collect();
            t.graph = Some(ck.graph);
            t.node(ui, &ck.output, "output", 0, None);
        }
        Some(Err(e)) => {
            ui.colored_label(ui.visuals().warn_fg_color, format!("⚠ {e}"));
        }
        None => {}
    }
    // Stages the tree does not reach (left out: no prepass, no meshes).
    let left: Vec<Slot> = comp.slots().filter(|s| !s.secondary() && !t.shown.contains(s)).collect();
    for slot in left {
        let current = comp.get(slot).to_string();
        ui.horizontal_wrapped(|ui| {
            label(ui, 0, slot.label(), false, "A stage this pipeline leaves out: pick a component to add it");
            t.slot_picker(ui, slot, &current);
            t.knob_row(ui, &|s| s.owned_by(slot.name(), &current));
        });
    }
    if let Some(note) = mr_components::caps::adapter_note(comp) {
        ui.weak(format!("ⓘ {note}"));
    }
    t.edits
}

/// A fixed-width row label, indented by tree depth (the pickers stay aligned per level).
/// Where node `id` runs: its device and API (set, inherited from its callers, or the back
/// end's for its stage; "with its callers" for a module compiled into callers on two APIs).
fn runs_on(g: &Graph, comp: &Composition, id: &str) -> Option<String> {
    let ck = g.check().ok()?;
    let n = &ck.graph.nodes[id];
    let b = mr_components::backend(&ck.graph.executor)?;
    let fused = mr_components::graph::output(n.role, &n.component).fused();
    let api = match ck.apis.get(id) {
        Some(a) => a.name().to_string(),
        None if fused => "with its callers".into(),
        None => match n.role.slot().map(|s| (b.api(s, comp), b.place(s))) {
            Some((Some(a), _)) => a.name().to_string(),
            Some((None, p)) => format!("{} (knob `{}`)", p.apis.iter().map(|a| a.name()).collect::<Vec<_>>().join("/"), b.api_choice.map_or("-", |ch| ch.knob)),
            None => return Some(ck.devices[id].to_string()),
        },
    };
    Some(format!("{} ({api})", ck.devices[id]))
}

fn label(ui: &mut Ui, depth: usize, text: &str, strong: bool, tip: &str) {
    if depth > 0 {
        ui.add_space(INDENT * depth as f32 - ui.spacing().item_spacing.x);
    }
    let t = if strong { egui::RichText::new(text).strong() } else { egui::RichText::new(text) };
    ui.add_sized([96.0, 18.0], egui::Label::new(t).truncate()).on_hover_text(tip);
}

/// The value of a site in the sites form, split: shared with another site, or a kernel of
/// its own over the lead's storage (`None`) or the storage named.
enum SiteValue {
    Shared(String),
    Own { storage: Option<String>, kernel: String },
}

impl SiteValue {
    fn parse(v: &str, sites: &[String]) -> Self {
        if sites.iter().any(|s| s == v) {
            return SiteValue::Shared(v.into());
        }
        match v.split_once('/') {
            Some((s, k)) => SiteValue::Own { storage: Some(s.into()), kernel: k.into() },
            None => SiteValue::Own { storage: None, kernel: v.into() },
        }
    }

    fn text(storage: &Option<String>, kernel: &str) -> String {
        storage.as_ref().map_or(kernel.to_string(), |s| format!("{s}/{kernel}"))
    }
}

struct Tree<'a> {
    spec: &'a RendererSpec,
    comp: &'a Composition,
    knobs: &'a [(ParamSpec, ParamValue)],
    restart_marks: bool,
    after_knob: &'a mut dyn FnMut(&mut Ui, &ParamSpec),
    edits: Vec<StageEdit>,
    /// Nodes drawn so far → where (for "shared" rows).
    seen: HashMap<String, String>,
    /// The composition's slots drawn as nodes.
    shown: Vec<Slot>,
    graph: Option<Graph>,
    /// Every site's binding (sites form, nothing left out).
    values: BTreeMap<String, String>,
    lead: Option<String>,
}

impl Tree<'_> {
    fn knob_row(&mut self, ui: &mut Ui, filter: &dyn Fn(&ParamSpec) -> bool) {
        for (spec, value) in self.knobs.iter().filter(|(s, _)| !is_slot_param(&s.name) && filter(s)) {
            let label = if self.restart_marks && !spec.live { format!("{} ↻", spec.name) } else { spec.name.clone() };
            // Keep label, value and extras on one line: wrap before them if they won't fit.
            if ui.available_size_before_wrap().x < knob_width(ui, &label, spec) {
                ui.end_row();
            }
            ui.add(egui::Label::new(egui::RichText::new(label).weak()).wrap_mode(egui::TextWrapMode::Extend)).on_hover_text(knob_tip(spec));
            if let Some((v, commit)) = knob_widget(ui, spec, value) {
                self.edits.push(StageEdit::Knob { name: spec.name.clone(), value: v, commit });
            }
            (self.after_knob)(ui, spec);
            ui.add_space(6.0);
        }
    }

    /// A stage's component picker: every component of the slot, greyed with the conflict.
    fn slot_picker(&mut self, ui: &mut Ui, slot: Slot, current: &str) {
        ComboBox::from_id_salt(slot.name())
            .selected_text(current)
            .show_ui(ui, |ui| {
                for c in components(slot) {
                    let why = mr_components::conflict(self.comp, slot, c.name);
                    let resp = ui.add_enabled(why.is_none(), egui::Button::selectable(current == c.name, c.name));
                    match why {
                        None => {
                            if resp.on_hover_text(c.description).clicked() && current != c.name {
                                self.edits.push(StageEdit::Slot(slot, c.name.into()));
                            }
                        }
                        Some(why) => drop(resp.on_disabled_hover_text(format!("{}\n\nConflicts: {why}", c.description))),
                    }
                }
            })
            .response
            .on_hover_text(components(slot).find(|c| c.name == current).map_or("", |c| c.description));
    }

    /// The sites with site `name` bound to `value` (others as the spec has them), if that runs.
    fn try_site(&self, name: &str, value: &str) -> Result<BTreeMap<String, String>, String> {
        if self.spec.graph.is_some() {
            return Err("this renderer is written as a graph (its own kernels have knobs): edit it in the plan".into());
        }
        let mut sites = self.spec.sites.clone();
        sites.insert(name.into(), value.into());
        let mut s = self.spec.clone();
        s.set_sites(sites.clone()).map_err(|e| format!("{e:#}"))?;
        Ok(sites)
    }

    /// A picker of site values: (shown text, value, hover); greyed with the reason when the
    /// value does not run.
    fn value_picker(&mut self, ui: &mut Ui, id: (&str, &str), name: &str, selected: &str, current: &str, choices: Vec<(String, String, String)>) {
        let mut pick = None;
        ComboBox::from_id_salt(id).selected_text(selected).show_ui(ui, |ui| {
            for (text, value, tip) in &choices {
                let on = *value == current;
                let r = if on { Ok(BTreeMap::new()) } else { self.try_site(name, value) };
                let resp = ui.add_enabled(r.is_ok(), egui::Button::selectable(on, text));
                match r {
                    Ok(sites) => {
                        if resp.on_hover_text(tip).clicked() && !on {
                            pick = Some(sites);
                        }
                    }
                    Err(why) => drop(resp.on_disabled_hover_text(format!("{tip}\n\nCannot run: {why}"))),
                }
            }
        });
        if let Some(sites) = pick {
            self.edits.push(StageEdit::Sites(sites));
        }
    }

    /// Node `id` bound to port `port`, at `depth`; `site` is the off-lead site it belongs to
    /// (its kernel and storage are picked as that site's value).
    fn node(&mut self, ui: &mut Ui, id: &str, port: &str, depth: usize, site: Option<&str>) {
        let g = self.graph.clone().unwrap();
        let n = &g.nodes[id];
        if n.role == Role::World {
            return;
        }
        let mut tip = format!("{port}: {} `{}`", n.role.name(), n.component);
        if let Some(on) = runs_on(&g, self.comp, id) {
            tip += &format!("\nRuns on {on}");
        }
        // A storage under a site's own kernel: shared with a site, or its own.
        if let (Some(s), Role::Storage | Role::Geometry) = (site, n.role) {
            self.site_storage(ui, s, port, depth, &tip);
            return;
        }
        if let Some(at) = self.seen.get(id) {
            ui.horizontal_wrapped(|ui| {
                label(ui, depth, port, false, &tip);
                ui.weak(format!("↺ shared: the {} of {at}", n.role.name()));
            });
            return;
        }
        self.seen.insert(id.into(), port.into());
        ui.horizontal_wrapped(|ui| {
            label(ui, depth, port, depth == 0, &tip);
            match (site, n.role.slot()) {
                (Some(s), Some(Slot::Traversal)) => {
                    let Some(SiteValue::Own { storage, kernel }) = self.values.get(s).map(|v| SiteValue::parse(v, &self.site_names())) else { return };
                    let choices = components(Slot::Traversal).map(|c| (c.name.to_string(), SiteValue::text(&storage, c.name), c.description.to_string())).collect();
                    let cur = SiteValue::text(&storage, &kernel);
                    self.value_picker(ui, (s, "kernel"), s, &kernel, &cur, choices);
                    ui.weak("(its own kernel)").on_hover_text("Knobs of a site's own kernel are set in the plan's graph form for now");
                }
                (_, Some(slot)) => {
                    let slot = if self.comp.slots().any(|x| x == slot) { slot } else { return };
                    self.shown.push(slot);
                    self.slot_picker(ui, slot, &n.component);
                    let c = n.component.clone();
                    self.knob_row(ui, &|s| s.owned_by(slot.name(), &c));
                }
                _ => {}
            }
        });
        for p in inputs(n.role, &n.component) {
            if p.data == Data::Changes {
                continue;
            }
            let name = mr_components::graph::site_name(n.role, p.name);
            let is_site = self.values.contains_key(&name) && self.lead.as_deref() != Some(&name);
            // An edge carrying the previous frame's value is shown, not descended; an
            // iteration (a loop within the frame) is noted and descended (shared nodes stop it).
            if let (Some(src), w) = (n.inputs.get(p.name), n.when(p.name)) {
                match w {
                    mr_components::graph::When::Prev => {
                        ui.horizontal_wrapped(|ui| {
                            label(ui, depth + 1, p.name, false, p.description);
                            ui.weak(format!("← {src} (previous frame)"));
                        });
                        continue;
                    }
                    mr_components::graph::When::Iterate(k) => {
                        ui.horizontal_wrapped(|ui| {
                            label(ui, depth + 1, "", false, p.description);
                            ui.weak(format!("↻ {} {k} passes a frame: first last frame's {}", p.name, p.data.name()));
                        });
                    }
                    mr_components::graph::When::Now => {}
                }
            }
            match n.inputs.get(p.name) {
                Some(src) if is_site => self.site(ui, &name, src, depth + 1),
                Some(src) => self.node(ui, src, p.name, depth + 1, site),
                None => {}
            }
        }
    }

    fn site_names(&self) -> Vec<String> {
        self.values.keys().cloned().chain(self.lead.clone()).collect()
    }

    /// An off-lead site: its binding, then (when its own) its kernel and storage.
    fn site(&mut self, ui: &mut Ui, name: &str, src: &str, depth: usize) {
        let g = self.graph.clone().unwrap();
        let names = self.site_names();
        let current = self.values[name].clone();
        let ports = g.site_ports();
        let data = ports.iter().find(|(n, ..)| n == name).map(|(_, _, p)| p.data);
        let tip = ports.iter().find(|(n, ..)| n == name).map_or(String::new(), |(_, c, p)| format!("{name}: {} (`{c}`)", p.description));
        let others: Vec<String> = ports.iter().filter(|(n, _, p)| n != name && Some(p.data) == data && (self.values.contains_key(n) || self.lead.as_ref() == Some(n))).map(|(n, ..)| n.clone()).collect();
        let mut choices: Vec<(String, String, String)> = others.iter().map(|o| (format!("shared: {o}"), o.clone(), format!("The same {} as the {o} site", if data == Some(Data::Storage) { "storage" } else { "kernel and storage" }))).collect();
        let parsed = SiteValue::parse(&current, &names);
        if data == Some(Data::Storage) {
            choices.extend(components(Slot::Storage).map(|c| (format!("own: {}", c.name), c.name.to_string(), c.description.to_string())));
            let selected = if matches!(parsed, SiteValue::Shared(_)) { format!("shared: {current}") } else { format!("own: {current}") };
            ui.horizontal_wrapped(|ui| {
                label(ui, depth, name, false, &tip);
                self.value_picker(ui, (name, "bind"), name, &selected, &current, choices);
            });
            return;
        }
        // Own: a kernel of its own (the one it has, else the lead's) over the lead's storage.
        let own = match &parsed {
            SiteValue::Own { storage, kernel } => SiteValue::text(storage, kernel),
            SiteValue::Shared(_) => g.nodes.get(src).map_or("refloor".into(), |k| k.component.clone()),
        };
        choices.push(("own kernel".into(), own.clone(), "A kernel of its own (then pick its kernel and storage below)".into()));
        let selected = match &parsed {
            SiteValue::Shared(o) => format!("shared: {o}"),
            SiteValue::Own { .. } => "own kernel".into(),
        };
        ui.horizontal_wrapped(|ui| {
            label(ui, depth, name, false, &tip);
            self.value_picker(ui, (name, "bind"), name, &selected, &current, choices);
        });
        if let SiteValue::Own { .. } = parsed {
            self.node(ui, src, "kernel", depth + 1, Some(name));
        }
    }

    /// The storage under site `name`'s own kernel.
    fn site_storage(&mut self, ui: &mut Ui, name: &str, port: &str, depth: usize, tip: &str) {
        let names = self.site_names();
        let Some(SiteValue::Own { storage, kernel }) = self.values.get(name).map(|v| SiteValue::parse(v, &names)) else { return };
        let lead = self.lead.clone().unwrap_or_default();
        let mut choices = vec![(format!("shared: {lead}"), kernel.clone(), format!("The storage the {lead} rays walk"))];
        for o in names.iter().filter(|o| **o != name && **o != lead && self.values.get(*o).is_some_and(|v| !names.contains(v))) {
            choices.push((format!("shared: {o}"), format!("{o}/{kernel}"), format!("The storage the {o} site reads")));
        }
        choices.extend(components(Slot::Storage).map(|c| (format!("own: {}", c.name), format!("{}/{kernel}", c.name), c.description.to_string())));
        let selected = match &storage {
            None => format!("shared: {lead}"),
            Some(s) if names.contains(s) => format!("shared: {s}"),
            Some(s) => format!("own: {s}"),
        };
        let cur = SiteValue::text(&storage, &kernel);
        ui.horizontal_wrapped(|ui| {
            label(ui, depth, port, false, tip);
            self.value_picker(ui, (name, "storage"), name, &selected, &cur, choices);
        });
    }
}

/// Roughly how wide a knob's label, widget and extras are (for wrapping them as a unit).
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
