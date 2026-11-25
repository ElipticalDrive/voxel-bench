//! The graph-first pipeline editor. A renderer is its graph: the side
//! panels show it compactly (`summary`: a row per node, its component and knobs), the
//! editor window draws it (`canvas`) with an inspector for the selected node. Every choice
//! is listed; one whose graph cannot run is greyed and says why (`model::Trial`), or, on a
//! graph a node executor runs, can still be made: a draft, shown with why it does not run
//! while the renderer keeps its last graph. A renderer is started from a graph file
//! (`start_files`).

mod canvas;
pub mod model;

use std::collections::HashMap;

use egui::{ComboBox, RichText, Ui};
use mr_components::graph::{Data, Device, Graph, Role, When, inputs, output, role_components, state};
use mr_components::{Api, Kind};
use mr_core::{ParamSpec, ParamValue, RendererInfo};

use super::knobs;
use crate::plan::{RendererSpec, RunSpec};
use model::{Change, Target, Trial};

/// What the user changed (at most one a frame).
#[allow(clippy::large_enum_variant)]
pub enum Edit {
    /// The renderer with another graph (already loaded: it runs).
    Spec(RendererSpec),
    /// One of the executor's knobs; `commit` is false while a drag is still in progress.
    Knob { name: String, value: ParamValue, commit: bool },
    /// Open the editor window, on this node if given.
    Open(Option<String>),
}

/// One `Trial` per renderer shown, rebuilt when its spec changes.
#[derive(Default)]
pub struct Trials(HashMap<String, (String, Option<Trial>)>);

impl Trials {
    pub fn get(&mut self, key: &str, spec: &RendererSpec) -> Option<&mut Trial> {
        let print = format!("{}\n{}", spec.name, toml::to_string(spec).unwrap_or_default());
        let e = self.0.entry(key.into()).or_insert_with(|| (String::new(), None));
        if e.0 != print {
            *e = (print, model::graph_of(spec).map(|g| Trial::new(spec.clone(), g)));
        }
        e.1.as_mut()
    }

    /// The trial shown for `key`, if one was made (screenshots make drafts with it).
    #[cfg(test)]
    pub fn shown(&mut self, key: &str) -> Option<&mut Trial> {
        self.0.get_mut(key).and_then(|e| e.1.as_mut())
    }
}

/// Shows a live lane's faults (`model::faults`) on the nodes of the renderer shown under `key`.
pub fn set_faults(trials: &mut Trials, key: &str, spec: &RendererSpec, faults: std::collections::BTreeMap<String, Vec<String>>) {
    if let Some(t) = trials.get(key, spec) {
        t.faults = faults;
    }
}

/// The colour of a node a live lane's failure or refusal is about.
pub const FAULT: egui::Color32 = egui::Color32::from_rgb(235, 90, 75);

/// The colour of a choice that gives a draft (a graph that does not run yet).
const DRAFT: egui::Color32 = egui::Color32::from_rgb(230, 160, 60);

/// A list of choices, each a change of the graph: greyed with the reason where the result
/// cannot run (on a graph edited freely: marked, and picking it makes a draft). Returns the
/// renderer picked.
fn choices(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, selected: &str, tip: &str, options: Vec<(String, Change, bool, String)>, trial: &mut Trial, infos: &[RendererInfo]) -> Option<RendererSpec> {
    let mut pick = None;
    ComboBox::from_id_salt(id)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (text, change, on, hover) in options {
                if on {
                    ui.add(egui::Button::selectable(true, &text)).on_hover_text(hover);
                    continue;
                }
                match trial.try_change(&change, infos) {
                    Ok((_, spec)) => {
                        if ui.add(egui::Button::selectable(false, text)).on_hover_text(format!("{hover}\n\nGives: {}", spec.label())).clicked() {
                            trial.drop_draft();
                            pick = Some(spec);
                        }
                    }
                    Err(why) => {
                        if ui.add(egui::Button::selectable(false, RichText::new(format!("{text}  ✱")).color(DRAFT))).on_hover_text(format!("{hover}\n\nDoes not run: {why}\nPick it anyway to keep editing (a draft; the renderer keeps its last graph)")).clicked() {
                            trial.make_draft(&change, infos);
                        }
                    }
                }
            }
        })
        .response
        .on_hover_text(tip);
    pick
}

fn description(role: Role, component: &str) -> String {
    match mr_components::component(role, component) {
        Some(c) => c.description.to_string(),
        None => mr_components::graph::host_description(role, component).unwrap_or_default().to_string(),
    }
}

/// The component choices for node `id`.
pub(crate) fn component_options(g: &Graph, id: &str) -> Vec<(String, Change, bool, String)> {
    let n = &g.nodes[id];
    role_components(n.role).into_iter().map(|c| (c.to_string(), Change::Component(id.into(), c.into()), n.component == c, description(n.role, c))).collect()
}

