//! Docking: every pane of the main window sits in one of three docks or in an OS window of
//! its own. The bottom dock spans the full width, the left and right docks run down to its
//! upper edge, and the renderers' viewport takes the middle. Tabs are dragged between docks
//! (or out over the viewport, which tears the pane off into its own window); a tab's context
//! menu does the same. An empty dock disappears, a collapsed one is a thin strip at the
//! window's edge (click to open). The layout is remembered across sessions.

use egui::{Color32, Sense};
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Pane {
    Form,
    Toml,
    Metrics,
    Compare,
    Benchmark,
    Schemes,
    Keys,
    Log,
}

impl Pane {
    const ALL: [Pane; 8] = [Pane::Form, Pane::Toml, Pane::Metrics, Pane::Compare, Pane::Benchmark, Pane::Schemes, Pane::Keys, Pane::Log];

    fn title(self) -> &'static str {
        match self {
            Pane::Form => "📝 Form",
            Pane::Toml => "🗒 TOML",
            Pane::Metrics => "📈 Metrics",
            Pane::Compare => "⚖ Compare",
            Pane::Benchmark => "⏱ Benchmark",
            Pane::Schemes => "📖 Schemes",
            Pane::Keys => "❓ Keys",
            Pane::Log => "📜 Log",
        }
    }

    fn hover(self) -> &'static str {
        match self {
            Pane::Form => "Edit the plan with forms",
            Pane::Toml => "Edit the plan as raw TOML (what Save writes)",
            Pane::Metrics => "Live statistics, knobs and auto-tuners per renderer",
            Pane::Compare => "Image comparison of the current view against the reference (C)",
            Pane::Benchmark => "Results of the last benchmark run",
            Pane::Schemes => "The encyclopedia: every rendering scheme and idea (docs/schemes), with what was measured",
            Pane::Keys => "Key bindings (H)",
            Pane::Log => "The live session's log",
        }
    }

    fn viewport(self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("pane", self))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Side {
    Left,
    Right,
    Bottom,
}

impl Side {
    const ALL: [Side; 3] = [Side::Left, Side::Right, Side::Bottom];

    fn name(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Bottom => "bottom",
        }
    }
}

#[derive(Clone, PartialEq, Default, Serialize, Deserialize)]
pub(super) struct Dock {
    panes: Vec<Pane>,
    active: usize,
    collapsed: bool,
}

/// A pane in an OS window of its own, and the dock it returns to.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
struct Torn {
    pane: Pane,
    back: Side,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct Docks {
    left: Dock,
    right: Dock,
    bottom: Dock,
    #[serde(default)]
    windows: Vec<Torn>,
    #[serde(skip)]
    saved: bool,
}

impl Default for Docks {
    fn default() -> Self {
        let dock = |panes: &[Pane]| Dock { panes: panes.to_vec(), active: 0, collapsed: false };
        Self {
            left: dock(&[Pane::Form, Pane::Toml]),
            right: dock(&[Pane::Metrics, Pane::Compare, Pane::Benchmark, Pane::Schemes, Pane::Keys]),
            bottom: dock(&[Pane::Log]),
            windows: vec![],
            saved: true,
        }
    }
}

enum Action {
    Select(Side, usize),
    Move(Pane, Side),
    Tear(Pane),
    Collapse(Side, bool),
}

impl Docks {
    fn dock(&self, s: Side) -> &Dock {
        match s {
            Side::Left => &self.left,
            Side::Right => &self.right,
            Side::Bottom => &self.bottom,
        }
    }

    fn dock_mut(&mut self, s: Side) -> &mut Dock {
        match s {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
            Side::Bottom => &mut self.bottom,
        }
    }

    fn side_of(&self, p: Pane) -> Option<Side> {
        Side::ALL.into_iter().find(|&s| self.dock(s).panes.contains(&p))
    }

    /// Takes `p` out of its dock or window; returns the dock it belongs to.
    fn remove(&mut self, p: Pane) -> Side {
        if let Some(i) = self.windows.iter().position(|t| t.pane == p) {
            return self.windows.remove(i).back;
        }
        let s = self.side_of(p).unwrap_or(Side::Right);
        let d = self.dock_mut(s);
        let i = d.panes.iter().position(|&q| q == p).unwrap_or(0);
        d.panes.retain(|&q| q != p);
        if d.active > i || d.active >= d.panes.len() {
            d.active = d.active.saturating_sub(1);
        }
        s
    }

