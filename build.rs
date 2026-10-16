//! Embed the Silo icon (and version metadata) into the built `.exe`.
//!
//! Windows Explorer, desktop/start-menu shortcuts and "Apps & features" all
//! read the icon from the PE resource section, which only exists if we link
//! an `.rc`/`.res` into the binary — the runtime window icon alone is not
//! enough.

fn main() {
    let png = std::path::Path::new("assets/icon.png");
    println!("cargo:rerun-if-changed={}", png.display());

    // Wrap the PNG in an ICO container (single 256px PNG-compressed entry,
    // supported since Vista) and hand it to winres.
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out_dir.join("silo.ico");
    write_ico(include_bytes!("assets/icon.png"), &ico);

    let mut resource = winres::WindowsResource::new();
    resource.set_icon(ico.to_str().expect("icon path must be utf-8"));
    resource.set("ProductName", "Silo");
    resource.set("FileDescription", "Silo");
    resource.set("OriginalFilename", "silo.exe");
    resource
        .compile()
        .expect("failed to embed the exe icon resource");
}

/// Build a one-image `.ico` file around PNG bytes.
fn write_ico(png: &[u8], out: &std::path::Path) {
    let mut ico: Vec<u8> = Vec::with_capacity(22 + png.len());
    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    ico.extend_from_slice(&1u16.to_le_bytes()); // one image
    // ICONDIRENTRY
    ico.push(0); // width: 0 = 256
    ico.push(0); // height: 0 = 256
    ico.push(0); // palette color count: none
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // color planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes()); // offset: 6 + 16
    ico.extend_from_slice(png);
    std::fs::write(out, ico).expect("could not write icon.ico");
}
