//! The installer embeds the release build of Silo as its payload, so this
//! build script refuses to compile until `target/release/silo.exe` exists.
//! It also links the Silo icon into the installer's own `.exe`.

fn main() {
    let payload = std::path::Path::new("../target/release/silo.exe");
    println!("cargo:rerun-if-changed={}", payload.display());
    if !payload.exists() {
        panic!(
            "\n\nmissing release payload: {}\nbuild the app first:\n    cargo build --release   (run this in the project root)\n\n",
            payload.display()
        );
    }

    let png = std::path::Path::new("../assets/icon.png");
    println!("cargo:rerun-if-changed={}", png.display());

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = out_dir.join("silo-installer.ico");
    write_ico(include_bytes!("../assets/icon.png"), &ico);

    let mut resource = winres::WindowsResource::new();
    resource.set_icon(ico.to_str().expect("icon path must be utf-8"));
    resource.set("ProductName", "Silo Installer");
    resource.set("FileDescription", "Silo Installer");
    resource.set("OriginalFilename", "silo-installer.exe");
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
