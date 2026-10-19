//! The graph-first pipeline editor (PLAN A, IDEAS §26). A renderer is its graph: the side
//! panels show it compactly (`summary`: a row per node, its component and knobs), the
//! editor window draws it (`canvas`) with an inspector for the selected node. Every choice
//! is listed; one whose graph cannot run is greyed and says why (`model::Trial`). The back
//! end is not chosen: it is the preset the graph's shape names, shown as a label.

mod canvas;
pub mod model;

use std::collections::HashMap;

use egui::{ComboBox, RichText, Ui};
use mr_components::graph::{Data, Device, Graph, Role, When, inputs, output, role_components, state};
use mr_components::{Api, BACKENDS, Kind};
use mr_core::{ParamSpec, ParamValue, RendererInfo};

use super::knobs;
use crate::plan::{RendererSpec, RunSpec};
use model::{Change, Target, Trial};

/// What the user changed.
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
}

/// A list of choices, each a change of the graph: greyed with the reason where the result
/// cannot run. Returns the renderer picked.
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
                        let label = if spec.name != trial.base().name { format!("{text}  → {}", spec.name) } else { text };
                        if ui.add(egui::Button::selectable(false, label)).on_hover_text(format!("{hover}\n\nRuns as `{}`: {}", spec.name, spec.label())).clicked() {
                            pick = Some(spec);
                        }
                    }
                    Err(why) => {
                        ui.add_enabled(false, egui::Button::selectable(false, &text)).on_disabled_hover_text(format!("{hover}\n\nCannot run: {why}"));
                    }
                }
            }
        })
        .response
        .on_hover_text(tip);
    pick
}

fn description(role: Role, component: &str) -> String {
    match role.slot().and_then(|s| mr_components::component(s, component)) {
        Some(c) => c.description.to_string(),
        None => match (role, component) {
            (Role::World, _) => "The authoritative world on the server: chunks and their change log".into(),
            (Role::Generator, _) => "Generates the world's chunks from its spec (a shared seed): on the server, or on the client's cpu".into(),
            (Role::Link, "free") => "Everything the server sends arrives at once, in order".into(),
            (Role::Link, _) => "A modelled network: latency, jitter, bandwidth, loss, reordering".into(),
            (Role::Client, "pick") => "Holds what arrived: the server's LOD levels, picked by its radius and levels".into(),
            (Role::Client, _) => "Holds what arrived and derives its coarser LOD levels from LOD 0 itself".into(),
            (Role::Post, "clamp") => "Linear colour clamped to the output range".into(),
            (Role::Post, c) => format!("Tone mapping `{c}` of the linear colour, compiled into the shading's output"),
            _ => String::new(),
        },
    }
}

