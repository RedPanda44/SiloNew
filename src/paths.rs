//! Centralised location of the app's data folder, with a one-time migration
//! from the pre-rename `AirCard` folder to `Silo`.

use std::fs;
use std::path::{Path, PathBuf};

/// `%LOCALAPPDATA%\Silo` — holds settings.json, cards.json and wallet backups.
///
/// On the first run after the rename, whatever is already in the legacy
/// `%LOCALAPPDATA%\AirCard` folder is moved over so existing settings, saved
/// cards and backups are preserved.
pub fn app_data_dir() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| r"C:\Users\Default\AppData\Local".to_string());
    let new = PathBuf::from(&local_app_data).join("Silo");
    let old = PathBuf::from(&local_app_data).join("AirCard");

    if old.is_dir() {
        if new.is_dir() {
            migrate_missing(&old, &new);
        } else if fs::rename(&old, &new).is_err() {
            // Rename refused (locked file, different volume, ...): copy instead.
            migrate_missing(&old, &new);
        }
    }
    new
}

/// Best-effort: bring over everything that exists in `from` but not in `to`.
fn migrate_missing(from: &Path, to: &Path) {
    let Ok(entries) = fs::read_dir(from) else {
        return;
    };
    let _ = fs::create_dir_all(to);
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if dst.exists() {
            continue;
        }
        if src.is_dir() {
            if fs::rename(&src, &dst).is_err() {
                let _ = copy_dir(&src, &dst);
            }
        } else {
            let _ = fs::copy(&src, &dst);
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)?.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}
