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
mod keycard;
mod passthm;
mod paths;
mod scanner;
mod ui;
mod updater;
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
    // Detached helper mode: replace the old executable and relaunch it. This
    // runs from the freshly downloaded build, so no window is shown.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(updater::APPLY_UPDATE_FLAG) {
        let target = args.get(2).cloned().unwrap_or_default();
        let source = args.get(3).cloned().unwrap_or_default();
        let code =
            updater::run_apply_update(std::path::Path::new(&target), std::path::Path::new(&source));
        std::process::exit(code);
    }
    // Hidden diagnostic: `silo.exe --silo-check-update [version]` prints the
    // result of a release check to stdout (only visible in debug builds, which
    // keep a console) and exits. An optional version overrides the built-in one
    // so older builds can be simulated.
    if args.get(1).map(String::as_str) == Some("--silo-check-update") {
        let current = args.get(2).map(String::as_str).unwrap_or(app::APP_VERSION);
        match updater::check_for_update(current) {
            Ok(Some(info)) => println!(
                "update available: {} -> {} ({})",
                current, info.version, info.tag
            ),
            Ok(None) => println!("up to date ({})", current),
            Err(e) => println!("check failed: {e:#}"),
        }
        return Ok(());
    }

    // Hidden diagnostic: `silo.exe --silo-list-cards [hash]` lists the
    // on-device Wallet card directory and probes the given hash.
    if args.get(1).map(String::as_str) == Some("--silo-list-cards") {
        match wallet_backup::diagnose_cards(args.get(2).map(String::as_str)) {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-read-card <hash>` attempts to read
    // the card artwork through the symlink escape.
    if args.get(1).map(String::as_str) == Some("--silo-read-card") {
        let hash = args.get(2).cloned().unwrap_or_default();
        match wallet_backup::diagnose_read(&hash) {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-ls <path>` lists an on-device
    // directory through the symlink escape.
    if args.get(1).map(String::as_str) == Some("--silo-ls") {
        let path = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "/var/mobile/Library/Passes/Cards".to_string());
        match wallet_backup::diagnose_ls(&path) {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-roundtrip` validates the full
    // write+read round-trip on a probe file in a protected Library directory.
    if args.get(1).map(String::as_str) == Some("--silo-roundtrip") {
        match wallet_backup::diagnose_roundtrip() {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-cleanup` removes leftover staging.
    if args.get(1).map(String::as_str) == Some("--silo-cleanup") {
        match wallet_backup::diagnose_cleanup() {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-read-card2 <hash>` reads the card
    // artwork via a two-stage device-side move out of the Library.
    if args.get(1).map(String::as_str) == Some("--silo-read-card2") {
        let hash = args.get(2).cloned().unwrap_or_default();
        match wallet_backup::diagnose_read2(&hash) {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-test-move` verifies the AirTraffic
    // exploit moves a staged file to a Media destination without host bytes.
    if args.get(1).map(String::as_str) == Some("--silo-test-move") {
        match wallet_backup::diagnose_move() {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }
    // Hidden diagnostic: `silo.exe --silo-test-stage` validates the
    // streaming-zip conduit staging path.
    if args.get(1).map(String::as_str) == Some("--silo-test-stage") {
        match wallet_backup::diagnose_stage() {
            Ok(report) => println!("{report}"),
            Err(e) => println!("diagnostic failed: {e:#}"),
        }
        return Ok(());
    }

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