/// The device choices for node `id`: where the executor runs it, or a device of the run's.
fn device_options(g: &Graph, id: &str, run: &RunSpec, resolved: Option<Device>) -> Vec<(String, Change, bool, String)> {
    let n = &g.nodes[id];
    let gpus = run.devices.gpus.len().max(1) as u8;
    let mut v = vec![(format!("default ({})", resolved.map_or("-".into(), |d| d.to_string())), Change::Device(id.into(), None), n.device.is_none(), "Where its executor runs this role".into())];
    let mut devs = vec![Device::Cpu];
    devs.extend((0..gpus).map(Device::Gpu));
    if n.role == Role::Generator {
        devs.insert(0, Device::Server);
    }
    for d in devs {
        let hover = match d {
            Device::Cpu => "The client's CPU".to_string(),
            Device::Server => "The world's machine (reached over the link)".to_string(),
            Device::Gpu(i) => format!("GPU slot {i} of run.devices (`{}`)", run.devices.gpu(i as usize).unwrap_or("the default adapter")),
        };
        v.push((d.to_string(), Change::Device(id.into(), Some(d)), n.device == Some(d), hover));
    }
    v
}

/// The API choices for node `id` on device kind `kind`.
fn api_options(g: &Graph, id: &str, kind: Kind) -> Vec<(String, Change, bool, String)> {
    let n = &g.nodes[id];
    let mut v = vec![("default".to_string(), Change::Api(id.into(), None), n.api.is_none(), "As its executor chooses".to_string())];
    if let Some(c) = mr_components::component(n.role, &n.component) {
        for i in c.impls.iter().filter(|i| i.kind == kind) {
            for a in i.apis {
                v.push((a.name().to_string(), Change::Api(id.into(), Some(*a)), n.api == Some(*a), format!("Run it with {}", a.name())));
            }
        }
    }
    v
}

/// The binding choices for input `port` of node `id`.
fn bind_options(g: &Graph, id: &str, port: &mr_components::graph::Port) -> Vec<(String, Change, bool, String)> {
    let n = &g.nodes[id];
    let cur = n.inputs.get(port.name);
    let mut v = vec![];
    if !port.required {
        v.push(("(unbound)".into(), Change::Bind(id.into(), port.name.into(), Target::Unbound), cur.is_none(), "Leave this optional input unbound".into()));
    }
    for k in model::producers_in(g, port.data, id) {
        let m = &g.nodes[&k];
        let shared = cur != Some(&k) && !model::readers(g, &k).is_empty();
        v.push((format!("{k} ({}){}", m.component, if shared { " · shared" } else { "" }), Change::Bind(id.into(), port.name.into(), Target::Node(k.clone())), cur == Some(&k), format!("The existing {} node `{k}`{}", m.role.name(), if shared { ": shared with what reads it already" } else { "" })));
    }
    for (role, c) in model::producers_new(port.data) {
        v.push((format!("new {} {c}", role.name()), Change::Bind(id.into(), port.name.into(), Target::New(role, c.into())), false, format!("A new {} node: {}", role.name(), description(role, c))));
    }
    v
}

fn timing_options(n: &mr_components::graph::Node, id: &str, port: &str) -> Vec<(String, Change, bool, String)> {
    let w = n.when(port);
    [(When::Now, "this frame's value"), (When::Prev, "the previous frame's value (closes a cycle; across devices: overlaps the two)"), (When::Iterate(2), "two passes a frame, the first on last frame's value")]
        .into_iter()
        .map(|(t, h)| (t.label(), Change::Timing(id.into(), port.into(), t), w == t, h.to_string()))
        .collect()
}

/// A live lane's host knobs that are its client node's (the loader's reach): shown there,
/// and in place of the client's own graph knobs of the same meaning, which a lane applies
/// without a restart through these.
const CLIENT_HOST: [(&str, &str); 2] = [("host.view_radius", "radius"), ("host.max_lod", "max_lod")];

/// The executor's knobs that belong to node `id` (or, for `None`, to no node), with values.
fn node_knobs<'a>(g: &Graph, id: Option<&str>, knobs: &'a [(ParamSpec, ParamValue)]) -> Vec<(&'a ParamSpec, ParamValue)> {
    let client = |s: &ParamSpec| CLIENT_HOST.iter().any(|(h, _)| *h == s.name) && g.nodes.values().any(|n| n.role == Role::Client);
    knobs
        .iter()
        .filter(|(s, _)| crate::plan::component_knob(&s.name).is_none())
        .filter(|(s, _)| match (&s.owner, id) {
            (None, None) => !client(s),
            (None, Some(id)) => client(s) && g.nodes[id].role == Role::Client,
            (Some((slot, comps)), Some(id)) => comps.iter().any(|c| model::knob_owner(g, slot, c).as_deref() == Some(id)),
            _ => false,
        })
        .map(|(s, v)| (s, v.clone()))
        .collect()
}

