//! Docking: every pane of the main window sits in one of three docks or in an OS window of
//! its own. The bottom dock spans the full width, the left and right docks run down to its
//! upper edge, and the renderers' viewport takes the middle. Tabs are dragged between docks
//! (or out over the viewport, which tears the pane off into its own window); a tab's context
//! menu does the same. An empty dock disappears, a collapsed one is a thin strip at the
//! window's edge (hover to look in, click to open). The pipeline editor is a pane too, and may
//! also take the middle in place of the viewport. The layout and dock sizes are remembered
//! across sessions.

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
    Pipeline,
}

impl Pane {
    const ALL: [Pane; 9] = [Pane::Form, Pane::Toml, Pane::Metrics, Pane::Compare, Pane::Benchmark, Pane::Schemes, Pane::Keys, Pane::Log, Pane::Pipeline];

    fn title(self) -> &'static str {
        match self {
            Pane::Form => "📝 Form",
            Pane::Toml => "📄 TOML",
            Pane::Metrics => "📈 Metrics",
            Pane::Compare => "⚖ Compare",
            Pane::Benchmark => "⏱ Benchmark",
            Pane::Schemes => "📖 Schemes",
            Pane::Keys => "❓ Keys",
            Pane::Log => "📜 Log",
            Pane::Pipeline => "✏ Pipeline",
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
            Pane::Pipeline => "The pipeline editor: a renderer's graph (✏ graph on a renderer opens it)",
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
    /// Width (left, right) or height (bottom) the user gave it.
    #[serde(default)]
    size: Option<f32>,
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
    /// The pane shown in place of the viewport.
    #[serde(default)]
    middle: Option<Torn>,
    /// The collapsed dock shown over the view while the pointer is on it, and since when the
    /// pointer has been on a collapsed strip.
    #[serde(skip)]
    peek: Option<Side>,
    #[serde(skip)]
    hovered: Option<(Side, f64)>,
    #[serde(skip)]
    saved: bool,
}

impl Default for Docks {
    fn default() -> Self {
        let dock = |panes: &[Pane]| Dock { panes: panes.to_vec(), active: 0, collapsed: false, size: None };
        Self {
            left: dock(&[Pane::Form, Pane::Toml]),
            right: dock(&[Pane::Metrics, Pane::Compare, Pane::Benchmark, Pane::Schemes, Pane::Keys]),
            bottom: dock(&[Pane::Log]),
            windows: vec![Torn { pane: Pane::Pipeline, back: Side::Bottom }],
            middle: None,
            peek: None,
            hovered: None,
            saved: true,
        }
    }
}

enum Action {
    Select(Side, usize),
    Move(Pane, Side),
    Tear(Pane),
    Middle(Pane),
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

