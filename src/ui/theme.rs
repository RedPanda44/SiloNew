//! Apple Wallet-inspired visual language: soft surfaces, generous whitespace,
//! capsule accents, hairline separators, and a full dark + light appearance.

use std::path::PathBuf;

use eframe::egui;
use egui::Color32;

/// Font family bound to a real bold face (Segoe UI Bold). `RichText::strong`
/// only tints the color in egui, so headings opt into weight via this.
pub fn strong() -> egui::FontFamily {
    egui::FontFamily::Name("strong".into())
}

/// User-facing appearance selection, persisted in settings.json.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Appearance {
    /// Follow the Windows app theme.
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// Order used by the segmented control and the header cycle.
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    pub fn index(self) -> usize {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }

    /// Translation key / display label.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// Whether the theme resolved for this preference is dark right now.
    pub fn is_dark(self) -> bool {
        match self {
            Self::Dark => true,
            Self::Light => false,
            Self::System => system_prefers_dark(),
        }
    }

    /// Cycle System -> Light -> Dark -> System (used by the header toggle).
    pub fn cycle(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }
}

/// Whether Windows is currently set to the dark app theme.
/// Reads `AppsUseLightTheme` from the user's Personalize settings.
#[cfg(windows)]
pub fn system_prefers_dark() -> bool {
    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as isize;
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
    unsafe extern "system" {
        fn RegGetValueW(
            hkey: isize,
            lpsubkey: *const u16,
            lpvalue: *const u16,
            dwflags: u32,
            pdwtype: *mut u32,
            pvdata: *mut core::ffi::c_void,
            pcbdata: *mut u32,
        ) -> i32;
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let subkey = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("AppsUseLightTheme");
    // Defaults to 1 (light) when the value is absent.
    let mut data: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    status == 0 && data == 0
}

#[cfg(not(windows))]
pub fn system_prefers_dark() -> bool {
    false
}

/// Every color the UI needs, resolved once per appearance.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub background_top: Color32,
    pub background_bottom: Color32,
    pub sidebar: Color32,
    pub card: Color32,
    pub elevated: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub separator: Color32,
    pub hairline: Color32,
    pub success: Color32,
    pub error: Color32,
}

impl Palette {
    pub fn of(appearance: Appearance) -> Self {
        if appearance.is_dark() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn dark() -> Self {
        Self {
            dark: true,
            bg: Color32::from_rgb(20, 20, 24),
            background_top: Color32::from_rgb(31, 31, 40),
            background_bottom: Color32::from_rgb(13, 13, 17),
            sidebar: Color32::from_rgba_unmultiplied(30, 30, 37, 198),
            card: Color32::from_rgba_unmultiplied(38, 38, 47, 214),
            elevated: Color32::from_rgba_unmultiplied(54, 54, 64, 192),
            text: Color32::from_rgb(245, 245, 247),
            text_secondary: Color32::from_rgb(158, 158, 165),
            text_tertiary: Color32::from_rgb(116, 116, 124),
            accent: Color32::from_rgb(10, 132, 255),
            on_accent: Color32::WHITE,
            accent_soft: Color32::from_rgba_unmultiplied(10, 132, 255, 52),
            separator: Color32::from_white_alpha(22),
            hairline: Color32::from_white_alpha(30),
            success: Color32::from_rgb(48, 209, 88),
            error: Color32::from_rgb(255, 69, 58),
        }
    }

    fn light() -> Self {
        Self {
            dark: false,
            bg: Color32::from_rgb(244, 244, 247),
            background_top: Color32::from_rgb(251, 251, 253),
            background_bottom: Color32::from_rgb(229, 232, 241),
            sidebar: Color32::from_rgba_unmultiplied(255, 255, 255, 198),
            card: Color32::from_rgba_unmultiplied(255, 255, 255, 236),
            elevated: Color32::from_rgba_unmultiplied(240, 240, 245, 212),
            text: Color32::from_rgb(29, 29, 31),
            text_secondary: Color32::from_rgb(81, 81, 86),
            text_tertiary: Color32::from_rgb(134, 134, 139),
            accent: Color32::from_rgb(0, 122, 255),
            on_accent: Color32::WHITE,
            accent_soft: Color32::from_rgba_unmultiplied(0, 122, 255, 34),
            separator: Color32::from_black_alpha(18),
            hairline: Color32::from_black_alpha(26),
            success: Color32::from_rgb(52, 199, 89),
            error: Color32::from_rgb(255, 59, 48),
        }
    }

    /// Subtle elevation shadow for cards, tuned per appearance.
    pub fn card_shadow(self) -> egui::Shadow {
        if self.dark {
            egui::Shadow {
                offset: [0, 2],
                blur: 10,
                spread: 0,
                color: Color32::from_black_alpha(70),
            }
        } else {
            egui::Shadow {
                offset: [0, 2],
                blur: 12,
                spread: 0,
                color: Color32::from_black_alpha(28),
            }
        }
    }

    /// Brighten (dark mode) or darken (light mode) a fill for hover states.
    pub fn hover_tint(self, color: Color32) -> Color32 {
        if self.dark {
            mix(color, Color32::WHITE, 0.08)
        } else {
            mix(color, Color32::from_black_alpha(255), 0.06)
        }
    }
}

fn palette_id() -> egui::Id {
    egui::Id::new("silo-palette")
}

/// The palette currently in effect.
pub fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|data| {
        data.get_temp::<Palette>(palette_id())
            .unwrap_or_else(Palette::dark)
    })
}

