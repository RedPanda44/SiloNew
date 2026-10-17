//! House keycard: build a Wallet pass (NFC door key), optionally sign it with
//! an Apple Pass Type ID certificate, package it as a `.pkpass`, and deliver it
//! to the iPhone (local file, LAN download link, and/or the device's Media
//! folder).

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::afc::AfcClient;
use crate::device::{ActiveDeviceSession, ConnectionMode};

/// Everything the user can tweak about the keycard pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeycardDraft {
    pub house_name: String,
    pub owner_name: String,
    pub access_label: String,
    /// NFC payload written to the pass `nfc` dictionary (hex, as the reader expects).
    pub nfc_payload: String,
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub label_color: [u8; 3],
    pub icon_path: Option<PathBuf>,
    pub logo_path: Option<PathBuf>,
}

impl Default for KeycardDraft {
    fn default() -> Self {
        Self {
            house_name: "My House".to_string(),
            owner_name: String::new(),
            access_label: "Front Door".to_string(),
            nfc_payload: String::new(),
            background: [26, 32, 56],
            foreground: [255, 255, 255],
            label_color: [176, 186, 214],
            icon_path: None,
            logo_path: None,
        }
    }
}

/// Signing material. Any of the `*_path` fields may be empty while the user is
/// still obtaining an Apple Pass Type ID certificate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SigningPrefs {
    pub p12_path: Option<PathBuf>,
    #[serde(default)]
    pub p12_password: String,
    pub cert_pem: Option<PathBuf>,
    pub key_pem: Option<PathBuf>,
    pub wwdr_path: Option<PathBuf>,
    #[serde(default)]
    pub pass_type_id: String,
    #[serde(default)]
    pub team_id: String,
}

impl SigningPrefs {
    /// True when enough material is present to attempt signing.
    pub fn is_configured(&self) -> bool {
        let has_cert = self.p12_path.is_some()
            || (self.cert_pem.is_some() && self.key_pem.is_some());
        has_cert
            && self.wwdr_path.is_some()
            && !self.pass_type_id.trim().is_empty()
            && !self.team_id.trim().is_empty()
    }
}

/// Persisted keycard state: the draft pass and its signing material.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeycardPrefs {
    #[serde(default)]
    pub draft: KeycardDraft,
    #[serde(default)]
    pub signing: SigningPrefs,
}

const DEFAULT_ICON: &[u8] = include_bytes!("../assets/icon.png");

/// Build the pass, sign it if possible, and write `<stem>.pkpass` into `out_dir`.
/// Returns the written path and whether the pass was signed.
pub fn build_pkpass(
    draft: &KeycardDraft,
    signing: &SigningPrefs,
    out_dir: &Path,
    stem: &str,
) -> Result<(PathBuf, bool)> {
    let token = crate::flasher::generate_token();
    let work = std::env::temp_dir().join(format!("silo-keycard-{token}"));
    fs::create_dir_all(&work).context("Failed to create the keycard workspace")?;

    let result = (|| -> Result<(PathBuf, bool)> {
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        files.push((
            "pass.json".to_string(),
            serde_json::to_vec_pretty(&pass_json(draft, signing))?,
        ));
        files.push(("icon.png".to_string(), icon_png(draft, 29)?));
        files.push(("icon@2x.png".to_string(), icon_png(draft, 58)?));
        if let Some(logo_path) = draft.logo_path.as_ref() {
            files.push(("logo.png".to_string(), logo_png(logo_path)?));
        }

        for (name, bytes) in &files {
            fs::write(work.join(name), bytes)
                .with_context(|| format!("Failed to write pass asset {name}"))?;
        }

        let manifest = build_manifest(&files);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        fs::write(work.join("manifest.json"), &manifest_bytes)?;

        let mut signed = false;
        let signature = if signing.is_configured() {
            let openssl = find_openssl().context(
                "OpenSSL is required to sign the pass but was not found. Install OpenSSL or Git for Windows.",
            )?;
            let bytes = sign_manifest(&openssl, &work.join("manifest.json"), signing, &work)?;
            signed = true;
            Some(bytes)
        } else {
            None
        };

        fs::create_dir_all(out_dir).context("Failed to create the output folder")?;
        let out_path = out_dir.join(format!("{stem}.pkpass"));
        let file = fs::File::create(&out_path).context("Failed to create the .pkpass file")?;
        let mut zipw = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in &files {
            zipw.start_file(name.clone(), opts)?;
            zipw.write_all(bytes)?;
        }
        zipw.start_file("manifest.json", opts)?;
        zipw.write_all(&manifest_bytes)?;
        if let Some(sig) = &signature {
            zipw.start_file("signature", opts)?;
            zipw.write_all(sig)?;
        }
        zipw.finish().context("Failed to finalize the .pkpass archive")?;

        Ok((out_path, signed))
    })();

    let _ = fs::remove_dir_all(&work);
    result
}