/// Node `id`'s knobs as one grid: the executor's knobs it owns, then (client and link) its
/// graph knobs, which run as a hot swap. `lock` adds the tuner's column.
fn node_grid<'l>(ui: &mut Ui, salt: &str, g: &Graph, id: Option<&str>, knobs: &[(ParamSpec, ParamValue)], marks: bool, lock: Option<&mut (dyn FnMut(&mut Ui, &ParamSpec) + 'l)>, run: &RunSpec, trial: &mut Trial, infos: &[RendererInfo]) -> Vec<Edit> {
    let mut vals = node_knobs(g, id, knobs);
    let own = vals.len();
    let world = id.map(|id| &g.nodes[id]).map_or(vec![], |n| {
        let shadowed = |s: &ParamSpec| CLIENT_HOST.iter().any(|(h, k)| *k == s.name && knobs.iter().any(|(x, _)| x.name == *h));
        model::world_knobs(n.role, &n.component, run, crate::pipelines::graph_info(g, &run.devices).map(|i| i.features).as_ref()).into_iter().filter(|s| n.role != Role::Client || !shadowed(s)).collect()
    });
    if let Some(n) = id.map(|id| &g.nodes[id]) {
        vals.extend(world.iter().map(|s| (s, n.knobs.get(&s.name).and_then(|v| serde_json::to_value(v).ok()).and_then(|j| s.from_json(&j)).unwrap_or_else(|| s.default.clone()))));
    }
    let mut edits = vec![];
    // A choice the graph leaves out (a view its traversal lacks) is greyed with the reason.
    let checked = g.check().ok();
    let left_out = |s: &ParamSpec, v: &str| checked.as_ref().and_then(|c| c.value_left_out(s, v).into_iter().next());
    for (i, value, commit) in knobs::knob_grid(ui, (salt, id), &vals, marks, lock, &left_out) {
        let name = vals[i].0.name.clone();
        if i < own {
            edits.push(Edit::Knob { name, value, commit });
        } else if commit {
            let change = Change::Knob(id.unwrap().into(), name.clone(), Some(super::param_to_toml(&value)));
            match trial.try_change(&change, infos) {
                Ok((_, s)) => {
                    trial.drop_draft();
                    edits.push(Edit::Spec(s));
                }
                Err(_) => trial.make_draft(&change, infos),
            }
        }
    }
    edits
}

/// Where each node runs, as the checked graph resolves it.
fn devices(g: &Graph) -> HashMap<String, Device> {
    g.check().map(|c| c.devices).unwrap_or_default()
}

pub use mr_components::graph::start_files;

/// The compact form for a side panel: its executor, then per node its component
/// and its knobs. `marks` labels restart-only knobs; `lock` adds the tuner's lock column.
pub fn summary<'l>(ui: &mut Ui, key: &str, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], marks: bool, mut lock: Option<&mut (dyn FnMut(&mut Ui, &ParamSpec) + 'l)>, trials: &mut Trials, infos: &[RendererInfo], run: &RunSpec) -> Vec<Edit> {
    let mut edits = vec![];
    let Some(trial) = trials.get(key, spec) else {
        // A standalone renderer: its knobs only.
        let vals: Vec<(&ParamSpec, ParamValue)> = knobs.iter().map(|(s, v)| (s, v.clone())).collect();
        for (i, value, commit) in knobs::knob_grid(ui, (key, "knobs"), &vals, marks, lock, &knobs::takes_all) {
            edits.push(Edit::Knob { name: vals[i].0.name.clone(), value, commit });
        }
        return edits;
    };
    let g = trial.graph().clone();
    let devs = devices(&g);
    ui.horizontal_wrapped(|ui| {
        if ui.button("✏ graph").on_hover_text("Open the pipeline editor: the graph drawn, every node's component, device, API, inputs and knobs").clicked() {
            edits.push(Edit::Open(None));
        }
        ui.label(RichText::new(format!("{} executor", g.runs_on())).strong()).on_hover_text("The executor that builds this graph: where its image is made");
        if let Some(why) = trial.draft() {
            ui.label(RichText::new("✱ draft").color(DRAFT)).on_hover_text(format!("The graph shown does not run: {why}\nThe renderer keeps running its last graph"));
        }
    });
    edits.extend(node_grid(ui, key, &g, None, knobs, marks, lock.as_deref_mut(), run, trial, infos));
    for id in model::order(&g) {
        let n = &g.nodes[&id];
        // The world's side when it is not the default, and the post.
        let plain = match n.role {
            Role::World => true,
            Role::Generator => n.device.is_none_or(|d| d == Device::Server),
            Role::Link => n.component == "free",
            _ => false,
        };
        if plain {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            let dev = devs.get(&id).map_or(String::new(), |d| d.to_string());
            let fused = output(n.role, &n.component).fused();
            let tip = format!("node `{id}` ({}); {}\nRead by: {}", n.role.name(), if fused { "compiled into its callers".to_string() } else { format!("runs on {dev}") }, model::readers(&g, &id).iter().map(|(r, p)| format!("{r}.{p}")).collect::<Vec<_>>().join(", "));
            let fault = trial.faults.get(&id).map(|v| v.join("\n"));
            let label = match &fault {
                Some(_) => RichText::new(format!("⚠ {id}")).small().color(FAULT),
                None => RichText::new(&id).small(),
            };
            let tip = match &fault {
                Some(f) => format!("{tip}\n\nThe lane says this node is at fault:\n{f}"),
                None => tip,
            };
            if ui.add_sized([86.0, 18.0], egui::Button::new(label).frame(false)).on_hover_text(format!("{tip}\n\nClick: open it in the editor")).clicked() {
                edits.push(Edit::Open(Some(id.clone())));
            }
            if let Some(s) = choices(ui, (key, &id, "comp"), &n.component, &description(n.role, &n.component), component_options(&g, &id), trial, infos) {
                edits.push(Edit::Spec(s));
            }
            if !fused && !matches!(n.role, Role::Client | Role::Link) {
                ui.weak(dev);
            }
        });
        edits.extend(node_grid(ui, key, &g, Some(&id), knobs, marks, lock.as_deref_mut(), run, trial, infos));
    }
    edits
}