pub fn mix(from: Color32, to: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |start: u8, end: u8| (start as f32 + (end as f32 - start as f32) * amount).round() as u8;
    Color32::from_rgba_unmultiplied(
        channel(from.r(), to.r()),
        channel(from.g(), to.g()),
        channel(from.b(), to.b()),
        channel(from.a(), to.a()),
    )
}

/// Paint a vertical gradient with rounded corners (egui has no native
/// gradient shape, so we stack one-pixel rows shaped to the corner radius).
pub fn gradient_rounded(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: f32,
    top: Color32,
    bottom: Color32,
) {
    let steps = (rect.height().round() as usize).clamp(16, 96);
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let row_h = rect.height() / steps as f32;
    for i in 0..steps {
        let t = (i as f32 + 0.5) / steps as f32;
        let y_mid = rect.min.y + t * rect.height();
        let edge_dist = (y_mid - rect.min.y).min(rect.max.y - y_mid);
        let inset = if edge_dist < radius {
            radius
                - (radius * radius - (radius - edge_dist) * (radius - edge_dist))
                    .max(0.0)
                    .sqrt()
        } else {
            0.0
        };
        let y0 = rect.min.y + i as f32 * row_h;
        let y1 = (y0 + row_h + 0.5).min(rect.max.y);
        if inset > rect.width() / 2.0 - 1.0 {
            continue;
        }
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x + inset, y0),
                egui::pos2(rect.max.x - inset, y1),
            ),
            0.0,
            mix(top, bottom, t),
        );
    }
}

/// A soft radial glow drawn as a triangle fan: opaque at the centre, fading to
/// transparent at the rim. Used to tint the translucent backdrop.
pub fn radial_glow(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32) {
    let clear = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0);
    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: egui::epaint::WHITE_UV,
        color,
    });
    let segments = 48u32;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center + egui::vec2(angle.cos(), angle.sin()) * radius,
            uv: egui::epaint::WHITE_UV,
            color: clear,
        });
    }
    for i in 0..segments {
        mesh.add_triangle(0, 1 + i, 2 + i);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Paint the translucent backdrop: a soft vertical gradient with two accent
/// glows. Drawn on the background layer, so the translucent sidebar, cards and
/// header all blend over it for a frosted-glass look.
pub fn paint_background(ctx: &egui::Context, p: Palette) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    gradient_rounded(&painter, rect, 0.0, p.background_top, p.background_bottom);

    let span = rect.width().max(rect.height());
    let glow_alpha = if p.dark { 66 } else { 42 };
    radial_glow(
        &painter,
        egui::pos2(
            rect.left() + rect.width() * 0.16,
            rect.top() + rect.height() * 0.06,
        ),
        span * 0.55,
        Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), glow_alpha),
    );
    let violet = if p.dark {
        Color32::from_rgba_unmultiplied(140, 92, 232, 56)
    } else {
        Color32::from_rgba_unmultiplied(152, 112, 236, 34)
    };
    radial_glow(
        &painter,
        egui::pos2(
            rect.right() - rect.width() * 0.05,
            rect.bottom() - rect.height() * 0.05,
        ),
        span * 0.5,
        violet,
    );
}

/// Apply an appearance: palette, egui visuals and spacing.
pub fn apply_theme(ctx: &egui::Context, appearance: Appearance) {
    let p = Palette::of(appearance);
    ctx.data_mut(|data| data.insert_temp(palette_id(), p));

    let mut visuals = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = p.card;
    visuals.extreme_bg_color = p.elevated;
    visuals.faint_bg_color = p.elevated;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.accent;

    visuals.window_shadow = p.card_shadow();
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: if p.dark {
            Color32::from_black_alpha(120)
        } else {
            Color32::from_black_alpha(40)
        },
    };
    visuals.window_corner_radius = 14.into();
    visuals.menu_corner_radius = 10.into();

    visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, p.text);
    visuals.widgets.noninteractive.corner_radius = 9.into();

    visuals.widgets.inactive.bg_fill = p.elevated;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, p.hairline);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, p.text);
    visuals.widgets.inactive.corner_radius = 9.into();

    visuals.widgets.hovered.bg_fill = p.hover_tint(p.elevated);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, p.hairline);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, p.text);
    visuals.widgets.hovered.corner_radius = 9.into();

    visuals.widgets.active.bg_fill = p.accent_soft;
    visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, p.accent);
    visuals.widgets.active.corner_radius = 9.into();

    visuals.widgets.open.bg_fill = p.hover_tint(p.elevated);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, p.hairline);
    visuals.widgets.open.corner_radius = 9.into();

    visuals.selection.bg_fill = p.accent;
    visuals.selection.stroke = egui::Stroke::NONE;

    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 7.0);
        style.spacing.window_margin = egui::Margin::same(10);
        style.spacing.interact_size.y = 28.0;
    });
}