/// Copy the finished `.pkpass` onto the iPhone's Media storage through AFC.
pub fn push_to_device(
    udid: &str,
    mode: ConnectionMode,
    local: &Path,
    file_name: &str,
) -> Result<String> {
    let session = ActiveDeviceSession::open(Some(udid), mode)
        .context("Failed to open a device session to send the keycard")?;
    let afc = AfcClient::new(&session).context("Failed to open AFC to send the keycard")?;
    afc.make_directory_recursive("Silo")?;
    let bytes = fs::read(local).context("Failed to read the generated .pkpass")?;
    let device_path = format!("Silo/{file_name}");
    afc.write_file(&device_path, &bytes)?;
    Ok(format!("/var/mobile/Media/{device_path}"))
}

/// A one-shot HTTP server that hands the pass to the iPhone over the LAN, so
/// tapping the link in Safari triggers the Wallet "Add" flow.
pub struct PassServer {
    pub url: String,
    stop: Arc<AtomicBool>,
}

impl PassServer {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for PassServer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn serve_pkpass(local: &Path, file_name: &str) -> Result<PassServer> {
    let body = fs::read(local).context("Failed to read the generated .pkpass")?;
    let listener = TcpListener::bind(("0.0.0.0", 0)).context("Failed to start the keycard server")?;
    let port = listener.local_addr()?.port();
    let ip = local_ipv4().unwrap_or_else(|| "127.0.0.1".to_string());
    let url = format!("http://{ip}:{port}/{file_name}");
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let name = file_name.to_string();
    thread::spawn(move || {
        for stream in listener.incoming() {
            if stop_thread.load(Ordering::SeqCst) {
                break;
            }
            if let Ok(mut stream) = stream {
                let _ = serve_connection(&mut stream, &name, &body);
            }
        }
    });
    Ok(PassServer { url, stop })
}

fn serve_connection(stream: &mut TcpStream, file_name: &str, body: &[u8]) -> std::io::Result<()> {
    let mut scratch = [0u8; 2048];
    let _ = stream.read(&mut scratch);
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.pkpass\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"{}\"\r\nConnection: close\r\n\r\n",
        body.len(),
        file_name
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn local_ipv4() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

fn pass_json(draft: &KeycardDraft, signing: &SigningPrefs) -> serde_json::Value {
    let barcode_message = if draft.nfc_payload.trim().is_empty() {
        draft.owner_name.clone()
    } else {
        draft.nfc_payload.clone()
    };
    let barcode = serde_json::json!({
        "format": "PKBarcodeFormatQR",
        "message": barcode_message,
        "messageEncoding": "iso-8859-1"
    });
    let mut obj = serde_json::json!({
        "formatVersion": 1,
        "passTypeIdentifier": signing.pass_type_id,
        "serialNumber": crate::flasher::generate_token(),
        "teamIdentifier": signing.team_id,
        "organizationName": draft.house_name,
        "description": format!("{} keycard", draft.house_name),
        "logoText": draft.house_name,
        "foregroundColor": rgb(draft.foreground),
        "backgroundColor": rgb(draft.background),
        "labelColor": rgb(draft.label_color),
        "generic": {
            "primaryFields": [ { "key": "house", "label": "HOUSE", "value": draft.house_name } ],
            "secondaryFields": [ { "key": "owner", "label": "OWNER", "value": draft.owner_name } ],
            "auxiliaryFields": [ { "key": "access", "label": "ACCESS", "value": draft.access_label } ]
        },
        "barcode": barcode,
        "barcodes": [ barcode ],
    });
    if !draft.nfc_payload.trim().is_empty() {
        obj["nfc"] = serde_json::json!({ "message": draft.nfc_payload });
    }
    obj
}

fn rgb(color: [u8; 3]) -> String {
    format!("rgb({}, {}, {})", color[0], color[1], color[2])
}

fn icon_png(draft: &KeycardDraft, size: u32) -> Result<Vec<u8>> {
    let image = if let Some(path) = draft.icon_path.as_ref() {
        image::open(path).context("Failed to read the keycard icon image")?
    } else {
        image::load_from_memory(DEFAULT_ICON).context("Failed to load the default icon")?
    };
    let resized = image.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
    let mut buffer = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut buffer, image::ImageFormat::Png)
        .context("Failed to encode the pass icon")?;
    Ok(buffer.into_inner())
}

fn logo_png(path: &Path) -> Result<Vec<u8>> {
    let image = image::open(path).context("Failed to read the keycard logo image")?;
    let resized = image.resize(320, 100, image::imageops::FilterType::Lanczos3);
    let mut buffer = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut buffer, image::ImageFormat::Png)
        .context("Failed to encode the pass logo")?;
    Ok(buffer.into_inner())
}

