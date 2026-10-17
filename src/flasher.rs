use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::afc::AfcClient;
use crate::airlift::{
    LINK_PREFIX, RECOVERED_PREFIX, SOURCE_PREFIX, build_books_plist, build_streaming_zip_archive,
    build_streaming_zip_archive_multi, restore_books, snapshot_books, stage_streaming_zip,
};
use crate::airtraffic::sync_assets_via_airtraffic;
use crate::device::{ActiveDeviceSession, ConnectionMode};
use crate::wallet_backup::{
    capture_original_card, load_original_assets, resolve_device_card_hash,
};

#[allow(dead_code)]
pub const TARGET_WALLET_ASSETS: &[&str] = &[
    "cardBackgroundCombined@3x.png",
    "cardBackgroundCombined@2x.png",
    "cardBackgroundCombined.pdf",
];

#[allow(dead_code)]
pub const CACHE_FILES: &[&str] = &["FrontFace", "PlaceHolder", "Preview"];

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(
        hAlgorithm: *mut std::ffi::c_void,
        pbBuffer: *mut u8,
        cbBuffer: u32,
        dwFlags: u32,
    ) -> i32;
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 10];
    unsafe {
        let _ = BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            2, // BCRYPT_USE_SYSTEM_PREFERRED_RNG
        );
    }
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn write_system_file<L>(
    udid: &str,
    connection_mode: ConnectionMode,
    target_dir: &str,
    leaf_name: &str,
    payload: &[u8],
    mut log: L,
) -> Result<()>
where
    L: FnMut(&str),
{
    let token = generate_token();
    let source = format!("{}{}", SOURCE_PREFIX, token);
    let link_dest = format!("{}{}", LINK_PREFIX, token);
    let recovered = format!("{}{}", RECOVERED_PREFIX, token);

    let link_ident = format!("../../{}/p0/p1/p2/link", source);
    let payload_ident = format!("../../{}/payload", source);
    let target_dest = format!("{}/{}", link_dest, leaf_name);

    let books_identifiers = vec![link_ident.clone(), payload_ident.clone()];
    let assets_to_sync = [
        (link_ident.as_str(), link_dest.as_str()),
        (payload_ident.as_str(), target_dest.as_str()),
    ];

    log(&format!("Connecting AFC for {}...", leaf_name));
    let session = ActiveDeviceSession::open(Some(udid), connection_mode)
        .context("Failed to open device session for writing")?;
    log(&format!("Connected over {}.", session.transport.label()));
    let afc = AfcClient::new(&session).context("Failed to open AFC connection")?;

    let snapshot = snapshot_books(&afc).context("Failed to snapshot Books state before staging")?;

    let archive_data = build_streaming_zip_archive(target_dir, payload)
        .context("Failed to build streaming zip archive")?;

    let books_plist =
        build_books_plist(&books_identifiers).context("Failed to build Books.plist")?;

    let write_res = (|| -> Result<()> {
        log(&format!(
            "Staging payload archive ({} bytes) via MobileInstallation...",
            archive_data.len()
        ));
        stage_streaming_zip(&session, &source, &archive_data)
            .context("Failed to stage streaming zip conduit")?;

        let link_obj = format!("{}/p0/p1/p2/link", source);
        let payload_obj = format!("{}/payload", source);
        if !afc.exists(&source) || !afc.exists(&link_obj) || !afc.exists(&payload_obj) {
            bail!("StreamingZip completed but staging link/payload object missing on AFC");
        }

        afc.make_directory_recursive("Books/Sync")?;
        afc.write_file("Books/Sync/Books.plist", &books_plist)?;
        if !afc.exists("Books/Sync/Books.plist") {
            bail!("Failed to stage Books/Sync/Books.plist");
        }

        log(&format!(
            "Synchronizing {} with AirTraffic host daemon...",
            leaf_name
        ));
        sync_assets_via_airtraffic(udid, session.transport, &assets_to_sync, &mut log)
            .context("AirTraffic sync failed")?;

        Ok(())
    })();

    let _ = afc.remove_path(&link_dest);
    let _ = afc.remove_path(&recovered);
    let _ = afc.remove_tree(&source);
    sleep(Duration::from_millis(800));

    let restore_res = restore_books(&afc, &snapshot);

    write_res?;
    restore_res.context("Failed to restore Books state during cleanup")?;
    log(&format!("Successfully written: {}", leaf_name));

    Ok(())
}