    pub(super) fn move_to(&mut self, p: Pane, s: Side) {
        if self.side_of(p) == Some(s) {
            return;
        }
        self.remove(p);
        let d = self.dock_mut(s);
        d.panes.push(p);
        d.active = d.panes.len() - 1;
        d.collapsed = false;
        self.saved = false;
    }

    pub(super) fn tear(&mut self, p: Pane) {
        let back = self.remove(p);
        self.windows.push(Torn { pane: p, back });
        self.saved = false;
    }

    /// Brings `p` to the front: its dock's active tab (the dock opened), or its window focused.
    pub(super) fn show(&mut self, ctx: &egui::Context, p: Pane) {
        if self.windows.iter().any(|t| t.pane == p) {
            ctx.send_viewport_cmd_to(p.viewport(), egui::ViewportCommand::Focus);
        } else if let Some(s) = self.side_of(p) {
            let d = self.dock_mut(s);
            d.active = d.panes.iter().position(|&q| q == p).unwrap_or(0);
            d.collapsed = false;
            self.saved = false;
        }
    }

    /// Applies `pane:side`, `pane:window` and `side:collapsed` items, comma-separated.
    #[cfg(test)]
    pub(super) fn arrange(&mut self, spec: &str) -> anyhow::Result<()> {
        let name = |s: &str| toml::Value::String(s.to_string());
        for item in spec.split(',').filter(|s| !s.is_empty()) {
            let (a, b) = item.split_once(':').ok_or_else(|| anyhow::anyhow!("{item}: expected a:b"))?;
            if b == "collapsed" {
                self.dock_mut(Side::deserialize(name(a))?).collapsed = true;
            } else if b == "window" {
                self.tear(Pane::deserialize(name(a))?);
            } else {
                self.move_to(Pane::deserialize(name(a))?, Side::deserialize(name(b))?);
            }
        }
        Ok(())
    }

    /// Every pane exactly once (a stored layout may predate a pane, or be edited by hand).
    fn repaired(mut self) -> Self {
        let mut seen = HashSet::new();
        for s in Side::ALL {
            self.dock_mut(s).panes.retain(|&p| seen.insert(p));
        }
        self.windows.retain(|t| seen.insert(t.pane));
        let default = Docks::default();
        for p in Pane::ALL.into_iter().filter(|p| !seen.contains(p)) {
            self.dock_mut(default.side_of(p).unwrap_or(Side::Right)).panes.push(p);
        }
        for s in Side::ALL {
            let d = self.dock_mut(s);
            d.active = d.active.min(d.panes.len().saturating_sub(1));
        }
        self
    }

    fn file() -> Option<PathBuf> {
        Some(config_dir()?.join("docks.toml"))
    }

    /// The layout of the last session, or the default one.
    pub(super) fn load() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        let text = Self::file().and_then(|f| std::fs::read_to_string(f).ok());
        let stored = text.and_then(|t| toml::from_str::<Docks>(&t).ok());
        stored.map(|d| Docks { saved: true, ..d.repaired() }).unwrap_or_default()
    }

    fn save(&mut self) {
        if self.saved {
            return;
        }
        self.saved = true;
        if cfg!(test) {
            return;
        }
        if let (Some(f), Ok(text)) = (Self::file(), toml::to_string(self)) {
            if let Some(d) = f.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(f, text);
        }
    }
}

/// Width of a collapsed or empty dock.
const STRIP: f32 = 10.0;

impl App {
    /// The three docks; call before the central panel.
    pub(super) fn docks(&mut self, ui: &mut egui::Ui) {
        let dragging = egui::DragAndDrop::has_payload_of_type::<Pane>(ui.ctx());
        for side in [Side::Bottom, Side::Left, Side::Right] {
            let d = self.docks.dock(side);
            if d.panes.is_empty() && !dragging {
                continue;
            }
            let thin = d.panes.is_empty() || d.collapsed;
            let id = egui::Id::new(("dock", side, thin));
            let panel = match side {
                Side::Left => egui::Panel::left(id),
                Side::Right => egui::Panel::right(id),
                Side::Bottom => egui::Panel::bottom(id),
            };
            let size = match side {
                Side::Left => 400.0,
                Side::Right => 430.0,
                Side::Bottom => 110.0,
            };
            let panel = if thin { panel.exact_size(STRIP).resizable(false) } else { panel.resizable(true).default_size(size) };
            let action = panel.show(ui, |ui| if thin { strip(ui, side, &self.docks) } else { self.dock_ui(ui, side) }).inner;
            self.apply(action);
        }
    }

