use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use image::DynamicImage;

use crate::apple;
use crate::device::{
    ConnectionMode, DeviceInfo, DeviceTransport, UsbmuxPoller, devices_from_entries,
    list_connected_devices,
};
use crate::flasher::{flash_passcode_theme, flash_wallet_skin, restore_wallet_original};
use crate::i18n::{Language, Settings};
use crate::image_skin::PreparedSkin;
use crate::keycard::{KeycardPrefs, PassServer};
use crate::passthm::{PasscodeTheme, parse_passthm_file};
use crate::scanner::{SavedCard, load_saved_cards, scan_syslog_for_cards};
use crate::ui::theme::{self, Appearance};
use crate::ui::{AppTab, icons, widgets};
use crate::ui::widgets::ButtonKind;
use crate::updater::{self, ReleaseInfo};
use crate::wallet_backup::{backup_exists, load_current_card_artwork};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) enum BackgroundTaskMessage {
    Progress {
        step: usize,
        total: usize,
        message: String,
    },
    Log(String),
    CardFound {
        hash: String,
        name: String,
    },
    CardArtworkLoaded {
        hash: String,
        result: Result<Vec<u8>, String>,
    },
    Done(Result<String, String>),
}

/// What the Updates tab is currently showing.
#[derive(Clone)]
pub(crate) enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Downloading { done: u64, total: u64 },
    Ready(PathBuf),
    Error(String),
}

/// Messages sent from the background update worker to the UI thread.
enum UpdateEvent {
    Checked(Result<Option<ReleaseInfo>, String>),
    Progress(u64, u64),
    Downloaded(Result<PathBuf, String>),
}

#[cfg(windows)]
fn current_timestamp() -> String {
    #[repr(C)]
    struct SystemTime {
        w_year: u16,
        w_month: u16,
        w_day_of_week: u16,
        w_day: u16,
        w_hour: u16,
        w_minute: u16,
        w_second: u16,
        w_milliseconds: u16,
    }
    unsafe extern "system" {
        fn GetLocalTime(lpSystemTime: *mut SystemTime);
    }
    let mut st = std::mem::MaybeUninit::<SystemTime>::uninit();
    unsafe {
        GetLocalTime(st.as_mut_ptr());
        let st = st.assume_init();
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            st.w_hour, st.w_minute, st.w_second, st.w_milliseconds
        )
    }
}

#[cfg(not(windows))]
fn current_timestamp() -> String {
    "00:00:00.000".to_string()
}

/// Primary monitor work area (screen minus taskbar) in egui points.
#[cfg(windows)]
fn primary_work_area_points(ctx: &egui::Context) -> Option<egui::Vec2> {
    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[allow(non_upper_case_globals)]
    const SPI_GETWORKAREA: u32 = 0x0030;
    unsafe extern "system" {
        fn SystemParametersInfoW(
            ui_action: u32,
            ui_param: u32,
            pv_param: *mut WinRect,
            ui_win_ini: u32,
        ) -> i32;
    }
    let mut rect = WinRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect, 0) } != 0;
    if !ok {
        return None;
    }
    // The process is DPI-aware, so this is in physical pixels; convert to points.
    let ppp = ctx.pixels_per_point().max(1.0);
    Some(egui::vec2(
        (rect.right - rect.left) as f32 / ppp,
        (rect.bottom - rect.top) as f32 / ppp,
    ))
}

#[cfg(not(windows))]
fn primary_work_area_points(_ctx: &egui::Context) -> Option<egui::Vec2> {
    None
}

/// Whether the connected-device set differs in any way that matters to the UI.
fn devices_differ(current: &[DeviceInfo], incoming: &[DeviceInfo]) -> bool {
    if current.len() != incoming.len() {
        return true;
    }
    current.iter().zip(incoming).any(|(a, b)| {
        !a.udid.eq_ignore_ascii_case(&b.udid)
            || a.name != b.name
            || a.transport_summary() != b.transport_summary()
    })
}

pub struct SiloApp {
    pub(crate) current_tab: AppTab,
    pub(crate) displayed_tab: AppTab,
    pub(crate) page_transition_started: Option<f64>,
    did_fit_screen: bool,
    pub(crate) language: Language,
    pub(crate) appearance: Appearance,
    pub(crate) apple_status: String,
    pub(crate) apple_ready: bool,

    // Device management
    pub(crate) devices: Vec<DeviceInfo>,
    pub(crate) selected_udid: Option<String>,
    pub(crate) connection_mode: ConnectionMode,

    // Wallet tab
    pub(crate) card_hash: String,
    pub(crate) saved_cards: Vec<SavedCard>,
    pub(crate) source_path: Option<PathBuf>,
    pub(crate) source_image: Option<DynamicImage>,
    pub(crate) source_texture: Option<egui::TextureHandle>,
    pub(crate) current_card_texture: Option<egui::TextureHandle>,
    pub(crate) current_card_name: String,
    pub(crate) current_card_hash: String,
    pub(crate) current_card_loading: bool,
    pub(crate) artwork_in_flight_hash: Option<String>,
    pub(crate) current_card_error: Option<String>,
    pub(crate) card_reveal_started: Option<f64>,
    pub(crate) crop_focus: [f32; 2],
    pub(crate) crop_dirty: bool,
    pub(crate) skin: Option<PreparedSkin>,
    pub(crate) scanning_syslog: bool,
    pub(crate) scan_stop_flag: Option<Arc<AtomicBool>>,

    // Passcode tab
    pub(crate) theme_path: Option<PathBuf>,
    pub(crate) loaded_theme: Option<PasscodeTheme>,
    pub(crate) forced_telephony_ver: String,
    pub(crate) keypad_language: String,
    pub(crate) passcode_bold: bool,
    pub(crate) keypad_textures: Vec<(String, egui::TextureHandle)>,

    // Keycard tab
    pub(crate) keycard: KeycardPrefs,
    pub(crate) keycard_status: String,
    pub(crate) keycard_last_path: Option<PathBuf>,
    pub(crate) keycard_server: Option<PassServer>,
    pub(crate) keycard_output_dir: Option<PathBuf>,

    // Worker thread & progress
    pub(crate) is_busy: bool,
    pub(crate) progress_step: usize,
    pub(crate) progress_total: usize,
    pub(crate) progress_msg: String,
    pub(crate) status_msg: String,
    pub(crate) task_rx: Option<Receiver<BackgroundTaskMessage>>,
    pub(crate) artwork_tx: Sender<BackgroundTaskMessage>,
    pub(crate) artwork_rx: Receiver<BackgroundTaskMessage>,
    pub(crate) logs: Vec<String>,
    pub(crate) show_logs_window: bool,

    // Self-updater
    pub(crate) update_state: UpdateState,
    update_rx: Option<Receiver<UpdateEvent>>,
    update_silent: bool,

    // Automatic device detection
    device_rx: Receiver<Vec<DeviceInfo>>,
    last_theme_sync: Option<Instant>,
}

