//! Background install / uninstall operations.
//!
//! The worker thread reports progress over an mpsc channel; the UI polls it
//! every frame. All child processes are spawned without a console window.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub const APP_EXE: &str = "silo.exe";
pub const UNINSTALL_EXE: &str = "uninstall.exe";

const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Silo";

pub enum Msg {
    Progress(f32, String),
    Done,
    Failed(String),
}

/// Resolve the current user's local app-data folder. Avoids hard-coding
/// `C:\Users\Default`, which belongs to another profile and may not be
/// writable when `LOCALAPPDATA` is missing.
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

/// Default per-user install location (no admin rights required).
pub fn default_install_dir() -> PathBuf {
    local_app_data_dir().join("Programs").join("Silo")
}

/// Where to uninstall from: this exe's folder when it runs as the installed
/// `uninstall.exe` (or sits next to `silo.exe`), otherwise the default path.
pub fn uninstall_source_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let is_uninstaller = exe
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(UNINSTALL_EXE));
        if is_uninstaller || dir.join(APP_EXE).exists() {
            return dir.to_path_buf();
        }
    }
    default_install_dir()
}

fn no_console(cmd: &mut Command) {
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
}

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn send(tx: &Sender<Msg>, fraction: f32, label: &str) {
    let _ = tx.send(Msg::Progress(fraction, label.to_string()));
}

fn taskkill(name: &str) {
    let mut cmd = Command::new("taskkill");
    cmd.args(["/IM", name, "/F"]);
    no_console(&mut cmd);
    let _ = cmd.output();
}

/// Run a PowerShell script from a temp file (avoids quoting hell).
fn run_ps(script: &str) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("silo-setup-{}.ps1", std::process::id()));
    fs::write(&path, script).map_err(|e| format!("could not write temp script: {e}"))?;
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&path);
    no_console(&mut cmd);
    let result = cmd.output();
    let _ = fs::remove_file(&path);
    match result {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let msg = if !stderr.is_empty() { stderr } else { stdout };
            Err(if msg.is_empty() {
                "PowerShell failed".into()
            } else {
                msg
            })
        }
        Err(e) => Err(format!("could not run PowerShell: {e}")),
    }
}

fn ps_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn shortcut_script(dir: &Path, desktop: bool) -> String {
    let dir_q = ps_quote(&dir.to_string_lossy());
    let exe = format!("{dir_q}\\{APP_EXE}");
    let mut script = format!(
        r#"
$w = New-Object -ComObject WScript.Shell
$exe = '{exe}'
function Make-Shortcut([string]$folder, [string]$name) {{
    $path = Join-Path $folder $name
    $s = $w.CreateShortcut($path)
    $s.TargetPath = $exe
    $s.WorkingDirectory = '{dir_q}'
    $s.Description = 'Silo'
    $s.IconLocation = $exe + ',0'
    $s.Save()
    if (-not (Test-Path $path)) {{ throw ('Could not create ' + $path) }}
}}
Make-Shortcut ([Environment]::GetFolderPath('Programs')) 'Silo.lnk'
"#
    );
    if desktop {
        script.push_str("Make-Shortcut ([Environment]::GetFolderPath('Desktop')) 'Silo.lnk'\n");
    }
    script
}

fn remove_shortcuts_script() -> String {
    r#"
$targets = @(
    (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Silo.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'Silo.lnk')
)
foreach ($t in $targets) { if (Test-Path $t) { Remove-Item $t -Force } }
"#
    .to_string()
}

