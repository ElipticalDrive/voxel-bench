//! Colour scales of diagnostic views (`mr_core::Legend`), drawn over a lane's image like
//! a thermal camera's: the ramp with numeric ticks, or the category swatches, plus the
//! value under the cursor.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use mr_core::{Legend, Reading};

/// A scale value, compactly.
pub fn fmt(v: f64) -> String {
    if v >= 9999.5 {
        format!("{:.0}k", v / 1000.0)
    } else if v >= 9.95 || v == v.round() {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

/// The reading of the pixel at `(x, y)` of an RGBA8 frame `w` pixels wide.
pub fn read(legend: &Legend, pixels: &[u8], w: u32, x: u32, y: u32) -> Option<Reading> {
    let i = (y as usize * w as usize + x as usize) * 4;
    legend.read([*pixels.get(i)?, *pixels.get(i + 1)?, *pixels.get(i + 2)?])
}

/// Text for a reading, e.g. `≈ 37 loop iterations (primary ray)`.
pub fn describe(legend: &Legend, r: Reading) -> String {
    let (q, max) = match legend {
        Legend::Ramp { quantity, max, .. } => (quantity.as_str(), *max),
        Legend::Categories(_) => ("", 0.0),
    };
    match r {
        Reading::Value(v) => format!("≈ {} {q}", fmt(v)),
        Reading::Over => format!("> {} {q}", fmt(max)),
    }
}

/// Draws the legend in the bottom-right corner of `tile`, marking `reading` on a ramp.
pub fn draw(painter: &Painter, tile: Rect, legend: &Legend, reading: Option<Reading>) {
    let font = FontId::proportional(12.0);
    let pad = 6.0;
    match legend {
        Legend::Ramp { quantity, log, .. } => {
            let (bar_w, bar_h) = (200.0, 12.0);
            let size = Vec2::new(bar_w + 2.0 * pad + 34.0, 60.0);
            let bg = Rect::from_min_size(tile.right_bottom() - size - Vec2::splat(8.0), size);
            painter.rect_filled(bg, 3.0, Color32::from_black_alpha(190));
            let title = format!("{quantity}{}", if *log { " (log scale)" } else { "" });
            painter.text(bg.min + Vec2::splat(pad), Align2::LEFT_TOP, title, font.clone(), Color32::WHITE);
            let bar = Rect::from_min_size(bg.min + Vec2::new(pad, 24.0), Vec2::new(bar_w, bar_h));
            const STEPS: usize = 64;
            for k in 0..STEPS {
                let [r, g, b] = mr_core::legend::ramp_rgb8((k as f32 + 0.5) / STEPS as f32);
                let x0 = bar.min.x + bar_w * k as f32 / STEPS as f32;
                let seg = Rect::from_min_max(Pos2::new(x0, bar.min.y), Pos2::new(x0 + bar_w / STEPS as f32 + 0.5, bar.max.y));
                painter.rect_filled(seg, 0.0, Color32::from_rgb(r, g, b));
            }
            // Above the scale: white.
            let over = Rect::from_min_size(Pos2::new(bar.max.x + 6.0, bar.min.y), Vec2::new(bar_h, bar_h));
            painter.rect_filled(over, 0.0, Color32::WHITE);
            painter.text(over.right_center() + Vec2::new(3.0, 0.0), Align2::LEFT_CENTER, "over", FontId::proportional(10.0), Color32::LIGHT_GRAY);
            for k in 0..=4 {
                let t = k as f64 / 4.0;
                let x = bar.min.x + bar_w * t as f32;
                painter.line_segment([Pos2::new(x, bar.max.y), Pos2::new(x, bar.max.y + 3.0)], Stroke::new(1.0, Color32::LIGHT_GRAY));
                let align = match k {
                    0 => Align2::LEFT_TOP,
                    4 => Align2::RIGHT_TOP,
                    _ => Align2::CENTER_TOP,
                };
                painter.text(Pos2::new(x, bar.max.y + 4.0), align, fmt(legend.value_at(t)), FontId::proportional(11.0), Color32::LIGHT_GRAY);
            }
            if let Some(r) = reading {
                let x = match r {
                    Reading::Value(v) => bar.min.x + bar_w * legend.position(v).clamp(0.0, 1.0) as f32,
                    Reading::Over => over.center().x,
                };
                let tip = Pos2::new(x, bar.min.y);
                painter.add(egui::Shape::convex_polygon(vec![tip, tip + Vec2::new(-5.0, -7.0), tip + Vec2::new(5.0, -7.0)], Color32::WHITE, Stroke::new(1.0, Color32::BLACK)));
            }
        }
        Legend::Categories(items) => {
            let row = 16.0;
            let galleys: Vec<_> = items.iter().map(|(s, _)| painter.layout_no_wrap(s.clone(), font.clone(), Color32::WHITE)).collect();
            let text_w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
            let size = Vec2::new(text_w + 2.0 * pad + 18.0, items.len() as f32 * row + 2.0 * pad);
            let bg = Rect::from_min_size(tile.right_bottom() - size - Vec2::splat(8.0), size);
            painter.rect_filled(bg, 3.0, Color32::from_black_alpha(190));
            for (k, ((_, c), g)) in items.iter().zip(galleys).enumerate() {
                let y = bg.min.y + pad + k as f32 * row;
                if let Some([r, gr, b]) = c {
                    let sw = Rect::from_min_size(Pos2::new(bg.min.x + pad, y + 2.0), Vec2::splat(11.0));
                    painter.rect_filled(sw, 0.0, Color32::from_rgb(*r, *gr, *b));
                    painter.rect_stroke(sw, 0.0, Stroke::new(1.0, Color32::from_gray(140)), StrokeKind::Inside);
                }
                painter.galley(Pos2::new(bg.min.x + pad + 18.0, y), g, Color32::WHITE);
            }
        }
    }
}