fn build_manifest(files: &[(String, Vec<u8>)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (name, bytes) in files {
        map.insert(name.clone(), serde_json::Value::String(hex(&sha1(bytes))));
    }
    serde_json::Value::Object(map)
}

fn sign_manifest(
    openssl: &Path,
    manifest: &Path,
    signing: &SigningPrefs,
    work: &Path,
) -> Result<Vec<u8>> {
    let (cert, key) = if let Some(p12) = signing.p12_path.as_ref() {
        let cert = work.join("signer-cert.pem");
        let key = work.join("signer-key.pem");
        run_openssl(
            openssl,
            &[
                "pkcs12",
                "-in",
                p12.to_str().unwrap_or_default(),
                "-clcerts",
                "-nokeys",
                "-out",
                cert.to_str().unwrap_or_default(),
                "-passin",
                "env:SILO_P12_PASS",
            ],
            &signing.p12_password,
        )
        .context("Failed to extract the signing certificate from the .p12")?;
        run_openssl(
            openssl,
            &[
                "pkcs12",
                "-in",
                p12.to_str().unwrap_or_default(),
                "-nocerts",
                "-nodes",
                "-out",
                key.to_str().unwrap_or_default(),
                "-passin",
                "env:SILO_P12_PASS",
            ],
            &signing.p12_password,
        )
        .context("Failed to extract the private key from the .p12")?;
        (cert, key)
    } else {
        (
            signing.cert_pem.clone().context("Missing Pass Type ID certificate")?,
            signing.key_pem.clone().context("Missing private key file")?,
        )
    };
    let wwdr = signing
        .wwdr_path
        .clone()
        .context("Missing Apple WWDR certificate")?;

    let signature = work.join("signature");
    let status = Command::new(openssl)
        .arg("smime")
        .arg("-binary")
        .arg("-sign")
        .arg("-certfile")
        .arg(&wwdr)
        .arg("-signer")
        .arg(&cert)
        .arg("-inkey")
        .arg(&key)
        .arg("-in")
        .arg(manifest)
        .arg("-out")
        .arg(&signature)
        .arg("-outform")
        .arg("DER")
        .output()
        .context("Failed to run OpenSSL signing")?;
    if !status.status.success() {
        bail!(
            "OpenSSL failed to sign the pass: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        );
    }
    fs::read(&signature).context("Failed to read the generated signature")
}

fn run_openssl(openssl: &Path, args: &[&str], password: &str) -> Result<()> {
    let status = Command::new(openssl)
        .args(args)
        .env("SILO_P12_PASS", password)
        .output()
        .context("Failed to run OpenSSL")?;
    if !status.status.success() {
        bail!(
            "OpenSSL error: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        );
    }
    Ok(())
}

/// Locate an OpenSSL executable: `SILO_OPENSSL`, then `PATH`, then common
/// Windows install locations (notably the copy bundled with Git for Windows).
pub fn find_openssl() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("SILO_OPENSSL") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            for exe in ["openssl.exe", "openssl"] {
                let candidate = dir.join(exe);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    for candidate in [
        r"C:\Program Files\Git\usr\bin\openssl.exe",
        r"C:\Program Files\Git\mingw64\bin\openssl.exe",
        r"C:\Program Files\OpenSSL-Win64\bin\openssl.exe",
        r"C:\Program Files (x86)\OpenSSL-Win32\bin\openssl.exe",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Minimal SHA-1 (used for the pass `manifest.json`), avoiding an extra crate.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_known_vectors() {
        assert_eq!(
            hex(&sha1(b"")),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(b"The quick brown fox jumps over the lazy dog")),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
    }

    #[test]
    fn unsigned_pass_builds_without_certificate() {
        let dir = std::env::temp_dir().join("silo-keycard-test");
        let draft = KeycardDraft::default();
        let signing = SigningPrefs::default();
        let (path, signed) = build_pkpass(&draft, &signing, &dir, "unit-test").unwrap();
        assert!(!signed);
        assert!(path.is_file());
        let _ = fs::remove_file(&path);
    }
}
