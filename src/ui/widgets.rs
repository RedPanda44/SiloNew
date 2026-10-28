//! Shared Apple-style components: cards with soft elevation, capsule
//! buttons, toggle switches, segmented controls and sidebar navigation.

use eframe::egui;
use egui::{Color32, Pos2, Rect, vec2};

use crate::ui::AppTab;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, Palette};

pub fn pal(ui: &egui::Ui) -> Palette {
    theme::palette(ui.ctx())
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Large page title (macOS large-title style).
#[allow(dead_code)]
pub fn title(ui: &mut egui::Ui, text: &str) {
    let p = pal(ui);
    ui.label(
        egui::RichText::new(text)
            .family(theme::strong())
            .size(24.0)
            .color(p.text),
    );
}

pub fn heading(ui: &mut egui::Ui, text: &str) {
    let p = pal(ui);
    ui.label(
        egui::RichText::new(text)
            .family(theme::strong())
            .size(16.0)
            .color(p.text),
    );
}

#[allow(dead_code)]
pub fn body(ui: &mut egui::Ui, text: &str) {
    let p = pal(ui);
    ui.label(egui::RichText::new(text).size(13.0).color(p.text_secondary));
}

pub fn caption(ui: &mut egui::Ui, text: &str) {
    let p = pal(ui);
    ui.label(egui::RichText::new(text).size(11.0).color(p.text_secondary));
}

/// Small uppercase section label, like macOS inspector headers.
pub fn section_header(ui: &mut egui::Ui, text: &str) {
    let p = pal(ui);
    ui.label(
        egui::RichText::new(text)
            .family(theme::strong())
            .size(10.5)
            .color(p.text_tertiary),
    );
}

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// A soft elevated card, the primary content container.
pub fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.card)
        .stroke(egui::Stroke::new(1.0_f32, p.hairline))
        .corner_radius(16)
        .inner_margin(egui::Margin::same(18))
        .shadow(p.card_shadow())
        .show(ui, |ui| {
            // Fill the column width so sibling cards line up on both edges
            // (content-sized cards would ragged-right and overflow long text).
            ui.set_min_width(ui.available_width());
            ui.vertical(add_contents).inner
        })
        .inner
}

/// Hairline divider.
pub fn hairline(ui: &mut egui::Ui) {
    let p = pal(ui);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.separator);
    ui.add_space(7.0);
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Prominent capsule: accent fill, white text.
    Primary,
    /// Tinted capsule: soft accent fill, accent text.
    Secondary,
    /// Borderless with a hover fill.
    Quiet,
    /// Prominent capsule in the error color.
    Danger,
    /// White fill with accent text (for use on accent surfaces).
    OnAccent,
    /// Translucent white fill with white text (for use on accent surfaces).
    OnAccentGhost,
}

fn measure_text(ui: &egui::Ui, text: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(size),
            Color32::WHITE,
        )
        .size()
        .x
}

/// The workhorse button: capsule geometry, hover/active feedback, and
/// optional full-width sizing. Returns the response for tooltips.
pub fn button(
    ui: &mut egui::Ui,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    full_width: bool,
) -> egui::Response {
    let p = pal(ui);
    let size = 13.0;
    let height = 32.0;
    let padding = 20.0;
    let width = if full_width {
        ui.available_width()
    } else {
        (measure_text(ui, label, size) + padding * 2.0).max(76.0)
    };
    let (rect, response) = if enabled {
        ui.allocate_exact_size(vec2(width, height), egui::Sense::click())
    } else {
        // Scope the disabled state to just this button so the rest of the
        // card keeps its normal look and interactivity.
        ui.scope_builder(egui::UiBuilder::new().disabled(), |ui| {
            ui.allocate_exact_size(vec2(width, height), egui::Sense::click())
        })
        .inner
    };

    let painter = ui.painter_at(rect);
    let radius = egui::CornerRadius::same((height / 2.0) as u8);

    let (base_fill, base_text) = match kind {
        ButtonKind::Primary => (p.accent, p.on_accent),
        ButtonKind::Secondary => (p.accent_soft, p.accent),
        ButtonKind::Quiet => (Color32::TRANSPARENT, p.text),
        ButtonKind::Danger => (p.error, Color32::WHITE),
        ButtonKind::OnAccent => (Color32::WHITE, p.accent),
        ButtonKind::OnAccentGhost => (Color32::from_white_alpha(52), Color32::WHITE),
    };

    let (fill, text_color) = if enabled {
        let ctx = ui.ctx().clone();
        let hover_t =
            ctx.animate_bool_responsive(response.id.with("btn-hover"), response.hovered());
        let press_t = ctx.animate_bool_responsive(
            response.id.with("btn-press"),
            response.is_pointer_button_down_on(),
        );
        let mut fill = base_fill;
        if hover_t > 0.01 {
            let hover_fill = match kind {
                // Quiet buttons get a soft capsule on hover, like macOS.
                ButtonKind::Quiet => {
                    if p.dark {
                        Color32::from_white_alpha(24)
                    } else {
                        Color32::from_black_alpha(16)
                    }
                }
                _ => p.hover_tint(base_fill),
            };
            fill = theme::mix(base_fill, hover_fill, hover_t);
        }
        if press_t > 0.01 {
            fill = theme::mix(
                fill,
                if p.dark {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                },
                0.12 * press_t,
            );
        }
        (fill, base_text)
    } else {
        let fill = match kind {
            ButtonKind::Primary | ButtonKind::Danger => p.elevated,
            ButtonKind::Secondary | ButtonKind::Quiet => Color32::TRANSPARENT,
            ButtonKind::OnAccent | ButtonKind::OnAccentGhost => Color32::from_white_alpha(44),
        };
        let text = match kind {
            ButtonKind::OnAccent | ButtonKind::OnAccentGhost => Color32::from_white_alpha(160),
            _ => p.text_tertiary,
        };
        (fill, text)
    };

    if fill != Color32::TRANSPARENT {
        painter.rect_filled(rect, radius, fill);
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(size),
        text_color,
    );
    response
}

