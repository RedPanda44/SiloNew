//! Updates: check for a newer Silo release and install it in place.

use eframe::egui;

use crate::app::{APP_VERSION, SiloApp, UpdateState};
use crate::ui::icons::{self, Icon};
use crate::ui::theme;
use crate::ui::widgets::{self, ButtonKind};

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

impl SiloApp {
    pub(crate) fn show_updates(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let language = self.language;
        let p = widgets::pal(ui);
        let mut apply = false;

        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                let badge = egui::vec2(46.0, 46.0);
                let (rect, _) = ui.allocate_exact_size(badge, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, egui::CornerRadius::same(13), p.accent_soft);
                icons::draw(
                    ui,
                    Icon::Download,
                    egui::Rect::from_center_size(rect.center(), egui::vec2(23.0, 23.0)),
                    p.accent,
                );
                ui.add_space(13.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(language.text("Silo Updater"))
                            .family(theme::strong())
                            .size(16.5)
                            .color(p.text),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  v{}",
                            language.text("Current version"),
                            APP_VERSION
                        ))
                        .size(11.5)
                        .color(p.text_secondary),
                    );
                });
            });

            ui.add_space(14.0);
            widgets::hairline(ui);
            ui.add_space(6.0);

            match self.update_state.clone() {
                UpdateState::Idle => {
                    widgets::caption(ui, language.text("Check for a newer version of Silo."));
                    ui.add_space(12.0);
                    if widgets::button(
                        ui,
                        language.text("Check for updates"),
                        ButtonKind::Primary,
                        true,
                        false,
                    )
                    .clicked()
                    {
                        self.check_for_updates(false);
                    }
                }
                UpdateState::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(language.text("Checking for updates..."))
                                .size(12.5)
                                .color(p.text_secondary),
                        );
                    });
                }
                UpdateState::UpToDate => {
                    ui.horizontal(|ui| {
                        icons::status_dot(ui, p.success);
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(
                                language.text("You are running the latest version."),
                            )
                            .family(theme::strong())
                            .size(13.0)
                            .color(p.text),
                        );
                    });
                    ui.add_space(12.0);
                    if widgets::button(
                        ui,
                        language.text("Check again"),
                        ButtonKind::Secondary,
                        true,
                        false,
                    )
                    .clicked()
                    {
                        self.check_for_updates(false);
                    }
                }
                UpdateState::Available(info) => {
                    ui.horizontal(|ui| {
                        icons::status_dot(ui, p.accent);
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(language.text("A new version is available."))
                                .family(theme::strong())
                                .size(13.0)
                                .color(p.text),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!("v{}", info.version))
                                .family(theme::strong())
                                .size(12.5)
                                .color(p.accent),
                        );
                    });
                    if !info.name.is_empty() && info.name != info.version {
                        ui.add_space(2.0);
                        widgets::caption(ui, &info.name);
                    }
                    if info.download_url.is_none() {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(
                                language.text("This release has no Windows download."),
                            )
                            .size(11.5)
                            .color(p.error),
                        );
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if widgets::button(
                            ui,
                            language.text("Download & install"),
                            ButtonKind::Primary,
                            info.download_url.is_some(),
                            false,
                        )
                        .clicked()
                        {
                            self.download_update(info.clone());
                        }
                        if !info.html_url.is_empty()
                            && widgets::button(
                                ui,
                                language.text("View release notes"),
                                ButtonKind::Quiet,
                                true,
                                false,
                            )
                            .clicked()
                        {
                            crate::updater::open_url(&info.html_url);
                        }
                    });

                    if !info.notes.trim().is_empty() {
                        ui.add_space(16.0);
                        widgets::hairline(ui);
                        widgets::section_header(ui, &language.text("Release notes").to_uppercase());
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(info.notes.trim())
                                .size(12.0)
                                .color(p.text_secondary),
                        );
                    }
                }
                UpdateState::Downloading { done, total } => {
                    let fraction = if total > 0 {
                        (done as f32 / total as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    ui.label(
                        egui::RichText::new(language.text("Downloading update..."))
                            .family(theme::strong())
                            .size(13.0)
                            .color(p.text),
                    );
                    ui.add_space(10.0);
                    let text = if total > 0 {
                        format!(
                            "{:.0}%   {} / {}",
                            fraction * 100.0,
                            format_bytes(done),
                            format_bytes(total)
                        )
                    } else {
                        format_bytes(done)
                    };
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .fill(p.accent)
                            .desired_height(10.0)
                            .text(egui::RichText::new(text).size(10.5)),
                    );
                }
                UpdateState::Ready(_) => {
                    ui.horizontal(|ui| {
                        icons::status_dot(ui, p.success);
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(language.text("The update is ready to install."))
                                .family(theme::strong())
                                .size(13.0)
                                .color(p.text),
                        );
                    });
                    ui.add_space(2.0);
                    widgets::caption(
                        ui,
                        language
                            .text("Silo will close, apply the update, and reopen automatically."),
                    );
                    ui.add_space(12.0);
                    if widgets::button(
                        ui,
                        language.text("Restart & install"),
                        ButtonKind::Primary,
                        true,
                        false,
                    )
                    .clicked()
                    {
                        apply = true;
                    }
                }
                UpdateState::Error(error) => {
                    ui.label(
                        egui::RichText::new(language.text("Update check failed."))
                            .family(theme::strong())
                            .size(13.0)
                            .color(p.error),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(error)
                            .size(11.5)
                            .color(p.text_secondary),
                    );
                    ui.add_space(12.0);
                    if widgets::button(
                        ui,
                        language.text("Try again"),
                        ButtonKind::Secondary,
                        true,
                        false,
                    )
                    .clicked()
                    {
                        self.check_for_updates(false);
                    }
                }
            }
        });

        if apply {
            self.apply_update(ctx);
        }
    }
}
