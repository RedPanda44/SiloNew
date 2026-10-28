use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::device::ConnectionMode;
use crate::ui::theme::Appearance;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    SimplifiedChinese,
}

impl Language {
    fn detect_from_system() -> Self {
        if let Some(locale) = Self::environment_locale() {
            return Self::from_locale(&locale);
        }
        if let Some(locale) = Self::os_locale() {
            return Self::from_locale(&locale);
        }
        Self::English
    }

    fn environment_locale() -> Option<String> {
        for key in ["LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES"] {
            if let Ok(value) = std::env::var(key) {
                let value = value.trim().to_ascii_lowercase();
                // "C" and "POSIX" carry no language information; on Windows
                // they are sometimes set by tooling, so fall through to the
                // real UI language instead of forcing English.
                if !value.is_empty() && value != "c" && value != "posix" {
                    return Some(value);
                }
            }
        }
        None
    }

    #[cfg(windows)]
    fn os_locale() -> Option<String> {
        unsafe extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        let langid = unsafe { GetUserDefaultUILanguage() };
        // The low 10 bits are the primary language ID; 0x04 is Chinese.
        if langid & 0x03ff == 0x0004 {
            Some("zh".to_string())
        } else {
            None
        }
    }

    #[cfg(not(windows))]
    fn os_locale() -> Option<String> {
        None
    }

