use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::afc::AfcClient;
use crate::device::{ActiveDeviceSession, ConnectionMode};

pub const CARD_ARTWORK_ASSETS: [&str; 3] = [
    "cardBackgroundCombined@3x.png",
    "cardBackgroundCombined@2x.png",
    "cardBackgroundCombined.pdf",
];

fn backup_root() -> PathBuf {
    crate::paths::app_data_dir().join("wallet-backups")
}

fn backup_dir(udid: &str, card_hash: &str) -> PathBuf {
    backup_root().join(format!(
        "{}-{}",
        safe_component(udid),
        stable_hash(card_hash)
    ))
}

fn safe_component(value: &str) -> String {
    let component: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if component.is_empty() {
        "unknown".to_string()
    } else {
        component
    }
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn backup_exists(udid: &str, card_hash: &str) -> bool {
    let dir = backup_dir(udid, card_hash);
    CARD_ARTWORK_ASSETS
        .iter()
        .any(|asset| dir.join(asset).is_file())
}

fn card_hash_candidates(card_hash: &str) -> Vec<String> {
    let trimmed = card_hash.trim().trim_end_matches('=');
    if !is_safe_card_hash_component(trimmed) {
        return Vec::new();
    }
    let mut candidates = vec![
        card_hash.trim().to_string(),
        trimmed.to_string(),
        format!("{trimmed}="),
        format!("{trimmed}=="),
    ];
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| {
        is_safe_card_hash_component(candidate) && seen.insert(candidate.clone())
    });
    candidates
}

/// A card hash is interpolated into a device path, so reject anything that
/// could escape the Cards directory.
fn is_safe_card_hash_component(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('.')
        && !value.contains('\\')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '='))
}