/// Books staging root the device resolves AirTraffic asset identifiers against.
const AIRLOCK_ROOT: &str = "/var/mobile/Media/Airlock/Book";

/// Compute a path relative to [`AIRLOCK_ROOT`]. Asset identifiers are resolved
/// by the device against this root, so a `..` traversal here makes the sync
/// move a file from anywhere on the device (e.g. `/var/mobile/Library/...`) out
/// into Media.
fn airlock_relative(path: &str) -> String {
    let base: Vec<&str> = AIRLOCK_ROOT.trim_matches('/').split('/').collect();
    let target: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut common = 0;
    while common < base.len() && common < target.len() && base[common] == target[common] {
        common += 1;
    }
    let mut parts: Vec<String> = (0..base.len() - common)
        .map(|_| "..".to_string())
        .collect();
    parts.extend(target[common..].iter().map(|component| (*component).to_string()));
    parts.join("/")
}

/// Read a file from a protected system directory (e.g. a Wallet `.pkpass`) using
/// the AirTraffic move trick: a `..`-traversal asset identifier makes the device
/// move the original file *out* into Media, where the Media-rooted AFC service
/// can read it. The move is destructive, so the original bytes are written
/// straight back afterwards.
pub fn read_system_file<L>(
    udid: &str,
    connection_mode: ConnectionMode,
    target_dir: &str,
    leaf_name: &str,
    mut log: L,
) -> Result<Vec<u8>>
where
    L: FnMut(&str),
{
    let token = generate_token();
    let source = format!("{}{}", SOURCE_PREFIX, token);
    let link_dest = format!("{}{}", LINK_PREFIX, token);
    let recovered = format!("{}{}", RECOVERED_PREFIX, token);
    let link_ident = format!("../../{}/p0/p1/p2/link", source);
    let target_path = format!("{}/{}", target_dir.trim_end_matches('/'), leaf_name);
    let target_ident = airlock_relative(&target_path);

    log(&format!("Opening device session to read {}...", leaf_name));
    let session = ActiveDeviceSession::open(Some(udid), connection_mode)
        .context("Failed to open device session for reading")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC connection")?;

    let snapshot = snapshot_books(&afc).context("Failed to snapshot Books state before read")?;

    // The archive plants a symlink to the target directory (used by the
    // write-back below); the second identifier walks out of Media straight to
    // the target file so the device moves it into `recovered`.
    let archive = build_streaming_zip_archive(target_dir, b"aircard-backup-staging")
        .context("Failed to build read staging archive")?;
    let identifiers = vec![link_ident.clone(), target_ident.clone()];
    let books_plist = build_books_plist(&identifiers).context("Failed to build Books.plist")?;

    let read_res = (|| -> Result<Vec<u8>> {
        stage_streaming_zip(&session, &source, &archive)
            .context("Failed to stage read symlink conduit")?;

        afc.make_directory_recursive("Books/Sync")?;
        afc.write_file("Books/Sync/Books.plist", &books_plist)?;

        sync_assets_via_airtraffic(
            udid,
            session.transport,
            &[
                (link_ident.as_str(), link_dest.as_str()),
                (target_ident.as_str(), recovered.as_str()),
            ],
            &mut log,
        )
        .context("AirTraffic read relocation failed")?;

        afc.read_file(&recovered)
            .with_context(|| format!("Failed to read {target_path} from {recovered}"))
    })();

    let data = match read_res {
        Ok(data) => data,
        Err(error) => {
            let _ = afc.remove_path(&link_dest);
            let _ = afc.remove_path(&recovered);
            let _ = afc.remove_tree(&source);
            sleep(Duration::from_millis(800));
            let _ = restore_books(&afc, &snapshot);
            return Err(error);
        }
    };

    // The device moved the original out of the Library; write it straight back
    // through a fresh Airlift write, reusing this session.
    let restore_res = (|| -> Result<()> {
        let rtoken = generate_token();
        let rsource = format!("{}{}", SOURCE_PREFIX, rtoken);
        let rlink = format!("{}{}", LINK_PREFIX, rtoken);
        let rrec = format!("{}{}", RECOVERED_PREFIX, rtoken);
        let rlink_ident = format!("../../{}/p0/p1/p2/link", rsource);
        let rpayload_ident = format!("../../{}/payload", rsource);
        let rtarget_dest = format!("{}/{}", rlink, leaf_name);
        let rarchive = build_streaming_zip_archive(target_dir, &data)
            .context("Failed to build restore archive")?;
        let rbooks = build_books_plist(&[rlink_ident.clone(), rpayload_ident.clone()])
            .context("Failed to build restore Books.plist")?;
        let res = (|| -> Result<()> {
            stage_streaming_zip(&session, &rsource, &rarchive)
                .context("Failed to stage restore conduit")?;
            afc.make_directory_recursive("Books/Sync")?;
            afc.write_file("Books/Sync/Books.plist", &rbooks)?;
            sync_assets_via_airtraffic(
                udid,
                session.transport,
                &[
                    (rlink_ident.as_str(), rlink.as_str()),
                    (rpayload_ident.as_str(), rtarget_dest.as_str()),
                ],
                &mut log,
            )
            .context("AirTraffic restore sync failed")?;
            Ok(())
        })();
        let _ = afc.remove_path(&rlink);
        let _ = afc.remove_path(&rrec);
        let _ = afc.remove_tree(&rsource);
        res
    })();

    let _ = afc.remove_path(&link_dest);
    let _ = afc.remove_path(&recovered);
    let _ = afc.remove_tree(&source);
    sleep(Duration::from_millis(800));

    let books_res = restore_books(&afc, &snapshot);

    restore_res.context("Failed to write the original file back after reading it")?;
    books_res.context("Failed to restore Books state during read cleanup")?;
    log(&format!("Successfully read: {}", leaf_name));
    Ok(data)
}

