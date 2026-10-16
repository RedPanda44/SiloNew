#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod afc;
mod airlift;
mod airtraffic;
mod app;
mod apple;
mod device;
mod flasher;
mod i18n;
mod image_skin;
mod passthm;
mod paths;
mod scanner;
mod ui;
mod wallet_backup;

/// Decode the bundled logo for the window title bar / taskbar icon.
fn window_icon() -> Option<std::sync::Arc<egui::IconData>> {
    const PNG: &[u8] = include_bytes!("../assets/icon.png");
    image::load_from_memory(PNG).ok().map(|img| {
        let rgba = img.into_rgba8();
        let (width, height) = rgba.dimensions();
        std::sync::Arc::new(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    })
}

fn main() -> eframe::Result<()> {
    let app_title = format!("Silo v{}", app::APP_VERSION);
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 800.0])
        // Keep the minimum small enough that the whole window still fits on
        // 125-150% scaled laptops (the first frame shrinks it to the work
        // area when needed).
        .with_min_inner_size([900.0, 560.0])
        .with_title(app_title.as_str());
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        app_title.as_str(),
        options,
        Box::new(|cc| Ok(Box::new(app::SiloApp::new(cc)))),
    )
}
