//! Lightweight vector icons drawn with the painter, SF-Symbols-like.
//! No font glyph dependencies, so they render identically in any language.

use eframe::egui;
use egui::{Color32, Pos2, Rect, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Home,
    Card,
    Keypad,
    Sliders,
    Help,
    Sun,
    Moon,
    Image,
    Package,
    ChevronRight,
    Download,
    Monitor,
}

/// Signed angular delta from `from` to `to` along the direction that
/// passes through `through`.
fn sweep_through(from: f32, to: f32, through: f32) -> f32 {
    const TAU: f32 = std::f32::consts::TAU;
    let norm = |a: f32| a.rem_euclid(TAU);
    let start = norm(from);
    let full = (norm(to) - start).rem_euclid(TAU);
    let via = (norm(through) - start).rem_euclid(TAU);
    if via <= full { full } else { full - TAU }
}

/// The crescent for [`Icon::Moon`] as two arcs — `(outer, inner)` — both
/// running from tip p1 to tip p2. The outer arc lies on the moon's circle;
/// the inner arc is a bite from a LARGER offset circle (equal radii would
/// only clip a shallow, nearly straight edge). Each arc has the same number
/// of points so callers can build a triangle strip between them.
fn moon_arcs(center: Pos2, radius: f32) -> (Vec<Pos2>, Vec<Pos2>) {
    let bite_radius = radius * 1.30;
    let offset = radius * 0.95; // bite depth, from the upper right
    let axis = -0.72f32; // ~-41 degrees
    let (ax, ay) = (axis.cos(), axis.sin());
    // Distance from the center to the chord between the tips.
    let foot_dist =
        (offset * offset + radius * radius - bite_radius * bite_radius) / (2.0 * offset);
    let half = (radius * radius - foot_dist * foot_dist).max(0.0).sqrt();
    let foot = vec2(center.x + ax * foot_dist, center.y + ay * foot_dist);
    let perp = vec2(-ay, ax);
    let p1 = pos2(foot.x + perp.x * half, foot.y + perp.y * half);
    let p2 = pos2(foot.x - perp.x * half, foot.y - perp.y * half);
    let c2 = pos2(center.x + ax * offset, center.y + ay * offset);
    // Both arcs bend around the point opposite the bite.
    let far = axis + std::f32::consts::PI;
    let steps = 20;
    let ang = |p: Pos2, c: Pos2| (p.y - c.y).atan2(p.x - c.x);

    let start1 = ang(p1, center);
    let sweep1 = sweep_through(start1, ang(p2, center), far);
    let outer: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start1 + sweep1 * i as f32 / steps as f32;
            pos2(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect();

    let start2 = ang(p1, c2);
    let sweep2 = sweep_through(start2, ang(p2, c2), far);
    let inner: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start2 + sweep2 * i as f32 / steps as f32;
            pos2(c2.x + bite_radius * a.cos(), c2.y + bite_radius * a.sin())
        })
        .collect();

    (outer, inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_arcs_form_a_concave_crescent() {
        let (outer, inner) = moon_arcs(Pos2::ZERO, 6.0);
        assert_eq!(outer.len(), inner.len());
        let dist = |p: Pos2| (p.x * p.x + p.y * p.y).sqrt();
        // Outer arc sits on the circle; inner arc is the bite.
        let max_d = outer.iter().map(|p| dist(*p)).fold(0.0_f32, f32::max);
        assert!((max_d - 6.0).abs() < 0.01, "max {max_d}");
        let min_d = inner.iter().map(|p| dist(*p)).fold(f32::MAX, f32::min);
        assert!(min_d < 3.0, "min {min_d} - crescent is too shallow");
        // Tips are shared: both arcs start and end at the same points.
        assert!((outer[0] - inner[0]).length() < 0.01, "tips must meet");
        assert!(
            (*outer.last().unwrap() - *inner.last().unwrap()).length() < 0.01,
            "tips must meet"
        );
        // And the shared tips lie exactly on the outer circle.
        assert!((dist(outer[0]) - 6.0).abs() < 0.01);
    }
}

