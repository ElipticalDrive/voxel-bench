//! The graph drawn: nodes in columns from the world's side (left) to the image (right),
//! each column after what it reads; edges coloured by what they carry, the previous frame's
//! dashed. Click a node or an input to inspect it; drag a node to move it, the background to
//! pan, the wheel to zoom (about the pointer); drag from an output onto an input to rebind it.
//! The view fits the graph until the user moves, pans or zooms; a double click fits it again
//! (with auto-frame on, so does releasing a dragged node).

use std::collections::HashMap;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use mr_components::graph::{Data, Device, Graph, When, inputs, output};
use mr_core::RendererInfo;

use super::model::{self, Change, Target, Trial};
use crate::plan::RendererSpec;

const W: f32 = 136.0;
const HEAD: f32 = 36.0;
const ROW: f32 = 17.0;
const GAP_X: f32 = 40.0;
const GAP_Y: f32 = 14.0;

/// What the user did to the view that persists between frames.
#[derive(Default)]
pub struct View {
    offsets: HashMap<String, Vec2>,
    pan: Vec2,
    /// The layout point at the view's corner and the scale, once the user took the view over
    /// (`None`: fitted to the graph every frame).
    frame: Option<(Pos2, f32)>,
    /// The node whose output is being dragged to an input.
    wire: Option<String>,
}

#[derive(Default)]
pub struct Response {
    /// A node selected (`Some(None)`: the selection cleared).
    pub select: Option<Option<String>>,
    pub port: Option<String>,
    /// A rebinding made by dragging a wire.
    pub spec: Option<RendererSpec>,
}

pub fn data_color(d: Data) -> Color32 {
    match d {
        Data::Changes => Color32::from_rgb(130, 130, 140),
        Data::Chunks => Color32::from_rgb(170, 130, 90),
        Data::Storage => Color32::from_rgb(235, 150, 60),
        Data::Faces => Color32::from_rgb(225, 205, 70),
        Data::Starts => Color32::from_rgb(90, 200, 170),
        Data::Rays => Color32::from_rgb(235, 90, 95),
        Data::Hits => Color32::from_rgb(80, 185, 235),
        Data::Surfaces => Color32::from_rgb(200, 120, 230),
        Data::Tiles => Color32::from_rgb(150, 215, 90),
        Data::Image => Color32::from_rgb(235, 235, 235),
    }
}

fn device_color(d: Device) -> Color32 {
    match d {
        Device::Cpu => Color32::from_rgb(90, 140, 230),
        Device::Gpu(0) => Color32::from_rgb(90, 190, 110),
        Device::Gpu(_) => Color32::from_rgb(190, 180, 80),
        Device::Server => Color32::from_rgb(140, 140, 150),
    }
}

fn height(g: &Graph, id: &str) -> f32 {
    let n = &g.nodes[id];
    HEAD + ROW * inputs(n.role, &n.component).len() as f32 + 6.0
}

/// Columns by how far each node is from the world's side over this frame's edges (the
/// world's side in the first); within a column, ordered by where what they read sits.
fn layout(g: &Graph) -> HashMap<String, Pos2> {
    let order = model::order(g);
    let mut col: HashMap<&str, usize> = HashMap::new();
    for id in &order {
        let n = &g.nodes[id];
        let c = if model::world_side(n.role) {
            0
        } else {
            n.inputs.iter().filter(|(p, s)| n.when(p) == When::Now && !model::world_side(g.nodes[*s].role)).filter_map(|(_, s)| col.get(s.as_str())).max().map_or(1, |c| c + 1)
        };
        col.insert(id, c);
    }
    let cols = col.values().max().copied().unwrap_or(0);
    let mut pos: HashMap<String, Pos2> = HashMap::new();
    for c in 0..=cols {
        let mut ids: Vec<&String> = order.iter().filter(|id| col[id.as_str()] == c).collect();
        // Barycentre of the inputs already placed (the world's side keeps reading order).
        let key = |id: &String| -> f32 {
            let n = &g.nodes[id];
            let ys: Vec<f32> = n.inputs.values().filter_map(|s| pos.get(s)).map(|p| p.y).collect();
            if c == 0 || ys.is_empty() { f32::MAX } else { ys.iter().sum::<f32>() / ys.len() as f32 }
        };
        let mut keyed: Vec<(f32, usize, &String)> = ids.drain(..).enumerate().map(|(i, id)| (key(id), i, id)).collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut y = 0.0;
        for (want, _, id) in keyed {
            if want != f32::MAX && want > y {
                y = want;
            }
            pos.insert(id.clone(), pos2(c as f32 * (W + GAP_X), y));
            y += height(g, id) + GAP_Y;
        }
    }
    pos
}