/// Icon-only capsule button (kept for future header controls).
#[allow(dead_code)]
pub fn icon_button(ui: &mut egui::Ui, icon: Icon) -> egui::Response {
    let p = pal(ui);
    let size = 30.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let hover_t = ui
        .ctx()
        .animate_bool_responsive(response.id.with("icon-btn-hover"), response.hovered());
    let painter = ui.painter_at(rect);
    if hover_t > 0.01 {
        let alpha = if p.dark { 26.0 } else { 14.0 };
        let fill = if p.dark {
            Color32::from_white_alpha((alpha * hover_t) as u8)
        } else {
            Color32::from_black_alpha((alpha * hover_t) as u8)
        };
        painter.rect_filled(rect, egui::CornerRadius::same((size / 2.0) as u8), fill);
    }
    icons::draw(
        ui,
        icon,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        theme::mix(p.text_secondary, p.text, hover_t),
    );
    response
}

/// Sun/moon appearance toggle: like [`icon_button`], but the icon plays a
/// small "morph" pop whenever the theme flips (it shrinks and fades while
/// crossfading into the other glyph, then springs back).
pub fn theme_toggle_button(ui: &mut egui::Ui, icon: Icon, dark_now: bool) -> egui::Response {
    let p = pal(ui);
    let size = 30.0;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let ctx = ui.ctx().clone();
    let hover_t =
        ctx.animate_bool_responsive(response.id.with("icon-btn-hover"), response.hovered());
    // 0 -> 1 across a theme flip; deviation peaks mid-flip.
    let flip_t = ctx.animate_value_with_time(
        response.id.with("theme-flip"),
        if dark_now { 1.0 } else { 0.0 },
        0.30,
    );
    let deviation = 4.0 * flip_t * (1.0 - flip_t);

    let painter = ui.painter_at(rect);
    if hover_t > 0.01 {
        let alpha = if p.dark { 26.0 } else { 14.0 };
        let fill = if p.dark {
            Color32::from_white_alpha((alpha * hover_t) as u8)
        } else {
            Color32::from_black_alpha((alpha * hover_t) as u8)
        };
        painter.rect_filled(rect, egui::CornerRadius::same((size / 2.0) as u8), fill);
    }

    let icon_size = 16.0 * (1.0 - 0.35 * deviation);
    let mut icon_color = theme::mix(p.text_secondary, p.text, hover_t);
    icon_color = theme::mix(
        icon_color,
        p.bg,
        0.45 * deviation, // fade the glyph while it swaps
    );
    icons::draw(
        ui,
        icon,
        Rect::from_center_size(rect.center(), vec2(icon_size, icon_size)),
        icon_color,
    );
    response
}

// ---------------------------------------------------------------------------
// Toggle switch (iOS style)
// ---------------------------------------------------------------------------

pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool, id_salt: egui::Id) -> bool {
    let p = pal(ui);
    let size = vec2(42.0, 25.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let changed = response.clicked();
    if changed {
        *on = !*on;
    }

    let t = ui
        .ctx()
        .animate_bool_responsive(id_salt.with("toggle-anim"), *on);
    let painter = ui.painter_at(rect);
    let track = theme::mix(
        if p.dark {
            Color32::from_rgb(58, 58, 62)
        } else {
            Color32::from_rgb(222, 222, 226)
        },
        p.accent,
        t,
    );
    painter.rect_filled(rect, egui::CornerRadius::same((size.y / 2.0) as u8), track);
    let knob_r = 10.5;
    let travel = size.x - knob_r * 2.0 - 4.0;
    let cx = rect.min.x + 2.0 + knob_r + travel * t;
    let cy = rect.center().y;
    painter.circle_filled(Pos2::new(cx, cy), knob_r, Color32::WHITE);
    changed
}

// ---------------------------------------------------------------------------
// Segmented control (macOS style)
// ---------------------------------------------------------------------------

/// A two-or-more option segmented control. Returns the selected index.
pub fn segmented(
    ui: &mut egui::Ui,
    options: &[&str],
    selected: usize,
    id_salt: egui::Id,
) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let selected = selected.min(options.len() - 1);
    let p = pal(ui);
    let height = 30.0;
    let available = ui.available_width();
    let seg_w = available / options.len().max(1) as f32;
    let (rect, response) = ui.allocate_exact_size(vec2(available, height), egui::Sense::click());

    let painter = ui.painter_at(rect);

    // Track: clearly darker/lighter than the card so the pill reads as a
    // separate, raised element instead of a cutout.
    let track = if p.dark {
        Color32::from_rgb(40, 40, 44)
    } else {
        Color32::from_rgb(228, 228, 233)
    };
    painter.rect_filled(rect, egui::CornerRadius::same((height / 2.0) as u8), track);

    // Animated selection pill. NOTE: egui's signature is
    // `animate_value_with_time(id, target_value, animation_time)` — the
    // value being animated is the segment index, the duration is 0.22 s.
    let t = ui
        .ctx()
        .animate_value_with_time(id_salt.with("seg"), selected as f32, 0.22);
    let pill_w = seg_w - 6.0;
    let pill_x = rect.min.x + 3.0 + seg_w * t;
    let pill = Rect::from_min_size(
        Pos2::new(pill_x, rect.min.y + 3.0),
        vec2(pill_w, height - 6.0),
    );
    let pill_radius = egui::CornerRadius::same(((height - 6.0) / 2.0) as u8);

    // Raised pill: lighter than the track with a hairline edge. (A hard
    // offset shadow reads as a broken outline at this size since egui
    // shapes have no blur.)
    let pill_fill = if p.dark {
        Color32::from_rgb(66, 66, 72)
    } else {
        Color32::WHITE
    };
    painter.rect_filled(pill, pill_radius, pill_fill);
    let pill_edge = if p.dark {
        Color32::from_white_alpha(26)
    } else {
        Color32::from_rgb(206, 206, 214)
    };
    painter.rect_stroke(
        pill,
        pill_radius,
        egui::Stroke::new(1.0_f32, pill_edge),
        egui::StrokeKind::Inside,
    );

    for (i, option) in options.iter().enumerate() {
        let seg_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + seg_w * i as f32, rect.min.y),
            vec2(seg_w, height),
        );
        let selected_now = i == selected;
        painter.text(
            seg_rect.center(),
            egui::Align2::CENTER_CENTER,
            *option,
            egui::FontId::new(
                12.0,
                if selected_now {
                    theme::strong()
                } else {
                    egui::FontFamily::Proportional
                },
            ),
            if selected_now {
                p.text
            } else {
                p.text_secondary
            },
        );
    }

    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
        && rect.contains(pos)
    {
        let idx = (((pos.x - rect.min.x) / seg_w) as usize).min(options.len() - 1);
        if idx != selected {
            return Some(idx);
        }
    }
    None
}

/// Strip a leading "N. " numbering or "- " bullet prefix from localized text.
fn strip_leading_number(text: &str) -> &str {
    if let Some(rest) = text.strip_prefix("- ") {
        return rest;
    }
    match text.find(". ") {
        Some(idx) if idx <= 2 && text[..idx].chars().all(|c| c.is_ascii_digit()) => {
            &text[idx + 2..]
        }
        _ => text,
    }
}

// ---------------------------------------------------------------------------
// Sidebar navigation
// ---------------------------------------------------------------------------