/// Draw `icon` centered inside `rect`.
pub fn draw(ui: &egui::Ui, icon: Icon, rect: Rect, color: Color32) {
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let s = rect.width().min(rect.height());
    let stroke = egui::Stroke::new((s * 0.09).clamp(1.2, 2.0), color);
    let r = s * 0.42;

    match icon {
        Icon::Home => {
            let left = center.x - r;
            let right = center.x + r;
            let top = center.y - r;
            let mid = center.y - r * 0.1;
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos2(left + s * 0.04, mid),
                    pos2(center.x, top),
                    pos2(right - s * 0.04, mid),
                ],
                stroke,
            )));
            painter.rect_stroke(
                Rect::from_center_size(pos2(center.x, center.y + r * 0.28), vec2(s * 0.62, s * 0.44)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Card => {
            let rect_card = Rect::from_center_size(center, vec2(s * 0.86, s * 0.62));
            painter.rect_stroke(rect_card, s * 0.1, stroke, egui::StrokeKind::Inside);
            let band_y = rect_card.min.y + s * 0.17;
            painter.line_segment(
                [
                    pos2(rect_card.min.x, band_y),
                    pos2(rect_card.max.x, band_y),
                ],
                stroke,
            );
            painter.circle_filled(
                pos2(rect_card.max.x - s * 0.16, rect_card.max.y - s * 0.15),
                s * 0.055,
                color,
            );
        }
        Icon::Keypad => {
            let dot = (s * 0.055).max(1.3);
            for row in 0..4 {
                for col in 0..3 {
                    let x = center.x + (col as f32 - 1.0) * s * 0.30;
                    let y = center.y + (row as f32 - 1.5) * s * 0.26;
                    painter.circle_filled(pos2(x, y), dot, color);
                }
            }
        }
        Icon::Sliders => {
            for (idx, knob) in [0.30, 0.62, 0.44].into_iter().enumerate() {
                let y = center.y + (idx as f32 - 1.0) * s * 0.30;
                painter.line_segment(
                    [pos2(center.x - r, y), pos2(center.x + r, y)],
                    egui::Stroke::new(stroke.width * 0.8, color),
                );
                painter.circle_filled(
                    pos2(center.x - r + knob * 2.0 * r, y),
                    s * 0.085,
                    color,
                );
            }
        }
        Icon::Help => {
            painter.circle_stroke(center, r, stroke);
            painter.text(
                pos2(center.x, center.y + s * 0.02),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(s * 0.72),
                color,
            );
        }
        Icon::Sun => {
            painter.circle_filled(center, r * 0.46, color);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let inner = r * 0.68;
                let outer = r * 1.0;
                painter.line_segment(
                    [
                        pos2(center.x + inner * a.cos(), center.y + inner * a.sin()),
                        pos2(center.x + outer * a.cos(), center.y + outer * a.sin()),
                    ],
                    egui::Stroke::new(stroke.width * 0.9, color),
                );
            }
        }
        Icon::Moon => {
            // Triangle strip between the two arcs: epaint's closed-path
            // fill triangulates concave shapes incorrectly at this size.
            let (outer, inner) = moon_arcs(center, r * 0.92);
            let n = outer.len();
            if n >= 2 {
                let mut mesh = egui::epaint::Mesh::default();
                for i in 0..n {
                    for pos in [outer[i], inner[i]] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color,
                        });
                    }
                }
                for i in 0..(n - 1) as u32 {
                    let a = i * 2;
                    // (outer i, outer i+1, inner i) + (inner i, outer i+1, inner i+1)
                    mesh.indices.extend_from_slice(&[a, a + 2, a + 1, a + 1, a + 2, a + 3]);
                }
                painter.add(egui::Shape::Mesh(std::sync::Arc::new(mesh)));
            }
        }
        Icon::Image => {
            let rect_img = Rect::from_center_size(center, vec2(s * 0.82, s * 0.64));
            painter.rect_stroke(rect_img, s * 0.1, stroke, egui::StrokeKind::Inside);
            painter.circle_filled(
                pos2(rect_img.min.x + s * 0.20, rect_img.min.y + s * 0.18),
                s * 0.06,
                color,
            );
            let base_y = rect_img.max.y - s * 0.14;
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos2(rect_img.min.x + s * 0.08, base_y),
                    pos2(center.x - s * 0.05, center.y + s * 0.06),
                    pos2(center.x + s * 0.10, center.y - s * 0.04),
                    pos2(rect_img.max.x - s * 0.08, base_y),
                ],
                stroke,
            )));
        }
        Icon::Package => {
            let rect_box = Rect::from_center_size(center, vec2(s * 0.78, s * 0.60));
            painter.rect_stroke(rect_box, s * 0.08, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    pos2(center.x, rect_box.min.y),
                    pos2(center.x, rect_box.max.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(rect_box.min.x, center.y + s * 0.02),
                    pos2(rect_box.max.x, center.y + s * 0.02),
                ],
                stroke,
            );
        }
        Icon::ChevronRight => {
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos2(center.x - r * 0.35, center.y - r * 0.6),
                    pos2(center.x + r * 0.35, center.y),
                    pos2(center.x - r * 0.35, center.y + r * 0.6),
                ],
                egui::Stroke::new(stroke.width * 1.1, color),
            )));
        }
        Icon::Download => {
            // Down arrow into a tray: the classic "get update" glyph.
            let top = center.y - r * 0.95;
            let tip = center.y + r * 0.28;
            painter.line_segment(
                [pos2(center.x, top), pos2(center.x, tip)],
                stroke,
            );
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos2(center.x - r * 0.46, tip - r * 0.5),
                    pos2(center.x, tip),
                    pos2(center.x + r * 0.46, tip - r * 0.5),
                ],
                stroke,
            )));
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    pos2(center.x - r * 0.9, center.y + r * 0.35),
                    pos2(center.x - r * 0.9, center.y + r * 0.92),
                    pos2(center.x + r * 0.9, center.y + r * 0.92),
                    pos2(center.x + r * 0.9, center.y + r * 0.35),
                ],
                egui::Stroke::new(stroke.width * 0.9, color),
            )));
        }
        Icon::Monitor => {
            // A display with a small stand, for the "follow system" theme.
            let screen = Rect::from_center_size(
                pos2(center.x, center.y - r * 0.18),
                vec2(s * 0.84, s * 0.58),
            );
            painter.rect_stroke(screen, s * 0.08, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    pos2(center.x, screen.max.y),
                    pos2(center.x, screen.max.y + s * 0.16),
                ],
                egui::Stroke::new(stroke.width * 0.9, color),
            );
            painter.line_segment(
                [
                    pos2(center.x - s * 0.22, screen.max.y + s * 0.16),
                    pos2(center.x + s * 0.22, screen.max.y + s * 0.16),
                ],
                egui::Stroke::new(stroke.width * 0.9, color),
            );
        }
    }
}

/// Small status dot used across the shell.
pub fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, color);
}

/// Status dot with a gentle breathing halo for live states (connected,
/// scanning, flashing). Requests repaints while it is on screen.
pub fn status_dot_pulsing(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
    let now = ui.ctx().input(|i| i.time);
    let wave = (0.5 + 0.5 * (now * 2.2).sin()) as f32; // 0..1 breathing
    let painter = ui.painter_at(rect);
    let c = rect.center();
    let halo = Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (20.0 + 50.0 * (1.0 - wave)) as u8,
    );
    painter.circle_filled(c, 3.5 + wave * 2.6, halo);
    painter.circle_filled(c, 3.5, color);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}