    fn from_locale(locale: &str) -> Self {
        let locale = locale.trim().to_ascii_lowercase();
        if locale.starts_with("zh") || locale.starts_with("chinese") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    pub fn text(self, source: &str) -> &str {
        if self == Self::English {
            return source;
        }

        match source {
            "Wallet" => "钱包",
            "Passcode" => "锁屏密码",
            "Help" => "帮助",
            "Dashboard" => "控制面板",
            "Wallet Studio" => "钱包工作台",
            "Passcode Studio" => "锁屏主题工作台",
            "Settings" => "设置",
            "Help & Guides" => "帮助与指南",
            "WORKSPACE" => "工作区",
            "SUPPORT" => "支持",
            "PERSONALIZE YOUR DEVICE" => "个性化你的设备",
            "SILO PERSONALIZATION" => "Silo 个性化",
            "Make your iPhone feel like yours." => "让 iPhone 更有你的风格。",
            "Customize Wallet artwork or build a lock-screen keypad theme from one simple workspace." => {
                "在一个工作区中自定义钱包卡面或锁屏键盘主题。"
            }
            "Scan a card" => "扫描卡片",
            "Stop card scan" => "停止扫描",
            "Wallet studio" => "钱包工作台",
            "Your style. Your wallet." => "你的风格，你的钱包。",
            "CONNECTED DEVICE" => "已连接设备",
            "SAVED CARDS" => "已保存卡片",
            "CURRENT SETUP" => "当前设置",
            "Online" => "在线",
            "Offline" => "离线",
            "Connect an iPhone to get started." => "连接 iPhone 以开始使用。",
            "Tap one to continue editing" => "点击卡片继续编辑",
            "Apple support is available" => "Apple 支持组件已就绪",
            "Apple support runtime is missing" => "缺少 Apple 支持运行时",
            "Quick actions" => "快捷操作",
            "Jump straight into a workflow." => "直接开始一个工作流程。",
            "Choose artwork" => "选择卡面图片",
            "Load keypad theme" => "加载键盘主题",
            "Open settings" => "打开设置",
            "Recent activity" => "最近活动",
            "The latest events from this session." => "此会话中的最新事件。",
            "View activity log" => "查看活动日志",
            "Pick up where you left off" => "继续上次操作",
            "Choose a saved card to open it in Wallet Studio." => {
                "选择已保存的卡片，在钱包工作台中打开。"
            }
            "Connection" => "连接",
            "Choose how Silo talks to your iPhone." => "选择 Silo 与 iPhone 的连接方式。",
            "Preferred transport" => "首选连接方式",
            "Language & app info" => "语言与应用信息",
            "Language preference is saved on this PC." => "语言偏好保存在此电脑上。",
            "Interface language" => "界面语言",
            "Native Windows client" => "原生 Windows 客户端",
            "Device connected" => "设备已连接",
            "Apple services ready" => "Apple 服务已就绪",
            "Apple support not found" => "未找到 Apple 支持组件",
            "Connected" => "已连接",
            "Refresh devices" => "刷新设备",
            "Activity log" => "活动日志",
            "Close logs" => "关闭日志",
            "Card face comparison" => "卡面预览对比",
            "See the face currently on your iPhone, then preview your replacement." => {
                "查看 iPhone 当前卡面，再预览替换后的效果。"
            }
            "CURRENT ON IPHONE" => "iPhone 当前卡面",
            "Fetching live card face..." => "正在读取当前卡面...",
            "LIVE DEVICE ARTWORK" => "设备当前卡面",
            "Scan a Wallet card to load its current face" => "扫描 Wallet 卡片以读取当前卡面",
            "Loading the selected card..." => "正在加载所选卡片...",
            "Refresh current card face" => "刷新当前卡面",
            "YOUR NEW DESIGN" => "你的新设计",
            "Drag to adjust crop" => "拖动以调整裁切",
            "The top preview is read from the connected iPhone. The bottom preview is your prepared replacement." => {
                "上方预览读取自已连接的 iPhone；下方预览是准备替换的新卡面。"
            }
            "Hold your iPhone near Wallet and tap a card" => "请在 iPhone 上打开 Wallet 并点击卡片",
            "Connect an iPhone before loading its current card face." => {
                "请先连接 iPhone，再读取当前卡面。"
            }
            "Scan a card or enter its card hash first." => "请先扫描卡片或输入卡片 Hash。",
            "That does not look like a Wallet card hash." => "这看起来不像钱包卡片 Hash。",
            "The selected iPhone is unavailable on this transport." => {
                "所选 iPhone 当前无法通过此连接方式访问。"
            }
            "Could not decode the current card artwork:" => "无法解码当前卡面图片：",
            "Refresh" => "刷新",
            "Auto (USB preferred)" => "自动（优先 USB）",
            "USB only" => "仅 USB",
            "WiFi only" => "仅 WiFi",
            "No device" => "没有设备",
            "Ready" => "就绪",
            "Unavailable" => "不可用",
            "Logs" => "日志",
            "Logs [x]" => "日志 [x]",
            "Copy Logs" => "复制日志",
            "Save to File..." => "保存到文件...",
            "Clear" => "清空",
            "No events logged yet." => "暂无日志记录。",
            "Language" => "语言",
            "English" => "英语",
            "Simplified Chinese" => "简体中文",
            "Language changed." => "语言已切换。",
            "Appearance" => "外观",
            "Choose how Silo looks on this PC." => "选择 Silo 在此电脑上的显示方式。",
            "Light" => "浅色",
            "Dark" => "深色",
            "Theme preference is saved on this PC." => "主题偏好保存在此电脑上。",
            "Transport mode" => "连接方式",
            "System status" => "系统状态",
            "Runtime" => "运行状态",
            "Transport" => "传输",
            "Device(s)" => "设备数",
            "Selected" => "已选中",
            "Waiting" => "等待中",
            "Operational" => "正常运行",
            "Needs Apple support" => "需要 Apple 支持",
            "entries" => "条记录",
            "Ready. Connect iPhone via USB or paired WiFi and unlock it." => {
                "就绪。请通过 USB 或已配对的 WiFi 连接并解锁 iPhone。"
            }
            "No iPhone connected via USB or paired WiFi." => {
                "未检测到通过 USB 或已配对 WiFi 连接的 iPhone。"
            }
            "Please select a connected iPhone." => "请选择已连接的 iPhone。",
            "Disconnect the USB cable and refresh to guarantee the full AirTraffic path uses WiFi." => {
                "请断开 USB 线并刷新，以确保完整的 AirTraffic 路径使用 WiFi。"
            }
            "Please enter or scan a target card hash." => "请输入或扫描目标卡片 Hash。",
            "Please choose a card skin image first." => "请先选择卡片皮肤图片。",
            "Crop position updated." => "裁切位置已更新。",
            "Scanning syslog... Open Wallet or tap your card on iPhone." => {
                "正在扫描 syslog... 请在 iPhone 上打开钱包并点击卡片。"
            }
            "Syslog scanning stopped." => "syslog 扫描已停止。",
            "Writing card skin to iPhone..." => "正在将卡片皮肤写入 iPhone...",
            "Please select a .passthm theme file first." => "请先选择一个 .passthm 主题文件。",
            "Writing passcode theme buttons..." => "正在将锁屏密码主题按钮写入 iPhone...",
            "Syslog scan finished" => "syslog 扫描完成",
            "Card skin successfully flashed! Force quit Wallet on iPhone and reopen it." => {
                "卡片皮肤应用成功！请在 iPhone 上强制关闭并重新打开 Wallet。"
            }
            "Passcode theme applied! Lock your iPhone to view the new keypad." => {
                "锁屏密码主题应用成功！锁定 iPhone 查看新的键盘样式。"
            }
            "Card skin updated successfully!" => "卡片皮肤更新成功！",
            "Restoring original card face..." => "正在恢复原卡面...",
            "Restoring original card artwork..." => "正在恢复原卡面图片...",
            "Clearing .cache cache..." => "正在清理 .cache 缓存...",
            "Clearing .pkcache cache..." => "正在清理 .pkcache 缓存...",
            "Original card face restored successfully!" => "原卡面恢复成功！",
            "Original card face restored. Force close Wallet and reopen it." => {
                "原卡面已恢复。请强制关闭并重新打开 Wallet。"
            }
            "Original card backup not found." => "未找到原卡面备份。",
            "Apply a card skin once to create an original backup." => {
                "先应用一次卡片皮肤以创建原卡面备份。"
            }
            "Passcode theme applied successfully!" => "锁屏密码主题应用成功！",
            "Failed to parse theme:" => "解析主题失败：",
            "Card captured" => "已捕获卡片",
            "Loaded" => "已加载",
            "target:" => "目标：",
            "lang:" => "语言：",
            "bold:" => "粗体：",
            "ON" => "开",
            "OFF" => "关",
            "Error: " => "错误：",
            "Found" => "已找到",
            "connected device(s); transport mode:" => "台已连接设备；连接方式：",
            "Could not enumerate devices:" => "无法枚举设备：",
            "Selected iPhone has no" => "选中的 iPhone 没有",
            "connection. Refresh devices or change transport mode." => {
                "连接。请刷新设备或更改连接方式。"
            }
            "Scanning syslog..." => "正在扫描 syslog...",
            "Open Wallet on iPhone and tap your card" => "请在 iPhone 上打开钱包并点击目标卡片",
            "Stop" => "停止",
            "Scan" => "扫描",
            "Card Configuration" => "卡片配置",
            "Target your card and choose replacement artwork" => "选择目标卡片和替换图片",
            "Target Card Hash" => "目标卡片 Hash",
            "Base64 pass hash..." => "Base64 卡片 Hash...",
            "Saved cards" => "已保存的卡片",
            "Select..." => "请选择...",
            "Card Skin Artwork" => "卡片皮肤图片",
            "PNG, JPG, WebP - auto-scaled to 1536x969" => "PNG、JPG、WebP，将自动缩放到 1536x969",
            "Drag inside the preview to reposition the crop." => "在预览区域内拖动以调整裁切位置。",
            "Choose Image..." => "选择图片...",
            "Export PNG" => "导出 PNG",
            "Write to iPhone" => "写入 iPhone",
            "Apply Card Skin" => "应用卡片皮肤",
            "Restore Original" => "恢复原卡面",
            "connect iPhone" => "连接 iPhone",
            "choose available transport" => "选择可用的连接方式",
            "enter card hash" => "输入卡片 Hash",
            "choose image" => "选择图片",
            "select theme" => "选择主题",
            "Need: " => "需要：",
            "Wallet Preview" => "钱包预览",
            "1536 x 969 px pass canvas" => "1536 x 969 像素卡片画布",
            "No artwork loaded" => "尚未加载图片",
            "No image" => "没有图片",
            "After applying, force close Apple Wallet and reopen it." => {
                "应用后，请强制关闭 Apple Wallet 并重新打开。"
            }
            "Passcode Theme" => "锁屏密码主题",
            "Custom lockscreen keypad from Cowabunga or Nugget" => {
                "来自 Cowabunga 或 Nugget 的自定义锁屏键盘"
            }
            "Theme Package" => "主题包",
            "Choose a .passthm archive containing dialer artwork" => {
                "选择包含拨号键盘图片的 .passthm 压缩包"
            }
            "Choose .passthm..." => "选择 .passthm...",
            "assets" => "个资源",
            "Target iOS Cache" => "目标 iOS 缓存",
            "Select cache format based on connected iOS version" => {
                "根据连接设备的 iOS 版本选择缓存格式"
            }
            "Auto (TelephonyUI-10)" => "自动（TelephonyUI-10）",
            "TelephonyUI-10 (iOS 18+)" => "TelephonyUI-10（iOS 18+）",
            "TelephonyUI-9 (iOS 16-17)" => "TelephonyUI-9（iOS 16-17）",
            "TelephonyUI-8 (Legacy)" => "TelephonyUI-8（旧版本）",
            "Keypad Language" => "键盘语言",
            "Subtext alphabet layout (English, Russian, Ukrainian, Japanese, or Universal)" => {
                "按键副文字母布局（英语、俄语、乌克兰语、日语或通用）"
            }
            "Russian" => "俄语",
            "Ukrainian" => "乌克兰语",
            "Japanese" => "日语",
            "All Languages (Universal)" => "全部语言（通用）",
            "Bold Text (iOS Accessibility)" => "粗体文字（iOS 辅助功能）",
            "Generates *-bold.png for devices with Bold Text turned ON in iPhone Settings -> Display" => {
                "为 iPhone 设置 -> 显示与亮度中启用粗体文字的设备生成 *-bold.png"
            }
            "Apply Passcode Theme" => "应用锁屏密码主题",
            "Keypad Preview" => "键盘预览",
            "Dialer button artwork" => "拨号按钮图片",
            "No theme loaded" => "尚未加载主题",
            "3x4 Keypad" => "3x4 键盘",
            "No theme" => "没有主题",
            "After applying, lock your iPhone to see the new keypad." => {
                "应用后，请锁定 iPhone 查看新的键盘样式。"
            }
            "Setup & Card Hash Guide" => "设置与卡片 Hash 指南",
            "Everything you need to connect and capture your card" => {
                "连接设备并捕获卡片所需的全部信息"
            }
            "Prerequisites" => "使用前准备",
            "- 64-bit iTunes or Apple Mobile Device Support installed" => {
                "- 已安装 64 位 iTunes 或 Apple Mobile Device Support"
            }
            "- First-time setup: connect by USB and tap \"Trust this Computer\"" => {
                "- 首次使用：通过 USB 连接并点击“信任此电脑”"
            }
            "- WiFi: enable WiFi sync, then use the same local network" => {
                "- WiFi：启用 WiFi 同步，并确保双方处于同一局域网"
            }
            "- Select Auto, USB only, or WiFi only in the top bar" => {
                "- 在顶部栏选择自动、仅 USB 或仅 WiFi"
            }
            "Finding Your Card Hash" => "查找卡片 Hash",
            "1. Click \"Scan\" in the Wallet tab" => "1. 在钱包页点击“扫描”",
            "2. Open Apple Wallet on your iPhone" => "2. 在 iPhone 上打开 Apple Wallet",
            "3. Tap the card you want to customize" => "3. 点击要自定义的卡片",
            "4. Silo captures the pass hash automatically" => "4. Silo 会自动捕获卡片 Hash",
            "5. Click \"Stop\" once detected" => "5. 检测到后点击“停止”",
            "Activation & Theme Guide" => "应用与主题指南",
            "Applying skins and dialer keypad packages" => "应用卡片皮肤和拨号键盘主题",
            "Activating Apple Wallet Skin" => "应用 Apple Wallet 皮肤",
            "1. Click \"Apply Card Skin\" and wait for completion" => {
                "1. 点击“应用卡片皮肤”并等待完成"
            }
            "2. Open App Switcher on iPhone (swipe up from bottom)" => {
                "2. 在 iPhone 上打开 App 切换器（从底部向上滑动）"
            }
            "3. Force close Apple Wallet by swiping up on it" => {
                "3. 向上滑动并强制关闭 Apple Wallet"
            }
            "4. Reopen Wallet - your new skin appears!" => "4. 重新打开钱包，即可看到新的皮肤！",
            "Passcode Themes (.passthm)" => "锁屏密码主题（.passthm）",
            "- Compatible with Cowabunga & Nugget theme packages" => {
                "- 兼容 Cowabunga 和 Nugget 主题包"
            }
            "- iOS 18+: Select \"Auto (TelephonyUI-10)\"" => {
                "- iOS 18+：选择“自动（TelephonyUI-10）”"
            }
            "- iOS 16-17: Select \"TelephonyUI-9\"" => "- iOS 16-17：选择“TelephonyUI-9”",
            "- Lock screen to verify your updated keypad artwork" => {
                "- 锁定屏幕以查看更新后的键盘图片"
            }
            "Updates" => "更新",
            "Silo Updater" => "Silo 更新",
            "Current version" => "当前版本",
            "Check for a newer version of Silo." => "检查是否有更新版本的 Silo。",
            "Check for updates" => "检查更新",
            "Checking for updates..." => "正在检查更新...",
            "You are running the latest version." => "你正在使用最新版本。",
            "Check again" => "再次检查",
            "A new version is available." => "有新版本可用。",
            "This release has no Windows download." => "此版本没有提供 Windows 下载。",
            "Download & install" => "下载并安装",
            "View release notes" => "查看更新说明",
            "Release notes" => "更新说明",
            "Downloading update..." => "正在下载更新...",
            "The update is ready to install." => "更新已准备好安装。",
            "Silo will close, apply the update, and reopen automatically." => {
                "Silo 将自动关闭、应用更新并重新打开。"
            }
            "Restart & install" => "重启并安装",
            "Update check failed." => "检查更新失败。",
            "Try again" => "重试",
            "System" => "跟随系统",
            "System follows the Windows theme." => "“跟随系统”将使用 Windows 的主题设置。",
            "Devices are detected automatically." => "设备会自动检测。",
            "Keycard" => "门禁卡",
            "Keycard Studio" => "门禁卡工作台",
            "HOUSE KEYCARD" => "家庭门禁卡",
            "Create an NFC door-key pass and add it to Wallet." => {
                "创建 NFC 门禁卡并将其添加到钱包。"
            }
            "Pass details" => "卡片详情",
            "House name" => "家庭名称",
            "Owner name" => "持有人姓名",
            "Access label" => "权限标签",
            "NFC payload" => "NFC 数据",
            "NFC payload help" => "读取器需要的十六进制字符串；留空则仅使用二维码。",
            "Background" => "背景色",
            "Foreground" => "前景色",
            "Label color" => "标签颜色",
            "Card images" => "卡片图片",
            "Icon image" => "图标图片",
            "Logo image" => "标识图片",
            "Choose image" => "选择图片",
            "Default" => "默认",
            "Signing certificate" => "签名证书",
            "Apple only accepts signed passes. Add a Pass Type ID certificate to enable Wallet installation." => {
                "Apple 仅接受已签名的卡片。添加 Pass Type ID 证书后即可安装到钱包。"
            }
            "Certificate (.p12)" => "证书 (.p12)",
            "Choose .p12..." => "选择 .p12...",
            "Choose certificate..." => "选择证书...",
            "Certificate password" => "证书密码",
            "Pass Type ID" => "Pass Type ID",
            "Team ID" => "Team ID",
            "WWDR certificate" => "WWDR 证书",
            "Bundle" => "打包",
            "Generate keycard" => "生成门禁卡",
            "Save .pkpass" => "保存 .pkpass",
            "Send to iPhone" => "发送到 iPhone",
            "Share on LAN" => "局域网共享",
            "Stop sharing" => "停止共享",
            "Add to Wallet" => "添加到钱包",
            "Open the link on your iPhone to add the keycard to Wallet." => {
                "在 iPhone 上打开此链接即可将门禁卡添加到钱包。"
            }
            "Choose a folder" => "选择文件夹",
            _ => source,
        }
    }

    pub fn option_label(self, option: Self) -> &'static str {
        match (self, option) {
            (Self::English, Self::English) => "English",
            (Self::English, Self::SimplifiedChinese) => "Simplified Chinese",
            (Self::SimplifiedChinese, Self::English) => "英语",
            (Self::SimplifiedChinese, Self::SimplifiedChinese) => "简体中文",
        }
    }
}