    /// Takes `p` out of its dock, window or the middle; returns the dock it belongs to.
    fn remove(&mut self, p: Pane) -> Side {
        if let Some(i) = self.windows.iter().position(|t| t.pane == p) {
            return self.windows.remove(i).back;
        }
        if let Some(t) = self.middle.take_if(|t| t.pane == p) {
            return t.back;
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

    /// `p` in place of the viewport (a pane there before goes back to its dock).
    pub(super) fn set_middle(&mut self, p: Pane) {
        if let Some(t) = self.middle.take() {
            self.dock_mut(t.back).panes.push(t.pane);
        }
        let back = self.remove(p);
        self.middle = Some(Torn { pane: p, back });
        self.saved = false;
    }

    /// The pane in place of the viewport.
    pub(super) fn middle(&self) -> Option<Pane> {
        self.middle.map(|t| t.pane)
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

    /// Applies `pane:side`, `pane:window`, `pane:middle` and `side:collapsed` items, comma-separated.
    #[cfg(test)]
    pub(super) fn arrange(&mut self, spec: &str) -> anyhow::Result<()> {
        let name = |s: &str| toml::Value::String(s.to_string());
        for item in spec.split(',').filter(|s| !s.is_empty()) {
            let (a, b) = item.split_once(':').ok_or_else(|| anyhow::anyhow!("{item}: expected a:b"))?;
            if b == "collapsed" {
                self.dock_mut(Side::deserialize(name(a))?).collapsed = true;
            } else if b == "window" {
                self.tear(Pane::deserialize(name(a))?);
            } else if b == "middle" {
                self.set_middle(Pane::deserialize(name(a))?);
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
        self.middle = self.middle.filter(|t| seen.insert(t.pane));
        let default = Docks::default();
        for p in Pane::ALL.into_iter().filter(|p| !seen.contains(p)) {
            match default.windows.iter().find(|t| t.pane == p) {
                Some(&t) => self.windows.push(t),
                None => self.dock_mut(default.side_of(p).unwrap_or(Side::Right)).panes.push(p),
            }
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

/// How long the pointer rests on a collapsed dock's strip before the dock shows over the view.
const PEEK_DELAY: f64 = 0.3;

impl Side {
    fn default_size(self) -> f32 {
        match self {
            Side::Left => 400.0,
            Side::Right => 430.0,
            Side::Bottom => 110.0,
        }
    }
}

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
            let size = d.size.unwrap_or(side.default_size());
            let id = egui::Id::new(("dock", side, thin));
            let panel = match side {
                Side::Left => egui::Panel::left(id),
                Side::Right => egui::Panel::right(id),
                Side::Bottom => egui::Panel::bottom(id),
            };
            let panel = if thin { panel.exact_size(STRIP).resizable(false) } else { panel.resizable(true).default_size(size) };
            let shown = panel.show(ui, |ui| if thin { strip(ui, side, &mut self.docks) } else { self.dock_ui(ui, side, false) });
            let rect = shown.response.rect;
            let mut action = shown.inner;
            if !thin {
                let now = if side == Side::Bottom { rect.height() } else { rect.width() };
                if (now - size).abs() > 0.5 {
                    self.docks.dock_mut(side).size = Some(now);
                    self.docks.saved = false;
                }
            } else if self.docks.peek == Some(side) {
                action = self.peek(ui.ctx(), side, rect, size).or(action);
            }
            self.apply(action);
        }
    }

    /// A collapsed dock shown over the view beside its strip, until the pointer leaves both.
    fn peek(&mut self, ctx: &egui::Context, side: Side, strip: egui::Rect, size: f32) -> Option<Action> {
        let area = match side {
            Side::Left => egui::Rect::from_min_size(strip.min, egui::vec2(size, strip.height())),
            Side::Right => egui::Rect::from_min_max(egui::pos2(strip.max.x - size, strip.min.y), strip.max),
            Side::Bottom => egui::Rect::from_min_max(egui::pos2(strip.min.x, strip.max.y - size), strip.max),
        };
        let shown = egui::Area::new(egui::Id::new(("dock-peek", side))).order(egui::Order::Foreground).fixed_pos(area.min).constrain(false).show(ctx, |ui| {
            egui::Frame::side_top_panel(ui.style()).show(ui, |ui| {
                let inner = area.shrink(8.0).size();
                ui.set_min_size(inner);
                ui.set_max_size(inner);
                self.dock_ui(ui, side, true)
            })
        });
        let hold = egui::DragAndDrop::has_any_payload(ctx) || egui::Popup::is_any_open(ctx) || ctx.input(|i| i.pointer.any_down());
        let inside = ctx.pointer_latest_pos().is_some_and(|p| area.union(strip).expand(4.0).contains(p));
        if !inside && !hold {
            self.docks.peek = None;
        }
        shown.inner.inner
    }

    fn apply(&mut self, action: Option<Action>) {
        match action {
            Some(Action::Select(s, i)) => {
                self.docks.dock_mut(s).active = i;
                self.docks.saved = false;
            }
            Some(Action::Move(p, s)) => self.docks.move_to(p, s),
            Some(Action::Tear(p)) => self.docks.tear(p),
            Some(Action::Middle(p)) => self.docks.set_middle(p),
            Some(Action::Collapse(s, on)) => {
                self.docks.dock_mut(s).collapsed = on;
                self.docks.peek = None;
                self.docks.saved = false;
            }
            None => {}
        }
    }

    /// A dock's tab bar and shown pane; `peek`: shown over the view while collapsed.
    fn dock_ui(&mut self, ui: &mut egui::Ui, side: Side, peek: bool) -> Option<Action> {
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
                    if p == Pane::Pipeline && ui.button("⛶ Show in the middle, in place of the view").clicked() {
                        action = Some(Action::Middle(p));
                    }
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if peek {
                    if ui.small_button("📌").on_hover_text("Keep this dock open").clicked() {
                        action = Some(Action::Collapse(side, false));
                    }
                } else {
                    let arrow = match side {
                        Side::Left => "◀",
                        Side::Right => "▶",
                        Side::Bottom => "⏷",
                    };
                    if ui.small_button(arrow).on_hover_text("Collapse this dock to a strip at the edge (hovering the strip shows it again)").clicked() {
                        action = Some(Action::Collapse(side, true));
                    }
                }
                if let Some(&p) = d.panes.get(d.active) {
                    if ui.small_button("↗").on_hover_text(format!("Open {} in a window of its own", p.title())).clicked() {
                        action = Some(Action::Tear(p));
                    }
                    if p == Pane::Pipeline && ui.small_button("⛶").on_hover_text("Show the pipeline editor in the middle, in place of the view").clicked() {
                        action = Some(Action::Middle(p));
                    }
                }
            });
        });
        ui.separator();
        if let Some(&p) = d.panes.get(d.active) {
            self.pane(ui, p);
        }
        drop_zone(ui, area, side).or(action)
    }

    /// The panes in OS windows of their own (closing one docks it again; closing the
    /// pipeline editor's window closes the editor). The pipeline's window is there only while
    /// the editor is open.
    pub(super) fn pane_windows(&mut self, ctx: &egui::Context) {
        for t in self.docks.windows.clone() {
            if t.pane == Pane::Pipeline && self.graph_win.is_none() {
                continue;
            }
            let size = match t.pane {
                Pane::Log => [900.0, 260.0],
                Pane::Pipeline => [1400.0, 640.0],
                _ => [520.0, 760.0],
            };
            let vb = egui::ViewportBuilder::default().with_title(format!("MultiRenderer — {}", t.pane.title())).with_inner_size(size);
            let mut back = false;
            let mut closed = false;
            ctx.show_viewport_immediate(t.pane.viewport(), vb, |ui, _| {
                closed |= ui.ctx().input(|i| i.viewport().close_requested());
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(t.pane.title());
                        if ui.button("↙ Dock").on_hover_text(format!("Back into the {} dock", t.back.name())).clicked() {
                            back = true;
                        }
                        if t.pane == Pane::Pipeline && ui.button("⛶ Middle").on_hover_text("Show the editor in the main window, in place of the view").clicked() {
                            self.docks.set_middle(t.pane);
                        }
                    });
                    ui.separator();
                    self.pane(ui, t.pane);
                });
            });
            if closed && t.pane == Pane::Pipeline {
                self.graph_win = None;
            } else if back || closed {
                self.docks.move_to(t.pane, t.back);
            }
        }
    }

    /// Whether the middle shows a pane instead of the viewport (the pipeline editor only while
    /// it is open).
    pub(super) fn middle_pane(&self) -> Option<Pane> {
        self.docks.middle().filter(|&p| p != Pane::Pipeline || self.graph_win.is_some())
    }

    /// The pane in place of the viewport, with buttons to send it back.
    pub(super) fn middle(&mut self, ui: &mut egui::Ui, p: Pane) {
        let back = self.docks.middle.map_or(Side::Bottom, |t| t.back);
        ui.horizontal(|ui| {
            ui.strong(p.title());
            if ui.button("↙ Dock").on_hover_text(format!("Into the {} dock; the view comes back", back.name())).clicked() {
                self.docks.move_to(p, back);
            }
            if ui.button("↗ Own window").on_hover_text("In an OS window of its own; the view comes back").clicked() {
                self.docks.tear(p);
            }
        });
        ui.separator();
        self.pane(ui, p);
    }

    /// After the central panel: a tab dropped over the view gets a window of its own; the
    /// layout is saved when it changed (not while a dock is being resized).
    pub(super) fn finish_docks(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.any_released())
            && let Some(p) = egui::DragAndDrop::take_payload::<Pane>(ctx)
        {
            self.docks.tear(*p);
        }
        if !ctx.input(|i| i.pointer.any_down()) {
            self.docks.save();
        }
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
            Pane::Pipeline => self.pipeline(ui),
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

