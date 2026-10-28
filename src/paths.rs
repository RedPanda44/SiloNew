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
    let local_app_data = local_app_data_dir();
    let new = local_app_data.join("Silo");
    let old = local_app_data.join("AirCard");

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

/// Resolve the per-user local app-data folder without falling back to another
/// user's profile. `LOCALAPPDATA` can be missing in stripped-down or service
/// environments, where using `C:\Users\Default` would silently write into a
/// profile the current user cannot own.
fn local_app_data_dir() -> PathBuf {
    if let Ok(value) = std::env::var("LOCALAPPDATA") {
        let value = value.trim();
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let profile = profile.trim();
        if !profile.is_empty() {
            return PathBuf::from(profile).join("AppData").join("Local");
        }
    }
    std::env::temp_dir()
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

/// Write a file through a temporary sibling and rename it into place, so a
/// crash or power loss cannot leave a half-written settings/cards file.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());
    let temporary = parent.join(format!(".{file_name}.tmp"));
    fs::write(&temporary, contents)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}