/// Load Windows fonts with an SF-like stack: Segoe UI Variable when
/// available (Windows 11), plain Segoe UI otherwise, plus CJK fallbacks.
pub fn setup_fonts(ctx: &egui::Context) {
    let windows_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts_dir = windows_dir.join("Fonts");

    let mut fonts = egui::FontDefinitions::default();

    // Segoe UI Variable (Windows 11) has the nicest metrics; fall back to Segoe UI.
    let ui_candidates = ["SegUIVar.ttf", "segoeui.ttf"];
    if let Some(ui_bytes) = ui_candidates
        .iter()
        .find_map(|name| std::fs::read(fonts_dir.join(name)).ok())
    {
        fonts.font_data.insert(
            "apple-ui".to_owned(),
            egui::FontData::from_owned(ui_bytes).into(),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.insert(0, "apple-ui".to_owned());
        }
    }

    // A real bold face for headings (`STRONG` family).
    let bold_candidates = ["segoeuib.ttf", "seguisb.ttf"];
    if let Some(bold_bytes) = bold_candidates
        .iter()
        .find_map(|name| std::fs::read(fonts_dir.join(name)).ok())
    {
        fonts.font_data.insert(
            "apple-ui-bold".to_owned(),
            egui::FontData::from_owned(bold_bytes).into(),
        );
        fonts
            .families
            .insert(strong(), vec!["apple-ui-bold".to_owned()]);
    }

    // Fallback CJK font for Chinese characters.
    let cjk_candidates = [
        fonts_dir.join("msyh.ttc"),
        fonts_dir.join("msyh.ttf"),
        fonts_dir.join("Deng.ttf"),
        fonts_dir.join("simhei.ttf"),
        fonts_dir.join("simsun.ttc"),
        fonts_dir.join("simsunb.ttf"),
    ];
    if let Some(cjk_bytes) = cjk_candidates.iter().find_map(|p| std::fs::read(p).ok()) {
        fonts.font_data.insert(
            "windows-cjk".to_owned(),
            egui::FontData::from_owned(cjk_bytes).into(),
        );
        // Push to the back as a fallback so Latin glyphs stay Segoe.
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.push("windows-cjk".to_owned());
        }
        if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            mono.push("windows-cjk".to_owned());
        }
        // Bold Latin + regular CJK: bold headings still show Chinese glyphs.
        if let Some(strong_fonts) = fonts.families.get_mut(&strong()) {
            strong_fonts.push("windows-cjk".to_owned());
        }
    }

    // epaint panics on unbound families, so keep `strong()` valid even if no
    // bold face was found: fall back to the proportional stack (no weight).
    if !fonts.families.contains_key(&strong()) {
        let fallback = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        fonts.families.insert(strong(), fallback);
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_fonts_loads_cjk_glyphs() {
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            ctx.fonts_mut(|fonts| {
                let w1 = fonts.glyph_width(&egui::FontId::proportional(14.0), 'A');
                let w2 = fonts.glyph_width(&egui::FontId::proportional(14.0), '中');
                assert!(w1 > 0.0);
                assert!(w2 > 0.0);
            });
        });
    }

    #[test]
    fn setup_fonts_always_binds_strong_family() {
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            ctx.fonts_mut(|fonts| {
                let w = fonts.glyph_width(&egui::FontId::new(14.0, strong()), 'A');
                assert!(w > 0.0);
            });
        });
    }

    #[test]
    fn appearance_cycles_through_all_three() {
        assert_eq!(Appearance::System.cycle(), Appearance::Light);
        assert_eq!(Appearance::Light.cycle(), Appearance::Dark);
        assert_eq!(Appearance::Dark.cycle(), Appearance::System);
        assert_eq!(Appearance::from_index(0), Appearance::System);
        assert_eq!(Appearance::from_index(1), Appearance::Light);
        assert_eq!(Appearance::from_index(2), Appearance::Dark);
    }

    #[test]
    fn palettes_have_distinct_surfaces() {
        let dark = Palette::dark();
        let light = Palette::light();
        assert!(dark.dark);
        assert!(!light.dark);
        assert_ne!(dark.bg, light.bg);
        assert_ne!(dark.card, light.card);
    }
}