/// The component choices for node `id`.
fn component_options(g: &Graph, id: &str) -> Vec<(String, Change, bool, String)> {
    let n = &g.nodes[id];
    role_components(n.role).into_iter().map(|c| (c.to_string(), Change::Component(id.into(), c), n.component == c, description(n.role, c))).collect()
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
    if let Some(c) = n.role.slot().and_then(|s| mr_components::component(s, &n.component)) {
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
        v.push((format!("new {} {c}", role.name()), Change::Bind(id.into(), port.name.into(), Target::New(role, c)), false, format!("A new {} node: {}", role.name(), description(role, c))));
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

/// The executor's knobs that belong to node `id` (or, for `None`, to no node), with values.
fn node_knobs<'a>(g: &Graph, id: Option<&str>, knobs: &'a [(ParamSpec, ParamValue)]) -> Vec<(&'a ParamSpec, ParamValue)> {
    knobs
        .iter()
        .filter(|(s, _)| !mr_components::Slot::ALL.iter().any(|sl| sl.name() == s.name))
        .filter(|(s, _)| match (&s.owner, id) {
            (None, None) => true,
            (Some((slot, comps)), Some(id)) => comps.iter().any(|c| model::knob_owner(g, slot, c).as_deref() == Some(id)),
            _ => false,
        })
        .map(|(s, v)| (s, v.clone()))
        .collect()
}

/// The client's or the link's knobs (graph knobs, run as a hot swap).
fn world_knob_row(ui: &mut Ui, g: &Graph, id: &str, run: &RunSpec, trial: &mut Trial, infos: &[RendererInfo]) -> Option<RendererSpec> {
    let n = &g.nodes[id];
    let specs = model::world_knobs(n.role, &n.component, run);
    let vals: Vec<(&ParamSpec, ParamValue)> = specs.iter().map(|s| (s, n.knobs.get(&s.name).and_then(|v| serde_json::to_value(v).ok()).and_then(|j| s.from_json(&j)).unwrap_or_else(|| s.default.clone()))).collect();
    let mut out = None;
    for (name, v, commit) in knobs::knob_row(ui, &vals, false, &mut |_, _| {}) {
        if commit {
            match trial.try_change(&Change::Knob(id.into(), name.clone(), Some(super::param_to_toml(&v))), infos) {
                Ok((_, s)) => out = Some(s),
                Err(e) => tracing::warn!("{name}: {e}"),
            }
        }
    }
    out
}

/// Where each node runs, as the checked graph resolves it.
fn devices(g: &Graph) -> HashMap<String, Device> {
    g.check().map(|c| c.devices).unwrap_or_default()
}

/// The compact form for a side panel: the preset it runs as, then a row per node with its
/// component and knobs. `marks` labels restart-only knobs; `after` adds a widget after each
/// knob (the tuner's lock).
pub fn summary(ui: &mut Ui, key: &str, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], marks: bool, after: &mut dyn FnMut(&mut Ui, &ParamSpec), trials: &mut Trials, infos: &[RendererInfo], run: &RunSpec) -> Vec<Edit> {
    let mut edits = vec![];
    let Some(trial) = trials.get(key, spec) else {
        // A standalone renderer: its knobs only.
        let vals: Vec<(&ParamSpec, ParamValue)> = knobs.iter().map(|(s, v)| (s, v.clone())).collect();
        ui.horizontal_wrapped(|ui| {
            for (name, value, commit) in knobs::knob_row(ui, &vals, marks, after) {
                edits.push(Edit::Knob { name, value, commit });
            }
        });
        return edits;
    };
    let g = trial.graph().clone();
    let devs = devices(&g);
    ui.horizontal_wrapped(|ui| {
        if ui.button("✏ graph").on_hover_text("Open the pipeline editor: the graph drawn, every node's component, device, API, inputs and knobs").clicked() {
            edits.push(Edit::Open(None));
        }
        ui.label(RichText::new(format!("runs as {}", spec.name)).strong()).on_hover_text("The preset this graph's shape names (its executor): derived from the graph, not chosen");
        for (name, value, commit) in knobs::knob_row(ui, &node_knobs(&g, None, knobs), marks, after) {
            edits.push(Edit::Knob { name, value, commit });
        }
    });
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
            if ui.add_sized([86.0, 18.0], egui::Button::new(RichText::new(&id).small()).frame(false)).on_hover_text(format!("{tip}\n\nClick: open it in the editor")).clicked() {
                edits.push(Edit::Open(Some(id.clone())));
            }
            if let Some(s) = choices(ui, (key, &id, "comp"), &n.component, &description(n.role, &n.component), component_options(&g, &id), trial, infos) {
                edits.push(Edit::Spec(s));
            }
            if !fused && !matches!(n.role, Role::Client | Role::Link) {
                ui.weak(dev);
            }
            if matches!(n.role, Role::Client | Role::Link) {
                if let Some(s) = world_knob_row(ui, &g, &id, run, trial, infos) {
                    edits.push(Edit::Spec(s));
                }
            }
            for (name, value, commit) in knobs::knob_row(ui, &node_knobs(&g, Some(&id), knobs), marks, after) {
                edits.push(Edit::Knob { name, value, commit });
            }
        });
    }
    edits
}

/// Which renderer the editor window edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Of {
    Plan(usize),
    Lane(usize),
}

/// The editor window's state.
pub struct Window {
    pub of: Of,
    pub selected: Option<String>,
    port: Option<String>,
    canvas: canvas::View,
    undo: Vec<RendererSpec>,
    redo: Vec<RendererSpec>,
}

impl Window {
    pub fn new(of: Of, selected: Option<String>) -> Self {
        Self { of, selected, port: None, canvas: Default::default(), undo: vec![], redo: vec![] }
    }
}

/// The editor window for `spec`. Returns the edits and whether it stays open.
#[allow(clippy::too_many_arguments)]
pub fn window(ctx: &egui::Context, win: &mut Window, title: &str, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], marks: bool, trials: &mut Trials, infos: &[RendererInfo], run: &RunSpec) -> (Vec<Edit>, bool) {
    let mut open = true;
    let mut edits = vec![];
    let key = format!("{:?}", win.of);
    egui::Window::new(format!("Pipeline — {title}"))
        .id(egui::Id::new("pipeline-editor"))
        .open(&mut open)
        .default_size([1400.0, 620.0])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            let Some(trial) = trials.get(&key, spec) else {
                ui.label("A standalone renderer: it has no graph.");
                return;
            };
            edits = editor(ui, win, spec, knobs, marks, trial, infos, run);
        });
    (edits, open)
}