    fn apply(&mut self, action: Option<Action>) {
        match action {
            Some(Action::Select(s, i)) => {
                self.docks.dock_mut(s).active = i;
                self.docks.saved = false;
            }
            Some(Action::Move(p, s)) => self.docks.move_to(p, s),
            Some(Action::Tear(p)) => self.docks.tear(p),
            Some(Action::Collapse(s, on)) => {
                self.docks.dock_mut(s).collapsed = on;
                self.docks.saved = false;
            }
            None => {}
        }
    }

    fn dock_ui(&mut self, ui: &mut egui::Ui, side: Side) -> Option<Action> {
        let area = ui.max_rect();
        let d = self.docks.dock(side).clone();
        let mut action = None;
        ui.horizontal(|ui| {
            for (i, &p) in d.panes.iter().enumerate() {
                let r = ui.selectable_label(i == d.active, p.title()).on_hover_text(format!("{}\n\nDrag to another dock, or over the view for a window of its own; right-click for more.", p.hover()));
                let r = r.interact(Sense::drag());
                if r.clicked() {
                    action = Some(Action::Select(side, i));
                }
                if r.drag_started() {
                    r.dnd_set_drag_payload(p);
                }
                r.context_menu(|ui| {
                    for s in Side::ALL.into_iter().filter(|&s| s != side) {
                        if ui.button(format!("Move to the {} dock", s.name())).clicked() {
                            action = Some(Action::Move(p, s));
                        }
                    }
                    if ui.button("↗ Open in a window of its own").clicked() {
                        action = Some(Action::Tear(p));
                    }
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let arrow = match side {
                    Side::Left => "◀",
                    Side::Right => "▶",
                    Side::Bottom => "▼",
                };
                if ui.small_button(arrow).on_hover_text("Collapse this dock to a strip at the edge").clicked() {
                    action = Some(Action::Collapse(side, true));
                }
                if let Some(&p) = d.panes.get(d.active)
                    && ui.small_button("↗").on_hover_text(format!("Open {} in a window of its own", p.title())).clicked()
                {
                    action = Some(Action::Tear(p));
                }
            });
        });
        ui.separator();
        if let Some(&p) = d.panes.get(d.active) {
            self.pane(ui, p);
        }
        drop_zone(ui, area, side).or(action)
    }

    /// The panes in OS windows of their own (closing one docks it again).
    pub(super) fn pane_windows(&mut self, ctx: &egui::Context) {
        for t in self.docks.windows.clone() {
            let vb = egui::ViewportBuilder::default().with_title(format!("MultiRenderer — {}", t.pane.title())).with_inner_size(if t.pane == Pane::Log { [900.0, 260.0] } else { [520.0, 760.0] });
            let mut back = false;
            ctx.show_viewport_immediate(t.pane.viewport(), vb, |ui, _| {
                back |= ui.ctx().input(|i| i.viewport().close_requested());
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(t.pane.title());
                        if ui.button("↙ Dock").on_hover_text(format!("Back into the {} dock (closing the window does the same)", t.back.name())).clicked() {
                            back = true;
                        }
                    });
                    ui.separator();
                    self.pane(ui, t.pane);
                });
            });
            if back {
                self.docks.move_to(t.pane, t.back);
            }
        }
    }

    /// After the central panel: a tab dropped over the view gets a window of its own; the
    /// layout is saved when it changed.
    pub(super) fn finish_docks(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.any_released())
            && let Some(p) = egui::DragAndDrop::take_payload::<Pane>(ctx)
        {
            self.docks.tear(*p);
        }
        self.docks.save();
    }

    fn pane(&mut self, ui: &mut egui::Ui, p: Pane) {
        match p {
            Pane::Form => self.form(ui),
            Pane::Toml => self.toml(ui),
            Pane::Metrics => steady_scroll(ui, "metrics", |ui| self.metrics(ui)),
            Pane::Compare => steady_scroll(ui, "compare", |ui| self.compare(ui)),
            Pane::Benchmark => steady_scroll(ui, "benchmark", |ui| self.benchmark(ui)),
            Pane::Schemes => steady_scroll(ui, "schemes", |ui| schemes::browser(ui, &mut self.schemes)),
            Pane::Keys => steady_scroll(ui, "keys", input::help),
            Pane::Log => {
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    if let Some(l) = &self.live {
                        for line in l.shared.lock().unwrap().log.iter() {
                            ui.monospace(line);
                        }
                    }
                });
            }
        }
    }
}