/// Which renderer the editor window edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Of {
    Plan(usize),
    Lane(usize),
}

/// The pipeline editor's state (its place, a dock, a window or the middle, is the dock layer's).
pub struct Window {
    pub of: Of,
    pub selected: Option<String>,
    port: Option<String>,
    canvas: canvas::View,
    undo: Vec<RendererSpec>,
    redo: Vec<RendererSpec>,
    /// Fit the whole graph again whenever a dragged node is released.
    auto_frame: bool,
}

/// Whether to fit the graph on release outlives the editor (kept in egui's memory).
fn auto_frame_id() -> egui::Id {
    egui::Id::new("pipeline-editor-auto-frame")
}

impl Window {
    pub fn new(ctx: &egui::Context, of: Of, selected: Option<String>) -> Self {
        let auto_frame = ctx.data(|d| d.get_temp::<bool>(auto_frame_id())).unwrap_or(false);
        Self { of, selected, port: None, canvas: Default::default(), undo: vec![], redo: vec![], auto_frame }
    }
}

/// The editor for `spec`, titled `title`. Returns the edits and whether it stays open.
pub fn pane(ui: &mut Ui, win: &mut Window, title: &str, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], marks: bool, trials: &mut Trials, infos: &[RendererInfo], run: &RunSpec) -> (Vec<Edit>, bool) {
    let mut open = true;
    let key = format!("{:?}", win.of);
    let edits = match trials.get(&key, spec) {
        Some(trial) => editor(ui, win, title, spec, knobs, marks, trial, infos, run, &mut open),
        None => {
            ui.horizontal(|ui| {
                ui.strong(title);
                open = !ui.button("✖ close").clicked();
            });
            ui.label("A standalone renderer: it has no graph.");
            vec![]
        }
    };
    ui.ctx().data_mut(|d| d.insert_temp(auto_frame_id(), win.auto_frame));
    (edits, open)
}

