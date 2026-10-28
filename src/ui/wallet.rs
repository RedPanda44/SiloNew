//! Wallet Studio: card-hash scanning, artwork crop preview and the
//! live device-face comparison.

use std::time::Duration;

use eframe::egui;
use egui::{Color32, Rect, vec2};

use crate::app::SiloApp;
use crate::image_skin::crop_uv_for_card;
use crate::ui::theme;
use crate::ui::widgets::{self, ButtonKind};
use crate::wallet_backup::backup_exists;

impl SiloApp {
    pub(crate) fn show_wallet_tab(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let language = self.language;
        let p = widgets::pal(ui);
        if self.card_hash.trim() != self.current_card_hash {
            self.current_card_texture = None;
            self.current_card_loading = false;
            self.current_card_error = None;
            self.artwork_pending_hash = None;
            self.current_card_name = self
                .saved_cards
                .iter()
                .find(|card| card.hash == self.card_hash.trim())
                .map(|card| card.name.clone())
                .unwrap_or_default();
        }

        // Live scanning banner.
        if self.scanning_syslog {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 62.0), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, egui::CornerRadius::same(14), p.accent_soft);
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(14),
                egui::Stroke::new(1.0_f32, p.hairline),
                egui::StrokeKind::Inside,
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(10.0)), |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0).color(p.accent));
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(language.text("Scanning syslog..."))
                                .family(theme::strong())
                                .size(13.0)
                                .color(p.text),
                        );
                        ui.label(
                            egui::RichText::new(
                                language.text("Open Wallet on iPhone and tap your card"),
                            )
                            .size(11.5)
                            .color(p.text_secondary),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(
                            ui,
                            language.text("Stop"),
                            ButtonKind::Danger,
                            true,
                            false,
                        )
                        .clicked()
                        {
                            self.toggle_syslog_scan();
                        }
                    });
                });
            });
            ui.add_space(10.0);
        }

        ui.columns(2, |cols| {
            // Left: configuration.
            widgets::card(&mut cols[0], |ui| {
                widgets::heading(ui, language.text("Card Configuration"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Target your card and choose replacement artwork"),
                );
                ui.add_space(14.0);
                widgets::hairline(ui);

                // Target card hash.
                widgets::section_header(ui, &language.text("Target Card Hash").to_uppercase());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let btn_w = 92.0;
                    let text_w = (ui.available_width() - btn_w - 10.0).max(150.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.card_hash)
                            .hint_text(language.text("Base64 pass hash..."))
                            .desired_width(text_w),
                    );
                    let scan_label = if self.scanning_syslog {
                        language.text("Stop")
                    } else {
                        language.text("Scan")
                    };
                    let scan_kind = if self.scanning_syslog {
                        ButtonKind::Danger
                    } else {
                        ButtonKind::Primary
                    };
                    if widgets::button(ui, scan_label, scan_kind, true, false).clicked() {
                        self.toggle_syslog_scan();
                    }
                });

                if !self.saved_cards.is_empty() {
                    ui.add_space(10.0);
                    widgets::caption(ui, language.text("Saved cards"));
                    ui.add_space(3.0);
                    let combo_w = (ui.available_width() - 4.0).max(150.0);
                    let sel_label = self
                        .saved_cards
                        .iter()
                        .find(|c| c.hash == self.card_hash)
                        .map(|c| format!("{} ({})", c.name, &c.hash[..8.min(c.hash.len())]))
                        .unwrap_or_else(|| language.text("Select...").into());

                    egui::ComboBox::from_id_salt("saved_cards_box")
                        .width(combo_w)
                        .selected_text(egui::RichText::new(sel_label).color(p.text))
                        .show_ui(ui, |ui| {
                            for card in &self.saved_cards {
                                let is_selected = self.card_hash == card.hash;
                                let label = format!(
                                    "{} ({}...)",
                                    card.name,
                                    &card.hash[..8.min(card.hash.len())]
                                );
                                let text = egui::RichText::new(label)
                                    .color(if is_selected { p.accent } else { p.text })
                                    .family(theme::strong());
                                if ui.selectable_label(is_selected, text).clicked() {
                                    self.card_hash = card.hash.clone();
                                }
                            }
                        });
                }

                ui.add_space(14.0);
                widgets::hairline(ui);

                // Artwork.
                widgets::section_header(ui, &language.text("Card Skin Artwork").to_uppercase());
                ui.add_space(2.0);
                widgets::caption(ui, language.text("PNG, JPG, WebP - auto-scaled to 1536x969"));
                widgets::caption(
                    ui,
                    language.text("Drag inside the preview to reposition the crop."),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if widgets::button(
                        ui,
                        language.text("Choose Image..."),
                        ButtonKind::Primary,
                        true,
                        false,
                    )
                    .clicked()
                    {
                        self.select_skin(ctx);
                    }
                    if self.skin.is_some()
                        && widgets::button(
                            ui,
                            language.text("Export PNG"),
                            ButtonKind::Secondary,
                            true,
                            false,
                        )
                        .clicked()
                    {
                        self.save_prepared_png();
                    }
                });

                if let Some(skin) = &self.skin {
                    ui.add_space(6.0);
                    let fname = self
                        .source_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("image");
                    ui.label(
                        egui::RichText::new(format!(
                            "{} - 1536x969 - {:.0} KB",
                            fname,
                            skin.png.len() as f32 / 1024.0
                        ))
                        .size(11.0)
                        .color(p.accent),
                    );
                }

                ui.add_space(14.0);
                widgets::hairline(ui);

                // Write to device.
                widgets::section_header(ui, &language.text("Write to iPhone").to_uppercase());
                ui.add_space(6.0);

                let can_flash = !self.is_busy
                    && self.selected_transport_available()
                    && !self.card_hash.trim().is_empty()
                    && self.skin.is_some();
                let flash_resp = widgets::button(
                    ui,
                    language.text("Apply Card Skin"),
                    ButtonKind::Primary,
                    can_flash,
                    true,
                );
                if flash_resp.clicked() {
                    self.flash_card();
                }
                if !can_flash {
                    let mut r = Vec::new();
                    if self.selected_udid.is_none() {
                        r.push(language.text("connect iPhone"));
                    } else if !self.selected_transport_available() {
                        r.push(language.text("choose available transport"));
                    }
                    if self.card_hash.trim().is_empty() {
                        r.push(language.text("enter card hash"));
                    }
                    if self.skin.is_none() {
                        r.push(language.text("choose image"));
                    }
                    if !r.is_empty() {
                        flash_resp.on_disabled_hover_text(format!(
                            "{}{}",
                            language.text("Need: "),
                            r.join(", ")
                        ));
                    }
                }

                ui.add_space(8.0);
                let can_restore = !self.is_busy
                    && !self.scanning_syslog
                    && self.selected_transport_available()
                    && !self.card_hash.trim().is_empty()
                    && self
                        .selected_udid
                        .as_ref()
                        .is_some_and(|udid| backup_exists(udid, self.card_hash.trim()));
                let restore_resp = widgets::button(
                    ui,
                    language.text("Restore Original"),
                    ButtonKind::Secondary,
                    can_restore,
                    true,
                );
                if restore_resp.clicked() {
                    self.restore_original_card();
                }
                if !can_restore && !self.card_hash.trim().is_empty() {
                    restore_resp.on_disabled_hover_text(
                        language.text("Apply a card skin once to create an original backup."),
                    );
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

            // Right: comparison preview.
            widgets::card(&mut cols[1], |ui| {
                widgets::heading(ui, language.text("Card face comparison"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text(
                        "See the face currently on your iPhone, then preview your replacement.",
                    ),
                );
                ui.add_space(14.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(language.text("CURRENT ON IPHONE"))
                            .family(theme::strong())
                            .size(10.0)
                            .color(p.success),
                    );
                    if self.current_card_loading {
                        ui.add(egui::Spinner::new().size(12.0).color(p.accent));
                        ui.label(
                            egui::RichText::new(language.text("Fetching live card face..."))
                                .size(10.5)
                                .color(p.text_secondary),
                        );
                    } else if !self.current_card_name.is_empty() {
                        ui.label(
                            egui::RichText::new(&self.current_card_name)
                                .family(theme::strong())
                                .size(11.0)
                                .color(p.text),
                        );
                    }
                });
                ui.add_space(8.0);

                let pass_w = (ui.available_width() - 8.0).clamp(240.0, 430.0);
                let pass_h = pass_w * (969.0 / 1536.0);
                ui.vertical_centered(|ui| {
                    let (current_rect, _) = ui.allocate_exact_size(
                        vec2(pass_w, pass_h),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    let now = ctx.input(|input| input.time);
                    let reveal = self
                        .card_reveal_started
                        .map(|started| ((now - started) / 0.55).clamp(0.0, 1.0) as f32)
                        .unwrap_or(1.0);
                    let slide_y = (1.0 - reveal) * 18.0;
                    let card_rect = current_rect.translate(vec2(0.0, slide_y));
                    painter.rect_filled(card_rect, 16.0, p.elevated);

                    if let Some(texture) = &self.current_card_texture {
                        painter.image(
                            texture.id(),
                            card_rect,
                            Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            Color32::from_white_alpha((reveal * 255.0) as u8),
                        );
                        painter.rect_filled(
                            Rect::from_min_max(
                                card_rect.left_bottom() - vec2(0.0, 48.0),
                                card_rect.right_bottom(),
                            ),
                            egui::CornerRadius {
                                nw: 0,
                                ne: 0,
                                sw: 16,
                                se: 16,
                            },
                            Color32::from_black_alpha(115),
                        );
                        painter.text(
                            card_rect.left_bottom() + vec2(16.0, -29.0),
                            egui::Align2::LEFT_CENTER,
                            &self.current_card_name,
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                        painter.text(
                            card_rect.right_bottom() - vec2(16.0, 17.0),
                            egui::Align2::RIGHT_CENTER,
                            language.text("LIVE DEVICE ARTWORK"),
                            egui::FontId::proportional(8.5),
                            Color32::from_white_alpha(205),
                        );
                        if reveal < 1.0 {
                            ctx.request_repaint_after(Duration::from_millis(16));
                        }
                    } else if self.current_card_loading || self.scanning_syslog {
                        let center = card_rect.center();
                        let phase = (now * 2.4).sin() as f32;
                        let radius = 14.0 + phase * 3.0;
                        for ring in 0..3 {
                            let ring_radius = radius + ring as f32 * 19.0;
                            painter.circle_stroke(
                                center,
                                ring_radius,
                                egui::Stroke::new(
                                    1.3_f32,
                                    Color32::from_rgba_unmultiplied(
                                        p.accent.r(),
                                        p.accent.g(),
                                        p.accent.b(),
                                        150u8.saturating_sub(ring as u8 * 38),
                                    ),
                                ),
                            );
                        }
                        painter.circle_filled(center, 5.0, p.accent);
                        painter.text(
                            center + vec2(0.0, 48.0),
                            egui::Align2::CENTER_CENTER,
                            if self.scanning_syslog {
                                language.text("Hold your iPhone near Wallet and tap a card")
                            } else {
                                language.text("Loading the selected card...")
                            },
                            egui::FontId::proportional(10.5),
                            p.text_secondary,
                        );
                        ctx.request_repaint_after(Duration::from_millis(30));
                    } else {
                        painter.text(
                            card_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            if let Some(error) = &self.current_card_error {
                                error.as_str()
                            } else if self.card_hash.is_empty() {
                                language.text("Scan a Wallet card to load its current face")
                            } else {
                                language.text("Current card face has not been loaded")
                            },
                            egui::FontId::proportional(10.5),
                            p.text_secondary,
                        );
                    }
                    painter.rect_stroke(
                        card_rect,
                        16.0,
                        egui::Stroke::new(1.0_f32, p.hairline),
                        egui::StrokeKind::Inside,
                    );
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::button(
                        ui,
                        language.text("Refresh current card face"),
                        ButtonKind::Quiet,
                        !self.current_card_loading && !self.is_busy && !self.scanning_syslog,
                        false,
                    )
                    .clicked()
                    {
                        self.request_current_card_artwork();
                    }
                });

                ui.add_space(10.0);
                widgets::hairline(ui);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(language.text("YOUR NEW DESIGN"))
                            .family(theme::strong())
                            .size(10.0)
                            .color(p.accent),
                    );
                    if self.skin.is_some() {
                        ui.label(
                            egui::RichText::new(language.text("Drag to adjust crop"))
                                .size(10.0)
                                .color(p.text_secondary),
                        );
                    }
                });
                ui.add_space(8.0);

                ui.vertical_centered(|ui| {
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(pass_w, pass_h), egui::Sense::drag());
                    let source_dimensions = self
                        .source_image
                        .as_ref()
                        .map(|image| (image.width(), image.height()));
                    if response.dragged()
                        && let Some((source_width, source_height)) = source_dimensions {
                            let crop_uv = crop_uv_for_card(
                                source_width,
                                source_height,
                                self.crop_focus[0],
                                self.crop_focus[1],
                            );
                            let visible_width = crop_uv[2] - crop_uv[0];
                            let visible_height = crop_uv[3] - crop_uv[1];
                            if rect.width() > 0.0 {
                                self.crop_focus[0] = (self.crop_focus[0]
                                    - response.drag_motion().x / rect.width()
                                        * (1.0 - visible_width))
                                    .clamp(0.0, 1.0);
                            }
                            if rect.height() > 0.0 {
                                self.crop_focus[1] = (self.crop_focus[1]
                                    - response.drag_motion().y / rect.height()
                                        * (1.0 - visible_height))
                                    .clamp(0.0, 1.0);
                            }
                            self.crop_dirty = true;
                            ctx.request_repaint();
                        }
                    if response.drag_stopped() && self.crop_dirty {
                        self.rebuild_skin_from_source();
                    }

                    let painter = ui.painter();
                    if let (Some(tex), Some((source_width, source_height))) =
                        (self.source_texture.as_ref(), source_dimensions)
                    {
                        let crop_uv = crop_uv_for_card(
                            source_width,
                            source_height,
                            self.crop_focus[0],
                            self.crop_focus[1],
                        );
                        painter.image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(
                                egui::pos2(crop_uv[0], crop_uv[1]),
                                egui::pos2(crop_uv[2], crop_uv[3]),
                            ),
                            Color32::WHITE,
                        );
                        painter.rect_stroke(
                            rect,
                            16.0,
                            egui::Stroke::new(
                                1.0_f32,
                                Color32::from_rgba_unmultiplied(255, 255, 255, 40),
                            ),
                            egui::StrokeKind::Inside,
                        );
                    } else {
                        painter.rect_filled(rect, 16.0, p.elevated);
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            language.text("No artwork loaded"),
                            egui::FontId::proportional(14.0),
                            p.text_tertiary,
                        );
                    }
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("1536x969")
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                    ui.label(
                        egui::RichText::new("|")
                            .size(11.0)
                            .color(p.text_tertiary),
                    );
                    ui.label(
                        egui::RichText::new("1.585 ratio")
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                    ui.label(
                        egui::RichText::new("|")
                            .size(11.0)
                            .color(p.text_tertiary),
                    );
                    if self.skin.is_some() {
                        ui.label(
                            egui::RichText::new(language.text("Ready"))
                                .size(11.0)
                                .color(p.success),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(language.text("No image"))
                                .size(11.0)
                                .color(p.text_secondary),
                        );
                    }
                });
                ui.add_space(6.0);
                widgets::caption(
                    ui,
                    language.text("The top preview is read from the connected iPhone. The bottom preview is your prepared replacement."),
                );
            });
        });
    }
}