/// A collapsed or (while a tab is dragged) empty dock: click to open, drop a tab to add it.
fn strip(ui: &mut egui::Ui, side: Side, docks: &Docks) -> Option<Action> {
    let area = ui.max_rect();
    let panes = &docks.dock(side).panes;
    let r = ui.allocate_rect(area, Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(area, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let names: Vec<&str> = panes.iter().map(|p| p.title()).collect();
    let r = r.on_hover_text(if names.is_empty() { format!("The {} dock: drop a tab here", side.name()) } else { format!("{} (click to open)", names.join(", ")) });
    let open = (r.clicked() && !panes.is_empty()).then_some(Action::Collapse(side, false));
    drop_zone(ui, area, side).or(open)
}

/// `area` accepts a dragged tab: highlighted while one hovers, moved here when released.
fn drop_zone(ui: &mut egui::Ui, area: egui::Rect, side: Side) -> Option<Action> {
    let r = ui.interact(area, ui.id().with(("dock-drop", side)), Sense::hover());
    if r.dnd_hover_payload::<Pane>().is_some() {
        ui.painter().rect_stroke(area.shrink(1.0), 2.0, egui::Stroke::new(2.0, Color32::LIGHT_BLUE), egui::StrokeKind::Inside);
    }
    r.dnd_release_payload::<Pane>().map(|p| Action::Move(*p, side))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(d: &Docks) -> Vec<Pane> {
        let mut v: Vec<Pane> = Side::ALL.into_iter().flat_map(|s| d.dock(s).panes.clone()).chain(d.windows.iter().map(|t| t.pane)).collect();
        v.sort_by_key(|p| Pane::ALL.iter().position(|q| q == p));
        v
    }

    #[test]
    fn panes_move_tear_off_and_dock_back() {
        let mut d = Docks::default();
        d.right.active = 3; // Schemes
        d.move_to(Pane::Compare, Side::Bottom);
        assert_eq!(d.right.panes[d.right.active], Pane::Schemes, "the active tab stays when one before it leaves");
        assert_eq!((d.bottom.panes.clone(), d.bottom.active), (vec![Pane::Log, Pane::Compare], 1));
        d.tear(Pane::Compare);
        assert_eq!(d.windows, vec![Torn { pane: Pane::Compare, back: Side::Bottom }]);
        d.move_to(Pane::Compare, Side::Bottom);
        assert!(d.windows.is_empty() && d.bottom.panes.contains(&Pane::Compare));
        d.move_to(Pane::Form, Side::Left);
        assert_eq!(d.left.panes, vec![Pane::Form, Pane::Toml], "a move to its own dock changes nothing");
        d.move_to(Pane::Form, Side::Right);
        d.move_to(Pane::Toml, Side::Right);
        assert!(d.left.panes.is_empty());
        assert_eq!(all(&d), Pane::ALL);
    }

    #[test]
    fn stored_layouts_round_trip_and_are_repaired() {
        let mut d = Docks::default();
        d.tear(Pane::Keys);
        d.bottom.collapsed = true;
        let back: Docks = toml::from_str(&toml::to_string(&d).unwrap()).unwrap();
        assert!(back == Docks { saved: back.saved, ..d.clone() });
        let odd: Docks = toml::from_str("[left]\npanes = [\"form\", \"form\"]\nactive = 7\ncollapsed = false\n[right]\npanes = []\nactive = 0\ncollapsed = false\n[bottom]\npanes = [\"log\"]\nactive = 0\ncollapsed = false\n").unwrap();
        let fixed = odd.repaired();
        assert_eq!(all(&fixed), Pane::ALL);
        assert_eq!(fixed.left.active, 1);
    }
}