/// macOS System Settings-style sidebar row: a colored rounded icon chip, the
/// label, and an optional accent dot on the right (for "something new").
pub fn nav_item_chip(
    ui: &mut egui::Ui,
    current: &mut AppTab,
    target: AppTab,
    label: &str,
    icon: Icon,
    chip: Color32,
    badge: bool,
) {
    let p = pal(ui);
    let selected = *current == target;
    let t = ui
        .ctx()
        .animate_bool_responsive(ui.make_persistent_id(("nav", label)), selected);

    let height = 36.0;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());

    let painter = ui.painter_at(rect);
    let radius = egui::CornerRadius::same(9);
    // Hover fill, then the accent selection blended on top.
    if response.hovered() && !selected {
        let hover = if p.dark {
            Color32::from_white_alpha(20)
        } else {
            Color32::from_black_alpha(12)
        };
        painter.rect_filled(rect, radius, hover);
    }
    if t > 0.01 {
        let fill = if p.dark {
            theme::mix(Color32::TRANSPARENT, p.accent, 0.34 * t)
        } else {
            theme::mix(Color32::TRANSPARENT, p.accent, 0.16 * t)
        };
        painter.rect_filled(rect, radius, fill);
    }

    // Colored icon chip (like the Settings icons in macOS).
    let chip_rect = Rect::from_center_size(
        Pos2::new(rect.min.x + 21.0, rect.center().y),
        vec2(24.0, 24.0),
    );
    painter.rect_filled(chip_rect, egui::CornerRadius::same(7), chip);
    icons::draw(
        ui,
        icon,
        Rect::from_center_size(chip_rect.center(), vec2(15.0, 15.0)),
        Color32::WHITE,
    );
    painter.text(
        Pos2::new(rect.min.x + 42.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        theme::mix(
            p.text_secondary,
            p.text,
            t.max(if selected { 1.0 } else { 0.0 }),
        ),
    );

    if badge {
        painter.circle_filled(Pos2::new(rect.max.x - 14.0, rect.center().y), 3.5, p.accent);
    }

    if response.clicked() {
        *current = target;
    }
}

// ---------------------------------------------------------------------------
// Composite pieces
// ---------------------------------------------------------------------------

/// Dashboard metric tile: icon badge + label + value + detail.
pub fn metric_tile(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    value: &str,
    detail: &str,
    accent: Color32,
) {
    let p = pal(ui);
    let response = egui::Frame::new()
        .fill(p.card)
        .stroke(egui::Stroke::new(1.0_f32, p.hairline))
        .corner_radius(16)
        .inner_margin(egui::Margin::same(15))
        .shadow(p.card_shadow())
        .show(ui, |ui| {
            ui.set_min_height(96.0);
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let badge = vec2(34.0, 34.0);
                let (badge_rect, _) = ui.allocate_exact_size(badge, egui::Sense::hover());
                let painter = ui.painter_at(badge_rect);
                painter.rect_filled(
                    badge_rect,
                    egui::CornerRadius::same(10),
                    theme::mix(accent, p.card, 0.82),
                );
                icons::draw(
                    ui,
                    icon,
                    Rect::from_center_size(badge_rect.center(), vec2(18.0, 18.0)),
                    accent,
                );
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .family(theme::strong())
                            .size(10.5)
                            .color(p.text_tertiary),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(value)
                            .family(theme::strong())
                            .size(15.0)
                            .color(p.text),
                    );
                    ui.label(
                        egui::RichText::new(detail)
                            .size(10.5)
                            .color(p.text_secondary),
                    );
                });
            });
        });
    let hover = ui.ctx().animate_bool_responsive(
        response.response.id.with("tile-hover"),
        response.response.hovered(),
    );
    if hover > 0.01 {
        ui.painter().rect_stroke(
            response.response.rect,
            16.0,
            egui::Stroke::new(1.0_f32, accent.linear_multiply(0.30 + hover * 0.5)),
            egui::StrokeKind::Inside,
        );
    }
}

/// Numbered guide row, like Apple's setup instructions.
/// `text` may carry its own legacy "N. " prefix (kept as a translation
/// key); it is stripped here since the badge shows the number.
pub fn step_row(ui: &mut egui::Ui, number: usize, text: &str) {
    let p = pal(ui);
    let text = strip_leading_number(text);
    ui.horizontal(|ui| {
        let badge = 20.0;
        let (rect, _) = ui.allocate_exact_size(vec2(badge, badge), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::CornerRadius::same(10), p.accent_soft);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            number.to_string(),
            egui::FontId::proportional(11.0),
            p.accent,
        );
        ui.add_space(6.0);
        ui.label(egui::RichText::new(text).size(12.5).color(p.text_secondary));
    });
    ui.add_space(4.0);
}

/// Bullet row for help lists.
pub fn bullet_row(ui: &mut egui::Ui, text: &str) {
    let p = pal(ui);
    let text = strip_leading_number(text);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(8.0, 16.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(rect.left_center() + vec2(1.0, 0.0), 2.0, p.accent);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(text).size(12.5).color(p.text_secondary));
    });
    ui.add_space(4.0);
}