/// Remove a file from a protected system directory by moving it out into Media
/// (via the AirTraffic traversal) and deleting the moved copy.
pub fn remove_system_file<L>(
    udid: &str,
    connection_mode: ConnectionMode,
    target_dir: &str,
    leaf_name: &str,
    mut log: L,
) -> Result<()>
where
    L: FnMut(&str),
{
    let token = generate_token();
    let source = format!("{}{}", SOURCE_PREFIX, token);
    let link_dest = format!("{}{}", LINK_PREFIX, token);
    let recovered = format!("{}{}", RECOVERED_PREFIX, token);
    let link_ident = format!("../../{}/p0/p1/p2/link", source);
    let target_path = format!("{}/{}", target_dir.trim_end_matches('/'), leaf_name);
    let target_ident = airlock_relative(&target_path);

    let session = ActiveDeviceSession::open(Some(udid), connection_mode)
        .context("Failed to open device session for removal")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC connection")?;

    let snapshot = snapshot_books(&afc).context("Failed to snapshot Books state before removal")?;

    let archive = build_streaming_zip_archive(target_dir, b"aircard-remove")
        .context("Failed to build removal staging archive")?;
    let identifiers = vec![link_ident.clone(), target_ident.clone()];
    let books_plist = build_books_plist(&identifiers).context("Failed to build Books.plist")?;

    let remove_res = (|| -> Result<()> {
        stage_streaming_zip(&session, &source, &archive)
            .context("Failed to stage removal conduit")?;
        afc.make_directory_recursive("Books/Sync")?;
        afc.write_file("Books/Sync/Books.plist", &books_plist)?;
        sync_assets_via_airtraffic(
            udid,
            session.transport,
            &[
                (link_ident.as_str(), link_dest.as_str()),
                (target_ident.as_str(), recovered.as_str()),
            ],
            &mut log,
        )
        .context("AirTraffic removal sync failed")?;
        if !afc.exists(&recovered) {
            bail!("Removal did not move {target_path} out of the Library");
        }
        afc.remove_path(&recovered)?;
        Ok(())
    })();

    let _ = afc.remove_path(&link_dest);
    let _ = afc.remove_path(&recovered);
    let _ = afc.remove_tree(&source);
    sleep(Duration::from_millis(800));
    let books_res = restore_books(&afc, &snapshot);

    remove_res?;
    books_res.context("Failed to restore Books state during removal cleanup")?;
    log(&format!("Successfully removed: {}", leaf_name));
    Ok(())
}