/// Persisted preferences: interface language, appearance, connection and
/// keycard settings, plus the passcode-studio target options.
#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub connection_mode: ConnectionMode,
    #[serde(default = "default_telephony_version")]
    pub forced_telephony_ver: String,
    #[serde(default = "default_keypad_language")]
    pub keypad_language: String,
    #[serde(default)]
    pub passcode_bold: bool,
    #[serde(default)]
    pub keycard: crate::keycard::KeycardPrefs,
}

fn default_telephony_version() -> String {
    crate::passthm::DEFAULT_TELEPHONY_VERSION.to_string()
}

fn default_keypad_language() -> String {
    crate::passthm::DEFAULT_KEYPAD_LANGUAGE.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::detect_from_system(),
            appearance: Appearance::default(),
            connection_mode: ConnectionMode::default(),
            forced_telephony_ver: default_telephony_version(),
            keypad_language: default_keypad_language(),
            passcode_bold: false,
            keycard: crate::keycard::KeycardPrefs::default(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        if let Ok(contents) = fs::read_to_string(&path)
            && let Ok(settings) = serde_json::from_str::<Settings>(&contents)
        {
            // Earlier builds stored the certificate password in clear text.
            // Re-save once so it is protected with DPAPI without waiting for
            // the user to change another preference.
            if !settings.keycard.signing.p12_password.is_empty() && !contents.contains("dpapi:") {
                settings.save();
            }
            return settings;
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = settings_path();
        if let Ok(contents) = serde_json::to_string_pretty(self) {
            let _ = crate::paths::write_atomic(&path, contents.as_bytes());
        }
    }
}

fn settings_path() -> PathBuf {
    crate::paths::app_data_dir().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_translation_has_english_fallback() {
        assert_eq!(Language::SimplifiedChinese.text("Wallet"), "钱包");
        assert_eq!(
            Language::SimplifiedChinese.text("unknown string"),
            "unknown string"
        );
    }

    #[test]
    fn language_option_labels_follow_current_language() {
        assert_eq!(
            Language::English.option_label(Language::SimplifiedChinese),
            "Simplified Chinese"
        );
        assert_eq!(
            Language::SimplifiedChinese.option_label(Language::SimplifiedChinese),
            "简体中文"
        );
    }

    #[test]
    fn legacy_settings_json_without_appearance_loads() {
        // Settings files written by 1.4.x only contain the language.
        let legacy = r#"{"language":"SimplifiedChinese"}"#;
        let settings: Settings = serde_json::from_str(legacy).unwrap();
        assert_eq!(settings.language, Language::SimplifiedChinese);
        assert_eq!(settings.appearance, Appearance::default());
        assert_eq!(
            settings.connection_mode,
            crate::device::ConnectionMode::Auto
        );
        assert_eq!(
            settings.forced_telephony_ver,
            crate::passthm::DEFAULT_TELEPHONY_VERSION
        );
        assert_eq!(
            settings.keypad_language,
            crate::passthm::DEFAULT_KEYPAD_LANGUAGE
        );
        assert!(!settings.passcode_bold);
    }

    #[test]
    fn settings_round_trip_preserves_appearance() {
        let settings = Settings {
            language: Language::English,
            appearance: Appearance::Light,
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.language, Language::English);
        assert_eq!(parsed.appearance, Appearance::Light);
    }

    #[test]
    fn settings_round_trip_preserves_studio_preferences() {
        let settings = Settings {
            language: Language::English,
            connection_mode: crate::device::ConnectionMode::Wifi,
            forced_telephony_ver: "TelephonyUI-9".to_string(),
            keypad_language: "Russian".to_string(),
            passcode_bold: true,
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.connection_mode, crate::device::ConnectionMode::Wifi);
        assert_eq!(parsed.forced_telephony_ver, "TelephonyUI-9");
        assert_eq!(parsed.keypad_language, "Russian");
        assert!(parsed.passcode_bold);
    }

    #[test]
    fn locale_detection_recognises_chinese() {
        assert_eq!(
            Language::from_locale("zh_CN.UTF-8"),
            Language::SimplifiedChinese
        );
        assert_eq!(
            Language::from_locale("zh-Hans"),
            Language::SimplifiedChinese
        );
        assert_eq!(Language::from_locale("en_US"), Language::English);
        assert_eq!(Language::from_locale("de-DE"), Language::English);
    }
}
