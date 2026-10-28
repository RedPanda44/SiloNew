# Silo (Windows) 🎴

> **Apple Wallet Card Skinner & Lockscreen Passcode Themer for iOS 18+ (No Jailbreak Required)**  
> Native Windows client written in Rust. Powered by the `airlift` AirTraffic sync exploit.

---

## What's New in 1.5.4
- 🔐 **Certificate password protection:** keycard signing passwords are now encrypted on disk with Windows DPAPI (readable only by your Windows account). Existing plain-text passwords are migrated automatically.
- 💾 **Preferences actually persist:** the transport mode (Auto/USB/WiFi), passcode target cache, keypad language and Bold Text toggle now survive restarts instead of resetting to defaults.
- 🛡️ **Safer device writes and restores:** if writing an original card file back fails, Silo keeps the recovered copy on the iPhone instead of deleting the only copy; wallet backups, saved cards and settings are now written atomically.
- 🎨 **More reliable artwork loading:** concurrent card-artwork reads are queued instead of stacked (which could crash the iPhone's sync service), and stale responses no longer clear the current preview.
- 📡 **Keycard sharing fixed:** stopping a LAN share now releases the port promptly, the certificate password field is masked, and regenerating a keycard invalidates the old share link.
- 🧹 **Updater and installer hardening:** interrupted downloads are deleted, downloaded builds are verified, failed app launches surface an error instead of silently closing, and uninstall cleans up legacy `AirCard` data too.
- 🐛 No changes to your saved cards, themes or backups.

## What's New in 1.5.3
- 🪟 **Translucent redesign:** the familiar sidebar interface now floats as **frosted glass** over a soft gradient backdrop — the sidebar, cards and status bar are semi-transparent instead of solid.
- 🌫️ **Gradient backdrop:** a subtle vertical gradient with soft accent glows sits behind the whole app, so every surface blends over it.
- 🎨 **Tuned palette:** softer surfaces, refined secondary text and deeper hairline edges, in both light and dark appearances.
- 🔧 **Everything from 1.5.2 unchanged:** the built-in updater, the System appearance option and automatic device detection all work exactly as before.
- 🐛 No changes to your saved cards, themes or backups.

## What's New in 1.5.2
- 🔄 **Built-in updater:** Silo now checks for new releases on its own and can download and install them in place — no need to re-run the installer. Check manually from the new **Updates** tab or let it check on launch.
- 🌗 **System theme:** the appearance setting now offers **System** (follows your Windows light/dark setting, and updates live), alongside Light and Dark.
- 🔌 **Automatic device detection:** Silo now refreshes itself the moment an iPhone connects or disconnects — the manual refresh buttons are gone.
- 🎛️ **Refreshed sidebar:** macOS System Settings-style navigation with colored icon chips, plus an accent badge on the Updates tab when a new version is available.
- 🐛 Plus assorted internal cleanups. No changes to your saved cards, themes or backups.

## What's New in 1.5.1
- 🩹 **Safer restores:** *Restore Original* now resolves the exact on-device card folder (padded/unpadded hash variant) before writing, so it always lands on the correct card instead of a stray folder.
- 🧹 **Cleaner transport shutdown:** the AirTraffic sync worker now cancels promptly when the host timeout is reached, releasing the device connection instead of leaving a stalled session behind.
- 🔎 **Smarter card detection:** URL-safe base64 card hashes that contain `-`/`_` separators are no longer rejected as system strings.
- ⚡ **Faster artwork updates:** the card preview channel is now persistent, so rapid rescans no longer drop the in-flight artwork response.
- 🐛 Plus assorted internal cleanups. No changes to your saved cards, themes or backups.

## What's New in 1.5.0
- 🏷️ **Rebranded as Silo:** the app, window title, binary (`silo.exe`) and data folder (`%LOCALAPPDATA%\Silo`) are now named Silo — existing settings, saved cards and backups from the AirCard folder are migrated automatically on first launch.
- 🎨 **Complete UI redesign:** a new Apple Wallet-inspired interface — soft elevated cards, capsule buttons, hairline separators, generous whitespace, hand-drawn vector icons, and smooth micro-animations across every screen.
- 🌗 **Light & Dark themes:** switch instantly from the sun/moon button in the top bar or in Settings; the choice is saved on your PC.
- 🧭 **New sidebar:** macOS-style navigation with animated selection, app badge, and a device status card docked at the bottom.
- 📱 **Lock-screen keypad preview:** the Passcode Studio now renders your theme inside a realistic dark device screen with a clock and circular key backplates.
- 🪄 **Drag & drop import:** drop artwork (PNG/JPG/WebP) anywhere on the window to load it into the Wallet studio, or drop a `.passthm` package to open it in the Passcode studio.
- 🧩 **Refactored UI layer:** the interface is now split into focused modules (theme, icons, widgets, and one file per page) for faster future development.
- 📦 **New installer:** a from-scratch dark setup wizard (`installer/target/release/silo-installer.exe`) with animated pages, progress and transitions — installs Silo per-user to `%LOCALAPPDATA%\Programs\Silo` with shortcuts, an *Apps & features* entry, and a matching uninstaller (the installer is itself the uninstaller via `--uninstall`).
- ✨ Plus: version is now read from a single source of truth, and the Settings page gained an Appearance section.

## Features
- 🎨 **Custom Card Skins:** Assign custom artwork, textures, or bank logos to Apple Pay and Apple Cash cards.
- 🔢 **Lock Screen Passcode Themes (.passthm):** Apply custom keypad button artwork from popular Cowabunga & Nugget `.passthm` themes directly to iOS lockscreen.
- 🏠 **Dashboard:** View device status, saved cards, recent activity, and one-click workflow shortcuts.
- ⚙️ **Workspace Settings:** Configure transport and interface language from a dedicated settings page.
- ⚡ **100% Native & Lightweight:** Single standalone `silo.exe` (under 10 MB). No Python, no Flet, no webview, no bloated runtimes.
- 🪟 **Workspace UI:** Sidebar navigation, focused Wallet and Passcode studios, and a redesigned dark theme built with `egui` and `eframe`.
- 📱 **Zero-Hassle Card Detection:** Tap any card in your iPhone's Wallet app while connected to detect its hash in real-time via `syslog_relay`.
- 📶 **USB & WiFi Transport:** Scan card events and apply Wallet or passcode assets through USB or a paired local WiFi connection.
- 🌐 **English / Simplified Chinese UI:** Switch the interface language from the top bar; the selection is saved locally for future launches.
- 🔄 **Safe & Reversible:** Complete Books state snapshot and automatic restore engine — preserves original device state and backs up the original Wallet card face before replacing it.
- 🚀 **Zero Jailbreak:** Utilizes Apple's built-in AirTraffic sync conduit without modifying system partitions or disabling security.

---

## Requirements
- **Windows 10 / 11 (64-bit)**
- **Apple Mobile Device Support / 64-bit iTunes** (required for Apple device communication).
- A Lightning or USB-C cable for the initial trust/pairing setup.
- For WiFi mode, enable WiFi sync and keep the PC and iPhone on the same local network.

---

## ⚠️ Troubleshooting & Driver Repair (If Nothing Works)

> [!TIP]
> **iPhone not detected, AirTraffic sync hangs, or operation fails?**  
> Corrupted or conflicting Apple USB drivers on Windows are the #1 root cause.
> 1. Download and install **[3uTools](https://www.3u.com/)**.
> 2. **Disconnect your iPhone** from your PC.
> 3. In 3uTools, go to **Toolbox ➔ Repair Driver**.
> 4. Click **Repair Now** and wait for the Apple driver reinstallation to finish.
> 5. Reconnect your unlocked iPhone, tap **Trust**, and launch **Silo**.

---

## Installation

### Pre-built Executable
1. Download **`silo.exe`** from [Releases](https://github.com/Lumid-Off/AirCard-Windows/releases).
2. Connect your iPhone via USB, unlock it, and tap **"Trust this Computer"** if prompted.
3. Run **`silo.exe`**. After WiFi sync is enabled, later sessions can work without the cable.

---

## WiFi Connection Setup
1. Connect the iPhone by USB for the initial pairing.
2. In Apple Devices or iTunes, enable **Show this iPhone when on Wi-Fi** / **Sync with this iPhone over Wi-Fi**.
3. Apply the setting, then keep the iPhone and PC on the same local network.
4. In Silo, click **Refresh** and confirm the device shows a **WiFi** transport.
5. Disconnect the cable, click **Refresh** again, and select **WiFi only**. Use **Auto (USB preferred)** when automatic fallback is desired.

If both transports are available, **Auto** uses USB first and falls back to WiFi. For a guaranteed end-to-end WiFi route, disconnect the USB cable, click **Refresh**, and then choose **WiFi only**. This is required because Apple's AirTraffic API selects its route by UDID rather than accepting a transport parameter.

---

## How to Customize Apple Wallet Cards
1. Connect your iPhone through USB or paired WiFi and ensure it is unlocked.
2. In Silo, stay on the **Wallet** tab and click **Scan**.
3. On your iPhone:
   - Open **Apple Wallet** (or double-click the Side/Power button).
   - Tap the card you want to customize.
   - Silo intercepts and saves the card hash automatically. Click **Stop**.
4. Click **Choose Image...** to pick your artwork (PNG, JPG, or WebP — drag inside the preview to position the crop, then scale it to `1536 × 969`).
5. Click **Apply Card Skin**.
6. Force-close the **Wallet** app on your iPhone from the App Switcher (swipe up from bottom, then swipe Wallet away) and reopen Wallet to see your new card!
7. The first apply stores a local backup of the original card face. Use **Restore Original** later to write it back and invalidate Wallet's cached artwork.

---

## How to Apply Lockscreen Passcode Themes (.passthm)
1. Switch to the **Passcode** tab in Silo.
2. Click **Choose .passthm...** and select any `.passthm` package (Cowabunga or Nugget).
3. Select your target iOS version cache:
   - **Auto (TelephonyUI-10)** — iOS 18+ (Default)
   - **TelephonyUI-9** — iOS 16 - 17
   - **TelephonyUI-8** — Legacy iOS
4. Click **Apply Passcode Theme**.
5. Lock your iPhone screen or open Phone dialer to see your new custom passcode keypad buttons!

> [!IMPORTANT]
> **Turn OFF Bold Text:**  
> On your iPhone, go to **Settings ➔ Display & Brightness** and make sure **Bold Text** is turned **OFF**. If Bold Text is enabled, iOS ignores cached dialer button graphics and renders system vector fonts instead.

---

## Building from Source

Prerequisites: [Rust toolchain](https://rustup.rs/) (`stable-x86_64-pc-windows-msvc`).

```powershell
# Clone the repository
git clone https://github.com/Lumid-Off/AirCard-Windows.git
cd AirCard-Windows

# Run tests
cargo test

# Build release binary
cargo build --release
```

The compiled binary will be in `target\release\silo.exe`.

---

## Contributors
- **[@Lumid-Off](https://github.com/Lumid-Off)** (Windows Native Rust Port & Maintainer) — [GitHub](https://github.com/Lumid-Off) · [Twitter / X](https://x.com/LumidOff)
- **[@mak5er](https://github.com/mak5er)** (Original macOS App & Exploit Research) — [GitHub](https://github.com/mak5er) · [Twitter / X](https://x.com/mak5er)
- **[AirLift](https://github.com/0xjohnnydev/airlift)** by **[0xjohnny (@0xjohnnydev)](https://github.com/0xjohnnydev)**: Original AirTraffic/ATAirlock sandbox escape and proof of concept underlying `AirliftFFI`.

## Credits
- Core exploit based on `airlift` (AirTraffic sync escape).
- Theme format inspired by [Cowabunga](https://github.com/leminlimez/Cowabunga) and [Nugget](https://github.com/leminlimez/Nugget).

## Support

If you find Silo useful, you can support future development:

- **TON**: `UQB5jbOhep98IvgKjCIsJ1hHGRh2iWkacAZh9jW2DMIVSWZm`
- **USDT (TRC20)**: `TLiVnkPZ7mVKwCD9RfE28uVTFk6sD6rux2`
- **USDT / BNB (BEP20)**: `0x8EA94e79e47FafBCE10E65342D875c86d1019541`