/// List a system directory by relocating a symlink to it into the Media
/// sandbox via the AirTraffic sync, then listing through the symlink.
pub fn list_system_dir<L>(
    udid: &str,
    connection_mode: ConnectionMode,
    target_dir: &str,
    mut log: L,
) -> Result<Vec<String>>
where
    L: FnMut(&str),
{
    let token = generate_token();
    let source = format!("{}{}", SOURCE_PREFIX, token);
    let link_dest = format!("{}{}", LINK_PREFIX, token);
    let recovered = format!("{}{}", RECOVERED_PREFIX, token);
    let link_ident = format!("../../{}/p0/p1/p2/link", source);

    let session = ActiveDeviceSession::open(Some(udid), connection_mode)
        .context("Failed to open device session for listing")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC connection")?;

    let snapshot = snapshot_books(&afc).context("Failed to snapshot Books state before listing")?;

    let archive = build_streaming_zip_archive(target_dir, b"")
        .context("Failed to build list staging archive")?;
    let books_plist = build_books_plist(std::slice::from_ref(&link_ident))
        .context("Failed to build Books.plist")?;

    let list_res = (|| -> Result<Vec<String>> {
        stage_streaming_zip(&session, &source, &archive)
            .context("Failed to stage list symlink conduit")?;

        afc.make_directory_recursive("Books/Sync")?;
        afc.write_file("Books/Sync/Books.plist", &books_plist)?;

        sync_assets_via_airtraffic(
            udid,
            session.transport,
            &[(link_ident.as_str(), link_dest.as_str())],
            &mut log,
        )
        .context("AirTraffic list relocation failed")?;

        afc.list_directory(&link_dest)
            .with_context(|| format!("Failed to list {target_dir} through symlink"))
    })();

    let _ = afc.remove_path(&link_dest);
    let _ = afc.remove_path(&recovered);
    let _ = afc.remove_tree(&source);
    sleep(Duration::from_millis(800));

    let restore_res = restore_books(&afc, &snapshot);

    let entries = list_res?;
    restore_res.context("Failed to restore Books state during list cleanup")?;
    Ok(entries)
}

pub fn write_system_files_batch<L>(
    udid: &str,
    connection_mode: ConnectionMode,
    target_dir: &str,
    items: &[(&str, &[u8])],
    mut log: L,
) -> Result<()>
where
    L: FnMut(&str),
{
    if items.is_empty() {
        return Ok(());
    }
    if items.len() == 1 {
        return write_system_file(
            udid,
            connection_mode,
            target_dir,
            items[0].0,
            items[0].1,
            log,
        );
    }

    log(&format!(
        "Packaging atomic batch of {} file(s) for {}...",
        items.len(),
        target_dir
    ));

    let token = generate_token();
    let source = format!("{}{}", SOURCE_PREFIX, token);
    let link_dest = format!("{}{}", LINK_PREFIX, token);
    let recovered = format!("{}{}", RECOVERED_PREFIX, token);

    let link_ident = format!("../../{}/p0/p1/p2/link", source);
    let mut books_identifiers = Vec::with_capacity(items.len() + 1);
    books_identifiers.push(link_ident.clone());

    let mut assets_to_sync: Vec<(String, String)> = Vec::with_capacity(items.len() + 1);
    assets_to_sync.push((link_ident, link_dest.clone()));

    for (idx, (leaf, _)) in items.iter().enumerate() {
        let payload_ident = format!("../../{}/payload_{}", source, idx);
        let target_dest = format!("{}/{}", link_dest, leaf);
        books_identifiers.push(payload_ident.clone());
        assets_to_sync.push((payload_ident, target_dest));
    }

    log(&format!(
        "Connecting AFC for batch of {} assets...",
        items.len()
    ));
    let session = ActiveDeviceSession::open(Some(udid), connection_mode)
        .context("Failed to open device session for writing")?;
    log(&format!("Connected over {}.", session.transport.label()));
    let afc = AfcClient::new(&session).context("Failed to open AFC connection")?;

    let snapshot = snapshot_books(&afc).context("Failed to snapshot Books state before staging")?;

    let archive_data = build_streaming_zip_archive_multi(target_dir, items)
        .context("Failed to build multi-payload streaming zip archive")?;

    let books_plist =
        build_books_plist(&books_identifiers).context("Failed to build Books.plist for batch")?;

    let write_res = (|| -> Result<()> {
        log(&format!(
            "Staging multi-payload archive ({} bytes, {} files) via MobileInstallation...",
            archive_data.len(),
            items.len()
        ));
        stage_streaming_zip(&session, &source, &archive_data)
            .context("Failed to stage streaming zip conduit")?;

        let link_obj = format!("{}/p0/p1/p2/link", source);
        let payload_obj = format!("{}/payload_0", source);
        let fallback_obj = format!("{}/payload", source);
        if !afc.exists(&source)
            || !afc.exists(&link_obj)
            || (!afc.exists(&payload_obj) && !afc.exists(&fallback_obj))
        {
            bail!("StreamingZip completed but staging link/payload object missing on AFC");
        }

        afc.make_directory_recursive("Books/Sync")?;
        afc.write_file("Books/Sync/Books.plist", &books_plist)?;
        if !afc.exists("Books/Sync/Books.plist") {
            bail!("Failed to stage Books/Sync/Books.plist");
        }

        log(&format!(
            "Synchronizing batch ({} items) with AirTraffic host daemon in single session...",
            items.len()
        ));
        let assets_refs: Vec<(&str, &str)> = assets_to_sync
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        sync_assets_via_airtraffic(udid, session.transport, &assets_refs, &mut log)
            .context("AirTraffic batch sync failed")?;

        Ok(())
    })();

    let _ = afc.remove_path(&link_dest);
    let _ = afc.remove_path(&recovered);
    let _ = afc.remove_tree(&source);
    sleep(Duration::from_millis(800));

    let restore_res = restore_books(&afc, &snapshot);

    write_res?;
    restore_res.context("Failed to restore Books state during cleanup")?;
    log(&format!(
        "Batch injection of {} file(s) completed successfully!",
        items.len()
    ));

    Ok(())
}