fn editor(ui: &mut Ui, win: &mut Window, title: &str, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], marks: bool, trial: &mut Trial, infos: &[RendererInfo], run: &RunSpec, open: &mut bool) -> Vec<Edit> {
    let mut edits = vec![];
    let g = trial.graph().clone();
    if win.selected.as_ref().is_some_and(|s| !g.nodes.contains_key(s)) {
        win.selected = None;
    }
    ui.horizontal_wrapped(|ui| {
        ui.strong(title);
        ui.separator();
        ui.label(RichText::new(format!("{} executor", g.runs_on())).strong()).on_hover_text("The executor that builds this graph: where its image is made");
        ui.weak(spec.label());
        if let Ok(k) = g.key() {
            ui.weak(format!("key {}", &k[..8.min(k.len())])).on_hover_text(format!("The graph's cache key (compiled pipelines are kept per key): {k}"));
        }
        ui.separator();
        if ui.add_enabled(!win.undo.is_empty(), egui::Button::new("⟲")).on_hover_text("Undo").clicked() {
            let prev = win.undo.pop().unwrap();
            win.redo.push(spec.clone());
            // Not recorded again as a change.
            return edits.push(Edit::Spec(prev));
        }
        if ui.add_enabled(!win.redo.is_empty(), egui::Button::new("⟳")).on_hover_text("Redo").clicked() {
            let next = win.redo.pop().unwrap();
            win.undo.push(spec.clone());
            return edits.push(Edit::Spec(next));
        }
        ui.menu_button("start from…", |ui| {
            ui.weak("A graph file's pipeline (then change anything)");
            for (file, about) in start_files() {
                if ui.button(file).on_hover_text(about).clicked() {
                    match RendererSpec::from_file(file) {
                        Ok(mut s) => {
                            s.label = spec.label.clone();
                            s.devices = spec.devices.clone();
                            s.compile = spec.compile;
                            win.undo.push(spec.clone());
                            edits.push(Edit::Spec(s));
                        }
                        Err(e) => tracing::warn!("graph file `{file}`: {e:#}"),
                    }
                    ui.close();
                }
            }
        });
        if ui.button("fit").on_hover_text("Lay the graph out again and fit it to the window (dragged nodes go back); a double click on the background fits without moving nodes").clicked() {
            win.canvas = Default::default();
        }
        ui.toggle_value(&mut win.auto_frame, "auto-frame").on_hover_text("Fit the whole graph to the window again whenever you release a dragged node (off: the view stays where you put it; the wheel zooms about the pointer)");
        ui.separator();
        if ui.button("✖ close").on_hover_text("Close the editor (where it was shown is remembered)").clicked() {
            *open = false;
        }
    });
    if let Some(why) = trial.draft().map(str::to_string) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("✱ Draft: does not run").color(DRAFT).strong());
            ui.label(RichText::new(why).color(DRAFT));
            ui.weak("(the renderer keeps running its last graph)");
            if ui.button("drop draft").on_hover_text("Back to the graph running").clicked() {
                trial.drop_draft();
            }
        });
    }
    ui.separator();
    let mut made = vec![];
    egui::Panel::right("pipeline-inspector").resizable(true).default_size(310.0).show(ui, |ui| {
        super::steady_scroll(ui, "inspector", |ui| {
            made.extend(inspector(ui, win, &g, knobs, marks, trial, infos, run));
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let r = canvas::show(ui, &mut win.canvas, win.auto_frame, &g, &devices(&g), win.selected.as_deref(), trial, infos);
        if let Some(sel) = r.select {
            win.selected = sel;
            win.port = r.port;
        }
        if let Some(s) = r.spec {
            made.push(Edit::Spec(s));
        }
    });
    // Undo keeps the renderer before each change made here.
    for e in &made {
        if let Edit::Spec(_) = e {
            win.undo.push(spec.clone());
            win.redo.clear();
        }
    }
    edits.extend(made);
    edits
}

fn inspector(ui: &mut Ui, win: &mut Window, g: &Graph, knobs: &[(ParamSpec, ParamValue)], marks: bool, trial: &mut Trial, infos: &[RendererInfo], run: &RunSpec) -> Vec<Edit> {
    let mut edits = vec![];
    let Some(id) = win.selected.clone() else {
        ui.heading("The renderer");
        ui.label("Click a node to change its component, device, API, inputs and knobs. Drag from a node's output (right) onto an input (left) to rebind it: green where that runs, red where not (hover for why).");
        ui.add_space(6.0);
        ui.label(RichText::new("Executor knobs").strong()).on_hover_text("Knobs of the executor itself, not of a node");
        edits.extend(node_grid(ui, "inspector", g, None, knobs, marks, None, run, trial, infos));
        ui.add_space(6.0);
        ui.label(RichText::new("Nodes").strong());
        for k in model::order(g) {
            let n = &g.nodes[&k];
            if ui.selectable_label(false, format!("{k}: {} {}", n.role.name(), n.component)).clicked() {
                win.selected = Some(k);
            }
        }
        return edits;
    };
    let n = g.nodes[&id].clone();
    let devs = devices(g);
    let fused = output(n.role, &n.component).fused();
    ui.horizontal(|ui| {
        ui.heading(&id);
        if id != n.role.name() {
            ui.weak(n.role.name());
        }
        if ui.small_button("✖").on_hover_text("Deselect").clicked() {
            win.selected = None;
        }
        if !model::world_side(n.role) && n.role != Role::Post {
            let change = Change::Remove(id.clone());
            let (b, tip) = match trial.try_change(&change, infos) {
                Ok((_, s)) => (Some(s), "Remove this node; the inputs reading it are unbound".to_string()),
                Err(why) => (None, format!("Remove this node; the inputs reading it are unbound.\nDoes not run then: {why}\nRemove it anyway (a draft; the renderer keeps its last graph)")),
            };
            let text = if b.is_none() { RichText::new("🗑 remove ✱").color(DRAFT) } else { RichText::new("🗑 remove") };
            if ui.add(egui::Button::new(text).small()).on_hover_text(&tip).clicked() {
                match b {
                    Some(s) => {
                        trial.drop_draft();
                        edits.push(Edit::Spec(s));
                    }
                    None => trial.make_draft(&change, infos),
                }
                win.selected = None;
            }
        }
    });
    let mut pick = |s: Option<RendererSpec>| {
        if let Some(s) = s {
            edits.push(Edit::Spec(s));
        }
    };
    egui::Grid::new("node").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        if n.role != Role::World {
            ui.label("component");
            pick(choices(ui, (&id, "c"), &n.component, &description(n.role, &n.component), component_options(g, &id), trial, infos));
            ui.end_row();
        }
        ui.label("device");
        if fused {
            ui.weak(format!("with its callers ({})", devs.get(&id).map_or("-".into(), |d| d.to_string()))).on_hover_text("Storages and kernels are compiled into what calls them and run there; give each device its own instance");
        } else if matches!(n.role, Role::World | Role::Link | Role::Client) {
            ui.weak(devs.get(&id).map_or("-".into(), |d| d.to_string()));
        } else {
            let cur = n.device.map_or_else(|| format!("default ({})", devs.get(&id).map_or("-".into(), |d| d.to_string())), |d| d.to_string());
            pick(choices(ui, (&id, "d"), &cur, "Where it runs", device_options(g, &id, run, devs.get(&id).copied()), trial, infos));
        }
        ui.end_row();
        if !fused && n.role.registered() {
            let kind = devs.get(&id).map_or(Kind::Gpu, |d| d.kind());
            ui.label("API");
            let cur = n.api.map_or("default".to_string(), |a: Api| a.name().to_string());
            pick(choices(ui, (&id, "a"), &cur, "How it runs there", api_options(g, &id, kind), trial, infos));
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.weak(description(n.role, &n.component));
    if let Some(s) = state(n.role, &n.component) {
        ui.weak(format!("Keeps: {s}"));
    }
    let ports = inputs(n.role, &n.component);
    if !ports.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Inputs").strong());
        egui::Grid::new("ports").num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
            for p in ports {
                let hl = win.port.as_deref() == Some(p.name);
                let t = RichText::new(p.name);
                ui.label(if hl { t.strong().underline() } else { t }).on_hover_text(format!("{} ({}{})", p.description, p.data.name(), if p.required { "" } else { ", optional" }));
                if p.data == Data::Changes {
                    ui.weak(n.inputs.get(p.name).map_or("the client", |s| s.as_str())).on_hover_text("The world's change log reaches a renderer only through its client");
                    ui.end_row();
                    continue;
                }
                let cur = n.inputs.get(p.name).map_or("(unbound)".to_string(), |s| format!("{s} ({})", g.nodes.get(s).map_or("?", |m| m.component.as_str())));
                pick(choices(ui, (&id, p.name, "b"), &cur, p.description, bind_options(g, &id, p), trial, infos));
                if n.inputs.contains_key(p.name) && !p.data.fused() {
                    pick(choices(ui, (&id, p.name, "t"), &n.when(p.name).label(), "When its value arrives", timing_options(&n, &id, p.name), trial, infos));
                }
                ui.end_row();
            }
        });
    }
    let readers = model::readers(g, &id);
    if !readers.is_empty() {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Read by").strong());
            for (r, p) in readers {
                if ui.link(format!("{r}.{p}")).clicked() {
                    win.selected = Some(r);
                    win.port = Some(p);
                }
            }
        });
    }
    let own = node_knobs(g, Some(&id), knobs);
    let world = matches!(n.role, Role::Client | Role::Link) && !model::world_knobs(n.role, &n.component, run, None).is_empty();
    if !own.is_empty() || world {
        ui.add_space(6.0);
        ui.label(RichText::new("Knobs").strong());
        edits.extend(node_grid(ui, "inspector", g, Some(&id), knobs, marks, None, run, trial, infos));
    } else if n.role.registered() && !g.nodes.iter().any(|(k, m)| *k != id && m.role == n.role && m.component == n.component) {
        // Nothing to show.
    } else if n.role.registered() {
        ui.weak("A second instance's knobs are set in the plan's graph form (TOML tab)");
    }
    edits
}