fn registry_script(dir: &Path, size_kb: u32) -> String {
    let dir_q = ps_quote(&dir.to_string_lossy());
    format!(
        r#"
$k = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Silo'
$dir = '{dir_q}'
New-Item -Path $k -Force | Out-Null
Set-ItemProperty -Path $k -Name 'DisplayName' -Value 'Silo'
Set-ItemProperty -Path $k -Name 'DisplayVersion' -Value '{version}'
Set-ItemProperty -Path $k -Name 'Publisher' -Value 'Silo'
Set-ItemProperty -Path $k -Name 'InstallLocation' -Value $dir
Set-ItemProperty -Path $k -Name 'UninstallString' -Value ('"' + $dir + '\{UNINSTALL_EXE}" --uninstall')
Set-ItemProperty -Path $k -Name 'DisplayIcon' -Value ($dir + '\{APP_EXE}')
Set-ItemProperty -Path $k -Name 'EstimatedSize' -Value {size_kb} -Type DWord
Set-ItemProperty -Path $k -Name 'NoModify' -Value 1 -Type DWord
Set-ItemProperty -Path $k -Name 'NoRepair' -Value 1 -Type DWord
if (-not (Get-ItemProperty -Path $k).UninstallString) {{ throw 'UninstallString missing' }}
"#,
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn remove_registry_key() {
    let mut cmd = Command::new("reg");
    cmd.args(["delete", UNINSTALL_KEY, "/f"]);
    no_console(&mut cmd);
    let _ = cmd.output();
}

/// Best-effort cleanup of a failed/partial install.
fn cleanup_temp(dir: &Path) {
    let _ = fs::remove_file(dir.join("silo.exe.tmp"));
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

pub fn start_install(
    dir: PathBuf,
    desktop_shortcut: bool,
    payload: &'static [u8],
    tx: Sender<Msg>,
) {
    std::thread::spawn(move || {
        if let Err(e) = install(&dir, desktop_shortcut, payload, &tx) {
            cleanup_temp(&dir);
            let _ = tx.send(Msg::Failed(e));
        }
    });
}

fn install(
    dir: &Path,
    desktop_shortcut: bool,
    payload: &[u8],
    tx: &Sender<Msg>,
) -> Result<(), String> {
    send(tx, 0.02, "Preparing…");
    sleep(350);

    send(tx, 0.05, "Closing running Silo…");
    taskkill(APP_EXE);
    sleep(450);

    send(tx, 0.10, "Creating install folder…");
    fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    sleep(300);

    // Write the payload in chunks so the progress bar has real data.
    send(tx, 0.14, "Copying Silo…");
    let total = payload.len().max(1) as f32;
    let tmp = dir.join("silo.exe.tmp");
    {
        let mut file =
            File::create(&tmp).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
        let mut written: f32 = 0.0;
        for (i, chunk) in payload.chunks(256 * 1024).enumerate() {
            file.write_all(chunk)
                .map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
            written += chunk.len() as f32;
            if i % 4 == 0 {
                send(tx, 0.14 + 0.55 * (written / total), "Copying Silo…");
            }
        }
        file.flush().map_err(|e| format!("Write failed: {e}"))?;
    }
    // The previous build may still be releasing its file lock right after
    // taskkill, so retry the replacement for a few seconds instead of failing
    // the whole installation immediately.
    let target_exe = dir.join(APP_EXE);
    let mut last_error = None;
    for attempt in 0..20 {
        match fs::rename(&tmp, &target_exe) {
            Ok(()) => {
                last_error = None;
                break;
            }
            Err(error) => {
                last_error = Some(error);
                if attempt < 19 {
                    sleep(250);
                }
            }
        }
    }
    if let Some(error) = last_error {
        return Err(format!(
            "Could not replace {dir}\\{APP_EXE}: {error}. Close Silo and try again.",
            dir = dir.display()
        ));
    }
    sleep(300);

    send(tx, 0.72, "Installing uninstaller…");
    let self_exe =
        std::env::current_exe().map_err(|e| format!("Could not locate this installer: {e}"))?;
    fs::copy(&self_exe, dir.join(UNINSTALL_EXE))
        .map_err(|e| format!("Could not install uninstaller: {e}"))?;
    sleep(350);

    send(tx, 0.80, "Creating shortcuts…");
    run_ps(&shortcut_script(dir, desktop_shortcut))?;
    sleep(350);

    send(tx, 0.88, "Registering in Apps & features…");
    let size_kb = (payload.len() / 1024) as u32;
    run_ps(&registry_script(dir, size_kb))?;
    sleep(350);

    send(tx, 1.0, "Finishing up…");
    let _ = tx.send(Msg::Done);
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall
// ---------------------------------------------------------------------------

pub fn start_uninstall(dir: PathBuf, remove_data: bool, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        if let Err(e) = uninstall(&dir, remove_data, &tx) {
            let _ = tx.send(Msg::Failed(e));
        }
    });
}

fn uninstall(dir: &Path, remove_data: bool, tx: &Sender<Msg>) -> Result<(), String> {
    send(tx, 0.05, "Closing Silo…");
    taskkill(APP_EXE);
    sleep(450);

    send(tx, 0.20, "Removing shortcuts…");
    let _ = run_ps(&remove_shortcuts_script());
    sleep(350);

    send(tx, 0.38, "Removing Apps & features entry…");
    remove_registry_key();
    sleep(300);

    send(tx, 0.50, "Removing application files…");
    let _ = fs::remove_file(dir.join(APP_EXE));
    let _ = fs::remove_file(dir.join("silo.exe.tmp"));
    sleep(300);

    if remove_data {
        send(tx, 0.68, "Deleting settings, cards and backups…");
        let base = local_app_data_dir();
        let _ = fs::remove_dir_all(base.join("Silo"));
        // The pre-rename data folder may still exist if migration copied
        // rather than moved it.
        let _ = fs::remove_dir_all(base.join("AirCard"));
        sleep(350);
    }

    send(tx, 0.82, "Cleaning up…");
    // Remove everything in the install folder except this running uninstaller.
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) == Some(UNINSTALL_EXE) {
                continue;
            }
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }
    schedule_self_delete(dir);

    send(tx, 1.0, "Done");
    let _ = tx.send(Msg::Done);
    Ok(())
}

/// Ask a hidden helper process to delete the install folder once this
/// process exits (an uninstaller cannot delete its own image while running).
fn schedule_self_delete(dir: &Path) {
    if !dir.exists() {
        return;
    }
    let d = ps_quote(&dir.to_string_lossy());
    // Wait 2s for us to exit, then retry for up to ~90s (the folder can only
    // disappear once uninstall.exe has been released by the OS).
    let script = format!(
        "Start-Sleep -Seconds 2; for ($i = 0; $i -lt 90; $i++) {{ if (-not (Test-Path -LiteralPath '{d}')) {{ break }}; Remove-Item -LiteralPath '{d}' -Recurse -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 1 }}"
    );
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &script,
    ]);
    no_console(&mut cmd);
    let _ = cmd.spawn();
}
