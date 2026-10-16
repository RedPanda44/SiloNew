//! Settings: appearance, interface language and transport preferences.

use eframe::egui;

use crate::app::SiloApp;
use crate::i18n::Language;
use crate::ui::theme::{self, Appearance};
use crate::ui::widgets;

impl SiloApp {
    pub(crate) fn show_settings(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let language = self.language;
        let p = widgets::pal(ui);

        ui.columns(2, |cols| {
            // Left column: look & feel.
            widgets::card(&mut cols[0], |ui| {
                widgets::heading(ui, language.text("Appearance"));
                ui.add_space(2.0);
                widgets::caption(ui, language.text("Choose how Silo looks on this PC."));
                ui.add_space(12.0);
                let selected = self.appearance.index();
                let options: Vec<&str> = Appearance::ALL
                    .iter()
                    .map(|appearance| language.text(appearance.label()))
                    .collect();
                if let Some(idx) =
                    widgets::segmented(ui, &options, selected, ui.id().with("appearance"))
                {
                    self.set_appearance(Appearance::from_index(idx), ctx);
                }
                ui.add_space(8.0);
                widgets::caption(
                    ui,
                    language.text("System follows the Windows theme."),
                );

                ui.add_space(16.0);
                widgets::hairline(ui);

                widgets::heading(ui, language.text("Language & app info"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Language preference is saved on this PC."),
                );
                ui.add_space(12.0);
                widgets::section_header(
                    ui,
                    &language.text("Interface language").to_uppercase(),
                );
                ui.add_space(4.0);
                let mut next_language = self.language;
                egui::ComboBox::from_id_salt("settings_language")
                    .selected_text(language.option_label(next_language))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut next_language,
                            Language::English,
                            language.option_label(Language::English),
                        );
                        ui.selectable_value(
                            &mut next_language,
                            Language::SimplifiedChinese,
                            language.option_label(Language::SimplifiedChinese),
                        );
                    });
                if next_language != self.language {
                    self.language = next_language;
                    self.persist_settings();
                    self.status_msg = self.language.text("Language changed.").to_string();
                }

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Silo  v{}", crate::app::APP_VERSION))
                            .family(theme::strong())
                            .size(12.0)
                            .color(p.accent),
                    );
                    ui.label(
                        egui::RichText::new(language.text("Native Windows client"))
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                });
            });

            // Right column: connection.
            widgets::card(&mut cols[1], |ui| {
                widgets::heading(ui, language.text("Connection"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Choose how Silo talks to your iPhone."),
                );
                ui.add_space(12.0);
                widgets::section_header(
                    ui,
                    &language.text("Preferred transport").to_uppercase(),
                );
                ui.add_space(4.0);
                let controls_enabled = !self.is_busy && !self.scanning_syslog;
                let mut next_mode = self.connection_mode;
                ui.add_enabled_ui(controls_enabled, |ui| {
                    egui::ComboBox::from_id_salt("settings_connection_mode")
                        .selected_text(language.text(next_mode.label()))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for mode in crate::device::ConnectionMode::ALL {
                                ui.selectable_value(
                                    &mut next_mode,
                                    mode,
                                    language.text(mode.label()),
                                );
                            }
                        });
                });
                if next_mode != self.connection_mode {
                    self.connection_mode = next_mode;
                    self.add_log(format!("Transport mode changed to {}.", next_mode.label()));
                    self.persist_settings();
                    self.status_msg = format!(
                        "{}: {}",
                        language.text("Transport mode"),
                        language.text(next_mode.label())
                    );
                }
                ui.add_space(10.0);
                widgets::caption(
                    ui,
                    language.text("Devices are detected automatically."),
                );
            });
        });
    }
}