fn bezier(from: Pos2, to: Pos2, s: f32) -> Vec<Pos2> {
    let dx = ((to.x - from.x).abs() * 0.5).max(50.0 * s);
    let c = egui::epaint::CubicBezierShape::from_points_stroke([from, from + vec2(dx, 0.0), to - vec2(dx, 0.0), to], false, Color32::TRANSPARENT, Stroke::NONE);
    c.flatten(Some(0.5))
}

pub fn show(ui: &mut Ui, view: &mut View, auto_frame: bool, g: &Graph, devs: &HashMap<String, Device>, selected: Option<&str>, trial: &mut Trial, infos: &[RendererInfo]) -> Response {
    let mut out = Response::default();
    let rect = ui.available_rect_before_wrap();
    let bg = ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals().clone();
    painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);

    let base = layout(g);
    let offsets = view.offsets.clone();
    let at = |id: &str| base.get(id).copied().unwrap_or_default() + offsets.get(id).copied().unwrap_or_default();
    let (mut lo, mut hi) = (pos2(f32::MAX, f32::MAX), pos2(f32::MIN, f32::MIN));
    for id in g.nodes.keys() {
        let p = at(id);
        lo = lo.min(p);
        hi = hi.max(p + vec2(W, height(g, id)));
    }
    let size = (hi - lo).max(vec2(1.0, 1.0));
    let fit = ((rect.width() - 40.0) / size.x).min((rect.height() - 40.0) / size.y).clamp(0.7, 1.0);
    if bg.double_clicked() {
        (view.frame, view.pan) = (None, Vec2::ZERO);
    }
    let (lo, mut s) = view.frame.unwrap_or((lo, fit));
    let corner = rect.min + vec2(20.0, 20.0);
    if bg.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let z = (s * (1.0 + scroll * 0.002)).clamp(0.3, 2.0);
            // The layout point under the pointer stays there.
            if let Some(c) = ui.input(|i| i.pointer.hover_pos()) {
                let q = (c - corner - view.pan) / s;
                view.pan = c - corner - q * z;
            }
            view.frame = Some((lo, z));
            s = z;
        }
    }
    let origin = corner - lo.to_vec2() * s + view.pan;
    let screen = |p: Pos2| origin + p.to_vec2() * s;
    let font = |px: f32| FontId::proportional(px * s);
    let node_rect = |id: &str| Rect::from_min_size(screen(at(id)), vec2(W, height(g, id)) * s);
    let in_pos = |id: &str, i: usize| screen(at(id)) + vec2(0.0, (HEAD + ROW * (i as f32 + 0.5)) * s);
    let out_pos = |id: &str| screen(at(id)) + vec2(W, HEAD * 0.5) * s;

    if bg.dragged() && view.wire.is_none() {
        view.pan += bg.drag_delta();
        view.frame.get_or_insert((lo, s));
    }
    if bg.clicked() {
        out.select = Some(None);
    }

    // Edges.
    for (id, n) in &g.nodes {
        let ports = inputs(n.role, &n.component);
        for (port, src) in &n.inputs {
            let Some(i) = ports.iter().position(|p| p.name == port) else { continue };
            let Some(m) = g.nodes.get(src) else { continue };
            let d = output(m.role, &m.component);
            let thin = d == Data::Changes || d == Data::Chunks;
            let color = if thin { data_color(d).gamma_multiply(0.45) } else { data_color(d) };
            let pts = bezier(out_pos(src), in_pos(id, i), s);
            let stroke = Stroke::new(if thin { 1.0 } else { 2.0 } * s.max(0.6), color);
            match n.when(port) {
                When::Now => drop(painter.add(Shape::line(pts, stroke))),
                w => {
                    let mid = pts[pts.len() / 2];
                    painter.extend(Shape::dashed_line(&pts, stroke, 6.0 * s, 4.0 * s));
                    let tag = if let When::Iterate(k) = w { format!("×{k}") } else { "prev".into() };
                    painter.text(mid, Align2::CENTER_BOTTOM, tag, font(10.0), color);
                }
            }
        }
    }

    // Nodes.
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_port: Option<(String, String)> = None;
    let mut moved: Vec<(String, Vec2)> = vec![];
    let mut refit = false;
    for id in model::order(g) {
        let n = &g.nodes[&id];
        let r = node_rect(&id);
        let sel = selected == Some(id.as_str());
        let resp = ui.interact(r, ui.id().with(("node", &id)), Sense::click_and_drag());
        let fill = visuals.widgets.noninteractive.bg_fill;
        painter.rect_filled(r, 5.0 * s, fill);
        let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD * s));
        painter.rect_filled(head, egui::CornerRadius { nw: (5.0 * s) as u8, ne: (5.0 * s) as u8, sw: 0, se: 0 }, data_color(output(n.role, &n.component)).gamma_multiply(0.28));
        let stroke = if sel { Stroke::new(2.0, visuals.selection.stroke.color) } else if resp.hovered() { Stroke::new(1.5, visuals.widgets.hovered.fg_stroke.color) } else { Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color) };
        painter.rect_stroke(r, 5.0 * s, stroke, StrokeKind::Inside);
        painter.text(r.min + vec2(7.0, 4.0) * s, Align2::LEFT_TOP, &id, font(10.0), visuals.weak_text_color());
        painter.text(r.min + vec2(7.0, 17.0) * s, Align2::LEFT_TOP, &n.component, FontId::proportional(13.0 * s), visuals.strong_text_color());
        // Where it runs (fused nodes: with their callers).
        if let Some(d) = devs.get(&id) {
            let fused = output(n.role, &n.component).fused();
            let label = match (fused, n.api) {
                (true, _) => format!("({d})"),
                (false, Some(a)) => format!("{d} {}", a.name()),
                (false, None) => d.to_string(),
            };
            let tr = r.right_top() + vec2(-6.0, 5.0) * s;
            let tr_rect = painter.text(tr, Align2::RIGHT_TOP, &label, font(10.0), if fused { visuals.weak_text_color() } else { device_color(*d) });
            if n.device.is_some() && !fused {
                painter.rect_stroke(tr_rect.expand(2.0 * s), 3.0 * s, Stroke::new(1.0, device_color(*d)), StrokeKind::Outside);
            }
        }
        // Inputs.
        for (i, p) in inputs(n.role, &n.component).iter().enumerate() {
            let c = in_pos(&id, i);
            let bound = n.inputs.contains_key(p.name);
            let col = data_color(p.data);
            if bound {
                painter.circle_filled(c, 4.0 * s, col);
            } else {
                painter.circle_stroke(c, 4.0 * s, Stroke::new(1.0, col.gamma_multiply(0.6)));
            }
            let tc = if bound { visuals.text_color() } else { visuals.weak_text_color() };
            painter.text(c + vec2(9.0 * s, 0.0), Align2::LEFT_CENTER, p.name, font(11.0), tc);
            if n.when(p.name) != When::Now {
                painter.text(pos2(r.right() - 6.0 * s, c.y), Align2::RIGHT_CENTER, n.when(p.name).label(), font(10.0), visuals.weak_text_color());
            }
            let row = Rect::from_min_max(pos2(r.left() - 6.0 * s, c.y - ROW * 0.5 * s), pos2(r.right(), c.y + ROW * 0.5 * s));
            let pr = ui.interact(row, ui.id().with(("port", &id, p.name)), Sense::click());
            if pr.clicked() {
                out.select = Some(Some(id.clone()));
                out.port = Some(p.name.to_string());
            }
            if pointer.is_some_and(|q| row.contains(q)) {
                hovered_port = Some((id.clone(), p.name.to_string()));
            }
            let pr = pr.on_hover_text(format!("{}: {} ({}{})", p.name, p.description, p.data.name(), if p.required { "" } else { ", optional" }));
            drop(pr);
        }
        // The output, with how many read it.
        let o = out_pos(&id);
        let d = output(n.role, &n.component);
        let readers = model::readers(g, &id);
        if !readers.is_empty() || d == Data::Image {
            painter.circle_filled(o, 5.0 * s, data_color(d));
            if readers.len() > 1 {
                painter.text(o + vec2(7.0, -7.0) * s, Align2::LEFT_BOTTOM, format!("×{}", readers.len()), font(10.0), visuals.weak_text_color());
            }
        }
        let orect = Rect::from_center_size(o, vec2(16.0, 16.0) * s);
        let or = ui.interact(orect, ui.id().with(("out", &id)), Sense::drag());
        if or.drag_started() {
            view.wire = Some(id.clone());
        }
        let or = or.on_hover_text(format!("{}: drag onto an input to bind it there", d.name()));
        drop(or);
        if resp.clicked() {
            out.select = Some(Some(id.clone()));
            out.port = None;
        }
        if resp.dragged() && view.wire.is_none() {
            moved.push((id.clone(), resp.drag_delta() / s));
        }
        if resp.drag_stopped() && auto_frame {
            refit = true;
        }
        resp.on_hover_ui(|ui| {
            ui.label(egui::RichText::new(format!("{id}: {} {}", n.role.name(), n.component)).strong());
            ui.label(super::description(n.role, &n.component));
            if let Some(e) = crate::gui::schemes::of(n.role.name(), &n.component) {
                crate::gui::schemes::hover(ui, e);
            }
            if let Some(st) = mr_components::graph::state(n.role, &n.component) {
                ui.weak(format!("Keeps: {st}"));
            }
            let rd: Vec<String> = model::readers(g, &id).into_iter().map(|(r, p)| format!("{r}.{p}")).collect();
            if !rd.is_empty() {
                ui.weak(format!("Read by: {}", rd.join(", ")));
            }
        });
    }

    // Moving a node keeps the view where it is (refitting would move and zoom everything).
    if !moved.is_empty() {
        view.frame.get_or_insert((lo, s));
    }
    for (id, d) in moved {
        *view.offsets.entry(id).or_default() += d;
    }
    if refit {
        (view.frame, view.pan) = (None, Vec2::ZERO);
    }

    // A wire being dragged: inputs of its type light up, green where binding it runs.
    if let Some(src) = view.wire.clone() {
        if !g.nodes.contains_key(&src) {
            view.wire = None;
        } else {
            let d = output(g.nodes[&src].role, &g.nodes[&src].component);
            for (id, n) in &g.nodes {
                for (i, p) in inputs(n.role, &n.component).iter().enumerate() {
                    if p.data == d && *id != src && p.data != Data::Changes {
                        painter.circle_stroke(in_pos(id, i), 7.0 * s, Stroke::new(1.5, data_color(d)));
                    }
                }
            }
            let end = pointer.unwrap_or(out_pos(&src));
            let mut verdict = None;
            if let Some((id, port)) = hovered_port.clone() {
                let n = &g.nodes[&id];
                let fits = inputs(n.role, &n.component).iter().find(|p| p.name == port).is_some_and(|p| p.data == d && p.data != Data::Changes) && id != src;
                verdict = Some(if !fits {
                    Err(format!("`{port}` takes {}, not {}", inputs(n.role, &n.component).iter().find(|p| p.name == port).map_or("-", |p| p.data.name()), d.name()))
                } else if n.inputs.get(&port) == Some(&src) {
                    Err("already bound there".into())
                } else {
                    trial.try_change(&Change::Bind(id.clone(), port.clone(), Target::Node(src.clone())), infos).map(|(_, spec)| (id, port, spec))
                });
            }
            let color = match &verdict {
                Some(Ok(_)) => Color32::from_rgb(90, 210, 110),
                Some(Err(_)) => Color32::from_rgb(230, 80, 80),
                None => data_color(d),
            };
            painter.add(Shape::line(bezier(out_pos(&src), end, s), Stroke::new(2.5, color)));
            if let Some(v) = &verdict {
                let text = match v {
                    Ok((_, _, spec)) => format!("runs as {}: {}", spec.name, spec.label()),
                    Err(e) => format!("cannot run: {e}"),
                };
                let galley = painter.layout(text, FontId::proportional(12.0), color, 420.0);
                let at = end + vec2(14.0, 10.0);
                painter.rect_filled(Rect::from_min_size(at, galley.size()).expand(4.0), 4.0, visuals.window_fill);
                painter.galley(at, galley, color);
            }
            if ui.input(|i| i.pointer.any_released()) {
                if let Some(Ok((_, _, spec))) = verdict {
                    out.spec = Some(spec);
                }
                view.wire = None;
            }
        }
    }
    // A legend of the edge colours.
    let mut x = rect.left() + 8.0;
    let y = rect.bottom() - 12.0;
    for d in [Data::Storage, Data::Rays, Data::Faces, Data::Hits, Data::Tiles, Data::Surfaces, Data::Starts, Data::Image, Data::Changes] {
        painter.circle_filled(pos2(x + 4.0, y), 4.0, data_color(d));
        let r = painter.text(pos2(x + 11.0, y), Align2::LEFT_CENTER, d.name(), FontId::proportional(11.0), visuals.weak_text_color());
        x = r.right() + 12.0;
    }
    out
}