impl SiloApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        theme::setup_fonts(&cc.egui_ctx);
        theme::apply_theme(&cc.egui_ctx, settings.appearance);

        let (apple_ready, apple_status) = match apple::verify_support() {
            Ok(msg) => (true, msg),
            Err(err) => (false, err.to_string()),
        };

        let language = settings.language;
        let (artwork_tx, artwork_rx) = channel();

        // Background device poller: keeps the device list current so an iPhone
        // is picked up (and dropped) automatically, with no manual refresh.
        let (device_tx, device_rx) = channel();
        {
            let poll_ctx = cc.egui_ctx.clone();
            thread::spawn(move || {
                // Hold one long-lived usbmuxd connection and only re-enrich device
                // metadata when the device set actually changes. Opening (and
                // abandoning) a fresh connection on every tick left thousands of
                // TIME_WAIT sockets that exhausted the ephemeral port range, and
                // reconnecting lockdownd each tick added more churn.
                let mut poller: Option<UsbmuxPoller> = None;
                let mut last_signature = String::new();
                let mut cached: Vec<DeviceInfo> = Vec::new();
                loop {
                    if poller.is_none() {
                        poller = UsbmuxPoller::connect();
                    }
                    if let Some(active) = poller.as_mut() {
                        match active.list_devices() {
                            Ok(entries) => {
                                let signature = entries
                                    .iter()
                                    .map(|entry| format!("{}:{:?}", entry.udid, entry.transport))
                                    .collect::<Vec<_>>()
                                    .join("|");
                                if signature != last_signature
                                    && let Ok(devices) = devices_from_entries(entries)
                                {
                                    cached = devices;
                                    last_signature = signature;
                                }
                                if device_tx.send(cached.clone()).is_err() {
                                    break;
                                }
                                poll_ctx.request_repaint();
                            }
                            Err(_) => {
                                // Connection dropped; reconnect on the next tick.
                                poller = None;
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(1500));
                }
            });
        }

        let mut app = Self {
            current_tab: AppTab::Dashboard,
            displayed_tab: AppTab::Dashboard,
            page_transition_started: None,
            did_fit_screen: false,
            language,
            appearance: settings.appearance,
            apple_status,
            apple_ready,

            devices: Vec::new(),
            selected_udid: None,
            connection_mode: ConnectionMode::Auto,

            card_hash: String::new(),
            saved_cards: load_saved_cards(),
            source_path: None,
            source_image: None,
            source_texture: None,
            current_card_texture: None,
            current_card_name: String::new(),
            current_card_hash: String::new(),
            current_card_loading: false,
            artwork_in_flight_hash: None,
            current_card_error: None,
            card_reveal_started: None,
            crop_focus: [0.5, 0.5],
            crop_dirty: false,
            skin: None,
            scanning_syslog: false,
            scan_stop_flag: None,

            theme_path: None,
            loaded_theme: None,
            forced_telephony_ver: "Auto (TelephonyUI-10)".to_string(),
            keypad_language: "English".to_string(),
            passcode_bold: false,
            keypad_textures: Vec::new(),

            keycard: settings.keycard.clone(),
            keycard_status: String::new(),
            keycard_last_path: None,
            keycard_server: None,
            keycard_output_dir: None,

            is_busy: false,
            progress_step: 0,
            progress_total: 0,
            progress_msg: String::new(),
            status_msg: language
                .text("Ready. Connect iPhone via USB or paired WiFi and unlock it.")
                .to_string(),
            task_rx: None,
            artwork_tx,
            artwork_rx,
            logs: Vec::new(),
            show_logs_window: false,

            update_state: UpdateState::Idle,
            update_rx: None,
            update_silent: false,

            device_rx,
            last_theme_sync: None,
        };

        app.add_log(format!("Silo Windows v{} initialized", APP_VERSION));
        app.add_log(format!(
            "Apple Support Runtime: {}",
            if app.apple_ready {
                "Loaded and operational"
            } else {
                "Not found (iTunes required)"
            }
        ));
        app.add_log(format!(
            "Loaded {} saved card(s) from database",
            app.saved_cards.len()
        ));

        if app.apple_ready {
            app.refresh_devices();
        }

        // Clear any leftover downloaded build, then quietly look for a newer
        // release in the background (the Updates tab reflects the result).
        updater::cleanup_updates();
        app.check_for_updates(true);

        app
    }

    pub(crate) fn add_log(&mut self, text: impl AsRef<str>) {
        let ts = current_timestamp();
        self.logs.push(format!("[{}] {}", ts, text.as_ref()));
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }

    pub(crate) fn persist_settings(&self) {
        Settings {
            language: self.language,
            appearance: self.appearance,
            keycard: self.keycard.clone(),
        }
        .save();
    }

    pub(crate) fn set_appearance(&mut self, appearance: Appearance, ctx: &egui::Context) {
        if self.appearance == appearance {
            return;
        }
        self.appearance = appearance;
        theme::apply_theme(ctx, appearance);
        self.persist_settings();
        self.add_log(format!("Appearance changed to {}.", appearance.label()));
        self.status_msg = format!(
            "{}: {}",
            self.language.text("Appearance"),
            self.language.text(appearance.label())
        );
    }

    /// Kick off a background release check. When `silent`, errors are swallowed
    /// (used for the quiet check on launch).
    pub(crate) fn check_for_updates(&mut self, silent: bool) {
        if self.update_rx.is_some() {
            return;
        }
        self.update_silent = silent;
        if !silent {
            self.update_state = UpdateState::Checking;
        }
        let (tx, rx) = channel();
        self.update_rx = Some(rx);
        let current = APP_VERSION.to_string();
        thread::spawn(move || {
            let result = updater::check_for_update(&current).map_err(|e| format!("{e:#}"));
            let _ = tx.send(UpdateEvent::Checked(result));
        });
    }

    /// Start downloading the given release on a background thread.
    pub(crate) fn download_update(&mut self, info: ReleaseInfo) {
        if self.update_rx.is_some() {
            return;
        }
        self.update_state = UpdateState::Downloading {
            done: 0,
            total: info.download_size,
        };
        let (tx, rx) = channel();
        self.update_rx = Some(rx);
        thread::spawn(move || {
            let result = updater::download_update(&info, |done, total| {
                let _ = tx.send(UpdateEvent::Progress(done, total));
            })
            .map_err(|e| format!("{e:#}"));
            let _ = tx.send(UpdateEvent::Downloaded(result));
        });
    }

    /// Swap the downloaded build over the running executable and restart.
    /// The window is closed so the file lock is released; the detached helper
    /// does the actual replacement.
    pub(crate) fn apply_update(&mut self, ctx: &egui::Context) {
        let UpdateState::Ready(path) = &self.update_state else {
            return;
        };
        let target = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.update_state =
                    UpdateState::Error(format!("Could not locate the running app: {error}"));
                return;
            }
        };
        match updater::launch_apply_update(path, &target) {
            Ok(()) => {
                self.add_log("Applying update; restarting Silo...");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(error) => self.update_state = UpdateState::Error(format!("{error:#}")),
        }
    }

    fn handle_update_messages(&mut self) {
        let mut events = Vec::new();
        if let Some(ref rx) = self.update_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                UpdateEvent::Checked(Ok(Some(info))) => {
                    self.update_state = UpdateState::Available(info);
                    self.update_rx = None;
                    self.update_silent = false;
                }
                UpdateEvent::Checked(Ok(None)) => {
                    self.update_state = if self.update_silent {
                        UpdateState::Idle
                    } else {
                        UpdateState::UpToDate
                    };
                    self.update_rx = None;
                    self.update_silent = false;
                }
                UpdateEvent::Checked(Err(error)) => {
                    self.update_state = if self.update_silent {
                        UpdateState::Idle
                    } else {
                        UpdateState::Error(error)
                    };
                    self.update_rx = None;
                    self.update_silent = false;
                }
                UpdateEvent::Progress(done, total) => {
                    self.update_state = UpdateState::Downloading { done, total };
                }
                UpdateEvent::Downloaded(Ok(path)) => {
                    self.update_state = UpdateState::Ready(path);
                    self.update_rx = None;
                }
                UpdateEvent::Downloaded(Err(error)) => {
                    self.update_state = UpdateState::Error(error);
                    self.update_rx = None;
                }
            }
        }
    }

    pub(crate) fn refresh_devices(&mut self) {
        self.add_log("Scanning for connected iOS devices via usbmuxd...");
        match list_connected_devices() {
            Ok(devs) => {
                self.devices = devs;
                let selection_still_exists = self.selected_udid.as_ref().is_some_and(|selected| {
                    self.devices
                        .iter()
                        .any(|device| device.udid.eq_ignore_ascii_case(selected))
                });
                if !selection_still_exists && !self.devices.is_empty() {
                    self.selected_udid = Some(self.devices[0].udid.clone());
                }
                if self.devices.is_empty() {
                    self.selected_udid = None;
                    self.add_log("No devices detected. Connect by USB, or enable WiFi sync after initial USB pairing.");
                    self.status_msg = self
                        .language
                        .text("No iPhone connected via USB or paired WiFi.")
                        .to_string();
                } else {
                    let dev_logs: Vec<String> = self
                        .devices
                        .iter()
                        .enumerate()
                        .map(|(i, d)| format!("Device #{}: {} - UDID: {}", i + 1, d, d.udid))
                        .collect();
                    for line in dev_logs {
                        self.add_log(line);
                    }
                    self.status_msg = format!(
                        "{} {} {} {}",
                        self.language.text("Found"),
                        self.devices.len(),
                        self.language.text("connected device(s); transport mode:"),
                        self.language.text(self.connection_mode.label())
                    );
                }
            }
            Err(err) => {
                self.add_log(format!("Device scan error: {}", err));
                self.status_msg = format!(
                    "{} {}",
                    self.language.text("Could not enumerate devices:"),
                    err
                );
            }
        }
    }

    /// Drain the background device poller and apply any change quietly.
    fn poll_devices(&mut self) {
        let mut latest = None;
        while let Ok(devices) = self.device_rx.try_recv() {
            latest = Some(devices);
        }
        if let Some(devices) = latest {
            self.apply_devices(devices);
        }
    }

    /// Update the device list from an automatic scan, logging only when the
    /// set of connected devices actually changes.
    fn apply_devices(&mut self, devices: Vec<DeviceInfo>) {
        if !devices_differ(&self.devices, &devices) {
            return;
        }
        let previous: Vec<String> = self.devices.iter().map(|d| d.udid.clone()).collect();
        self.devices = devices;

        let selection_still_exists = self.selected_udid.as_ref().is_some_and(|selected| {
            self.devices
                .iter()
                .any(|device| device.udid.eq_ignore_ascii_case(selected))
        });
        if !selection_still_exists {
            self.selected_udid = self.devices.first().map(|device| device.udid.clone());
        }

        // Change-driven logging: the manual refresh used to log every scan,
        // but automatic polling must stay quiet.
        let mut messages: Vec<String> = Vec::new();
        for device in &self.devices {
            if !previous
                .iter()
                .any(|udid| udid.eq_ignore_ascii_case(&device.udid))
            {
                messages.push(format!(
                    "iPhone connected: {} ({})",
                    device.name,
                    device.transport_summary()
                ));
            }
        }
        for udid in &previous {
            if !self
                .devices
                .iter()
                .any(|device| device.udid.eq_ignore_ascii_case(udid))
            {
                messages.push("iPhone disconnected.".to_string());
            }
        }
        for message in messages {
            self.add_log(message);
        }

        if self.devices.is_empty() {
            self.status_msg = self
                .language
                .text("No iPhone connected via USB or paired WiFi.")
                .to_string();
        } else {
            self.status_msg = format!(
                "{} {} {}",
                self.language.text("Found"),
                self.devices.len(),
                self.language.text("connected device(s); transport mode:")
            );
        }
    }

    /// When following the system theme, re-apply it if Windows flips.
    fn sync_system_theme(&mut self, ctx: &egui::Context) {
        if self.appearance != Appearance::System {
            return;
        }
        let due = self
            .last_theme_sync
            .is_none_or(|last| last.elapsed() >= Duration::from_millis(1500));
        if !due {
            return;
        }
        self.last_theme_sync = Some(Instant::now());
        if theme::palette(ctx).dark != self.appearance.is_dark() {
            theme::apply_theme(ctx, self.appearance);
        }
    }

    pub(crate) fn selected_transport_available(&self) -> bool {
        self.selected_udid.as_ref().is_some_and(|selected| {
            self.devices.iter().any(|device| {
                if !device.udid.eq_ignore_ascii_case(selected) {
                    return false;
                }
                if self.connection_mode == ConnectionMode::Wifi {
                    return device.has_transport(DeviceTransport::Wifi)
                        && !device.has_transport(DeviceTransport::Usb);
                }
                device.supports(self.connection_mode)
            })
        })
    }

    fn validate_selected_transport(&mut self, operation: &str) -> bool {
        if self.selected_udid.is_none() {
            self.add_log(format!(
                "{} failed: No connected iPhone selected.",
                operation
            ));
            self.status_msg = self
                .language
                .text("Please select a connected iPhone.")
                .to_string();
            return false;
        }
        if !self.selected_transport_available() {
            let wifi_has_usb_attached = self.connection_mode == ConnectionMode::Wifi
                && self.selected_udid.as_ref().is_some_and(|selected| {
                    self.devices.iter().any(|device| {
                        device.udid.eq_ignore_ascii_case(selected)
                            && device.has_transport(DeviceTransport::Wifi)
                            && device.has_transport(DeviceTransport::Usb)
                    })
                });
            self.add_log(format!(
                "{} failed: Selected device is unavailable in {} mode.",
                operation,
                self.connection_mode.label()
            ));
            self.status_msg = if wifi_has_usb_attached {
                self.language
                    .text("Disconnect the USB cable and refresh to guarantee the full AirTraffic path uses WiFi.")
                    .to_string()
            } else {
                format!(
                    "{} {} {}",
                    self.language.text("Selected iPhone has no"),
                    self.language.text(self.connection_mode.label()),
                    self.language
                        .text("connection. Refresh devices or change transport mode.")
                )
            };
            return false;
        }
        true
    }

    pub(crate) fn select_skin(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        self.load_skin_from_path(ctx, &path);
    }

    /// Load artwork from a path (file dialog or drag-and-drop).
    pub(crate) fn load_skin_from_path(&mut self, ctx: &egui::Context, path: &Path) {
        self.add_log(format!("Opening skin image: {}", path.display()));
        match image::open(path) {
            Ok(source_image) => {
                let source_width = source_image.width();
                let source_height = source_image.height();
                let skin = match PreparedSkin::from_image_with_focus(source_image.clone(), 0.5, 0.5)
                {
                    Ok(skin) => skin,
                    Err(error) => {
                        self.add_log(format!("Image preparation failed: {error:#}"));
                        self.status_msg = format!("Could not prepare image: {error:#}");
                        return;
                    }
                };
                let source_rgba = source_image.thumbnail(2048, 2048).to_rgba8();
                let source_preview = egui::ColorImage::from_rgba_unmultiplied(
                    [source_rgba.width() as usize, source_rgba.height() as usize],
                    source_rgba.as_raw(),
                );

                self.add_log(format!(
                    "Skin processed: source {}x{} resampled to 1536x969 PNG ({:.1} KB)",
                    skin.source_width,
                    skin.source_height,
                    skin.png.len() as f32 / 1024.0,
                ));
                self.source_texture = Some(ctx.load_texture(
                    "card-skin-source-preview",
                    source_preview,
                    egui::TextureOptions::LINEAR,
                ));
                self.status_msg = format!(
                    "Prepared {} ({}x{} -> 1536x969 PNG, {:.1} KB)",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("image"),
                    source_width,
                    source_height,
                    skin.png.len() as f32 / 1024.0,
                );
                self.source_path = Some(path.to_path_buf());
                self.source_image = Some(source_image);
                self.crop_focus = [0.5, 0.5];
                self.crop_dirty = false;
                self.skin = Some(skin);
            }
            Err(error) => {
                self.add_log(format!("Image decode failed: {error:#}"));
                self.status_msg = format!("Could not decode image: {error:#}");
            }
        }
    }

    pub(crate) fn rebuild_skin_from_source(&mut self) {
        let Some(source_image) = self.source_image.as_ref() else {
            return;
        };

        match PreparedSkin::from_image_with_focus(
            source_image.clone(),
            self.crop_focus[0],
            self.crop_focus[1],
        ) {
            Ok(skin) => {
                self.add_log(format!(
                    "Crop updated: focus ({:.2}, {:.2}), prepared PNG {:.1} KB",
                    self.crop_focus[0],
                    self.crop_focus[1],
                    skin.png.len() as f32 / 1024.0,
                ));
                self.skin = Some(skin);
                self.crop_dirty = false;
                self.status_msg = self.language.text("Crop position updated.").to_string();
            }
            Err(error) => {
                self.add_log(format!("Crop preparation failed: {error:#}"));
                self.status_msg = format!("Could not update crop: {error:#}");
            }
        }
    }

    pub(crate) fn save_prepared_png(&mut self) {
        let Some(skin) = &self.skin else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("silo-skin.png")
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, &skin.png) {
            Ok(()) => {
                self.add_log(format!(
                    "Exported prepared card skin PNG: {}",
                    path.display()
                ));
                self.status_msg = format!("Saved prepared PNG: {}", path.display());
            }
            Err(err) => {
                self.add_log(format!("Failed to save PNG: {err}"));
                self.status_msg = format!("Could not save PNG: {err}");
            }
        }
    }

    pub(crate) fn toggle_syslog_scan(&mut self) {
        if self.scanning_syslog {
            if let Some(flag) = self.scan_stop_flag.take() {
                flag.store(true, Ordering::Relaxed);
            }
            self.scanning_syslog = false;
            self.add_log("Syslog scanning stopped by user.");
            self.status_msg = self.language.text("Syslog scanning stopped.").to_string();
            return;
        }

        if !self.validate_selected_transport("Syslog scan") {
            return;
        }

        let stop_flag = Arc::new(AtomicBool::new(false));
        self.scan_stop_flag = Some(Arc::clone(&stop_flag));
        self.scanning_syslog = true;
        self.add_log("Initiating syslog monitor session...");
        self.status_msg = self
            .language
            .text("Scanning syslog... Open Wallet or tap your card on iPhone.")
            .to_string();

        let (tx, rx) = channel();
        self.task_rx = Some(rx);
        let udid = self.selected_udid.clone();
        let connection_mode = self.connection_mode;
        let language = self.language;

        thread::spawn(move || {
            let tx_card = tx.clone();
            let tx_log = tx.clone();
            let res = scan_syslog_for_cards(
                udid.as_deref(),
                connection_mode,
                stop_flag,
                move |hash, name| {
                    let _ = tx_card.send(BackgroundTaskMessage::CardFound { hash, name });
                },
                move |msg| {
                    let _ = tx_log.send(BackgroundTaskMessage::Log(msg));
                },
            );
            match res {
                Ok(()) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Ok(language
                        .text("Syslog scan finished")
                        .into())));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Err(e.to_string())));
                }
            }
        });
    }

    pub(crate) fn flash_card(&mut self) {
        if !self.validate_selected_transport("Card flash") {
            return;
        }
        let Some(udid) = self.selected_udid.clone() else {
            return;
        };
        let hash = self.card_hash.trim().to_string();
        if hash.is_empty() {
            self.add_log("Flash failed: Target card hash is empty.");
            self.status_msg = self
                .language
                .text("Please enter or scan a target card hash.")
                .to_string();
            return;
        }
        let Some(skin) = self.skin.as_ref() else {
            self.add_log("Flash failed: No skin image prepared.");
            self.status_msg = self
                .language
                .text("Please choose a card skin image first.")
                .to_string();
            return;
        };

        let png_bytes = skin.png.clone();
        let pdf_bytes = skin.pdf.clone();
        if let Some(ref flag) = self.scan_stop_flag {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.scanning_syslog = false;
        self.is_busy = true;
        self.progress_step = 0;
        self.progress_total = 3;
        self.progress_msg = "Initiating card flash...".to_string();
        self.status_msg = self
            .language
            .text("Writing card skin to iPhone...")
            .to_string();
        let connection_mode = self.connection_mode;
        let language = self.language;
        self.add_log(format!(
            "Starting card skin flash for hash: {} (UDID: {}, transport: {})",
            hash,
            udid,
            connection_mode.label()
        ));

        let (tx, rx) = channel();
        self.task_rx = Some(rx);

        thread::spawn(move || {
            let tx_progress = tx.clone();
            let tx_log = tx.clone();
            let res = flash_wallet_skin(
                &udid,
                connection_mode,
                &hash,
                &png_bytes,
                &pdf_bytes,
                move |step, total, msg| {
                    let _ = tx_progress.send(BackgroundTaskMessage::Progress {
                        step,
                        total,
                        message: msg.to_string(),
                    });
                },
                move |msg| {
                    let _ = tx_log.send(BackgroundTaskMessage::Log(msg.to_string()));
                },
            );

            match res {
                Ok(()) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Ok(
                        language
                            .text("Card skin successfully flashed! Force quit Wallet on iPhone and reopen it.")
                            .into(),
                    )));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Err(format!("{:#}", e))));
                }
            }
        });
    }

    pub(crate) fn restore_original_card(&mut self) {
        if !self.validate_selected_transport("Restore original card") {
            return;
        }
        let Some(udid) = self.selected_udid.clone() else {
            return;
        };
        let hash = self.card_hash.trim().to_string();
        if hash.is_empty() {
            self.add_log("Restore failed: Target card hash is empty.");
            self.status_msg = self
                .language
                .text("Please enter or scan a target card hash.")
                .to_string();
            return;
        }
        if !backup_exists(&udid, &hash) {
            self.add_log("Restore failed: Original card face backup not found.");
            self.status_msg = self
                .language
                .text("Original card backup not found.")
                .to_string();
            return;
        }

        if let Some(ref flag) = self.scan_stop_flag {
            flag.store(true, Ordering::Relaxed);
        }
        self.scanning_syslog = false;
        self.is_busy = true;
        self.progress_step = 0;
        self.progress_total = 3;
        self.progress_msg = "Restoring original card face...".to_string();
        self.status_msg = self
            .language
            .text("Restoring original card face...")
            .to_string();
        let connection_mode = self.connection_mode;
        let language = self.language;
        self.add_log(format!(
            "Starting original card face restore for hash: {} (UDID: {}, transport: {})",
            hash,
            udid,
            connection_mode.label()
        ));

        let (tx, rx) = channel();
        self.task_rx = Some(rx);

        thread::spawn(move || {
            let tx_progress = tx.clone();
            let tx_log = tx.clone();
            let res = restore_wallet_original(
                &udid,
                connection_mode,
                &hash,
                move |step, total, msg| {
                    let _ = tx_progress.send(BackgroundTaskMessage::Progress {
                        step,
                        total,
                        message: msg.to_string(),
                    });
                },
                move |msg| {
                    let _ = tx_log.send(BackgroundTaskMessage::Log(msg.to_string()));
                },
            );

            match res {
                Ok(()) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Ok(language
                        .text("Original card face restored. Force close Wallet and reopen it.")
                        .into())));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Err(format!("{:#}", e))));
                }
            }
        });
    }

    pub(crate) fn select_theme_file(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Passcode Theme", &["passthm", "passtheme", "zip"])
            .pick_file()
        else {
            return;
        };

        self.load_theme_from_path(ctx, &path);
    }

    pub(crate) fn load_theme_from_path(&mut self, ctx: &egui::Context, path: &Path) {
        self.add_log(format!(
            "Opening passcode theme package: {}",
            path.display()
        ));
        let target_ver = match self.forced_telephony_ver.as_str() {
            "TelephonyUI-10" => Some("TelephonyUI-10"),
            "TelephonyUI-9" => Some("TelephonyUI-9"),
            "TelephonyUI-8" => Some("TelephonyUI-8"),
            _ => Some("TelephonyUI-10"),
        };

        match parse_passthm_file(path, target_ver, &self.keypad_language, self.passcode_bold) {
            Ok(theme) => {
                self.keypad_textures.clear();
                for (digit, bytes) in &theme.key_previews {
                    if let Ok(img) = image::load_from_memory(bytes) {
                        let rgba = img.to_rgba8();
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            &rgba,
                        );
                        let tex = ctx.load_texture(
                            format!("keypad-{}", digit),
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.keypad_textures.push((digit.clone(), tex));
                    }
                }
                self.keypad_textures.sort_by(|a, b| a.0.cmp(&b.0));

                self.add_log(format!(
                    "Passcode theme loaded: '{}' (telephony: {}, lang: {}, bold: {}, {} assets)",
                    theme.name,
                    theme.detected_version,
                    self.keypad_language,
                    self.passcode_bold,
                    theme.items.len()
                ));
                self.status_msg = format!(
                    "{} '{}' - {} {}, {} {}, {} {}, {} {}",
                    self.language.text("Loaded"),
                    theme.name,
                    theme.items.len(),
                    self.language.text("assets"),
                    self.language.text("target:"),
                    theme.detected_version,
                    self.language.text("lang:"),
                    self.keypad_language,
                    self.language.text("bold:"),
                    self.language
                        .text(if self.passcode_bold { "ON" } else { "OFF" })
                );
                self.theme_path = Some(path.to_path_buf());
                self.loaded_theme = Some(theme);
            }
            Err(err) => {
                self.add_log(format!("Failed to parse theme: {err:#}"));
                self.status_msg =
                    format!("{} {err:#}", self.language.text("Failed to parse theme:"));
            }
        }
    }

    pub(crate) fn flash_theme(&mut self) {
        if !self.validate_selected_transport("Theme flash") {
            return;
        }
        let Some(udid) = self.selected_udid.clone() else {
            return;
        };
        let Some(theme) = self.loaded_theme.as_ref() else {
            self.add_log("Theme flash failed: No .passthm theme loaded.");
            self.status_msg = self
                .language
                .text("Please select a .passthm theme file first.")
                .to_string();
            return;
        };

        let items = theme.items.clone();
        self.is_busy = true;
        self.progress_step = 0;
        self.progress_total = items.len();
        self.progress_msg = "Starting passcode theme flash...".to_string();
        self.status_msg = self
            .language
            .text("Writing passcode theme buttons...")
            .to_string();
        let connection_mode = self.connection_mode;
        let language = self.language;
        self.add_log(format!(
            "Flashing passcode theme '{}' ({} button assets) to device {} over {}",
            theme.name,
            items.len(),
            udid,
            connection_mode.label()
        ));

        let (tx, rx) = channel();
        self.task_rx = Some(rx);

        thread::spawn(move || {
            let tx_progress = tx.clone();
            let tx_log = tx.clone();
            let res = flash_passcode_theme(
                &udid,
                connection_mode,
                &items,
                move |step, total, msg| {
                    let _ = tx_progress.send(BackgroundTaskMessage::Progress {
                        step,
                        total,
                        message: msg.to_string(),
                    });
                },
                move |msg| {
                    let _ = tx_log.send(BackgroundTaskMessage::Log(msg.to_string()));
                },
            );

            match res {
                Ok(()) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Ok(language
                        .text("Passcode theme applied! Lock your iPhone to view the new keypad.")
                        .into())));
                }
                Err(e) => {
                    let _ = tx.send(BackgroundTaskMessage::Done(Err(format!("{:#}", e))));
                }
            }
        });
    }

    pub(crate) fn request_current_card_artwork(&mut self) {
        let Some(udid) = self.selected_udid.clone() else {
            self.current_card_error = Some(
                self.language
                    .text("Connect an iPhone before loading its current card face.")
                    .to_string(),
            );
            return;
        };
        let hash = self.card_hash.trim().to_string();
        if hash.is_empty() {
            self.current_card_error = Some(
                self.language
                    .text("Scan a card or enter its card hash first.")
                    .to_string(),
            );
            return;
        }
        if !self.selected_transport_available() {
            self.current_card_error = Some(
                self.language
                    .text("The selected iPhone is unavailable on this transport.")
                    .to_string(),
            );
            return;
        }

        // Serialize artwork reads: never stack a second request for the same
        // card while one is in flight. Every read performs a device AirTraffic
        // sync, and repeated concurrent syncs crash the iPhone's `atc` daemon
        // and exhaust short-lived TCP connections.
        if self.artwork_in_flight_hash.as_deref() == Some(hash.as_str()) {
            return;
        }
        self.artwork_in_flight_hash = Some(hash.clone());

        self.current_card_hash = hash.clone();
        self.current_card_texture = None;
        self.current_card_error = None;
        self.current_card_loading = true;
        self.card_reveal_started = None;

        let connection_mode = self.connection_mode;
        let tx = self.artwork_tx.clone();
        thread::spawn(move || {
            let result = load_current_card_artwork(&udid, connection_mode, &hash)
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(BackgroundTaskMessage::CardArtworkLoaded { hash, result });
        });
    }

    /// Accept dropped artwork / theme packages anywhere in the window.
    fn handle_dropped_files(&mut self, ctx: &egui::Context, files: Vec<egui::DroppedFile>) {
        for file in files {
            let Some(path) = file.path.clone() else {
                continue;
            };
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                "png" | "jpg" | "jpeg" | "webp" => {
                    self.add_log(format!("Artwork dropped into workspace: {}", path.display()));
                    self.current_tab = AppTab::Wallet;
                    self.load_skin_from_path(ctx, &path);
                }
                "passthm" | "passtheme" | "zip" => {
                    self.add_log(format!(
                        "Passcode theme dropped into workspace: {}",
                        path.display()
                    ));
                    self.current_tab = AppTab::Passcode;
                    self.load_theme_from_path(ctx, &path);
                }
                other => {
                    self.add_log(format!(
                        "Ignored dropped file (unsupported type: {}).",
                        if other.is_empty() { "unknown" } else { other }
                    ));
                }
            }
        }
    }

    pub(crate) fn handle_messages(&mut self, ctx: &egui::Context) {
        let mut messages = Vec::new();
        if let Some(ref rx) = self.task_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }
        while let Ok(msg) = self.artwork_rx.try_recv() {
            messages.push(msg);
        }

        let mut finished = false;
        for msg in messages {
            match msg {
                BackgroundTaskMessage::Progress {
                    step,
                    total,
                    message,
                } => {
                    self.progress_step = step;
                    self.progress_total = total;
                    let localized_message = self.language.text(&message).to_string();
                    self.progress_msg = localized_message.clone();
                    let msg_str = format!("[{}/{}] {}", step, total, localized_message);
                    self.add_log(&msg_str);
                    self.status_msg = msg_str;
                }
                BackgroundTaskMessage::Log(log_line) => {
                    self.add_log(log_line);
                }
                BackgroundTaskMessage::CardFound { hash, name } => {
                    self.card_hash = hash.clone();
                    self.saved_cards = load_saved_cards();
                    self.current_card_name = name.clone();
                    self.current_card_hash = hash.clone();
                    self.current_card_texture = None;
                    self.current_card_error = None;
                    self.current_card_loading = true;
                    self.card_reveal_started = Some(ctx.input(|input| input.time));
                    let msg_str = format!(
                        "{}: {} ({})",
                        self.language.text("Card captured"),
                        name,
                        hash
                    );
                    self.add_log(&msg_str);
                    self.status_msg = msg_str;
                    self.request_current_card_artwork();
                }
                BackgroundTaskMessage::CardArtworkLoaded { hash, result } => {
                    if self.artwork_in_flight_hash.as_deref() == Some(hash.as_str()) {
                        self.artwork_in_flight_hash = None;
                    }
                    self.current_card_loading = false;
                    if hash == self.current_card_hash && hash == self.card_hash.trim() {
                        match result {
                            Ok(bytes) => match image::load_from_memory(&bytes) {
                                Ok(image) => {
                                    let rgba = image.to_rgba8();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                        [rgba.width() as usize, rgba.height() as usize],
                                        rgba.as_raw(),
                                    );
                                    self.current_card_texture = Some(ctx.load_texture(
                                        "wallet-current-card-face",
                                        color_image,
                                        egui::TextureOptions::LINEAR,
                                    ));
                                    self.current_card_error = None;
                                    self.card_reveal_started = Some(ctx.input(|input| input.time));
                                }
                                Err(error) => {
                                    self.current_card_error = Some(format!(
                                        "{} {error}",
                                        self.language
                                            .text("Could not decode the current card artwork:")
                                    ));
                                    self.add_log(format!(
                                        "Current card artwork decode failed for {hash}: {error}"
                                    ));
                                }
                            },
                            Err(error) => {
                                self.current_card_error = Some(error.clone());
                                self.add_log(format!(
                                    "Could not load current Wallet artwork for {hash}: {error}"
                                ));
                            }
                        }
                    }
                }
                BackgroundTaskMessage::Done(res) => {
                    self.is_busy = false;
                    self.scanning_syslog = false;
                    finished = true;
                    match res {
                        Ok(ok_msg) => {
                            self.add_log(format!("Operation completed: {}", ok_msg));
                            self.status_msg = ok_msg;
                        }
                        Err(err_msg) => {
                            self.add_log(format!("Operation failed: {}", err_msg));
                            self.status_msg =
                                format!("{}{}", self.language.text("Error: "), err_msg);
                        }
                    }
                }
            }
        }
        if finished {
            self.task_rx = None;
        }
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = theme::palette(ctx);

        egui::SidePanel::left("navigation")
            .exact_width(214.0)
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(egui::Margin::symmetric(14, 16)),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Brand badge.
                    ui.horizontal(|ui| {
                        let badge = egui::vec2(36.0, 36.0);
                        let (rect, _) =
                            ui.allocate_exact_size(badge, egui::Sense::hover());
                        let painter = ui.painter_at(rect);
                        theme::gradient_rounded(
                            &painter,
                            rect,
                            9.0,
                            egui::Color32::from_rgb(46, 46, 48),
                            egui::Color32::from_rgb(9, 9, 10),
                        );
                        // Wallet mark matching the app icon: a tilted pass
                        // peeking behind a solid pass with a black stripe.
                        let c = rect.center();
                        let angle: f32 = -12f32.to_radians();
                        let (s, co) = angle.sin_cos();
                        let half = egui::vec2(8.7, 6.3);
                        let back_center = c + egui::vec2(0.0, -1.0);
                        let back: Vec<egui::Pos2> = [
                            egui::vec2(-half.x, -half.y),
                            egui::vec2(half.x, -half.y),
                            egui::vec2(half.x, half.y),
                            egui::vec2(-half.x, half.y),
                        ]
                        .iter()
                        .map(|pt| {
                            egui::pos2(
                                back_center.x + pt.x * co - pt.y * s,
                                back_center.y + pt.x * s + pt.y * co,
                            )
                        })
                        .collect();
                        painter.add(egui::Shape::Path(egui::epaint::PathShape {
                            points: back,
                            closed: true,
                            fill: egui::Color32::from_white_alpha(125),
                            stroke: egui::Stroke::NONE.into(),
                        }));
                        let front = egui::Rect::from_center_size(
                            c + egui::vec2(0.0, 1.0),
                            egui::vec2(19.0, 13.5),
                        );
                        painter.rect_filled(front, 3.0, egui::Color32::WHITE);
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(front.min.x, front.min.y + 3.9),
                                egui::pos2(front.max.x, front.min.y + 8.1),
                            ),
                            0.0,
                            egui::Color32::from_rgb(26, 26, 28),
                        );
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Silo")
                                    .family(theme::strong())
                                    .size(17.0)
                                    .color(p.text),
                            );
                            ui.label(
                                egui::RichText::new(format!("v{}", APP_VERSION))
                                    .size(10.0)
                                    .color(p.text_tertiary),
                            );
                        });
                    });
                    ui.add_space(24.0);

                    ui.label(
                        egui::RichText::new(language.text("WORKSPACE"))
                            .family(theme::strong())
                            .size(10.0)
                            .color(p.text_tertiary),
                    );
                    ui.add_space(8.0);
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Dashboard,
                        language.text("Dashboard"),
                        icons::Icon::Home,
                        p.accent,
                        false,
                    );
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Wallet,
                        language.text("Wallet"),
                        icons::Icon::Card,
                        egui::Color32::from_rgb(52, 199, 89),
                        false,
                    );
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Passcode,
                        language.text("Passcode"),
                        icons::Icon::Keypad,
                        egui::Color32::from_rgb(175, 82, 222),
                        false,
                    );
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Keycard,
                        language.text("Keycard"),
                        icons::Icon::Card,
                        egui::Color32::from_rgb(255, 59, 48),
                        false,
                    );
                    ui.add_space(18.0);
                    ui.label(
                        egui::RichText::new(language.text("SUPPORT"))
                            .family(theme::strong())
                            .size(10.0)
                            .color(p.text_tertiary),
                    );
                    ui.add_space(8.0);
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Settings,
                        language.text("Settings"),
                        icons::Icon::Sliders,
                        egui::Color32::from_rgb(142, 142, 147),
                        false,
                    );
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Help,
                        language.text("Help"),
                        icons::Icon::Help,
                        egui::Color32::from_rgb(50, 173, 230),
                        false,
                    );
                    widgets::nav_item_chip(
                        ui,
                        &mut self.current_tab,
                        AppTab::Updates,
                        language.text("Updates"),
                        icons::Icon::Download,
                        egui::Color32::from_rgb(255, 149, 0),
                        matches!(self.update_state, UpdateState::Available(_)),
                    );

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        ui.add_space(10.0);
                        egui::Frame::new()
                            .fill(p.card)
                            .corner_radius(14)
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                let connected = self.selected_transport_available();
                                ui.horizontal(|ui| {
                                    let dot = if connected { p.success } else { p.error };
                                    if connected {
                                        icons::status_dot_pulsing(ui, dot);
                                    } else {
                                        icons::status_dot(ui, dot);
                                    }
                                    ui.label(
                                        egui::RichText::new(if connected {
                                            language.text("Device connected")
                                        } else {
                                            language.text("No device")
                                        })
                                        .family(theme::strong())
                                        .size(11.0)
                                        .color(p.text),
                                    );
                                });
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(if self.apple_ready {
                                        language.text("Apple services ready")
                                    } else {
                                        language.text("Apple support not found")
                                    })
                                    .size(10.0)
                                    .color(p.text_secondary),
                                )
                                .on_hover_text(&self.apple_status);
                            });
                    });
                });
            });
    }

    fn header_panel(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = theme::palette(ctx);

        egui::TopBottomPanel::top("page_header")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::symmetric(26, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(match self.current_tab {
                                AppTab::Dashboard => language.text("Dashboard"),
                                AppTab::Wallet => language.text("Wallet Studio"),
                                AppTab::Passcode => language.text("Passcode Studio"),
                                AppTab::Keycard => language.text("Keycard Studio"),
                                AppTab::Settings => language.text("Settings"),
                                AppTab::Help => language.text("Help & Guides"),
                                AppTab::Updates => language.text("Updates"),
                            })
                            .family(theme::strong())
                            .size(22.0)
                            .color(p.text),
                        );
                        ui.label(
                            egui::RichText::new(language.text("PERSONALIZE YOUR DEVICE"))
                                .size(10.0)
                                .color(p.text_secondary),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Appearance cycle: System -> Light -> Dark -> System.
                        let theme_icon = match self.appearance {
                            Appearance::System => icons::Icon::Monitor,
                            Appearance::Light => icons::Icon::Sun,
                            Appearance::Dark => icons::Icon::Moon,
                        };
                        if widgets::theme_toggle_button(ui, theme_icon, self.appearance.is_dark())
                            .on_hover_text(language.text(self.appearance.label()))
                            .clicked()
                        {
                            let next = self.appearance.cycle();
                            self.set_appearance(next, ctx);
                        }

                        let controls_enabled = !self.is_busy && !self.scanning_syslog;
                        let mut next_udid = self.selected_udid.clone();
                        let selected_label = self
                            .devices
                            .iter()
                            .find(|device| Some(&device.udid) == self.selected_udid.as_ref())
                            .map(|device| {
                                format!("{}  [{}]", device.name, device.transport_summary())
                            })
                            .unwrap_or_else(|| language.text("No device").to_string());
                        ui.add_space(8.0);
                        ui.add_enabled_ui(controls_enabled && !self.devices.is_empty(), |ui| {
                            egui::ComboBox::from_id_salt("device_selector_combo")
                                .selected_text(selected_label)
                                .width(185.0)
                                .show_ui(ui, |ui| {
                                    for device in &self.devices {
                                        ui.selectable_value(
                                            &mut next_udid,
                                            Some(device.udid.clone()),
                                            format!(
                                                "{}  [{}]",
                                                device.name,
                                                device.transport_summary()
                                            ),
                                        );
                                    }
                                });
                        });
                        if next_udid != self.selected_udid {
                            self.selected_udid = next_udid;
                            if let Some(selected) = self.selected_udid.clone() {
                                self.add_log(format!("Selected device: {}", selected));
                            }
                        }

                        let connected = self.selected_transport_available();
                        ui.add_space(12.0);
                        let dot = if connected { p.success } else { p.error };
                        if connected {
                            icons::status_dot_pulsing(ui, dot);
                        } else {
                            icons::status_dot(ui, dot);
                        }
                        ui.label(
                            egui::RichText::new(if connected {
                                language.text("Connected")
                            } else {
                                language.text("Offline")
                            })
                            .family(theme::strong())
                            .size(11.0)
                            .color(if connected { p.success } else { p.text_secondary }),
                        )
                        .on_hover_text(&self.apple_status);
                    });
                });
            });
    }

    fn status_panel(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = theme::palette(ctx);

        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let live = self.is_busy || self.scanning_syslog;
                    let err_prefix = self.language.text("Error: ");
                    let is_error = self.status_msg.starts_with("Error")
                        || self.status_msg.starts_with("Failed")
                        || (!err_prefix.is_empty() && self.status_msg.starts_with(err_prefix));
                    let dot_col = if live {
                        p.accent
                    } else if is_error {
                        p.error
                    } else {
                        p.success
                    };
                    if live {
                        icons::status_dot_pulsing(ui, dot_col);
                        ui.spinner();
                    } else {
                        icons::status_dot(ui, dot_col);
                    }
                    ui.label(
                        egui::RichText::new(&self.status_msg)
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let logs_label = if self.show_logs_window {
                            language.text("Close logs")
                        } else {
                            language.text("Activity log")
                        };
                        if widgets::button(ui, logs_label, ButtonKind::Quiet, true, false).clicked()
                        {
                            self.show_logs_window = !self.show_logs_window;
                        }
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{}  {}",
                                language.text("Transport"),
                                language.text(self.connection_mode.label())
                            ))
                            .size(10.5)
                            .color(p.text_secondary),
                        );
                    });
                });
            });
    }

    fn logs_window(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = theme::palette(ctx);

        let mut show_logs = self.show_logs_window;
        let mut file_saved_msg: Option<String> = None;
        if show_logs {
            egui::Window::new(language.text("Logs"))
                .open(&mut show_logs)
                .default_size([560.0, 320.0])
                .min_size([380.0, 200.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if widgets::button(
                            ui,
                            language.text("Copy Logs"),
                            ButtonKind::Secondary,
                            true,
                            false,
                        )
                        .clicked()
                        {
                            ctx.copy_text(self.logs.join("\n"));
                        }
                        if widgets::button(
                            ui,
                            language.text("Save to File..."),
                            ButtonKind::Secondary,
                            true,
                            false,
                        )
                        .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("silo-diagnostics.log")
                                .add_filter("Log files", &["log", "txt"])
                                .save_file()
                            {
                                let content = self.logs.join("\r\n");
                                let _ = std::fs::write(&path, content);
                                file_saved_msg =
                                    Some(format!("Saved log file to {}", path.display()));
                            }
                        }
                        if widgets::button(ui, language.text("Clear"), ButtonKind::Quiet, true, false)
                            .clicked()
                        {
                            self.logs.clear();
                        }
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                self.logs.len(),
                                language.text("entries")
                            ))
                            .size(11.0)
                            .color(p.text_secondary),
                        );
                    });
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(p.elevated)
                        .corner_radius(12)
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .stick_to_bottom(true)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.logs.is_empty() {
                                        ui.label(
                                            egui::RichText::new(
                                                language.text("No events logged yet."),
                                            )
                                            .size(11.0)
                                            .color(p.text_tertiary),
                                        );
                                    } else {
                                        for line in &self.logs {
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .size(10.5)
                                                    .monospace()
                                                    .color(p.text),
                                            );
                                        }
                                    }
                                });
                        });
                });
            self.show_logs_window = show_logs;
            if let Some(msg) = file_saved_msg {
                self.add_log(msg);
            }
        }
    }
}

