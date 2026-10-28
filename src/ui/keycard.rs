//! Keycard Studio: design an NFC door-key pass and add it to Apple Wallet.

use eframe::egui;

use crate::app::SiloApp;
use crate::i18n::Language;
use crate::keycard::KeycardDraft;
use crate::ui::theme;
use crate::ui::widgets::{self, ButtonKind};

impl SiloApp {
    pub(crate) fn show_keycard_tab(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        let language = self.language;
        let p = widgets::pal(ui);

        ui.columns(2, |cols| {
            // Left column: pass design + signing.
            widgets::card(&mut cols[0], |ui| {
                widgets::heading(ui, language.text("HOUSE KEYCARD"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Create an NFC door-key pass and add it to Wallet."),
                );
                ui.add_space(14.0);
                widgets::hairline(ui);

                widgets::heading(ui, language.text("Pass details"));
                ui.add_space(8.0);
                let draft = &mut self.keycard.draft;
                text_field(ui, language.text("House name"), &mut draft.house_name, "My House");
                text_field(ui, language.text("Owner name"), &mut draft.owner_name, "Alex Doe");
                text_field(
                    ui,
                    language.text("Access label"),
                    &mut draft.access_label,
                    "Front Door",
                );
                text_field(
                    ui,
                    language.text("NFC payload"),
                    &mut draft.nfc_payload,
                    "hex bytes read by the door",
                );

                color_row(ui, language.text("Background"), &mut draft.background);
                color_row(ui, language.text("Foreground"), &mut draft.foreground);
                color_row(ui, language.text("Label color"), &mut draft.label_color);

                ui.add_space(12.0);
                widgets::hairline(ui);
                widgets::heading(ui, language.text("Card images"));
                ui.add_space(8.0);
                image_row(
                    ui,
                    language,
                    language.text("Icon image"),
                    &mut draft.icon_path,
                );
                image_row(
                    ui,
                    language,
                    language.text("Logo image"),
                    &mut draft.logo_path,
                );

                ui.add_space(14.0);
                widgets::hairline(ui);
                widgets::heading(ui, language.text("Signing certificate"));
                ui.add_space(4.0);
                widgets::caption(
                    ui,
                    language.text(
                        "Apple only accepts signed passes. Add a Pass Type ID certificate to enable Wallet installation.",
                    ),
                );
                ui.add_space(8.0);
                let signing = &mut self.keycard.signing;
                p12_row(ui, language, &mut signing.p12_path);
                password_field(
                    ui,
                    language.text("Certificate password"),
                    &mut signing.p12_password,
                );
                text_field(
                    ui,
                    language.text("Pass Type ID"),
                    &mut signing.pass_type_id,
                    "pass.com.example.housekey",
                );
                text_field(ui, language.text("Team ID"), &mut signing.team_id, "ABCDE12345");
                cert_row(
                    ui,
                    language,
                    language.text("WWDR certificate"),
                    &mut signing.wwdr_path,
                );
            });

            // Right column: live preview + bundle actions.
            widgets::card(&mut cols[1], |ui| {
                widgets::heading(ui, language.text("Add to Wallet"));
                ui.add_space(2.0);
                widgets::caption(ui, language.text("Open the link on your iPhone to add the keycard to Wallet."));
                ui.add_space(12.0);
                preview(ui, &self.keycard.draft);
                ui.add_space(14.0);
                widgets::hairline(ui);
                widgets::heading(ui, language.text("Bundle"));
                ui.add_space(8.0);

                let can_generate = !self.keycard.draft.house_name.trim().is_empty();
                if widgets::button(
                    ui,
                    language.text("Generate keycard"),
                    ButtonKind::Primary,
                    can_generate,
                    true,
                )
                .clicked()
                {
                    self.generate_keycard();
                }

                ui.add_space(8.0);
                let has_pass = self.keycard_last_path.is_some();
                ui.horizontal(|ui| {
                    if widgets::button(
                        ui,
                        language.text("Save .pkpass"),
                        ButtonKind::Secondary,
                        has_pass,
                        false,
                    )
                    .clicked()
                    {
                        self.save_keycard_copy();
                    }
                    let device_ready = self.selected_transport_available();
                    if widgets::button(
                        ui,
                        language.text("Send to iPhone"),
                        ButtonKind::Secondary,
                        has_pass && device_ready,
                        false,
                    )
                    .clicked()
                    {
                        self.send_keycard_to_device();
                    }
                });

                ui.add_space(8.0);
                if self.keycard_server.is_some() {
                    if widgets::button(
                        ui,
                        language.text("Stop sharing"),
                        ButtonKind::Danger,
                        true,
                        true,
                    )
                    .clicked()
                    {
                        self.keycard_server = None;
                        self.keycard_status = language.text("Sharing stopped.").to_string();
                    }
                    if let Some(server) = self.keycard_server.as_ref() {
                        ui.add_space(10.0);
                        widgets::section_header(ui, &language.text("Add to Wallet").to_uppercase());
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(&server.url)
                                .family(theme::strong())
                                .size(15.0)
                                .color(p.accent),
                        );
                        ui.add_space(4.0);
                        widgets::caption(
                            ui,
                            language.text("Open the link on your iPhone to add the keycard to Wallet."),
                        );
                    }
                } else if widgets::button(
                    ui,
                    language.text("Share on LAN"),
                    ButtonKind::Secondary,
                    has_pass,
                    true,
                )
                .clicked()
                {
                    self.share_keycard_on_lan();
                }

                if !self.keycard_status.is_empty() {
                    ui.add_space(12.0);
                    widgets::hairline(ui);
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&self.keycard_status)
                            .size(12.0)
                            .color(p.text_secondary),
                    );
                }
            });
        });
    }

    fn generate_keycard(&mut self) {
        let dir = self
            .keycard_output_dir
            .clone()
            .unwrap_or_else(|| crate::paths::app_data_dir().join("keycards"));
        let stem = sanitize_stem(&self.keycard.draft.house_name);
        match crate::keycard::build_pkpass(&self.keycard.draft, &self.keycard.signing, &dir, &stem)
        {
            Ok((path, signed)) => {
                // A new bundle invalidates the old LAN share link.
                self.keycard_server = None;
                self.keycard_status = if signed {
                    format!("Signed keycard saved to {}", path.display())
                } else {
                    format!(
                        "Unsigned keycard saved to {} (add a certificate to install it on Wallet)",
                        path.display()
                    )
                };
                self.add_log(self.keycard_status.clone());
                self.keycard_last_path = Some(path);
            }
            Err(err) => {
                self.keycard_status = format!("Failed to build keycard: {err:#}");
                self.add_log(self.keycard_status.clone());
            }
        }
        self.persist_settings();
    }

    fn save_keycard_copy(&mut self) {
        let Some(source) = self.keycard_last_path.clone() else {
            return;
        };
        let default_name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "keycard.pkpass".to_string());
        if let Some(target) = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter("Wallet pass", &["pkpass"])
            .save_file()
        {
            match std::fs::copy(&source, &target) {
                Ok(_) => {
                    self.keycard_status = format!("Saved keycard to {}", target.display());
                    self.add_log(self.keycard_status.clone());
                }
                Err(err) => {
                    self.keycard_status = format!("Failed to save keycard: {err}");
                    self.add_log(self.keycard_status.clone());
                }
            }
        }
    }

    fn send_keycard_to_device(&mut self) {
        let Some(source) = self.keycard_last_path.clone() else {
            return;
        };
        let Some(udid) = self.selected_udid.clone() else {
            self.keycard_status = self
                .language
                .text("Connect an iPhone to get started.")
                .to_string();
            return;
        };
        let file_name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "keycard.pkpass".to_string());
        match crate::keycard::push_to_device(&udid, self.connection_mode, &source, &file_name) {
            Ok(device_path) => {
                self.keycard_status = format!("Keycard sent to {device_path}");
                self.add_log(self.keycard_status.clone());
            }
            Err(err) => {
                self.keycard_status = format!("Failed to send keycard: {err:#}");
                self.add_log(self.keycard_status.clone());
            }
        }
    }

    fn share_keycard_on_lan(&mut self) {
        let Some(source) = self.keycard_last_path.clone() else {
            return;
        };
        let file_name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "keycard.pkpass".to_string());
        match crate::keycard::serve_pkpass(&source, &file_name) {
            Ok(server) => {
                self.keycard_status = format!("Sharing at {}", server.url);
                self.add_log(self.keycard_status.clone());
                self.keycard_server = Some(server);
            }
            Err(err) => {
                self.keycard_status = format!("Failed to start sharing: {err:#}");
            }
        }
    }
}

fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    widgets::section_header(ui, &label.to_uppercase());
    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(10.0);
}

/// Like `text_field`, but masks the value (certificate passwords).
fn password_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    widgets::section_header(ui, &label.to_uppercase());
    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::singleline(value)
            .password(true)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(10.0);
}

fn color_row(ui: &mut egui::Ui, label: &str, value: &mut [u8; 3]) {
    ui.horizontal(|ui| {
        widgets::caption(ui, label);
        ui.color_edit_button_srgb(value);
    });
    ui.add_space(6.0);
}

fn image_row(
    ui: &mut egui::Ui,
    language: Language,
    label: &str,
    path: &mut Option<std::path::PathBuf>,
) {
    widgets::section_header(ui, &label.to_uppercase());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let current = path
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| language.text("Default").to_string());
        widgets::caption(ui, &current);
        if widgets::button(
            ui,
            language.text("Choose image"),
            ButtonKind::Secondary,
            true,
            false,
        )
        .clicked()
            && let Some(picked) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
        {
            *path = Some(picked);
        }
        if path.is_some()
            && widgets::button(ui, language.text("Clear"), ButtonKind::Quiet, true, false).clicked()
        {
            *path = None;
        }
    });
    ui.add_space(10.0);
}

fn p12_row(ui: &mut egui::Ui, language: Language, path: &mut Option<std::path::PathBuf>) {
    widgets::section_header(ui, &language.text("Certificate (.p12)").to_uppercase());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let current = path
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| language.text("Default").to_string());
        widgets::caption(ui, &current);
        if widgets::button(
            ui,
            language.text("Choose .p12..."),
            ButtonKind::Secondary,
            true,
            false,
        )
        .clicked()
            && let Some(picked) = rfd::FileDialog::new()
                .add_filter("PKCS#12", &["p12", "pfx"])
                .pick_file()
        {
            *path = Some(picked);
        }
        if path.is_some()
            && widgets::button(ui, language.text("Clear"), ButtonKind::Quiet, true, false).clicked()
        {
            *path = None;
        }
    });
    ui.add_space(10.0);
}

