//! Update checker and in-place updater.
//!
//! Queries the project's GitHub Releases, compares the published tag against
//! the running build, and — when the user asks — downloads the new
//! `silo.exe` and swaps it over the running executable. No installer
//! round-trip is required: a detached copy of the new build waits for the
//! old process to exit, replaces the file, and relaunches it.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::paths::app_data_dir;

/// GitHub repository that publishes the Windows builds.
pub const UPDATE_REPO: &str = "RedPanda44/SiloNew";
const USER_AGENT: &str = "Silo-Windows-Updater";

/// The argument the running app passes to the freshly downloaded build so it
/// enters "replace the old file and relaunch" mode instead of showing a UI.
pub const APPLY_UPDATE_FLAG: &str = "--silo-apply-update";

/// A published release that is newer than the running build.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// Cleaned version, e.g. `1.5.2` (the tag without its leading `v`).
    pub version: String,
    /// Raw tag, e.g. `v1.5.2`.
    pub tag: String,
    pub name: String,
    pub notes: String,
    pub download_url: Option<String>,
    pub download_size: u64,
    pub html_url: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

fn agent() -> ureq::Agent {
    // ureq's native-tls backend must be attached explicitly; on Windows this
    // uses the OS (schannel) certificate store, so no bundled roots are needed.
    let tls = ureq::native_tls::TlsConnector::new()
        .expect("failed to initialise the system TLS backend");
    ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .tls_connector(std::sync::Arc::new(tls))
        .timeout_connect(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
}

/// Turn `v1.5.2`, `1.5`, `1.6.0-rc1` into a comparable numeric tuple.
pub fn parse_version(v: &str) -> (u64, u64, u64) {
    let cleaned = v.trim().trim_start_matches(['v', 'V']);
    let mut parts = cleaned
        .split(['.', '-', '+'])
        .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>());
    let num = |s: Option<String>| s.and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    (num(parts.next()), num(parts.next()), num(parts.next()))
}

/// True when `remote` is a strictly newer version than `current`.
pub fn is_newer(remote: &str, current: &str) -> bool {
    parse_version(remote) > parse_version(current)
}

/// Ask GitHub for the latest release. `Ok(Some(_))` when a newer build exists,
/// `Ok(None)` when the running build is current.
pub fn check_for_update(current_version: &str) -> Result<Option<ReleaseInfo>> {
    let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
    let response = match agent().get(&url).call() {
        Ok(response) => response,
        // No releases published yet.
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(error) => {
            return Err(anyhow!(error).context("Could not reach the update server"));
        }
    };
    let release: ApiRelease = serde_json::from_reader(response.into_reader())
        .context("The update server returned an unexpected response")?;

    let version = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    if !is_newer(&version, current_version) {
        return Ok(None);
    }

    let asset = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("silo.exe"))
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|a| a.name.to_ascii_lowercase().ends_with(".exe"))
        });

    Ok(Some(ReleaseInfo {
        version,
        tag: release.tag_name,
        name: release.name.unwrap_or_default(),
        notes: release.body.unwrap_or_default(),
        download_url: asset.map(|a| a.browser_download_url.clone()),
        download_size: asset.map(|a| a.size).unwrap_or(0),
        html_url: release.html_url,
    }))
}

/// Folder that holds downloaded updates, under the app's data directory.
pub fn updates_dir() -> PathBuf {
    app_data_dir().join("updates")
}

/// Delete any leftover downloaded builds from an earlier session.
pub fn cleanup_updates() {
    if let Ok(entries) = fs::read_dir(updates_dir()) {
        for entry in entries.flatten() {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Download the release's executable to the updates folder. `progress` is
/// called with `(bytes_done, bytes_total)`; `total` is 0 when unknown.
pub fn download_update<F>(info: &ReleaseInfo, mut progress: F) -> Result<PathBuf>
where
    F: FnMut(u64, u64),
{
    let url = info
        .download_url
        .as_deref()
        .ok_or_else(|| anyhow!("This release does not publish a Windows executable"))?;

    let dir = updates_dir();
    fs::create_dir_all(&dir).context("Could not create the updates folder")?;
    let dest = dir.join(format!("silo-{}.new.exe", info.version));

    let response = agent()
        .get(url)
        .call()
        .context("Could not download the update")?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(info.download_size);

    let mut reader = response.into_reader();
    let mut file = fs::File::create(&dest).context("Could not write the downloaded update")?;
    let mut buf = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .context("The update download was interrupted")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .context("Could not write the downloaded update")?;
        done += n as u64;
        progress(done, total);
    }
    let _ = file.sync_all();

    // A Windows executable starts with the "MZ" signature.
    let mut header = [0u8; 2];
    fs::File::open(&dest)?.read_exact(&mut header)?;
    if &header != b"MZ" {
        let _ = fs::remove_file(&dest);
        bail!("The downloaded file is not a valid Windows executable");
    }
    Ok(dest)
}

/// Start the detached helper that will replace the running executable, then
/// relaunch it. The caller must exit the app right after this returns.
pub fn launch_apply_update(new_exe: &Path, target: &Path) -> Result<()> {
    let mut cmd = Command::new(new_exe);
    cmd.arg(APPLY_UPDATE_FLAG)
        .arg(target)
        .arg(new_exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    cmd.spawn()
        .context("Could not start the update helper")?;
    Ok(())
}

/// Entry point for the detached helper: copy ourselves over `target` once the
/// old process has released the lock, then relaunch it. Returns an exit code.
pub fn run_apply_update(target: &Path, source: &Path) -> i32 {
    let mut replaced = false;
    for _ in 0..240 {
        // While the old app is still running, `target` is locked and the copy
        // fails; once it exits, the copy succeeds.
        if fs::copy(source, target).is_ok() {
            replaced = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    // Relaunch whatever is at the target path now: the new build on success,
    // or the old one if the replacement could not be written, so the user is
    // never left without a window.
    let _ = Command::new(target).spawn();
    if replaced { 0 } else { 1 }
}

/// Open a URL in the user's default browser.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing_and_comparison() {
        assert_eq!(parse_version("v1.5.2"), (1, 5, 2));
        assert_eq!(parse_version("1.5"), (1, 5, 0));
        assert_eq!(parse_version("2.0.0-rc1"), (2, 0, 0));
        assert!(is_newer("1.5.2", "1.5.1"));
        assert!(is_newer("1.6.0", "1.5.9"));
        assert!(is_newer("v2.0.0", "1.99.99"));
        assert!(!is_newer("1.5.1", "1.5.1"));
        assert!(!is_newer("1.5.0", "1.5.1"));
    }
}