pub fn flash_wallet_skin<F, L>(
    udid: &str,
    connection_mode: ConnectionMode,
    card_hash: &str,
    skin_png: &[u8],
    skin_pdf: &[u8],
    mut progress: F,
    mut log: L,
) -> Result<()>
where
    F: FnMut(usize, usize, &str),
    L: FnMut(&str),
{
    anyhow::ensure!(
        crate::scanner::is_valid_card_hash(card_hash),
        "Invalid Wallet card path identifier"
    );

    log(&format!("Target Card Hash: {}", card_hash));
    log(&format!(
        "Skin payload size: {} bytes PNG, {} bytes PDF",
        skin_png.len(),
        skin_pdf.len()
    ));
    let resolved_hash = capture_original_card(udid, connection_mode, card_hash, &mut log)
        .context("Failed to inspect the original Wallet card face")?
        .unwrap_or_else(|| card_hash.to_string());
    let pkpass_dir = format!("/var/mobile/Library/Passes/Cards/{}.pkpass", resolved_hash);

    let total_steps = 3;
    progress(
        1,
        total_steps,
        "Writing card artwork assets (@3x, @2x, .pdf)...",
    );
    log("[1/3] Writing card artwork assets (@3x.png, @2x.png, cardBackgroundCombined.pdf)...");

    let card_assets: [(&str, &[u8]); 3] = [
        ("cardBackgroundCombined@3x.png", skin_png),
        ("cardBackgroundCombined@2x.png", skin_png),
        ("cardBackgroundCombined.pdf", skin_pdf),
    ];

    if let Err(err) =
        write_system_files_batch(udid, connection_mode, &pkpass_dir, &card_assets, &mut log)
    {
        log(&format!(
            "Notice: Batch write failed ({}), trying individual asset writes...",
            err
        ));
        for (asset, data) in &card_assets {
            write_system_file(udid, connection_mode, &pkpass_dir, asset, data, &mut log)
                .context(format!("Failed to write card asset {}", asset))?;
        }
    }

    invalidate_wallet_caches(
        udid,
        connection_mode,
        resolved_hash.as_str(),
        &mut progress,
        &mut log,
    );

    progress(total_steps, total_steps, "Card skin updated successfully!");
    log("Card skin write finished! Close and reopen Wallet on iPhone to view.");
    Ok(())
}