fn find_device_card_hash(afc: &AfcClient, card_hash: &str) -> Result<Option<String>> {
    for candidate in card_hash_candidates(card_hash) {
        let pkpass_dir = format!("/var/mobile/Library/Passes/Cards/{candidate}.pkpass");
        if afc.exists(&pkpass_dir) {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Resolve the exact on-device variant of a card hash (padded/unpadded) by
/// opening the device and probing `<hash>.pkpass` directory candidates.
pub fn resolve_device_card_hash(
    udid: &str,
    connection_mode: ConnectionMode,
    card_hash: &str,
) -> Result<Option<String>> {
    let session = ActiveDeviceSession::open(Some(udid), connection_mode)
        .context("Failed to open device session to resolve the Wallet card identifier")?;
    let afc = AfcClient::new(&session)
        .context("Failed to open AFC to resolve the Wallet card identifier")?;
    find_device_card_hash(&afc, card_hash)
}

pub fn load_current_card_artwork(
    udid: &str,
    connection_mode: ConnectionMode,
    card_hash: &str,
) -> Result<Vec<u8>> {
    eprintln!(
        "[dbg] load_current_card_artwork udid={udid} mode={connection_mode:?} hash={card_hash}"
    );
    let candidates = card_hash_candidates(card_hash);
    if candidates.is_empty() {
        bail!("'{}' is not a valid Wallet card hash", card_hash.trim());
    }
    for candidate in candidates {
        let card_dir = format!("/var/mobile/Library/Passes/Cards/{candidate}.pkpass");
        for asset in [
            "cardBackgroundCombined@3x.png",
            "cardBackgroundCombined@2x.png",
        ] {
            match crate::flasher::read_system_file(udid, connection_mode, &card_dir, asset, |_| {})
            {
                Ok(bytes) => {
                    eprintln!("[dbg] read OK {card_dir}/{asset} -> {} bytes", bytes.len());
                    return Ok(bytes);
                }
                Err(error) => {
                    eprintln!("[dbg] read FAIL {card_dir}/{asset}: {error:#}");
                }
            }
        }
    }
    bail!("Could not find this card on the iPhone. Open Wallet and scan the card again.")
}

pub fn capture_original_card<L>(
    udid: &str,
    connection_mode: ConnectionMode,
    card_hash: &str,
    mut log: L,
) -> Result<Option<String>>
where
    L: FnMut(&str),
{
    let dir = backup_dir(udid, card_hash);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Could not create backup directory {}", dir.display()))?;

    log("Backing up the original Wallet card face...");

    // The device only exposes the card directory through a real read, so we
    // resolve the exact hash variant (padded/unpadded) by trying to read each
    // asset. The read moves the file out and writes it straight back.
    for candidate in card_hash_candidates(card_hash) {
        let card_dir = format!("/var/mobile/Library/Passes/Cards/{candidate}.pkpass");
        let mut found_any = false;

        for asset in CARD_ARTWORK_ASSETS {
            let backup_path = dir.join(asset);
            if backup_path.is_file() {
                found_any = true;
                log(&format!("Original backup already contains {}", asset));
                continue;
            }

            let mut read_log = |message: &str| log(message);
            match crate::flasher::read_system_file(
                udid,
                connection_mode,
                &card_dir,
                asset,
                &mut read_log,
            ) {
                Ok(data) => {
                    write_backup_file(&backup_path, &data)
                        .with_context(|| format!("Failed to save original card asset {}", asset))?;
                    log(&format!("Backed up original card asset: {}", asset));
                    found_any = true;
                }
                Err(_) => {
                    log(&format!(
                        "Original card asset not present on device: {}",
                        asset
                    ));
                }
            }
        }

        if found_any {
            if candidate != card_hash {
                log(&format!(
                    "Using device Wallet card hash variant {} for the card path.",
                    candidate
                ));
            }
            return Ok(Some(candidate));
        }
    }

    if backup_exists(udid, card_hash) {
        log("Original backup exists, but the current Wallet card directory is not visible.");
        return Ok(Some(card_hash.to_string()));
    }

    log(&format!(
        "Wallet card directory not found for hash {} (also tried padded/unpadded variants); continuing without an original backup.",
        card_hash
    ));
    Ok(None)
}

pub fn load_original_assets(udid: &str, card_hash: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let dir = backup_dir(udid, card_hash);
    let mut assets = Vec::new();

    for asset in CARD_ARTWORK_ASSETS {
        let path = dir.join(asset);
        if !path.is_file() {
            continue;
        }
        let data = fs::read(&path)
            .with_context(|| format!("Could not read backup file {}", path.display()))?;
        assets.push((asset.to_string(), data));
    }

    if assets.is_empty() {
        bail!(
            "Original card face backup not found for card hash {}",
            card_hash
        );
    }

    Ok(assets)
}

fn write_backup_file(path: &Path, data: &[u8]) -> Result<()> {
    crate::paths::write_atomic(path, data)
        .with_context(|| format!("Could not write backup file {}", path.display()))
}

/// Diagnostic: stage a standard (production-shaped) archive and report where it
/// lands, to validate the streaming-zip conduit on this device.
pub fn diagnose_stage() -> Result<String> {
    let session = ActiveDeviceSession::open(None, ConnectionMode::Auto)
        .context("Failed to open device session")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC")?;

    let mut out = String::new();
    let before = afc.list_directory(".").unwrap_or_default();

    let token = crate::flasher::generate_token();
    let source = format!("airlift-src-{token}");
    let archive = crate::airlift::build_streaming_zip_archive(
        "/var/mobile/Library/Passes/Cards/diag.pkpass/diag.png",
        b"hello-diagnostic",
    )?;
    out.push_str(&format!("source={source}\n"));
    match crate::airlift::stage_streaming_zip(&session, &source, &archive) {
        Ok(()) => out.push_str("staged ok\n"),
        Err(error) => out.push_str(&format!("stage FAIL: {error:#}\n")),
    }

    for wait in [500u64, 1500, 3000] {
        std::thread::sleep(std::time::Duration::from_millis(wait));
        let after = afc.list_directory(".").unwrap_or_default();
        let new_entries: Vec<&String> = after.iter().filter(|e| !before.contains(e)).collect();
        let src_ok = afc.exists(&source);
        out.push_str(&format!(
            "after +{wait}ms: new={new_entries:?} source_exists={src_ok}\n"
        ));
    }

    if let Ok(entries) = afc.list_directory(&source) {
        out.push_str(&format!("source listing: {entries:?}\n"));
    }
    let _ = afc.remove_tree(&source);
    Ok(out)
}

/// Diagnostic: end-to-end write+read round-trip on a harmless probe file in a
/// protected Library directory (used by `--silo-roundtrip`).
pub fn diagnose_roundtrip() -> Result<String> {
    let devices = crate::device::list_connected_devices().context("Failed to list devices")?;
    let udid = devices
        .first()
        .map(|device| device.udid.clone())
        .context("No device connected")?;
    let mut out = format!("device: {udid}\n");

    let token = crate::flasher::generate_token();
    let target_dir = "/var/mobile/Library/Passes";
    let leaf = "__silo_probe__.txt";
    let content = format!("probe-{token}-hello");

    let mut logs: Vec<String> = Vec::new();
    out.push_str("write:\n");
    match crate::flasher::write_system_file(
        &udid,
        ConnectionMode::Auto,
        target_dir,
        leaf,
        content.as_bytes(),
        |message| logs.push(message.to_string()),
    ) {
        Ok(()) => out.push_str("  write OK\n"),
        Err(error) => out.push_str(&format!("  write FAIL: {error:#}\n")),
    }

    logs.clear();
    out.push_str("read:\n");
    match crate::flasher::read_system_file(
        &udid,
        ConnectionMode::Auto,
        target_dir,
        leaf,
        |message| logs.push(message.to_string()),
    ) {
        Ok(bytes) => out.push_str(&format!(
            "  read OK: {:?} (expected {:?})\n",
            String::from_utf8_lossy(&bytes),
            content
        )),
        Err(error) => out.push_str(&format!("  read FAIL: {error:#}\n")),
    }
    for line in &logs {
        out.push_str(&format!("  log: {line}\n"));
    }
    Ok(out)
}

/// Diagnostic: remove leftover `airlift-*` staging entries from the Media root
/// (used by `--silo-cleanup`).
pub fn diagnose_cleanup() -> Result<String> {
    let mut out = String::new();

    if let Some(device) = crate::device::list_connected_devices()?.first() {
        let udid = device.udid.clone();
        match crate::flasher::remove_system_file(
            &udid,
            ConnectionMode::Auto,
            "/var/mobile/Library/Passes",
            "__silo_probe__.txt",
            |_| {},
        ) {
            Ok(()) => out.push_str("probe: removed\n"),
            Err(error) => out.push_str(&format!("probe: {error:#}\n")),
        }
    }

    let session = ActiveDeviceSession::open(None, ConnectionMode::Auto)
        .context("Failed to open device session")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC")?;

    let entries = afc
        .list_directory(".")
        .context("Failed to list Media root")?;
    for entry in entries {
        if !entry.starts_with("airlift-") {
            continue;
        }
        let _ = afc.remove_tree(&entry);
        if afc.exists(&entry) {
            let _ = afc.remove_path(&entry);
        }
        out.push_str(&format!(
            "{entry}: {}\n",
            if afc.exists(&entry) {
                "STILL PRESENT"
            } else {
                "removed"
            }
        ));
    }
    Ok(out)
}

/// Diagnostic: two-stage read-out. First relocate a symlink to the card's
/// Library directory into Media, then have the device move the artwork file
/// *out* through that symlink into Media so the Media-rooted AFC can read it.
pub fn diagnose_read2(card_hash: &str) -> Result<String> {
    let devices = crate::device::list_connected_devices().context("Failed to list devices")?;
    let udid = devices
        .first()
        .map(|device| device.udid.clone())
        .context("No device connected")?;
    let mut out = format!("device: {udid}\n");

    let session = ActiveDeviceSession::open(Some(&udid), ConnectionMode::Auto)
        .context("Failed to open device session")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC")?;

    let token = crate::flasher::generate_token();
    let link_src = format!("airlift-src-{token}");
    let link_dest = format!("airlift-link-{token}");
    let out_dest = format!("airlift-out-{token}");
    let link_ident = format!("../../{link_src}/p0/p1/p2/link");

    let pkpass_dir = format!("/var/mobile/Library/Passes/Cards/{card_hash}.pkpass");
    out.push_str(&format!("card dir: {pkpass_dir}\n"));

    let archive = crate::airlift::build_streaming_zip_archive(&pkpass_dir, b"")?;
    crate::airlift::stage_streaming_zip(&session, &link_src, &archive)
        .context("Failed to stage read2 symlink archive")?;

    let mut logs: Vec<String> = Vec::new();
    let media_before = afc.list_directory(".").unwrap_or_default();
    out.push_str(&format!("media before: {media_before:?}\n"));
    let books_a = crate::airlift::build_books_plist(std::slice::from_ref(&link_ident))?;
    afc.make_directory_recursive("Books/Sync")?;
    afc.write_file("Books/Sync/Books.plist", &books_a)?;
    match crate::airtraffic::sync_assets_via_airtraffic(
        &udid,
        session.transport,
        &[(link_ident.as_str(), link_dest.as_str())],
        |message| logs.push(message.to_string()),
    ) {
        Ok(()) => out.push_str("sync A (relocate symlink) OK\n"),
        Err(error) => {
            out.push_str(&format!("sync A FAIL: {error:#}\n"));
            for line in &logs {
                out.push_str(&format!("  log: {line}\n"));
            }
            return Ok(out);
        }
    }
    sleep(Duration::from_millis(800));
    let media_a = afc.list_directory(".").unwrap_or_default();
    out.push_str(&format!("media after A: {media_a:?}\n"));
    out.push_str(&format!("link_dest exists: {}\n", afc.exists(&link_dest)));

    for asset in [
        "cardBackgroundCombined@2x.png",
        "cardBackgroundCombined@3x.png",
        "__silo_nonexistent__.png",
    ] {
        let read_ident = format!("../../{link_dest}/{asset}");
        let books_b = crate::airlift::build_books_plist(std::slice::from_ref(&read_ident))?;
        afc.write_file("Books/Sync/Books.plist", &books_b)?;
        match crate::airtraffic::sync_assets_via_airtraffic(
            &udid,
            session.transport,
            &[(read_ident.as_str(), out_dest.as_str())],
            |message| logs.push(message.to_string()),
        ) {
            Ok(()) => {
                sleep(Duration::from_millis(800));
                let media_b = afc.list_directory(".").unwrap_or_default();
                out.push_str(&format!(
                    "sync B {asset} OK; out_dest exists: {}; new media: {:?}\n",
                    afc.exists(&out_dest),
                    media_b
                        .iter()
                        .filter(|e| !media_a.contains(e))
                        .collect::<Vec<_>>()
                ));
                match afc.read_file(&out_dest) {
                    Ok(bytes) => {
                        let head: Vec<u8> = bytes.iter().take(8).copied().collect();
                        out.push_str(&format!(
                            "sync B {asset} read OK: {} bytes head={:02X?}\n",
                            bytes.len(),
                            head
                        ));
                    }
                    Err(error) => {
                        out.push_str(&format!("sync B {asset} dest read FAIL: {error:#}\n"))
                    }
                }
            }
            Err(error) => out.push_str(&format!("sync B {asset} FAIL: {error:#}\n")),
        }
        let _ = afc.remove_path(&out_dest);
    }

    let _ = afc.remove_path(&link_dest);
    let _ = afc.remove_tree(&link_src);
    for line in &logs {
        out.push_str(&format!("  log: {line}\n"));
    }
    Ok(out)
}

/// Diagnostic: verify the AirTraffic exploit moves a *staged* file to a
/// destination inside Media without the host sending bytes (used by
/// `--silo-test-move`). If this works, a device-side copy out of the Library
/// is feasible.
pub fn diagnose_move() -> Result<String> {
    let devices = crate::device::list_connected_devices().context("Failed to list devices")?;
    let udid = devices
        .first()
        .map(|device| device.udid.clone())
        .context("No device connected")?;
    let mut out = format!("device: {udid}\n");

    let session = ActiveDeviceSession::open(Some(&udid), ConnectionMode::Auto)
        .context("Failed to open device session")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC")?;

    let token = crate::flasher::generate_token();
    let source = format!("airlift-src-{token}");
    let dest = format!("airlift-out-{token}");
    let content = format!("hello-move-{token}");
    let ident = format!("../../{source}/payload");

    let archive = crate::airlift::build_streaming_zip_archive(
        "var/mobile/Media/__noop__",
        content.as_bytes(),
    )?;
    crate::airlift::stage_streaming_zip(&session, &source, &archive)
        .context("Failed to stage move test archive")?;
    out.push_str(&format!("staged {source}/payload\n"));

    let books = crate::airlift::build_books_plist(std::slice::from_ref(&ident))?;
    afc.make_directory_recursive("Books/Sync")?;
    afc.write_file("Books/Sync/Books.plist", &books)?;

    let mut logs: Vec<String> = Vec::new();
    crate::airtraffic::sync_assets_via_airtraffic(
        &udid,
        session.transport,
        &[(ident.as_str(), dest.as_str())],
        |message| logs.push(message.to_string()),
    )
    .context("AirTraffic move test failed")?;

    match afc.read_file(&dest) {
        Ok(bytes) => out.push_str(&format!(
            "dest read OK: {:?} (expected {:?})\n",
            String::from_utf8_lossy(&bytes),
            content
        )),
        Err(error) => out.push_str(&format!("dest read FAIL: {error:#}\n")),
    }

    let _ = afc.remove_path(&dest);
    let _ = afc.remove_tree(&source);
    for line in &logs {
        out.push_str(&format!("  log: {line}\n"));
    }
    Ok(out)
}

/// Diagnostic: try to read the given card's artwork using the AirTraffic
/// symlink relocation (the read counterpart of the flash path).
pub fn diagnose_read(card_hash: &str) -> Result<String> {
    let devices = crate::device::list_connected_devices().context("Failed to list devices")?;
    let udid = devices
        .first()
        .map(|device| device.udid.clone())
        .context("No device connected")?;
    let mut out = format!("device: {udid}\n");

    for candidate in card_hash_candidates(card_hash) {
        let card_dir = format!("/var/mobile/Library/Passes/Cards/{candidate}.pkpass");
        for asset in [
            "cardBackgroundCombined@3x.png",
            "cardBackgroundCombined@2x.png",
        ] {
            out.push_str(&format!("--- {card_dir}/{asset}\n"));
            let mut logs: Vec<String> = Vec::new();
            match crate::flasher::read_system_file(
                &udid,
                ConnectionMode::Auto,
                &card_dir,
                asset,
                |message| logs.push(message.to_string()),
            ) {
                Ok(bytes) => {
                    let head: Vec<u8> = bytes.iter().take(8).copied().collect();
                    out.push_str(&format!("  OK {} bytes head={:02X?}\n", bytes.len(), head));
                    for line in &logs {
                        out.push_str(&format!("    {line}\n"));
                    }
                    return Ok(out);
                }
                Err(error) => {
                    out.push_str(&format!("  FAIL {error:#}\n"));
                    for line in &logs {
                        out.push_str(&format!("    {line}\n"));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Diagnostic: list an arbitrary on-device directory through the symlink
/// escape (used by `--silo-ls <path>`).
pub fn diagnose_ls(target: &str) -> Result<String> {
    let devices = crate::device::list_connected_devices().context("Failed to list devices")?;
    let udid = devices
        .first()
        .map(|device| device.udid.clone())
        .context("No device connected")?;
    let mut out = format!("device: {udid}\n");

    let mut logs: Vec<String> = Vec::new();
    match crate::flasher::list_system_dir(&udid, ConnectionMode::Auto, target, |message| {
        logs.push(message.to_string())
    }) {
        Ok(entries) => {
            out.push_str(&format!("OK {} entries for {target}\n", entries.len()));
            for entry in entries {
                out.push_str(&format!("  {entry}\n"));
            }
        }
        Err(error) => out.push_str(&format!("FAIL {error:#}\n")),
    }
    for line in &logs {
        out.push_str(&format!("  log: {line}\n"));
    }
    Ok(out)
}

/// Diagnostic: list the on-device Wallet card directory so the real hash and
/// extension layout can be inspected (used by `--silo-list-cards`).
pub fn diagnose_cards(card_hash: Option<&str>) -> Result<String> {
    eprintln!("[dbg] opening device session...");
    let session = ActiveDeviceSession::open(None, ConnectionMode::Auto)
        .context("Failed to open device session")?;
    eprintln!(
        "[dbg] session open (udid={} transport={:?})",
        session.udid, session.transport
    );
    eprintln!("[dbg] opening AFC...");
    let afc = AfcClient::new(&session).context("Failed to open AFC")?;
    eprintln!("[dbg] AFC open");

    let dir = "/var/mobile/Library/Passes/Cards";
    let mut out = String::new();
    for probe in [
        ".",
        "/",
        "/var",
        "/var/mobile",
        "/var/mobile/Media",
        "/var/mobile/Library",
        "/var/mobile/Library/Passes",
        "/var/mobile/Library/Passes/Cards",
        "/Library",
        "/Library/Passes",
        "/Library/Passes/Cards",
        "/Media",
        "/Media/DCIM",
    ] {
        eprintln!("[dbg] list_directory {probe}...");
        match afc.list_directory(probe) {
            Ok(entries) => {
                let preview: Vec<String> = entries.iter().take(8).cloned().collect();
                out.push_str(&format!(
                    "OK   {probe}  ({} entries) {:?}\n",
                    entries.len(),
                    preview
                ));
            }
            Err(error) => out.push_str(&format!("FAIL {probe}  -> {error:#}\n")),
        }
    }

    if let Some(hash) = card_hash {
        out.push_str(&format!("\nprobing for hash: {hash}\n"));
        for candidate in card_hash_candidates(hash) {
            let pkpass_dir = format!("{dir}/{candidate}.pkpass");
            eprintln!("[dbg] exists {pkpass_dir}...");
            out.push_str(&format!(
                "  {candidate}.pkpass -> {}\n",
                afc.exists(&pkpass_dir)
            ));
        }
        eprintln!("[dbg] find_device_card_hash...");
        out.push_str(&format!(
            "  find_device_card_hash -> {:?}\n",
            find_device_card_hash(&afc, hash)?
        ));
    }

    Ok(out)
}
