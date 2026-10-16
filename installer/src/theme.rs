//! Dark installer palette and animation helpers (mirrors the Silo app look).

use egui::{Color32, Pos2, Rect};

pub const BG: Color32 = Color32::from_rgb(15, 15, 17);
pub const FOOTER: Color32 = Color32::from_rgb(20, 20, 23);
pub const HEADER_TOP: Color32 = Color32::from_rgb(28, 28, 31);
pub const HEADER_BOTTOM: Color32 = Color32::from_rgb(19, 19, 21);
pub const FIELD: Color32 = Color32::from_rgb(30, 30, 34);
pub const ELEVATED: Color32 = Color32::from_rgb(40, 40, 45);
pub const BORDER: Color32 = Color32::from_rgb(53, 53, 60);
pub const HAIRLINE: Color32 = Color32::from_rgb(44, 44, 50);
pub const TEXT: Color32 = Color32::from_rgb(243, 243, 246);
pub const DIM: Color32 = Color32::from_rgb(158, 158, 167);
pub const ACCENT: Color32 = Color32::from_rgb(10, 132, 255);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(42, 154, 255);
pub const SUCCESS: Color32 = Color32::from_rgb(48, 209, 88);
pub const ERROR: Color32 = Color32::from_rgb(255, 69, 58);

/// Linear interpolation between two colors (including alpha).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        ch(a.r(), b.r()),
        ch(a.g(), b.g()),
        ch(a.b(), b.b()),
        ch(a.a(), b.a()),
    )
}

/// Same color with scaled alpha.
pub fn fade(c: Color32, alpha: f32) -> Color32 {
    let a = (c.a() as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Overshoot easing for "pop" animations (peaks slightly above 1).
pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    let t = t.clamp(0.0, 1.0);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Vertical gradient approximated with horizontal bands.
pub fn gradient_v(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32, bands: usize) {
    if rect.height() < 1.0 {
        return;
    }
    let band_h = rect.height() / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1).max(1) as f32;
        let y0 = rect.min.y + band_h * i as f32;
        let y1 = if i + 1 == bands { rect.max.y } else { y0 + band_h };
        let strip = Rect::from_min_max(Pos2::new(rect.min.x, y0), Pos2::new(rect.max.x, y1));
        painter.rect_filled(strip, 0.0, mix(top, bottom, t));
    }
}

/// Shorten long paths for display: `C:\very\long…\tail`.
pub fn truncate_middle(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let head = keep * 2 / 3;
    let tail = keep - head;
    let start: String = s.chars().take(head).collect();
    let end: String = s.chars().skip(count - tail).collect();
    format!("{start}…{end}")
}