pub fn restore_wallet_original<F, L>(
    udid: &str,
    connection_mode: ConnectionMode,
    card_hash: &str,
    mut progress: F,
    mut log: L,
) -> Result<()>
where
    F: FnMut(usize, usize, &str),
    L: FnMut(&str),
{
    let original_assets = load_original_assets(udid, card_hash)
        .context("Could not load the original Wallet card face backup")?;
    let asset_refs: Vec<(&str, &[u8])> = original_assets
        .iter()
        .map(|(asset, data)| (asset.as_str(), data.as_slice()))
        .collect();

    log(&format!(
        "Restoring {} original card artwork asset(s) for hash {}...",
        asset_refs.len(),
        card_hash
    ));

    let resolved_hash = resolve_device_card_hash(udid, connection_mode, card_hash)
        .context("Failed to resolve the Wallet card identifier")?
        .unwrap_or_else(|| card_hash.to_string());
    progress(1, 3, "Restoring original card artwork...");

    let pkpass_dir = format!("/var/mobile/Library/Passes/Cards/{}.pkpass", resolved_hash);
    if let Err(err) =
        write_system_files_batch(udid, connection_mode, &pkpass_dir, &asset_refs, &mut log)
    {
        log(&format!(
            "Notice: Restore batch write failed ({}), trying individual asset writes...",
            err
        ));
        for (asset, data) in &original_assets {
            write_system_file(udid, connection_mode, &pkpass_dir, asset, data, &mut log)
                .context(format!("Failed to restore original card asset {}", asset))?;
        }
    }

    invalidate_wallet_caches(
        udid,
        connection_mode,
        resolved_hash.as_str(),
        &mut progress,
        &mut log,
    );

    progress(3, 3, "Original card face restored successfully!");
    log("Original card face restored. Close and reopen Wallet on iPhone to view it.");
    Ok(())
}

fn invalidate_wallet_caches<F, L>(
    udid: &str,
    connection_mode: ConnectionMode,
    card_hash: &str,
    progress: &mut F,
    log: &mut L,
) where
    F: FnMut(usize, usize, &str),
    L: FnMut(&str),
{
    let cache_leaves: [(&str, &[u8]); 3] = [
        ("FrontFace", b"corrupted"),
        ("PlaceHolder", b"corrupted"),
        ("Preview", b"corrupted"),
    ];

    for (c_idx, ext) in [".cache", ".pkcache"].iter().enumerate() {
        let step = 2 + c_idx;
        let cache_dir = format!("/var/mobile/Library/Passes/Cards/{}{}", card_hash, ext);
        progress(step, 3, &format!("Clearing {} cache...", ext));
        log(&format!(
            "[{}/3] Invalidating cache leaves in {}...",
            step, cache_dir
        ));

        if write_system_files_batch(udid, connection_mode, &cache_dir, &cache_leaves, &mut *log)
            .is_err()
        {
            for (leaf, data) in &cache_leaves {
                let _ = write_system_file(udid, connection_mode, &cache_dir, leaf, data, &mut *log);
            }
        }
    }
}

pub fn flash_passcode_theme<F, L>(
    udid: &str,
    connection_mode: ConnectionMode,
    items: &[(String, String, Vec<u8>)],
    mut progress: F,
    mut log: L,
) -> Result<()>
where
    F: FnMut(usize, usize, &str),
    L: FnMut(&str),
{
    let total = items.len();
    log(&format!("Flashing passcode theme ({} assets)...", total));

    let mut dirs_map: std::collections::BTreeMap<String, Vec<(&str, &[u8])>> =
        std::collections::BTreeMap::new();
    for (tdir, leaf, payload) in items {
        dirs_map
            .entry(tdir.clone())
            .or_default()
            .push((leaf.as_str(), payload.as_slice()));
    }

    let total_dirs = dirs_map.len();
    let mut dir_idx = 0;

    for (target_dir, dir_items) in &dirs_map {
        dir_idx += 1;
        let tdir_name = std::path::Path::new(target_dir)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(target_dir);

        progress(
            dir_idx,
            total_dirs,
            &format!(
                "Flashing {} ({} assets in atomic batch)...",
                tdir_name,
                dir_items.len()
            ),
        );
        log(&format!(
            "Flashing batch of {} assets into {}...",
            dir_items.len(),
            tdir_name
        ));

        let batch_res =
            write_system_files_batch(udid, connection_mode, target_dir, dir_items, &mut log);
        if let Err(err) = batch_res {
            log(&format!(
                "Warning: Batch write failed ({}), falling back to file-by-file write...",
                err
            ));
            for (f_idx, (leaf, payload)) in dir_items.iter().enumerate() {
                progress(
                    f_idx + 1,
                    dir_items.len(),
                    &format!(
                        "Fallback [{}/{}]: writing {}...",
                        f_idx + 1,
                        dir_items.len(),
                        leaf
                    ),
                );
                write_system_file(udid, connection_mode, target_dir, leaf, payload, &mut log)
                    .context(format!("Failed to write button asset {}", leaf))?;
            }
        }
    }

    progress(
        total_dirs,
        total_dirs,
        "Passcode theme applied successfully!",
    );
    log("Passcode theme successfully written! Lock or reboot iPhone to see new keypad.");
    Ok(())
}
