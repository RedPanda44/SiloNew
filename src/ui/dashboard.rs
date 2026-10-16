//! Dashboard: hero card, device metrics, quick actions and recent activity.

use eframe::egui;
use egui::{Color32, Rect, pos2, vec2};

use crate::app::SiloApp;
use crate::ui::AppTab;
use crate::ui::icons::{self, Icon};
use crate::ui::theme;
use crate::ui::widgets::{self, ButtonKind};

impl SiloApp {
    pub(crate) fn show_dashboard(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let language = self.language;
        let p = widgets::pal(ui);
        let connected = self.selected_transport_available();
        let selected_device = self
            .devices
            .iter()
            .find(|device| Some(&device.udid) == self.selected_udid.as_ref())
            .map(|device| format!("{}  /  {}", device.name, device.transport_summary()))
            .unwrap_or_else(|| {
                language
                    .text("Connect an iPhone to get started.")
                    .to_string()
            });

        self.show_dashboard_hero(ui, language);
        ui.add_space(16.0);

        // Metrics row.
        ui.columns(3, |cols| {
            widgets::metric_tile(
                &mut cols[0],
                Icon::Card,
                language.text("CONNECTED DEVICE"),
                if connected {
                    language.text("Online")
                } else {
                    language.text("Offline")
                },
                &selected_device,
                if connected { p.success } else { p.error },
            );
            widgets::metric_tile(
                &mut cols[1],
                Icon::Keypad,
                language.text("SAVED CARDS"),
                &self.saved_cards.len().to_string(),
                language.text("Tap one to continue editing"),
                p.accent,
            );
            widgets::metric_tile(
                &mut cols[2],
                Icon::Sliders,
                language.text("CURRENT SETUP"),
                language.text(self.connection_mode.label()),
                if self.apple_ready {
                    language.text("Apple support is available")
                } else {
                    language.text("Apple support runtime is missing")
                },
                if self.apple_ready { p.success } else { p.error },
            );
        });

        ui.add_space(16.0);
        ui.columns(2, |cols| {
            // Quick actions.
            widgets::card(&mut cols[0], |ui| {
                widgets::heading(ui, language.text("Quick actions"));
                ui.add_space(2.0);
                widgets::caption(ui, language.text("Jump straight into a workflow."));
                ui.add_space(12.0);

                let tile_w = (ui.available_width() - 10.0) / 2.0;
                ui.columns(2, |actions| {
                    if quick_action_tile(
                        &mut actions[0],
                        Icon::Image,
                        language.text("Choose artwork"),
                        tile_w,
                    ) {
                        self.current_tab = AppTab::Wallet;
                        self.select_skin(ctx);
                    }
                    if quick_action_tile(
                        &mut actions[1],
                        Icon::Package,
                        language.text("Load keypad theme"),
                        tile_w,
                    ) {
                        self.current_tab = AppTab::Passcode;
                        self.select_theme_file(ctx);
                    }
                });
                ui.add_space(10.0);
                ui.columns(2, |actions| {
                    if quick_action_tile(
                        &mut actions[0],
                        Icon::Download,
                        language.text("Check for updates"),
                        tile_w,
                    ) {
                        self.current_tab = AppTab::Updates;
                    }
                    if quick_action_tile(
                        &mut actions[1],
                        Icon::Sliders,
                        language.text("Open settings"),
                        tile_w,
                    ) {
                        self.current_tab = AppTab::Settings;
                    }
                });
            });

            // Recent activity.
            widgets::card(&mut cols[1], |ui| {
                widgets::heading(ui, language.text("Recent activity"));
                ui.add_space(2.0);
                widgets::caption(ui, language.text("The latest events from this session."));
                ui.add_space(12.0);
                if self.logs.is_empty() {
                    ui.label(
                        egui::RichText::new(language.text("No events logged yet."))
                            .size(12.0)
                            .color(p.text_tertiary),
                    );
                } else {
                    let start = self.logs.len().saturating_sub(4);
                    for line in self.logs[start..].iter().rev() {
                        ui.horizontal(|ui| {
                            icons::status_dot(ui, p.accent);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(line)
                                        .size(11.0)
                                        .color(p.text_secondary),
                                )
                                .wrap(),
                            );
                        });
                        ui.add_space(5.0);
                    }
                }
                ui.add_space(6.0);
                if widgets::button(
                    ui,
                    language.text("View activity log"),
                    ButtonKind::Secondary,
                    true,
                    false,
                )
                .clicked()
                {
                    self.show_logs_window = true;
                }
            });
        });

        if !self.saved_cards.is_empty() {
            ui.add_space(16.0);
            widgets::card(ui, |ui| {
                widgets::heading(ui, language.text("Pick up where you left off"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Choose a saved card to open it in Wallet Studio."),
                );
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    let saved_cards = self.saved_cards.clone();
                    for card in &saved_cards {
                        let label = format!(
                            "{}  /  {}...",
                            card.name,
                            &card.hash[..8.min(card.hash.len())]
                        );
                        if widgets::button(ui, &label, ButtonKind::Secondary, true, false).clicked()
                        {
                            self.card_hash = card.hash.clone();
                            self.current_card_name = card.name.clone();
                            self.current_tab = AppTab::Wallet;
                            self.request_current_card_artwork();
                        }
                    }
                });
            });
        }
    }

    fn show_dashboard_hero(&mut self, ui: &mut egui::Ui, language: crate::i18n::Language) {
        let p = widgets::pal(ui);
        let height = 176.0;
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Accent-to-indigo gradient, like an Apple Wallet pass.
        theme::gradient_rounded(
            &painter,
            rect,
            20.0,
            p.accent,
            if p.dark {
                Color32::from_rgb(94, 92, 230)
            } else {
                Color32::from_rgb(88, 86, 214)
            },
        );

        let content = Rect::from_min_max(
            rect.min + vec2(26.0, 22.0),
            rect.max - vec2(26.0, 22.0),
        );

        // Left: copy + actions.
        let left_w = (content.width() * 0.58).min(520.0);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                content.min,
                vec2(left_w, content.height()),
            )),
            |ui| {
                ui.label(
                    egui::RichText::new(language.text("SILO PERSONALIZATION"))
                        .family(theme::strong())
                        .size(10.0)
                        .color(Color32::from_white_alpha(190)),
                );
                ui.add_space(7.0);
                ui.label(
                    egui::RichText::new(language.text("Make your iPhone feel like yours."))
                        .family(theme::strong())
                        .size(22.0)
                        .color(Color32::WHITE),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        language.text(
                            "Customize Wallet artwork or build a lock-screen keypad theme from one simple workspace.",
                        ),
                    )
                    .size(11.5)
                    .color(Color32::from_white_alpha(205)),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let scan_label = if self.scanning_syslog {
                        language.text("Stop card scan")
                    } else {
                        language.text("Scan a card")
                    };
                    if widgets::button(ui, scan_label, ButtonKind::OnAccent, !self.is_busy, false)
                        .clicked()
                    {
                        self.current_tab = AppTab::Wallet;
                        self.toggle_syslog_scan();
                    }
                    if widgets::button(
                        ui,
                        language.text("Wallet studio"),
                        ButtonKind::OnAccentGhost,
                        true,
                        false,
                    )
                    .clicked()
                    {
                        self.current_tab = AppTab::Wallet;
                    }
                });
            },
        );

        // Right: decorative Wallet-style card stack.
        let stack_w = (content.width() * 0.30).min(250.0);
        let stack_rect = Rect::from_min_size(
            pos2(content.max.x - stack_w, content.min.y + 16.0),
            vec2(stack_w, content.height() - 32.0),
        );
        let back = Rect::from_min_size(
            stack_rect.min + vec2(10.0, 0.0),
            vec2(stack_rect.width() - 10.0, stack_rect.height() - 14.0),
        );
        painter.rect_filled(back, 14.0, Color32::from_white_alpha(36));
        let front = Rect::from_min_size(
            stack_rect.min + vec2(0.0, 14.0),
            vec2(stack_rect.width() - 10.0, stack_rect.height() - 14.0),
        );
        painter.rect_filled(front, 14.0, Color32::from_white_alpha(58));
        painter.text(
            front.left_top() + vec2(16.0, 16.0),
            egui::Align2::LEFT_TOP,
            "Wallet",
            egui::FontId::proportional(12.0),
            Color32::from_white_alpha(225),
        );
        painter.circle_filled(
            front.right_bottom() - vec2(28.0, 26.0),
            10.0,
            Color32::from_white_alpha(90),
        );
        painter.rect_filled(
            Rect::from_min_max(
                front.left_bottom() - vec2(0.0, 34.0),
                front.right_bottom(),
            ),
            egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 14,
                se: 14,
            },
            Color32::from_black_alpha(28),
        );
    }
}

/// A tappable quick-action tile with icon, label and chevron.
fn quick_action_tile(ui: &mut egui::Ui, icon: Icon, label: &str, width: f32) -> bool {
    let p = widgets::pal(ui);
    let height = 62.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width.min(ui.available_width()), height), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let radius = egui::CornerRadius::same(12);

    let fill = if response.hovered() {
        p.hover_tint(p.elevated)
    } else {
        p.elevated
    };
    painter.rect_filled(rect, radius, fill);

    let badge = Rect::from_min_size(rect.min + vec2(10.0, 14.0), vec2(34.0, 34.0));
    painter.rect_filled(badge, egui::CornerRadius::same(10), p.card);
    icons::draw(
        ui,
        icon,
        Rect::from_center_size(badge.center(), vec2(18.0, 18.0)),
        p.accent,
    );
    painter.text(
        pos2(rect.min.x + 54.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.5),
        p.text,
    );
    icons::draw(
        ui,
        Icon::ChevronRight,
        Rect::from_center_size(
            pos2(rect.max.x - 16.0, rect.center().y),
            vec2(14.0, 14.0),
        ),
        p.text_tertiary,
    );
    response.clicked()
}
