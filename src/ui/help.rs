//! Help & Guides: setup, card-hash finding and activation walkthroughs.

use eframe::egui;

use crate::app::SiloApp;
use crate::ui::widgets;

impl SiloApp {
    pub(crate) fn show_help_tab(&mut self, ui: &mut egui::Ui) {
        let language = self.language;

        ui.columns(2, |cols| {
            widgets::card(&mut cols[0], |ui| {
                widgets::heading(ui, language.text("Setup & Card Hash Guide"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Everything you need to connect and capture your card"),
                );
                ui.add_space(14.0);

                widgets::section_header(ui, &language.text("Prerequisites").to_uppercase());
                ui.add_space(8.0);
                widgets::bullet_row(
                    ui,
                    language.text("- 64-bit iTunes or Apple Mobile Device Support installed"),
                );
                widgets::bullet_row(
                    ui,
                    language
                        .text("- First-time setup: connect by USB and tap \"Trust this Computer\""),
                );
                widgets::bullet_row(
                    ui,
                    language.text("- WiFi: enable WiFi sync, then use the same local network"),
                );
                widgets::bullet_row(
                    ui,
                    language.text("- Select Auto, USB only, or WiFi only in the top bar"),
                );

                ui.add_space(14.0);
                widgets::hairline(ui);

                widgets::section_header(
                    ui,
                    &language.text("Finding Your Card Hash").to_uppercase(),
                );
                ui.add_space(8.0);
                widgets::step_row(ui, 1, language.text("1. Click \"Scan\" in the Wallet tab"));
                widgets::step_row(ui, 2, language.text("2. Open Apple Wallet on your iPhone"));
                widgets::step_row(
                    ui,
                    3,
                    language.text("3. Tap the card you want to customize"),
                );
                widgets::step_row(
                    ui,
                    4,
                    language.text("4. Silo captures the pass hash automatically"),
                );
                widgets::step_row(ui, 5, language.text("5. Click \"Stop\" once detected"));
            });

            widgets::card(&mut cols[1], |ui| {
                widgets::heading(ui, language.text("Activation & Theme Guide"));
                ui.add_space(2.0);
                widgets::caption(
                    ui,
                    language.text("Applying skins and dialer keypad packages"),
                );
                ui.add_space(14.0);

                widgets::section_header(
                    ui,
                    &language.text("Activating Apple Wallet Skin").to_uppercase(),
                );
                ui.add_space(8.0);
                widgets::step_row(
                    ui,
                    1,
                    language.text("1. Click \"Apply Card Skin\" and wait for completion"),
                );
                widgets::step_row(
                    ui,
                    2,
                    language.text("2. Open App Switcher on iPhone (swipe up from bottom)"),
                );
                widgets::step_row(
                    ui,
                    3,
                    language.text("3. Force close Apple Wallet by swiping up on it"),
                );
                widgets::step_row(
                    ui,
                    4,
                    language.text("4. Reopen Wallet - your new skin appears!"),
                );

                ui.add_space(14.0);
                widgets::hairline(ui);

                widgets::section_header(
                    ui,
                    &language.text("Passcode Themes (.passthm)").to_uppercase(),
                );
                ui.add_space(8.0);
                widgets::bullet_row(
                    ui,
                    language.text("- Compatible with Cowabunga & Nugget theme packages"),
                );
                widgets::bullet_row(
                    ui,
                    language.text("- iOS 18+: Select \"Auto (TelephonyUI-10)\""),
                );
                widgets::bullet_row(ui, language.text("- iOS 16-17: Select \"TelephonyUI-9\""));
                widgets::bullet_row(
                    ui,
                    language.text("- Lock screen to verify your updated keypad artwork"),
                );
            });
        });
    }
}