/// Applies edits to a plan's renderer. Returns whether it changed and the editor to open.
pub fn apply_to_plan(r: &mut RendererSpec, edits: Vec<Edit>) -> (bool, Option<Option<String>>) {
    let mut changed = false;
    let mut open = None;
    for e in edits {
        match e {
            Edit::Spec(s) => {
                *r = s;
                changed = true;
            }
            Edit::Knob { name, value, .. } => {
                r.options.insert(name.clone(), super::param_to_toml(&value));
                // One place per knob: the option, not also a node's.
                if let Some(g) = &mut r.graph {
                    g.nodes.values_mut().for_each(|n| drop(n.knobs.remove(&name)));
                }
                if let Err(e) = r.resolve() {
                    tracing::warn!("{name}: {e:#}");
                }
                changed = true;
            }
            Edit::Open(sel) => open = Some(sel),
        }
    }
    (changed, open)
}

/// Whether a lane running `old` goes on as `new` without a restart, and the knobs it is then
/// sent: the graph's edits are ones its executor takes while running
/// (`Graph::live_edits`: the session sends those), and every knob to set is live.
pub fn live_edits(old: &RendererSpec, new: &RendererSpec, params: &[(ParamSpec, ParamValue)]) -> Option<Vec<(String, ParamValue)>> {
    let (Ok(Some(g0)), Ok(Some(g1))) = (old.worker_graph(), new.worker_graph()) else { return None };
    g0.live_edits(&g1).ok()?;
    let specs: Vec<ParamSpec> = params.iter().map(|(p, _)| p.clone()).collect();
    let knobs = node_knob_changes(&g0, &g1, &specs);
    knobs.iter().all(|(k, _)| params.iter().any(|(p, _)| p.name == *k && p.live)).then_some(knobs)
}

