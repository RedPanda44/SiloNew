//! Silo Installer — a dark, animated setup wizard written from scratch
//! with egui. Embeds `target/release/silo.exe` as its payload.
//!
//! Run with `--uninstall` to drive the uninstall flow (this is what the
//! registered "Apps & features" entry invokes).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ops;
mod theme;

/// App version shown in the wizard (kept in lockstep with the app crate).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TITLE: &str = "Silo Installer";

/// The release build of Silo, embedded into this installer.
pub const PAYLOAD: &[u8] = include_bytes!("../../target/release/silo.exe");

/// Decode the bundled logo for the window title bar / taskbar icon.
fn window_icon() -> Option<std::sync::Arc<egui::IconData>> {
    const PNG: &[u8] = include_bytes!("../../assets/icon.png");
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

/// Paint our native title bar dark (DWMWA_USE_IMMERSIVE_DARK_MODE).
#[cfg(windows)]
fn enable_dark_title_bar() {
    unsafe extern "system" {
        fn FindWindowW(
            lp_class_name: *const u16,
            lp_window_name: *const u16,
        ) -> *mut std::ffi::c_void;
        fn DwmSetWindowAttribute(
            hwnd: *mut std::ffi::c_void,
            attribute: u32,
            value: *const u32,
            size: u32,
        ) -> i32;
    }

    let title = TITLE.to_string();
    std::thread::spawn(move || {
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        // The window appears shortly after startup; poll until we find it.
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let hwnd = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
            if !hwnd.is_null() {
                let on = 1u32;
                for attribute in [20u32, 19u32] {
                    unsafe { DwmSetWindowAttribute(hwnd, attribute, &on, 4) };
                }
                return;
            }
        }
    });
}

/// Bind Segoe UI Bold as the "strong" family (mirrors the Silo app).
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let strong = egui::FontFamily::Name("strong".into());
    let fonts_dir = std::path::Path::new(r"C:\Windows\Fonts");
    for candidate in ["segoeuib.ttf", "seguisb.ttf"] {
        let path = fonts_dir.join(candidate);
        if let Ok(bytes) = std::fs::read(&path) {
            fonts.font_data.insert(
                "strong-bold".to_owned(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts
                .families
                .insert(strong.clone(), vec!["strong-bold".to_owned()]);
            break;
        }
    }
    // Always bind the family so `FontFamily::Name("strong")` can never panic.
    if !fonts.families.contains_key(&strong) {
        let fallback = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        fonts.families.insert(strong, fallback);
    }
    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    let uninstall = std::env::args().any(|arg| arg == "--uninstall");

    #[cfg(windows)]
    enable_dark_title_bar();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_inner_size([560.0, 420.0])
        .with_min_inner_size([560.0, 420.0])
        .with_max_inner_size([560.0, 420.0])
        .with_resizable(false);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }

    eframe::run_native(
        TITLE,
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(app::InstallerApp::new(uninstall)))
        }),
    )
}
