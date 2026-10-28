//! Passcode Studio: .passthm theme loading, target options and a
//! lock-screen style keypad preview.

use eframe::egui;
use egui::{Color32, Rect, pos2, vec2};

use crate::app::SiloApp;
use crate::ui::theme;
use crate::ui::widgets::{self, ButtonKind};

const DIALER_LAYOUT: &[&[&str]] = &[
    &["1", "2", "3"],
    &["4", "5", "6"],
    &["7", "8", "9"],
    &["", "0", ""],
];

impl SiloApp {
    pub(crate) fn show_passcode_tab(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let language = self.language;
        let p = widgets::pal(ui);

        ui.columns(2, |cols| {
            // Left: configuration.
            widgets::card(&mut cols[0], |ui| {
                widgets::heading(ui, language.text("Passcode Theme"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Custom lockscreen keypad from Cowabunga or Nugget"),
                );
                ui.add_space(14.0);

                // Theme package.
                widgets::section_header(ui, &language.text("Theme Package").to_uppercase());
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Choose a .passthm archive containing dialer artwork"),
                );
                ui.add_space(6.0);
                if widgets::button(
                    ui,
                    language.text("Choose .passthm..."),
                    ButtonKind::Primary,
                    true,
                    false,
                )
                .clicked()
                {
                    self.select_theme_file(ctx);
                }

                if let Some(theme) = &self.loaded_theme {
                    let fname = self
                        .theme_path
                        .as_ref()
                        .and_then(|pp| pp.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("theme.passthm");
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!("{} - {} assets", fname, theme.items.len()))
                            .size(11.0)
                            .color(p.accent),
                    );
                }

                ui.add_space(14.0);
                widgets::hairline(ui);

                // Target iOS cache.
                widgets::section_header(ui, &language.text("Target iOS Cache").to_uppercase());
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Select cache format based on connected iOS version"),
                );
                ui.add_space(6.0);
                let combo_w = (ui.available_width() - 4.0).max(150.0);
                let mut ver_changed = false;
                egui::ComboBox::from_id_salt("telephony_combo")
                    .width(combo_w)
                    .selected_text(language.text(&self.forced_telephony_ver))
                    .show_ui(ui, |ui| {
                        ver_changed |= ui
                            .selectable_value(
                                &mut self.forced_telephony_ver,
                                "Auto (TelephonyUI-10)".into(),
                                language.text("Auto (TelephonyUI-10)"),
                            )
                            .clicked();
                        ver_changed |= ui
                            .selectable_value(
                                &mut self.forced_telephony_ver,
                                "TelephonyUI-10".into(),
                                language.text("TelephonyUI-10 (iOS 18+)"),
                            )
                            .clicked();
                        ver_changed |= ui
                            .selectable_value(
                                &mut self.forced_telephony_ver,
                                "TelephonyUI-9".into(),
                                language.text("TelephonyUI-9 (iOS 16-17)"),
                            )
                            .clicked();
                        ver_changed |= ui
                            .selectable_value(
                                &mut self.forced_telephony_ver,
                                "TelephonyUI-8".into(),
                                language.text("TelephonyUI-8 (Legacy)"),
                            )
                            .clicked();
                    });
                if ver_changed {
                    self.persist_settings();
                    if let Some(path) = self.theme_path.clone() {
                        self.load_theme_from_path(ctx, &path);
                    }
                }

                ui.add_space(14.0);
                widgets::hairline(ui);

                // Keypad language.
                widgets::section_header(ui, &language.text("Keypad Language").to_uppercase());
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text(
                        "Subtext alphabet layout (English, Russian, Ukrainian, Japanese, or Universal)",
                    ),
                );
                ui.add_space(6.0);
                let mut lang_changed = false;
                egui::ComboBox::from_id_salt("keypad_lang_combo")
                    .width(combo_w)
                    .selected_text(
                        egui::RichText::new(language.text(&self.keypad_language)).color(p.text),
                    )
                    .show_ui(ui, |ui| {
                        lang_changed |= ui
                            .selectable_value(
                                &mut self.keypad_language,
                                "English".into(),
                                language.text("English"),
                            )
                            .clicked();
                        lang_changed |= ui
                            .selectable_value(
                                &mut self.keypad_language,
                                "Russian".into(),
                                language.text("Russian"),
                            )
                            .clicked();
                        lang_changed |= ui
                            .selectable_value(
                                &mut self.keypad_language,
                                "Ukrainian".into(),
                                language.text("Ukrainian"),
                            )
                            .clicked();
                        lang_changed |= ui
                            .selectable_value(
                                &mut self.keypad_language,
                                "Japanese".into(),
                                language.text("Japanese"),
                            )
                            .clicked();
                        lang_changed |= ui
                            .selectable_value(
                                &mut self.keypad_language,
                                "All Languages (Universal)".into(),
                                language.text("All Languages (Universal)"),
                            )
                            .clicked();
                    });
                if lang_changed {
                    self.persist_settings();
                    if let Some(path) = self.theme_path.clone() {
                        self.load_theme_from_path(ctx, &path);
                    }
                }

                ui.add_space(12.0);
                widgets::hairline(ui);

                // Bold text toggle.
                let mut bold_changed = false;
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(language.text("Bold Text (iOS Accessibility)"))
                            .family(theme::strong())
                            .size(12.5)
                            .color(p.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::toggle_switch(ui, &mut self.passcode_bold, ui.id().with("bold"))
                        {
                            bold_changed = true;
                        }
                    });
                });
                widgets::caption(
                    ui,
                    language.text("Generates *-bold.png for devices with Bold Text turned ON in iPhone Settings -> Display"),
                );
                if bold_changed {
                    self.persist_settings();
                    if let Some(path) = self.theme_path.clone() {
                        self.load_theme_from_path(ctx, &path);
                    }
                }

                ui.add_space(14.0);
                widgets::hairline(ui);

                // Apply.
                widgets::section_header(ui, &language.text("Write to iPhone").to_uppercase());
                ui.add_space(6.0);

                let can_flash = !self.is_busy
                    && !self.scanning_syslog
                    && self.selected_transport_available()
                    && self.loaded_theme.is_some();
                let resp = widgets::button(
                    ui,
                    language.text("Apply Passcode Theme"),
                    ButtonKind::Primary,
                    can_flash,
                    true,
                );
                if resp.clicked() {
                    self.flash_theme();
                }
                if !can_flash {
                    let mut r = Vec::new();
                    if self.selected_udid.is_none() {
                        r.push(language.text("connect iPhone"));
                    } else if !self.selected_transport_available() {
                        r.push(language.text("choose available transport"));
                    }
                    if self.loaded_theme.is_none() {
                        r.push(language.text("select theme"));
                    }
                    if !r.is_empty() {
                        resp.on_disabled_hover_text(format!(
                            "{}{}",
                            language.text("Need: "),
                            r.join(", ")
                        ));
                    }
                }

                if self.is_busy {
                    ui.add_space(10.0);
                    if self.progress_total > 0 {
                        ui.add(
                            egui::ProgressBar::new(
                                self.progress_step as f32 / self.progress_total as f32,
                            )
                            .animate(true),
                        );
                    }
                    ui.label(
                        egui::RichText::new(&self.progress_msg)
                            .size(11.0)
                            .color(p.accent),
                    );
                }
            });

            // Right: lock-screen style keypad preview.
            widgets::card(&mut cols[1], |ui| {
                widgets::heading(ui, language.text("Keypad Preview"));
                ui.add_space(2.0);
                widgets::caption(ui, language.text("Dialer button artwork"));
                ui.add_space(12.0);

                let screen_w = (ui.available_width() - 8.0).clamp(250.0, 360.0);
                let screen_h = 344.0;
                ui.vertical_centered(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(screen_w, screen_h), egui::Sense::hover());
                    let painter = ui.painter_at(rect);
                    // The screen stays dark in both themes, like a real device.
                    theme::gradient_rounded(
                        &painter,
                        rect,
                        24.0,
                        Color32::from_rgb(24, 24, 28),
                        Color32::from_rgb(10, 10, 12),
                    );
                    painter.rect_stroke(
                        rect,
                        24.0,
                        egui::Stroke::new(1.0_f32, p.hairline),
                        egui::StrokeKind::Inside,
                    );

                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(12.0)), |ui| {
                        let inner = ui.max_rect();

                        // Clock block, centered.
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("9:41")
                                    .family(theme::strong())
                                    .size(30.0)
                                    .color(Color32::from_white_alpha(235)),
                            );
                            ui.label(
                                egui::RichText::new("Monday, June 9")
                                    .size(11.0)
                                    .color(Color32::from_white_alpha(170)),
                            );
                        });

                        // Keypad: placed at computed positions so the grid is
                        // perfectly centered and never clipped by the screen.
                        let key = 46.0;
                        let dx = 62.0; // column center-to-center
                        let dy = 58.0; // row center-to-center
                        let cx = inner.center().x;
                        let top = inner.min.y + 88.0;
                        for (r, row) in DIALER_LAYOUT.iter().enumerate() {
                            for (c, &d) in row.iter().enumerate() {
                                if d.is_empty() {
                                    continue;
                                }
                                let center = pos2(
                                    cx + (c as f32 - 1.0) * dx,
                                    top + r as f32 * dy + key / 2.0,
                                );
                                let key_rect = Rect::from_center_size(center, vec2(key, key));
                                ui.allocate_rect(key_rect, egui::Sense::hover());
                                let kp = ui.painter_at(key_rect);
                                kp.circle_filled(center, key / 2.0, Color32::from_white_alpha(36));
                                if let Some((_, tex)) =
                                    self.keypad_textures.iter().find(|(k, _)| k == d)
                                {
                                    kp.image(
                                        tex.id(),
                                        Rect::from_center_size(center, vec2(38.0, 38.0)),
                                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                } else {
                                    kp.text(
                                        center,
                                        egui::Align2::CENTER_CENTER,
                                        d,
                                        egui::FontId::proportional(20.0),
                                        Color32::from_white_alpha(230),
                                    );
                                }
                            }
                        }
                    });
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(language.text("3x4 Keypad"))
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                    ui.label(
                        egui::RichText::new("|").size(11.0).color(p.text_tertiary),
                    );
                    ui.label(
                        egui::RichText::new("TelephonyUI")
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                    ui.label(
                        egui::RichText::new("|").size(11.0).color(p.text_tertiary),
                    );
                    if self.loaded_theme.is_some() {
                        ui.label(
                            egui::RichText::new(language.text("Ready"))
                                .size(11.0)
                                .color(p.success),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(language.text("No theme"))
                                .size(11.0)
                                .color(p.text_secondary),
                        );
                    }
                });
                ui.add_space(6.0);
                widgets::caption(
                    ui,
                    language.text("After applying, lock your iPhone to see the new keypad."),
                );
            });
        });
    }
}