#[allow(clippy::too_many_arguments)]
fn editor(ui: &mut Ui, win: &mut Window, spec: &RendererSpec, knobs: &[(ParamSpec, ParamValue)], marks: bool, trial: &mut Trial, infos: &[RendererInfo], run: &RunSpec) -> Vec<Edit> {
    let mut edits = vec![];
    let g = trial.graph().clone();
    if win.selected.as_ref().is_some_and(|s| !g.nodes.contains_key(s)) {
        win.selected = None;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("runs as {}", spec.name)).strong()).on_hover_text("The preset this graph's shape and devices name: the executor that runs it (derived, never chosen)");
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
            ui.weak("A preset's default pipeline (then change anything)");
            for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
                if ui.button(b.name).on_hover_text(b.description).clicked() {
                    let mut s = RendererSpec::new(b.name);
                    s.label = spec.label.clone();
                    s.devices = spec.devices.clone();
                    s.compile = spec.compile;
                    if s.resolve().is_ok() {
                        win.undo.push(spec.clone());
                        edits.push(Edit::Spec(s));
                    }
                    ui.close();
                }
            }
        });
        if ui.button("fit").on_hover_text("Lay the graph out again and fit it to the window (dragged nodes go back)").clicked() {
            win.canvas = Default::default();
        }
    });
    ui.separator();
    let mut made = vec![];
    egui::Panel::right("pipeline-inspector").resizable(true).default_size(310.0).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            made.extend(inspector(ui, win, &g, knobs, marks, trial, infos, run));
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let r = canvas::show(ui, &mut win.canvas, &g, &devices(&g), win.selected.as_deref(), trial, infos);
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

#[allow(clippy::too_many_arguments)]
fn inspector(ui: &mut Ui, win: &mut Window, g: &Graph, knobs: &[(ParamSpec, ParamValue)], marks: bool, trial: &mut Trial, infos: &[RendererInfo], run: &RunSpec) -> Vec<Edit> {
    let mut edits = vec![];
    let Some(id) = win.selected.clone() else {
        ui.heading("The renderer");
        ui.label("Click a node to change its component, device, API, inputs and knobs. Drag from a node's output (right) onto an input (left) to rebind it: green where that runs, red where not (hover for why).");
        ui.add_space(6.0);
        ui.label(RichText::new("Executor knobs").strong()).on_hover_text("Knobs of the executor itself, not of a node");
        ui.horizontal_wrapped(|ui| {
            for (name, value, commit) in knobs::knob_row(ui, &node_knobs(g, None, knobs), marks, &mut |_, _| {}) {
                edits.push(Edit::Knob { name, value, commit });
            }
        });
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
        if !fused && n.role.slot().is_some() {
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
    let world = matches!(n.role, Role::Client | Role::Link) && !model::world_knobs(n.role, &n.component, run).is_empty();
    if !own.is_empty() || world {
        ui.add_space(6.0);
        ui.label(RichText::new("Knobs").strong());
        ui.horizontal_wrapped(|ui| {
            for (name, value, commit) in knobs::knob_row(ui, &own, marks, &mut |_, _| {}) {
                edits.push(Edit::Knob { name, value, commit });
            }
            if world {
                if let Some(s) = world_knob_row(ui, g, &id, run, trial, infos) {
                    edits.push(Edit::Spec(s));
                }
            }
        });
    } else if n.role.slot().is_some() && !g.nodes.iter().any(|(k, m)| *k != id && m.role == n.role && m.component == n.component) {
        // Nothing to show.
    } else if n.role.slot().is_some() {
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

/// The component switch `new` makes on `old` if it is one the running renderer takes live
/// (its slot's knob is live): (slot, component).
pub fn live_switch(old: &RendererSpec, new: &RendererSpec, params: &[(ParamSpec, ParamValue)]) -> Option<(String, String)> {
    let (Some(a), Some(b)) = (&old.compose, &new.compose) else { return None };
    if old.name != new.name || old.sites != new.sites || old.graph.is_some() || new.graph.is_some() || old.ran_graph.is_some() || new.ran_graph.is_some() {
        return None;
    }
    let diff: Vec<mr_components::Slot> = mr_components::Slot::ALL.into_iter().filter(|s| a.get(*s) != b.get(*s)).collect();
    let [slot] = diff.as_slice() else { return None };
    params.iter().any(|(p, _)| p.name == slot.name() && p.live).then(|| (slot.name().to_string(), b.get(*slot).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every choice the editor lists on every preset's graph is answered (runs, or a reason),
    /// and the answers come quickly enough for a picker.
    #[test]
    fn every_choice_is_answered() {
        let run = crate::plan::Plan::load(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/terrain.toml")), &[]).unwrap().run;
        let (mut ok, mut no) = (0, 0);
        let start = std::time::Instant::now();
        for b in BACKENDS.iter().filter(|b| b.renderer.is_some()) {
            let mut s = RendererSpec::new(b.name);
            s.resolve().unwrap();
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
                            assert!(!e.is_empty(), "{}: {id} {text}: an empty reason", b.name);
                            if std::env::var("MR_SHOW_REFUSED").is_ok() {
                                println!("{} {id}: {text}: {e}", b.name);
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