/// The knob changes that take a renderer running graph `old` to `new`, node by node (by
/// id): of a node whose component changed or that is new, every knob its component owns (the
/// node's value, else the default: nothing of the old component's state carries over); of a
/// node that kept its component, the knobs that changed (one removed back to its default).
pub fn node_knob_changes(old: &Graph, new: &Graph, params: &[ParamSpec]) -> Vec<(String, ParamValue)> {
    let value = |p: &ParamSpec, n: &mr_components::graph::Node| knobs::option_value(p, &n.knobs);
    let owns = |p: &ParamSpec, n: &mr_components::graph::Node| p.owner.as_ref().is_some_and(|(role, comps)| n.role.name() == role && comps.contains(&n.component));
    let mut v = vec![];
    for (id, b) in new.nodes.iter().filter(|(_, n)| n.role.registered()) {
        match old.nodes.get(id).filter(|a| a.role == b.role && a.component == b.component) {
            Some(a) => v.extend(params.iter().filter(|p| owns(p, b) && value(p, a) != value(p, b)).map(|p| (p.name.clone(), value(p, b)))),
            None => v.extend(params.iter().filter(|p| owns(p, b)).map(|p| (p.name.clone(), value(p, b)))),
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A live switch sends the new node's knobs as the plan has them (not the component's
    /// defaults), and a switch the preset cannot take live is none.
    #[test]
    fn live_switch_sends_the_nodes_knobs() {
        let spec = |edits: &str| {
            let mut r = RendererSpec::from_cli(&format!("discover-gpu{edits}")).unwrap();
            r.options.insert("refresh".into(), 4.into());
            r.resolve().unwrap();
            r
        };
        // The holes node, as `plans/demo-pathtrace-discovery.toml` writes it.
        let coverage = r#",{add="holes",role="holes",component="coverage"},{node="draw",input="faces",when={iterate=2}},{node="holes",input="covered",from="draw"},{node="holes",input="sky",from="draw"},{node="holes",input="sky",when="prev"},{node="visibility",input="where",from="holes"}"#;
        let old = spec("");
        let (_, params) = crate::pipelines::executor_of(&old.pipeline().unwrap()).unwrap();
        let knobs: Vec<_> = params.iter().map(|p| (p.clone(), knobs::option_value(p, &old.knob_values()))).collect();
        let new = spec(coverage);
        let edits = live_edits(&old, &new, &knobs).unwrap();
        assert!(edits.contains(&("refresh".into(), ParamValue::Int(4))), "{edits:?}");
        assert!(edits.contains(&("keep_unseen".into(), ParamValue::Int(64))), "{edits:?}");
        // The worker is sent the graph's edits: the node, what reads it, the drawer's second draw.
        let (g0, g1) = (old.worker_graph().unwrap().unwrap(), new.worker_graph().unwrap().unwrap());
        let (sent, switches) = g0.live_edits(&g1).unwrap();
        let shape = |mut g: Graph| {
            g.nodes.values_mut().for_each(|n| n.knobs.clear());
            g
        };
        assert_eq!(shape(g0.with_edits(&sent).unwrap()), shape(g1.clone()));
        assert_eq!(switches.iter().map(|s| (s.role, s.component.as_deref())).collect::<Vec<_>>(), [(Role::Holes, Some("coverage"))]);
        let back = live_edits(&new, &old, &knobs).unwrap();
        assert_eq!(back, vec![]);
        assert_eq!(g1.live_edits(&g0).unwrap().1.iter().map(|s| (s.role, s.component.as_deref())).collect::<Vec<_>>(), [(Role::Holes, None)]);
        // Storage is not live on discover-gpu.
        assert!(live_edits(&old, &spec(",storage=tree64"), &knobs).is_none());
    }

    /// A live lane's failure and refused knobs land on the node at fault: the failure by the
    /// component it names (whatever the host put in front), a refusal by the knob's owner;
    /// the side panel and the editor's canvas then draw that node marked.
    #[test]
    fn faults_are_shown_on_the_node_at_fault() {
        let spec = RendererSpec::from_cli("raycast-gpu,storage=clipmap").unwrap();
        let g = model::graph_of(&spec).unwrap();
        let (_, params) = crate::pipelines::executor_of(&spec.pipeline().unwrap()).unwrap();
        let storage = g.nodes.iter().find(|(_, n)| n.component == "clipmap").map(|(k, _)| k.clone()).unwrap();
        let failure = "failed: worker error: clipmap: a view radius of 70 chunks needs a volume 141 chunks (4512 texels) wide, over what X allows";
        let f = model::faults(&g, Some(failure), &[], &params);
        assert_eq!(f.keys().collect::<Vec<_>>(), [&storage], "{f:?}");
        assert!(model::faults(&g, Some("failed: worker error: the voxel pool is full"), &[], &params).is_empty());
        assert!(model::faults(&g, Some("worker exited (signal 11)"), &[], &params).is_empty());
        // A refused knob: its owner, as the editor files the knob under that node.
        let (knob, owner) = params
            .iter()
            .find_map(|p| {
                let (slot, comps) = p.owner.as_ref()?;
                comps.iter().find_map(|c| model::knob_owner(&g, slot, c)).map(|o| (p.name.clone(), o))
            })
            .expect("a knob owned by a node");
        let f = model::faults(&g, None, &[(knob.clone(), "out of range".into())], &params);
        assert_eq!(f.keys().collect::<Vec<_>>(), [&owner], "{f:?}");
        assert_eq!(f[&owner], [format!("refused {knob}: out of range")]);

        // Drawn: both the side panel and the editor mark the storage node.
        let run = RunSpec::default();
        let knobs: Vec<_> = params.iter().map(|p| (p.clone(), knobs::option_value(p, &spec.knob_values()))).collect();
        let ctx = egui::Context::default();
        let key = format!("{:?}", Of::Lane(0));
        let mut trials = Trials::default();
        let marked = |out: &egui::FullOutput| {
            fn texts(s: &egui::Shape, out: &mut Vec<String>) {
                match s {
                    egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                    _ => {}
                }
            }
            let mut all = vec![];
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut all));
            all.iter().any(|t| *t == format!("⚠ {storage}"))
        };
        for faulty in [false, true] {
            let faults = if faulty { model::faults(&g, Some(failure), &[], &params) } else { Default::default() };
            set_faults(&mut trials, &key, &spec, faults.clone());
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| drop(summary(ui, &key, &spec, &knobs, true, None, &mut trials, &[], &run)));
            out.textures_delta.clear();
            assert_eq!(marked(&out), faulty, "side panel");
            let mut win = Window::new(&ctx, Of::Lane(0), None);
            set_faults(&mut trials, &key, &spec, faults);
            let size = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))), ..Default::default() };
            let mut out = ctx.run_ui(size, |ui| drop(pane(ui, &mut win, "lane", &spec, &knobs, true, &mut trials, &[], &run)));
            out.textures_delta.clear();
            assert_eq!(marked(&out), faulty, "canvas");
        }
    }

    /// Every choice the editor lists on every preset's graph is answered (runs, or a reason),
    /// and the answers come quickly enough for a picker.
    #[test]
    fn every_choice_is_answered() {
        let run = crate::plan::Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/terrain.toml")), &[]).unwrap().run;
        let (mut ok, mut no) = (0, 0);
        let start = std::time::Instant::now();
        assert!(start_files().count() >= 10 && start_files().all(|(_, about)| about.len() > 20), "every file says what it is");
        for (file, _) in start_files() {
            let s = RendererSpec::from_file(file).unwrap();
            let g = model::graph_of(&s).unwrap();
            let mut t = Trial::new(s, g.clone());
            let devs = devices(&g);
            for (id, n) in &g.nodes {
                let mut opts = component_options(&g, id);
                opts.extend(device_options(&g, id, &run, devs.get(id).copied()));
                opts.extend(api_options(&g, id, devs.get(id).map_or(Kind::Gpu, |d| d.kind())));
                for p in inputs(n.role, &n.component).iter().filter(|p| p.data != Data::Changes) {
                    opts.extend(bind_options(&g, id, p));
                    if n.inputs.contains_key(p.name) && !p.data.fused() {
                        opts.extend(timing_options(n, id, p.name));
                    }
                }
                for (text, change, on, _) in opts.into_iter().filter(|o| !o.2) {
                    let _ = on;
                    match t.try_change(&change, &[]) {
                        Ok(_) => ok += 1,
                        Err(e) => {
                            assert!(!e.is_empty(), "{file}: {id} {text}: an empty reason");
                            if std::env::var("MR_SHOW_REFUSED").is_ok() {
                                println!("{file} {id}: {text}: {e}");
                            }
                            no += 1;
                        }
                    }
                }
            }
        }
        let ms = start.elapsed().as_secs_f64() * 1e3;
        println!("{ok} choices run, {no} refused, {ms:.0} ms ({:.2} ms each)", ms / (ok + no) as f64);
        assert!(ok > 100 && no > 0);
    }
}