/// A collapsed or (while a tab is dragged) empty dock: hover to look in, click to open, drop a
/// tab to add it.
fn strip(ui: &mut egui::Ui, side: Side, docks: &mut Docks) -> Option<Action> {
    let area = ui.max_rect();
    let panes = &docks.dock(side).panes;
    let r = ui.allocate_rect(area, Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(area, 2.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let names: Vec<&str> = panes.iter().map(|p| p.title()).collect();
    let r = r.on_hover_text(if names.is_empty() { format!("The {} dock: drop a tab here", side.name()) } else { format!("{} (click to keep it open)", names.join(", ")) });
    let open = (r.clicked() && !panes.is_empty()).then_some(Action::Collapse(side, false));
    let dragging = egui::DragAndDrop::has_any_payload(ui.ctx());
    if r.hovered() && !panes.is_empty() && !dragging {
        let now = ui.input(|i| i.time);
        match docks.hovered {
            Some((s, since)) if s == side => {
                if now - since >= PEEK_DELAY {
                    docks.peek = Some(side);
                } else {
                    ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(PEEK_DELAY));
                }
            }
            _ => {
                docks.hovered = Some((side, now));
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(PEEK_DELAY));
            }
        }
    } else if docks.hovered.is_some_and(|(s, _)| s == side) {
        docks.hovered = None;
    }
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
        let mut v: Vec<Pane> = Side::ALL.into_iter().flat_map(|s| d.dock(s).panes.clone()).chain(d.windows.iter().map(|t| t.pane)).chain(d.middle()).collect();
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
        assert_eq!(d.windows.last(), Some(&Torn { pane: Pane::Compare, back: Side::Bottom }));
        d.move_to(Pane::Compare, Side::Bottom);
        assert!(!d.windows.iter().any(|t| t.pane == Pane::Compare) && d.bottom.panes.contains(&Pane::Compare));
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
        assert_eq!(fixed.windows, vec![Torn { pane: Pane::Pipeline, back: Side::Bottom }], "a pane missing from an older layout goes where it starts");
        let mut sized = Docks::default();
        sized.left.size = Some(512.0);
        let back: Docks = toml::from_str(&toml::to_string(&sized).unwrap()).unwrap();
        assert_eq!(back.left.size, Some(512.0));
    }

    #[test]
    fn the_pipeline_takes_the_middle_and_goes_back() {
        let mut d = Docks::default();
        d.set_middle(Pane::Pipeline);
        assert!(d.windows.is_empty() && d.middle() == Some(Pane::Pipeline));
        d.move_to(Pane::Pipeline, Side::Right);
        assert_eq!((d.middle(), d.right.panes.last()), (None, Some(&Pane::Pipeline)));
        d.set_middle(Pane::Pipeline);
        d.set_middle(Pane::Schemes);
        assert_eq!(d.middle(), Some(Pane::Schemes));
        assert!(d.right.panes.contains(&Pane::Pipeline), "the pane in the middle before goes back to its dock");
        assert_eq!(all(&d), Pane::ALL);
    }
}