impl eframe::App for SiloApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages(ctx);
        self.handle_update_messages();
        self.poll_devices();
        self.sync_system_theme(ctx);
        let animation_time = ctx.input(|input| input.time);

        // Translucent backdrop: painted first so the frosted sidebar, header,
        // cards and status bar all blend over the gradient.
        theme::paint_background(ctx, theme::palette(ctx));

        if self.is_busy || self.scanning_syslog || self.current_card_loading || self.update_rx.is_some()
        {
            ctx.request_repaint();
        }

        // On launch, shrink the window if it is larger than this monitor's
        // work area (high-DPI screens): otherwise the header controls and
        // the status bar end up off-screen and unreachable.
        if !self.did_fit_screen {
            self.did_fit_screen = true;
            if let Some(work) = primary_work_area_points(ctx) {
                let current = ctx.viewport_rect().size();
                let max = work - egui::vec2(28.0, 28.0);
                if max.x > 320.0 && max.y > 240.0 && (current.x > max.x || current.y > max.y) {
                    let fitted = egui::vec2(current.x.min(max.x), current.y.min(max.y));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(fitted));
                }
            }
        }

        // Drag-and-drop artwork / theme packages.
        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            self.handle_dropped_files(ctx, dropped_files);
        }

        if self.current_tab != self.displayed_tab {
            self.displayed_tab = self.current_tab;
            self.page_transition_started = Some(animation_time);
        }
        if let Some(started) = self.card_reveal_started {
            if animation_time - started < 0.9 {
                ctx.request_repaint_after(Duration::from_millis(16));
            } else {
                self.card_reveal_started = None;
            }
        }

        self.sidebar(ctx);
        self.header_panel(ctx);
        self.status_panel(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::symmetric(26, 18)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let progress = self
                            .page_transition_started
                            .map(|started| {
                                ((animation_time - started) / 0.28).clamp(0.0, 1.0) as f32
                            })
                            .unwrap_or(1.0);
                        let eased = 1.0 - (1.0 - progress).powi(3);
                        if progress < 1.0 {
                            ctx.request_repaint_after(Duration::from_millis(16));
                        } else {
                            self.page_transition_started = None;
                        }
                        ui.set_opacity(0.35 + 0.65 * eased);
                        ui.add_space((1.0 - eased) * 12.0);
                        match self.current_tab {
                            AppTab::Dashboard => self.show_dashboard(ctx, ui),
                            AppTab::Wallet => self.show_wallet_tab(ctx, ui),
                            AppTab::Passcode => self.show_passcode_tab(ctx, ui),
                            AppTab::Keycard => self.show_keycard_tab(ctx, ui),
                            AppTab::Settings => self.show_settings(ctx, ui),
                            AppTab::Help => self.show_help_tab(ui),
                            AppTab::Updates => self.show_updates(ctx, ui),
                        }
                    });
            });

        self.logs_window(ctx);
    }
}