fn cert_row(
    ui: &mut egui::Ui,
    language: Language,
    label: &str,
    path: &mut Option<std::path::PathBuf>,
) {
    widgets::section_header(ui, &label.to_uppercase());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let current = path
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| language.text("Default").to_string());
        widgets::caption(ui, &current);
        if widgets::button(
            ui,
            language.text("Choose certificate..."),
            ButtonKind::Secondary,
            true,
            false,
        )
        .clicked()
            && let Some(picked) = rfd::FileDialog::new()
                .add_filter("Certificates", &["pem", "cer", "crt"])
                .pick_file()
        {
            *path = Some(picked);
        }
        if path.is_some()
            && widgets::button(ui, language.text("Clear"), ButtonKind::Quiet, true, false).clicked()
        {
            *path = None;
        }
    });
    ui.add_space(10.0);
}

fn preview(ui: &mut egui::Ui, draft: &KeycardDraft) {
    let width = ui.available_width().clamp(220.0, 340.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, width * 0.62), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(18),
        egui::Color32::from_rgb(
            draft.background[0],
            draft.background[1],
            draft.background[2],
        ),
    );
    let fg = egui::Color32::from_rgb(
        draft.foreground[0],
        draft.foreground[1],
        draft.foreground[2],
    );
    let label = egui::Color32::from_rgb(
        draft.label_color[0],
        draft.label_color[1],
        draft.label_color[2],
    );
    let pad = 18.0;
    let left = rect.left() + pad;

    painter.text(
        egui::pos2(left, rect.top() + pad),
        egui::Align2::LEFT_TOP,
        &draft.house_name,
        egui::FontId::proportional(19.0),
        fg,
    );

    let mut y = rect.top() + pad + 44.0;
    for (caption, value) in [
        ("OWNER", draft.owner_name.as_str()),
        ("ACCESS", draft.access_label.as_str()),
    ] {
        painter.text(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            caption,
            egui::FontId::proportional(10.0),
            label,
        );
        painter.text(
            egui::pos2(left, y + 14.0),
            egui::Align2::LEFT_TOP,
            if value.trim().is_empty() {
                "—"
            } else {
                value
            },
            egui::FontId::proportional(15.0),
            fg,
        );
        y += 40.0;
    }

    if !draft.nfc_payload.trim().is_empty() {
        painter.text(
            egui::pos2(rect.right() - pad, rect.bottom() - pad),
            egui::Align2::RIGHT_BOTTOM,
            "NFC",
            egui::FontId::proportional(12.0),
            label,
        );
    }
}

fn sanitize_stem(name: &str) -> String {
    // Keep letters/digits from any script, but make the result safe to use as
    // a Windows file name: no separators or reserved characters, no trailing
    // dots/spaces, and no device names such as CON or NUL.
    let mut stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while stem.contains("--") {
        stem = stem.replace("--", "-");
    }
    let mut stem = stem.trim_matches('-').trim().to_string();
    if stem.is_empty() {
        return "keycard".to_string();
    }
    stem = stem.chars().take(64).collect();
    stem = stem.trim_end_matches(['.', ' ', '-']).to_string();
    if stem.is_empty() {
        return "keycard".to_string();
    }

    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.to_ascii_uppercase().as_str()) {
        format!("keycard-{stem}")
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_stem;

    #[test]
    fn sanitize_stem_produces_safe_file_names() {
        assert_eq!(sanitize_stem("My House"), "My-House");
        assert_eq!(sanitize_stem("  Front/Door  "), "Front-Door");
        assert_eq!(sanitize_stem("Café ☕"), "Café");
        assert_eq!(sanitize_stem("..."), "keycard");
        assert_eq!(sanitize_stem("CON"), "keycard-CON");
        assert_eq!(sanitize_stem("nul.pass"), "nul-pass");
        assert!(sanitize_stem(&"a".repeat(200)).len() <= 64);
    }
}
