#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod fluent_menu;

#[cfg(not(windows))]
fn main() {
    println!("Task Cleaner GUI is designed for Windows 11. Run on Windows to launch tray application.");
}

#[cfg(windows)]
mod win_gui {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::time::Instant;

    use serde::{Deserialize, Serialize};
    use task_cleaner_core::{
        detect_system_language, get_caller_lineage, is_explorer, purge_process_working_set,
        scan_foreground_apps, sort_protected, sort_targets, tiered_terminate, tr, AppTarget,
        I18nKey, Language, LanguagePreference, SortMode, TerminationMode, WhitelistManager,
        WhitelistMatch,
    };
    use windows_sys::Win32::Foundation::{
        CloseHandle, COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
    };
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, BitBlt, ClientToScreen, CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC,
        CreateDIBSection, CreateFontW, CreatePen, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW,
        EndPaint, FillRect, GetDC, GetMonitorInfoW, InvalidateRect, MonitorFromPoint,
        ReleaseDC, RoundRect, SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, CLEARTYPE_QUALITY, DIB_RGB_COLORS, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT,
        DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD, FW_MEDIUM, FW_NORMAL, FW_SEMIBOLD, HBITMAP,
        HBRUSH, HDC, HFONT, MONITORINFO, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, PS_NULL, PS_SOLID,
        SRCCOPY, TRANSPARENT,
    };
    use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, KEY_WRITE, REG_SZ,
    };
    use windows_sys::Win32::System::Threading::{
        CreateMutexW, OpenProcess, TerminateProcess, PROCESS_TERMINATE,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        EnableWindow, GetKeyState, RegisterHotKey, TrackMouseEvent, UnregisterHotKey, MOD_ALT,
        MOD_CONTROL, MOD_SHIFT, MOD_WIN, TME_LEAVE, TRACKMOUSEEVENT,
    };
    use windows_sys::Win32::UI::HiDpi::{
        GetDpiForSystem, GetDpiForWindow, GetSystemMetricsForDpi, SetProcessDpiAwarenessContext,
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use windows_sys::Win32::UI::Shell::{
        ExtractIconExW, ShellExecuteExW, ShellExecuteW, Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON,
        NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyWindow, DispatchMessageW, DrawIconEx, FindWindowW, GetClientRect,
        GetCursorPos, GetMessageW, GetSystemMetrics, IsWindow, KillTimer, LoadCursorW, LoadIconW, MessageBoxW,
        PostQuitMessage, PrivateExtractIconsW, RegisterClassExW, SetForegroundWindow, SetTimer,
        SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage,
        CS_DROPSHADOW, DI_NORMAL, HICON, HMENU, HWND_TOPMOST, ICONINFO, IDC_ARROW,
        MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_TOPMOST,
        MSG, SM_CXSMICON, SM_CYSMICON, SPI_GETWORKAREA, SWP_NOACTIVATE,
        SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, SW_SHOWNORMAL,
        WM_ACTIVATE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_HOTKEY, WM_KEYDOWN, WM_KEYUP,
        WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_SETTINGCHANGE,
        WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WM_USER, WNDCLASSEXW, WS_CAPTION, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU, WS_VISIBLE,
    };

    use crate::fluent_menu::*;

    const WM_TRAYICON: u32 = WM_USER + 101;
    const WM_MOUSELEAVE: u32 = 0x02A3;
    const HOTKEY_TOGGLE_ID: i32 = 0x1001;
    const TIMER_HEARTBEAT_ID: usize = 1002;

    // 基础菜单 ID 定义
    const IDM_OPEN: usize = 1001;
    const IDM_REFRESH: usize = 1002;
    const IDM_CLEAN_ALL: usize = 1003;
    const IDM_QUIT: usize = 1005;

    // 排序菜单 ID
    const IDM_SORT_COMPOSITE: usize = 1101;
    const IDM_SORT_MEMORY: usize = 1102;
    const IDM_SORT_CPU: usize = 1103;
    const IDM_SORT_WINDOWS: usize = 1104;
    const IDM_SORT_DEFAULT: usize = 1105;

    // 核心卡片操作下拉菜单 ID
    const IDM_ACTION_GRACEFUL: usize = 1201;
    const IDM_ACTION_FORCE: usize = 1202;
    const IDM_ACTION_PURGE: usize = 1203;

    // 列表行操作菜单 ID
    const IDM_ROW_WHITELIST_ADD: usize = 1301;
    const IDM_ROW_WHITELIST_REMOVE: usize = 1302;
    const IDM_ROW_REVEAL: usize = 1303;
    const IDM_ROW_COPY_NAME: usize = 1304;
    const IDM_ROW_COPY_PID: usize = 1305;
    const IDM_ROW_FORCE_KILL: usize = 1306;
    const IDM_ROW_PURGE_MEMORY: usize = 1307;
    const IDM_ROW_PROPERTIES: usize = 1308;

    // 底栏设置菜单 ID
    const IDM_CFG_STARTUP: usize = 1401;
    const IDM_CFG_RELOAD: usize = 1402;
    const IDM_CFG_ABOUT: usize = 1403;
    const IDM_CFG_OPEN_FILE: usize = 1404;
    const IDM_CFG_OPEN_DIR: usize = 1405;
    const IDM_CFG_GITHUB: usize = 1406;

    // 显示偏好开关 ID
    const IDM_CFG_TOGGLE_DETAILED_METRICS: usize = 1410;
    const IDM_CFG_TOGGLE_APP_ID: usize = 1411;
    const IDM_CFG_TOGGLE_SORT_BTN: usize = 1412;

    // CLI 工具管理 ID
    const IDM_CLI_INSTALL_USER: usize = 1420;
    const IDM_CLI_TEST_TERMINAL: usize = 1421;
    const IDM_CLI_REVEAL: usize = 1422;
    const IDM_CLI_UNINSTALL: usize = 1423;

    // 全局快捷键管理 ID
    const IDM_HOTKEY_TOGGLE_ENABLE: usize = 1430;
    const IDM_HOTKEY_CUSTOM_RECORDER: usize = 1431;
    const IDM_HOTKEY_PRESET_BASE: usize = 1440;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShortcutPreset {
        pub name: &'static str,
        pub modifiers: u32,
        pub vk: u32,
        pub display: &'static str,
    }

    const PRESETS: &[ShortcutPreset] = &[
        ShortcutPreset {
            name: "CtrlAltK",
            modifiers: 0x0002 | 0x0001, // MOD_CONTROL | MOD_ALT
            vk: 0x4B,                   // 'K'
            display: "Ctrl + Alt + K (默认)",
        },
        ShortcutPreset {
            name: "CtrlShiftK",
            modifiers: 0x0002 | 0x0004, // MOD_CONTROL | MOD_SHIFT
            vk: 0x4B,                   // 'K'
            display: "Ctrl + Shift + K",
        },
        ShortcutPreset {
            name: "AltShiftK",
            modifiers: 0x0001 | 0x0004, // MOD_ALT | MOD_SHIFT
            vk: 0x4B,                   // 'K'
            display: "Alt + Shift + K",
        },
        ShortcutPreset {
            name: "CtrlAltX",
            modifiers: 0x0002 | 0x0001, // MOD_CONTROL | MOD_ALT
            vk: 0x58,                   // 'X'
            display: "Ctrl + Alt + X",
        },
        ShortcutPreset {
            name: "WinAltK",
            modifiers: 0x0008 | 0x0001, // MOD_WIN | MOD_ALT
            vk: 0x4B,                   // 'K'
            display: "Win + Alt + K",
        },
        ShortcutPreset {
            name: "WinShiftK",
            modifiers: 0x0008 | 0x0004, // MOD_WIN | MOD_SHIFT
            vk: 0x4B,                   // 'K'
            display: "Win + Shift + K",
        },
    ];

    // 语言切换菜单基址 (支持 24 种语言 + 自动)
    const IDM_LANG_AUTO: usize = 1500;
    const IDM_LANG_BASE: usize = 1501;

    // 动态目标操作基址 (用于 TranslucentTB 级联分级菜单)
    const IDM_KILL_TARGET_BASE: usize = 2000;
    const IDM_PROTECT_TARGET_BASE: usize = 2100;
    const IDM_REVEAL_TARGET_BASE: usize = 2200;
    const IDM_PURGE_TARGET_BASE: usize = 2300;
    const IDM_UNPROTECT_BASE: usize = 2400;
    const IDM_REVEAL_PROTECTED_BASE: usize = 2500;

    // 窗口尺寸: 严格对齐 macOS 版精修比例 (宽 320, 高 480)
    const WINDOW_WIDTH: i32 = 320;
    const WINDOW_HEIGHT: i32 = 480;
    const VISIBLE_ROWS: usize = 6;
    const ROW_HEIGHT: i32 = 43;

    // Windows 11 Fluent 2.0 / macOS 拟真色彩规范矩阵 (COLORREF: 0x00BBGGRR)
    #[inline]
    const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
        (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
    }

    const COLOR_CANVAS_BG: COLORREF = rgb(242, 244, 247);
    const COLOR_CARD_BG: COLORREF = rgb(255, 255, 255);
    const COLOR_CARD_BORDER: COLORREF = rgb(228, 231, 236);
    const COLOR_CARD_HOVER: COLORREF = rgb(248, 249, 250);
    const COLOR_ROW_SEP: COLORREF = rgb(243, 244, 246);

    const COLOR_TEXT_PRIMARY: COLORREF = rgb(26, 26, 26);
    const COLOR_TEXT_SECONDARY: COLORREF = rgb(107, 114, 128);
    const COLOR_TEXT_MUTED: COLORREF = rgb(156, 163, 175);
    const COLOR_ACCENT_BLUE: COLORREF = rgb(0, 120, 215);

    const COLOR_BADGE_BG: COLORREF = rgb(229, 231, 235);
    const COLOR_BADGE_TEXT: COLORREF = rgb(75, 85, 99);

    const COLOR_STATUS_PENDING_BG: COLORREF = rgb(243, 244, 246);
    const COLOR_STATUS_PENDING_TEXT: COLORREF = rgb(107, 114, 128);
    const COLOR_STATUS_READY_BG: COLORREF = rgb(235, 245, 255);
    const COLOR_STATUS_READY_TEXT: COLORREF = rgb(0, 102, 204);

    const COLOR_HERO_BTN: COLORREF = rgb(229, 231, 235);
    const COLOR_HERO_BTN_HOVER: COLORREF = rgb(217, 220, 226);
    const COLOR_BTN_HOVER: COLORREF = rgb(228, 231, 236);

    const COLOR_TAB_BG: COLORREF = rgb(223, 226, 232);
    const COLOR_TAB_ACTIVE: COLORREF = rgb(0, 120, 215);
    const COLOR_TAB_INACTIVE_TEXT: COLORREF = rgb(75, 85, 99);
    const COLOR_TAB_BADGE_INACTIVE: COLORREF = rgb(210, 214, 220);

    static IS_VISIBLE: AtomicBool = AtomicBool::new(false);
    static IS_MENU_ACTIVE: AtomicBool = AtomicBool::new(false);

    /// 全局快捷键持久化配置
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct HotkeyConfig {
        #[serde(default = "default_true")]
        pub enabled: bool,
        #[serde(default = "default_hotkey_modifiers")]
        pub modifiers: u32,
        #[serde(default = "default_hotkey_vk")]
        pub vk: u32,
        #[serde(default = "default_hotkey_display")]
        pub display: String,
    }

    fn default_hotkey_modifiers() -> u32 {
        0x0002 /* MOD_CONTROL */ | 0x0001 /* MOD_ALT */
    }
    fn default_hotkey_vk() -> u32 {
        0x4B // 'K'
    }
    fn default_hotkey_display() -> String {
        "Ctrl + Alt + K".to_string()
    }

    impl Default for HotkeyConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                modifiers: default_hotkey_modifiers(),
                vk: default_hotkey_vk(),
                display: default_hotkey_display(),
            }
        }
    }

    /// GUI 持久化偏好设置
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GuiPreferences {
        #[serde(default)]
        pub language_pref: LanguagePreference,
        #[serde(default = "default_sort_mode")]
        pub sort_mode: SortMode,
        #[serde(default = "default_true")]
        pub show_detailed_metrics: bool,
        #[serde(default = "default_false")]
        pub show_app_identifier: bool,
        #[serde(default = "default_true")]
        pub show_sort_button: bool,
        #[serde(default)]
        pub hotkey: HotkeyConfig,
    }

    fn default_sort_mode() -> SortMode {
        SortMode::Composite
    }
    fn default_true() -> bool {
        true
    }
    fn default_false() -> bool {
        false
    }

    impl Default for GuiPreferences {
        fn default() -> Self {
            Self {
                language_pref: LanguagePreference::Auto,
                sort_mode: SortMode::Composite,
                show_detailed_metrics: true,
                show_app_identifier: false,
                show_sort_button: true,
                hotkey: HotkeyConfig::default(),
            }
        }
    }

    impl GuiPreferences {
        pub fn load() -> Self {
            let path = WhitelistManager::get_config_dir().join("gui_settings.json");
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(prefs) = serde_json::from_str::<GuiPreferences>(&content) {
                    return prefs;
                }
            }
            Self::default()
        }

        pub fn save(&self) {
            let dir = WhitelistManager::get_config_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("gui_settings.json");
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// CLI 工具 (mtc.exe) 系统集成管理器
    struct CliManager;

    impl CliManager {
        fn candidate_user_dirs() -> Vec<PathBuf> {
            let mut dirs = Vec::new();
            // 1. Cargo bin 目录 (开发机首选，通常已在系统 PATH 中)
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let cargo_bin = PathBuf::from(&user_profile).join(".cargo").join("bin");
                if cargo_bin.is_dir() {
                    dirs.push(cargo_bin);
                }
            }
            // 2. 独立 TaskCleaner 专属 bin 目录
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                dirs.push(PathBuf::from(&local_app_data).join("TaskCleaner").join("bin"));
            }
            // 3. 用户主目录 .local\bin
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                dirs.push(PathBuf::from(&user_profile).join(".local").join("bin"));
            }
            // 4. WindowsApps (Win10/11 用户原生在 PATH 的目录)
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                dirs.push(PathBuf::from(&local_app_data).join("Microsoft").join("WindowsApps"));
            }
            dirs
        }

        fn get_bundled_cli_path() -> Option<PathBuf> {
            // 1. 同级目录查找 (发布打包与常规运行环境)
            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    let candidate = parent.join("mtc.exe");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
            // 2. 虚拟机开发高 IO 缓存目录 (C:\Temp\taskcleaner-target)
            let temp_candidates = [
                PathBuf::from(r"C:\Temp\taskcleaner-target\debug\mtc.exe"),
                PathBuf::from(r"C:\Temp\taskcleaner-target\release\mtc.exe"),
            ];
            for c in &temp_candidates {
                if c.is_file() {
                    return Some(c.clone());
                }
            }
            // 3. 当前工作区 target 编译目录
            if let Ok(cwd) = std::env::current_dir() {
                let candidates = [
                    cwd.join("mtc.exe"),
                    cwd.join("target").join("debug").join("mtc.exe"),
                    cwd.join("target").join("release").join("mtc.exe"),
                ];
                for c in &candidates {
                    if c.is_file() {
                        return Some(c.clone());
                    }
                }
            }
            None
        }

        fn find_installed_cli() -> Option<PathBuf> {
            for dir in Self::candidate_user_dirs() {
                let path = dir.join("mtc.exe");
                if path.is_file() {
                    return Some(path);
                }
            }
            // 安全扫描局部 PATH (仅限本地盘符 C:, D: 等，绝对不发起外部进程，杜绝网络驱动器死锁)
            if let Some(path_var) = std::env::var_os("PATH") {
                for dir in std::env::split_paths(&path_var) {
                    if let Some(s) = dir.to_str() {
                        if s.starts_with(r"\\") {
                            continue;
                        }
                    }
                    let candidate = dir.join("mtc.exe");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
            None
        }

        #[allow(dead_code)]
        fn is_installed() -> bool {
            Self::find_installed_cli().is_some()
        }

        fn ensure_dir_in_user_path(dir: &std::path::Path) {
            let dir_str = dir.to_string_lossy().to_string();
            if dir_str.to_lowercase().contains("windowsapps") {
                return;
            }
            #[cfg(windows)]
            unsafe {
                let env_subkey = to_wstring("Environment");
                let val_name = to_wstring("Path");
                let mut hkey = std::ptr::null_mut();
                if RegOpenKeyExW(HKEY_CURRENT_USER, env_subkey.as_ptr(), 0, KEY_QUERY_VALUE | KEY_SET_VALUE, &mut hkey) == 0 {
                    let mut val_type = 0u32;
                    let mut size = 0u32;
                    if RegQueryValueExW(hkey, val_name.as_ptr(), std::ptr::null(), &mut val_type, std::ptr::null_mut(), &mut size) == 0 && size > 0 {
                        let mut buf: Vec<u16> = vec![0; (size as usize / 2) + 2];
                        if RegQueryValueExW(hkey, val_name.as_ptr(), std::ptr::null(), &mut val_type, buf.as_mut_ptr() as *mut u8, &mut size) == 0 {
                            let curr_path = String::from_utf16_lossy(&buf);
                            let normalized = curr_path.trim_matches('\0');
                            let contains = normalized.split(';').any(|p| p.trim().eq_ignore_ascii_case(&dir_str));
                            if !contains {
                                let new_path = if normalized.is_empty() {
                                    dir_str.clone()
                                } else {
                                    format!("{};{}", normalized.trim_end_matches(';'), dir_str)
                                };
                                let w_new = to_wstring(&new_path);
                                let bytes = (w_new.len() * 2) as u32;
                                RegSetValueExW(hkey, val_name.as_ptr(), 0, REG_SZ, w_new.as_ptr() as *const u8, bytes);
                            }
                        }
                    }
                    RegCloseKey(hkey);
                }
            }
        }

        fn install() -> Result<PathBuf, String> {
            let bundled = Self::get_bundled_cli_path()
                .ok_or_else(|| "未找到配套的 mtc.exe，请先执行 cargo build 生成".to_string())?;
            let target_dirs = Self::candidate_user_dirs();
            let mut last_err = String::new();
            for target_dir in target_dirs {
                if let Err(e) = std::fs::create_dir_all(&target_dir) {
                    last_err = format!("创建目录失败: {}", e);
                    continue;
                }
                let dest = target_dir.join("mtc.exe");
                match std::fs::copy(&bundled, &dest) {
                    Ok(_) => {
                        Self::ensure_dir_in_user_path(&target_dir);
                        return Ok(dest);
                    }
                    Err(e) => last_err = format!("复制失败: {}", e),
                }
            }
            Err(if last_err.is_empty() { "无法写入用户应用目录".to_string() } else { last_err })
        }

        fn uninstall() -> Result<(), String> {
            let mut removed = false;
            for dir in Self::candidate_user_dirs() {
                let p = dir.join("mtc.exe");
                if p.is_file() && std::fs::remove_file(&p).is_ok() {
                    removed = true;
                }
            }
            if removed {
                Ok(())
            } else {
                Err("未找到已安装的 mtc.exe 副本".to_string())
            }
        }

        fn test_in_terminal() {
            let cli_cmd = if let Some(p) = Self::find_installed_cli().or_else(Self::get_bundled_cli_path) {
                format!("& '{}' -n", p.to_string_lossy())
            } else {
                "mtc -n".to_string()
            };

            let wt_res = std::process::Command::new("wt.exe")
                .args(["powershell.exe", "-NoExit", "-Command", &cli_cmd])
                .spawn();
            if wt_res.is_err() {
                let _ = std::process::Command::new("powershell.exe")
                    .args(["-NoExit", "-Command", &cli_cmd])
                    .spawn();
            }
        }

        fn reveal_in_explorer() {
            if let Some(p) = Self::find_installed_cli().or_else(Self::get_bundled_cli_path) {
                reveal_file_in_explorer(&p.to_string_lossy());
            }
        }
    }

    struct GuiState {
        whitelist: WhitelistManager,
        targets: Vec<AppTarget>,
        protected: Vec<(AppTarget, WhitelistMatch)>,
        prefs: GuiPreferences,
        active_language: Language,
        active_tab: usize, // 0: 待结束, 1: 已保护, 2: 全部活动
        scroll_offset: usize,
        hovered_row: Option<usize>,
        hovered_btn: Option<HoverButton>,
        icon_cache: HashMap<String, isize>,
        status_message: Option<String>,
        status_timestamp: Option<Instant>,
        last_scan: Instant,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HoverButton {
        SortMenu,
        Refresh,
        CloseToTray,
        HeroMain,
        HeroChevron,
        Tab(usize),
        RowTrash(usize),
        RowMore(usize),
        ViewAllFromEmpty,
        Settings,
        Language,
        Quit,
    }

    static STATE: Mutex<Option<GuiState>> = Mutex::new(None);

    fn to_wstring(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// 高精度 DPI 线性缩放助手 (96 DPI 为基准 100%)
    #[inline]
    fn scale_dpi(val: i32, dpi: u32) -> i32 {
        ((val as i64 * dpi as i64 + 48) / 96) as i32
    }

    /// 高精度 DPI 反向映射为逻辑 96 DPI 坐标 (保持命中检测逻辑 100% 稳定一致)
    #[inline]
    fn unscale_dpi(val: i32, dpi: u32) -> i32 {
        if dpi == 0 || dpi == 96 {
            val
        } else {
            ((val as i64 * 96 + (dpi as i64 / 2)) / dpi as i64) as i32
        }
    }

    /// 应用名本地化与友好解析
    fn resolve_friendly_name(app: &AppTarget) -> String {
        let name_lower = app.name.to_lowercase();
        match name_lower.as_str() {
            "explorer.exe" | "explorer" => "Windows 资源管理器".to_string(),
            "chrome.exe" => "Google Chrome".to_string(),
            "msedge.exe" => "Microsoft Edge".to_string(),
            "code.exe" => "Visual Studio Code".to_string(),
            "devenv.exe" => "Visual Studio".to_string(),
            "windowsterminal.exe" | "wt.exe" => "Windows 终端".to_string(),
            "powershell.exe" => "Windows PowerShell".to_string(),
            "pwsh.exe" => "PowerShell 7".to_string(),
            "cmd.exe" => "命令提示符".to_string(),
            "notepad.exe" => "记事本".to_string(),
            "taskmgr.exe" => "任务管理器".to_string(),
            "wechat.exe" => "微信".to_string(),
            "qq.exe" => "QQ".to_string(),
            "feishu.exe" | "lark.exe" => "飞书".to_string(),
            "dingtalk.exe" => "钉钉".to_string(),
            "telegram.exe" => "Telegram".to_string(),
            "discord.exe" => "Discord".to_string(),
            "spotify.exe" => "Spotify".to_string(),
            "steam.exe" => "Steam".to_string(),
            _ => {
                let clean_title = app.title.trim();
                if !clean_title.is_empty() && clean_title.chars().count() <= 20 {
                    clean_title.to_string()
                } else if let Some(stem) =
                    std::path::Path::new(&app.name).file_stem().and_then(|s| s.to_str())
                {
                    stem.to_string()
                } else {
                    app.name.clone()
                }
            }
        }
    }

    /// 提取真实高分辨率应用图标 (优先使用 PrivateExtractIconsW 原生指定像素，兜底 ExtractIconExW 大图标)
    unsafe fn get_app_icon(
        exe_path: &str,
        target_size: i32,
        cache: &mut HashMap<String, isize>,
    ) -> Option<HICON> {
        if exe_path.is_empty() {
            return None;
        }

        let cache_key = format!("{}:{}", exe_path, target_size);
        if let Some(&h) = cache.get(&cache_key) {
            return if h != 0 { Some(h as HICON) } else { None };
        }

        let wpath = to_wstring(exe_path);
        let mut h_icon: HICON = 0 as HICON;
        let mut icon_id: u32 = 0;

        // 1. 优先使用 Win32 原生 PrivateExtractIconsW 提取指定目标物理像素高精度图标
        let count = PrivateExtractIconsW(
            wpath.as_ptr(),
            0,
            target_size,
            target_size,
            &mut h_icon,
            &mut icon_id,
            1,
            0,
        );

        if count > 0 && h_icon != 0 as HICON {
            cache.insert(cache_key, h_icon as isize);
            return Some(h_icon);
        }

        // 2. 备用兜底: 使用 ExtractIconExW 提取大图标 (至少 32x32，杜绝模糊 16x16 拉伸)
        let mut h_large: HICON = 0 as HICON;
        let mut h_small: HICON = 0 as HICON;
        let count_ex = ExtractIconExW(wpath.as_ptr(), 0, &mut h_large, &mut h_small, 1);
        if count_ex > 0 {
            if h_small != 0 as HICON {
                DestroyIcon(h_small);
            }
            if h_large != 0 as HICON {
                cache.insert(cache_key, h_large as isize);
                return Some(h_large);
            }
        }

        cache.insert(cache_key, 0);
        None
    }

    /// 检测当前 Windows 任务栏是否为深色模式 (默认 Windows 11 为深色)
    unsafe fn is_dark_taskbar() -> bool {
        let subkey = to_wstring(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        let val_name = to_wstring("SystemUsesLightTheme");
        let mut hkey = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) == 0 {
            let mut val = 0u32;
            let mut val_type = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            let res = RegQueryValueExW(
                hkey,
                val_name.as_ptr(),
                std::ptr::null(),
                &mut val_type,
                &mut val as *mut _ as *mut u8,
                &mut size,
            );
            RegCloseKey(hkey);
            if res == 0 {
                return val == 0;
            }
        }
        true
    }

    /// 更新系统托盘气泡/悬停提示信息
    unsafe fn update_tray_tooltip(hwnd: HWND, text: &str) {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_TIP;
        let tip = to_wstring(text);
        let max_len = 127.min(tip.len());
        for i in 0..max_len {
            nid.szTip[i] = tip[i];
        }
        nid.szTip[max_len] = 0;
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }

    /// 动态合成 100% 对齐 macOS 规范的系统级原生胶囊 X (Pill-X) 镂空托盘图标
    unsafe fn create_default_tray_icon() -> HICON {
        let dpi = GetDpiForSystem().max(96);
        let cx = GetSystemMetricsForDpi(SM_CXSMICON, dpi).max(16);
        let cy = GetSystemMetricsForDpi(SM_CYSMICON, dpi).max(16);

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = cx;
        bmi.bmiHeader.biHeight = -cy; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB as u32;

        let mut bits_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let screen_dc = GetDC(0 as HWND);
        let hbm_color = CreateDIBSection(
            screen_dc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            0 as HANDLE,
            0,
        );
        ReleaseDC(0 as HWND, screen_dc);

        if hbm_color == 0 as HBITMAP || bits_ptr.is_null() {
            return 0 as HICON;
        }

        let dark = is_dark_taskbar();
        let (pill_r, pill_g, pill_b) = if dark {
            (255u8, 255u8, 255u8) // 深色任务栏使用纯白高亮胶囊
        } else {
            (30u8, 30u8, 32u8)    // 浅色任务栏使用暗色石墨胶囊
        };

        // 几何参数: 药丸胶囊与中心 X 镂空 (宽高比 ~1.65:1，完美对齐 macOS TrayIconHelper.pillXIcon)
        let pw = (cx as f64) * 0.90;
        let ph = (cy as f64) * 0.65;
        let r = ph / 2.0;

        let center_x = ((cx - 1) as f64) / 2.0;
        let center_y = ((cy - 1) as f64) / 2.0;

        let x0 = center_x - (pw / 2.0) + r;
        let x1 = center_x + (pw / 2.0) - r;

        let x_arm = ph * 0.28;
        let x_stroke = (ph * 0.16).max(1.2);

        #[inline]
        fn dist_to_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
            let pax = px - ax;
            let pay = py - ay;
            let bax = bx - ax;
            let bay = by - ay;
            let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay + 1e-9)).clamp(0.0, 1.0);
            let dx = pax - bax * h;
            let dy = pay - bay * h;
            (dx * dx + dy * dy).sqrt()
        }

        let pixels = std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (cx * cy) as usize);

        for y in 0..cy {
            for x in 0..cx {
                let mut pill_cov = 0;
                let mut x_cov = 0;

                // 4x4 超采样抗锯齿
                for sy in 0..4 {
                    let py = (y as f64) + ((sy as f64) + 0.5) / 4.0;
                    for sx in 0..4 {
                        let px = (x as f64) + ((sx as f64) + 0.5) / 4.0;

                        // 胶囊外边界距离
                        let qx = px.clamp(x0, x1);
                        let qy = center_y;
                        let dp = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt() - r;

                        if dp <= 0.0 {
                            pill_cov += 1;

                            // X 符号镂空距离
                            let d1 = dist_to_segment(px, py, center_x - x_arm, center_y - x_arm, center_x + x_arm, center_y + x_arm);
                            let d2 = dist_to_segment(px, py, center_x - x_arm, center_y + x_arm, center_x + x_arm, center_y - x_arm);
                            let dx = d1.min(d2) - (x_stroke / 2.0);
                            if dx <= 0.0 {
                                x_cov += 1;
                            }
                        }
                    }
                }

                // 核心算法: 仅在胶囊内部且排除 X 镂空区域产生像素覆盖 (透明 X 穿透任务栏背景)
                let effective_cov = (pill_cov as i32 - x_cov as i32).max(0) as f64 / 16.0;
                let alpha = (effective_cov * 255.0).round() as u8;
                let pr = ((pill_r as f64) * effective_cov).round() as u8;
                let pg = ((pill_g as f64) * effective_cov).round() as u8;
                let pb = ((pill_b as f64) * effective_cov).round() as u8;

                let pixel_val = ((alpha as u32) << 24) | ((pr as u32) << 16) | ((pg as u32) << 8) | (pb as u32);
                pixels[(y * cx + x) as usize] = pixel_val;
            }
        }

        let bytes_per_line = ((cx + 15) / 16) * 2;
        let mask_bytes = vec![0u8; (bytes_per_line * cy) as usize];
        let hbm_mask = CreateBitmap(cx, cy, 1, 1, mask_bytes.as_ptr() as *const core::ffi::c_void);

        let icon_info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let h_icon = CreateIconIndirect(&icon_info);
        DeleteObject(hbm_color);
        DeleteObject(hbm_mask);
        h_icon
    }

    /// 动态更新系统托盘图标 (例如在深浅色主题切换时自适应重绘)
    unsafe fn update_tray_icon(hwnd: HWND) {
        let h_icon = create_default_tray_icon();
        if h_icon != 0 as HICON {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_ICON;
            nid.hIcon = h_icon;
            Shell_NotifyIconW(NIM_MODIFY, &nid);
            DestroyIcon(h_icon);
        }
    }

    /// 高精度圆角矩形渲染助手
    unsafe fn draw_rounded_box(
        hdc: HDC,
        rect: &RECT,
        radius: i32,
        fill_color: COLORREF,
        border_color: Option<COLORREF>,
    ) {
        let brush = CreateSolidBrush(fill_color);
        let pen = if let Some(b) = border_color {
            CreatePen(PS_SOLID, 1, b)
        } else {
            CreatePen(PS_NULL, 0, 0)
        };
        let old_brush = SelectObject(hdc, brush);
        let old_pen = SelectObject(hdc, pen);

        RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);

        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        DeleteObject(pen);
        DeleteObject(brush);
    }

    /// 高精度字体创建器
    unsafe fn create_font(size_pt: i32, weight: i32) -> HFONT {
        CreateFontW(
            -size_pt,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            1, // DEFAULT_CHARSET
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            to_wstring("Segoe UI Variable Text").as_ptr(),
        )
    }

    /// 在 Windows 资源管理器中安全精准高亮定位文件 (解决引号转义崩溃缺陷)
    fn reveal_file_in_explorer(path: &str) {
        if path.is_empty() {
            return;
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new("explorer.exe");
            cmd.raw_arg(format!("/select,\"{}\"", path));
            if cmd.spawn().is_err() {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    let _ = std::process::Command::new("explorer.exe").arg(parent).spawn();
                }
            }
        }
    }

    /// 弹出原生 Windows 文件属性对话框 (基于 COM 单线程套间)
    unsafe fn show_file_properties(hwnd: HWND, exe_path: &str) {
        if exe_path.is_empty() {
            return;
        }
        let wpath = to_wstring(exe_path);
        let wverb = to_wstring("properties");
        let mut sei: SHELLEXECUTEINFOW = std::mem::zeroed();
        sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        sei.fMask = SEE_MASK_INVOKEIDLIST;
        sei.hwnd = hwnd;
        sei.lpVerb = wverb.as_ptr();
        sei.lpFile = wpath.as_ptr();
        sei.nShow = SW_SHOWNORMAL as i32;
        let res = ShellExecuteExW(&mut sei);
        if res == 0 {
            reveal_file_in_explorer(exe_path);
        }
    }

    /// 统一从当前激活 Tab 获取指定行项目及保护属性
    fn get_item_at(state: &GuiState, idx: usize) -> Option<(AppTarget, bool)> {
        match state.active_tab {
            0 => state.targets.get(idx).map(|a| (a.clone(), false)),
            1 => state.protected.get(idx).map(|(a, _)| (a.clone(), true)),
            2 => {
                if idx < state.targets.len() {
                    Some((state.targets[idx].clone(), false))
                } else {
                    let p_idx = idx - state.targets.len();
                    state.protected.get(p_idx).map(|(a, _)| (a.clone(), true))
                }
            }
            _ => None,
        }
    }

    fn refresh_scan() {
        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            let caller_lineage = get_caller_lineage();
            let mut scanned = scan_foreground_apps();

            let mut targets = Vec::new();
            let mut protected = Vec::new();

            for app in scanned.drain(..) {
                if let Some(matched) = state.whitelist.classify(&app, &caller_lineage) {
                    protected.push((app, matched));
                } else {
                    targets.push(app);
                }
            }

            sort_targets(&mut targets, state.prefs.sort_mode);
            sort_protected(&mut protected, state.prefs.sort_mode);

            state.targets = targets;
            state.protected = protected;
            state.scroll_offset = 0;
            state.last_scan = Instant::now();
        }
    }

    fn refresh_scan_silent() {
        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            let caller_lineage = get_caller_lineage();
            let mut scanned = scan_foreground_apps();

            let mut targets = Vec::new();
            let mut protected = Vec::new();

            for app in scanned.drain(..) {
                if let Some(matched) = state.whitelist.classify(&app, &caller_lineage) {
                    protected.push((app, matched));
                } else {
                    targets.push(app);
                }
            }

            sort_targets(&mut targets, state.prefs.sort_mode);
            sort_protected(&mut protected, state.prefs.sort_mode);

            state.targets = targets;
            state.protected = protected;
            state.last_scan = Instant::now();

            // 状态提示气泡若超过 3 秒自动平滑隐退
            if let Some(ts) = state.status_timestamp {
                if ts.elapsed().as_secs() >= 3 {
                    state.status_message = None;
                    state.status_timestamp = None;
                }
            }
        }
    }

    /// 自动将窗口精确吸附在屏幕右下角任务栏正上方 (支持高 DPI 动态定位与多显示器自适应)
    unsafe fn position_window(hwnd: HWND) {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let win_w = scale_dpi(WINDOW_WIDTH, dpi);
        let win_h = scale_dpi(WINDOW_HEIGHT, dpi);

        let mut cursor: POINT = std::mem::zeroed();
        GetCursorPos(&mut cursor);

        let hmon = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        let work_area = if GetMonitorInfoW(hmon, &mut mi) != 0 {
            mi.rcWork
        } else {
            let mut wa: RECT = std::mem::zeroed();
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut wa as *mut _ as *mut _, 0);
            wa
        };

        let screen_w = work_area.right - work_area.left;
        let screen_h = work_area.bottom - work_area.top;

        let margin_x = scale_dpi(16, dpi);
        let margin_y = scale_dpi(12, dpi);

        let x = if screen_w > win_w {
            work_area.right - win_w - margin_x
        } else {
            work_area.left
        };

        let y = if screen_h > win_h {
            work_area.bottom - win_h - margin_y
        } else {
            work_area.top
        };

        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            win_w,
            win_h,
            SWP_SHOWWINDOW,
        );
    }

    /// 原生 Win32 零延迟剪贴板复制
    unsafe fn copy_to_clipboard(hwnd: HWND, text: &str) -> bool {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();

        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if hmem.is_null() {
            return false;
        }
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            GlobalUnlock(hmem);
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes);
        GlobalUnlock(hmem);

        if OpenClipboard(hwnd) == 0 {
            return false;
        }
        EmptyClipboard();
        SetClipboardData(13 /* CF_UNICODETEXT */, hmem as _);
        CloseClipboard();
        true
    }

    const REG_RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const APP_REG_NAME: &str = "TaskCleaner";

    unsafe fn is_autostart_enabled() -> bool {
        let subkey = to_wstring(REG_RUN_SUBKEY);
        let val_name = to_wstring(APP_REG_NAME);
        let mut hkey = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
            return false;
        }
        let mut val_type = 0u32;
        let res = RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            std::ptr::null(),
            &mut val_type,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        RegCloseKey(hkey);
        res == 0
    }

    unsafe fn toggle_autostart() -> bool {
        let subkey = to_wstring(REG_RUN_SUBKEY);
        let val_name = to_wstring(APP_REG_NAME);
        let currently_enabled = is_autostart_enabled();

        if currently_enabled {
            let mut hkey = std::ptr::null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_WRITE, &mut hkey) == 0 {
                RegDeleteValueW(hkey, val_name.as_ptr());
                RegCloseKey(hkey);
            }
            false
        } else {
            if let Ok(exe_path) = std::env::current_exe() {
                let path_str = format!("\"{}\"", exe_path.to_string_lossy());
                let wide_path = to_wstring(&path_str);
                let bytes = (wide_path.len() * 2) as u32;

                let mut hkey = std::ptr::null_mut();
                if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey)
                    == 0
                {
                    RegSetValueExW(
                        hkey,
                        val_name.as_ptr(),
                        0,
                        REG_SZ,
                        wide_path.as_ptr() as *const u8,
                        bytes,
                    );
                    RegCloseKey(hkey);
                }
            }
            true
        }
    }

    /// 将修饰键掩码与虚拟键码格式化为直观文本 (如 Ctrl + Alt + K)
    fn format_hotkey_string(mods: u32, vk: u32) -> String {
        let mut parts = Vec::new();
        if (mods & 0x0002 /* MOD_CONTROL */) != 0 {
            parts.push("Ctrl");
        }
        if (mods & 0x0008 /* MOD_WIN */) != 0 {
            parts.push("Win");
        }
        if (mods & 0x0001 /* MOD_ALT */) != 0 {
            parts.push("Alt");
        }
        if (mods & 0x0004 /* MOD_SHIFT */) != 0 {
            parts.push("Shift");
        }
        let key_name = match vk {
            0x30..=0x39 => format!("{}", (vk as u8) as char),
            0x41..=0x5A => format!("{}", (vk as u8) as char),
            0x70..=0x87 => format!("F{}", vk - 0x70 + 1),
            0x20 => "Space".to_string(),
            0x09 => "Tab".to_string(),
            0xC0 => "`".to_string(),
            0xBD => "-".to_string(),
            0xBB => "=".to_string(),
            0xDB => "[".to_string(),
            0xDD => "]".to_string(),
            0xDC => "\\".to_string(),
            0xBA => ";".to_string(),
            0xDE => "'".to_string(),
            0xBC => ",".to_string(),
            0xBE => ".".to_string(),
            0xBF => "/".to_string(),
            _ => format!("Key(0x{:X})", vk),
        };
        parts.push(&key_name);
        parts.join(" + ")
    }

    /// 动态应用并向 Windows 系统注册/反注册全局快捷键
    fn apply_hotkey(hwnd: HWND, cfg: &HotkeyConfig) -> bool {
        unsafe {
            UnregisterHotKey(hwnd, HOTKEY_TOGGLE_ID);
            if cfg.enabled && cfg.vk > 0 {
                RegisterHotKey(hwnd, HOTKEY_TOGGLE_ID, cfg.modifiers, cfg.vk) != 0
            } else {
                true
            }
        }
    }

    struct RecorderData {
        parent_hwnd: isize,
        mods: u32,
        vk: u32,
        display: String,
    }

    static RECORDER_STATE: Mutex<Option<RecorderData>> = Mutex::new(None);
    static RECORDER_CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

    /// 原生快捷键实时录制窗口过程
    unsafe extern "system" fn recorder_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);

                let dpi = GetDpiForWindow(hwnd).max(96);
                let s = |v: i32| scale_dpi(v, dpi);
                let s_rect = |l: i32, t: i32, r: i32, b: i32| RECT {
                    left: scale_dpi(l, dpi),
                    top: scale_dpi(t, dpi),
                    right: scale_dpi(r, dpi),
                    bottom: scale_dpi(b, dpi),
                };

                // 背景
                let bg_brush = CreateSolidBrush(rgb(28, 28, 32));
                FillRect(hdc, &rc, bg_brush);
                DeleteObject(bg_brush);

                SetBkMode(hdc, TRANSPARENT as i32);

                // 标题
                SetTextColor(hdc, rgb(255, 255, 255));
                let title_font = create_font(s(15), FW_BOLD as i32);
                let old_font = SelectObject(hdc, title_font);
                let mut tr = s_rect(24, 14, 380 - 24, 36);
                let title_txt = to_wstring("录制一键清理快捷键");
                DrawTextW(hdc, title_txt.as_ptr(), -1, &mut tr, DT_LEFT | DT_SINGLELINE);

                // 副标题提示
                let sub_font = create_font(s(12), FW_NORMAL as i32);
                SelectObject(hdc, sub_font);
                SetTextColor(hdc, rgb(156, 163, 175));
                let mut sr = s_rect(24, 38, 380 - 24, 58);
                let sub_txt = to_wstring("请在键盘上按下组合键 (按下后直接执行一键退出未保护任务)");
                DrawTextW(hdc, sub_txt.as_ptr(), -1, &mut sr, DT_LEFT | DT_SINGLELINE);
                DeleteObject(sub_font);

                // 按键展示框 (居中圆角卡片)
                let box_rect = s_rect(24, 66, 380 - 24, 114);
                let box_brush = CreateSolidBrush(rgb(40, 40, 46));
                let box_pen = CreatePen(PS_SOLID as i32, 1, rgb(0, 120, 215));
                let old_pen = SelectObject(hdc, box_pen);
                let old_brush = SelectObject(hdc, box_brush);
                RoundRect(hdc, box_rect.left, box_rect.top, box_rect.right, box_rect.bottom, s(8), s(8));
                SelectObject(hdc, old_brush);
                SelectObject(hdc, old_pen);
                DeleteObject(box_brush);
                DeleteObject(box_pen);

                let disp_text = {
                    let guard = RECORDER_STATE.lock().unwrap();
                    guard.as_ref().map(|d| d.display.clone()).unwrap_or_else(|| "按下快捷键...".to_string())
                };
                let key_font = create_font(s(16), FW_BOLD as i32);
                SelectObject(hdc, key_font);
                SetTextColor(hdc, rgb(255, 255, 255));
                let mut kr = box_rect;
                let wide_disp = to_wstring(&disp_text);
                DrawTextW(hdc, wide_disp.as_ptr(), -1, &mut kr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                DeleteObject(key_font);

                // 底部三按钮
                let btn_font = create_font(s(13), FW_SEMIBOLD as i32);
                SelectObject(hdc, btn_font);

                let draw_btn = |hdc: HDC, rect: RECT, text: &str, bg_color: COLORREF, text_color: COLORREF| {
                    let b_brush = CreateSolidBrush(bg_color);
                    let p_pen = CreatePen(PS_SOLID as i32, 1, bg_color);
                    let o_p = SelectObject(hdc, p_pen);
                    let o_b = SelectObject(hdc, b_brush);
                    RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, s(6), s(6));
                    SelectObject(hdc, o_b);
                    SelectObject(hdc, o_p);
                    DeleteObject(b_brush);
                    DeleteObject(p_pen);

                    SetTextColor(hdc, text_color);
                    let mut r = rect;
                    let wt = to_wstring(text);
                    DrawTextW(hdc, wt.as_ptr(), -1, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                };

                let btn_y = s(126);
                let btn_h = s(32);
                let btn_w = s(98);
                let btn_save = RECT { left: s(24), top: btn_y, right: s(24) + btn_w, bottom: btn_y + btn_h };
                let btn_reset = RECT { left: s(132), top: btn_y, right: s(132) + btn_w, bottom: btn_y + btn_h };
                let btn_cancel = RECT { left: s(240), top: btn_y, right: s(240) + btn_w, bottom: btn_y + btn_h };

                draw_btn(hdc, btn_save, "保存生效", rgb(0, 120, 215), rgb(255, 255, 255));
                draw_btn(hdc, btn_reset, "恢复默认", rgb(52, 52, 58), rgb(220, 220, 225));
                draw_btn(hdc, btn_cancel, "取消", rgb(52, 52, 58), rgb(220, 220, 225));

                SelectObject(hdc, old_font);
                DeleteObject(title_font);
                DeleteObject(btn_font);

                EndPaint(hwnd, &ps);
                0
            }
            WM_SYSKEYDOWN | WM_KEYDOWN => {
                let vk = wparam as u32;
                if vk == 27 /* VK_ESCAPE */ {
                    DestroyWindow(hwnd);
                    return 0;
                }
                if vk == 13 /* VK_RETURN */ {
                    save_recorded_hotkey(hwnd);
                    return 0;
                }

                let is_modifier = matches!(vk, 16 | 17 | 18 | 91 | 92);
                let ctrl = (GetKeyState(17) as u16 & 0x8000) != 0;
                let alt = (GetKeyState(18) as u16 & 0x8000) != 0;
                let shift = (GetKeyState(16) as u16 & 0x8000) != 0;
                let win = ((GetKeyState(91) as u16 & 0x8000) != 0) || ((GetKeyState(92) as u16 & 0x8000) != 0);

                let mut mods = 0u32;
                if ctrl { mods |= MOD_CONTROL; }
                if alt { mods |= MOD_ALT; }
                if shift { mods |= MOD_SHIFT; }
                if win { mods |= MOD_WIN; }

                if !is_modifier && mods > 0 {
                    let display = format_hotkey_string(mods, vk);
                    let mut guard = RECORDER_STATE.lock().unwrap();
                    if let Some(data) = guard.as_mut() {
                        data.mods = mods;
                        data.vk = vk;
                        data.display = display;
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                } else if is_modifier && mods > 0 {
                    let mut parts = Vec::new();
                    if ctrl { parts.push("Ctrl"); }
                    if win { parts.push("Win"); }
                    if alt { parts.push("Alt"); }
                    if shift { parts.push("Shift"); }
                    parts.push("...");
                    let display = parts.join(" + ");
                    let mut guard = RECORDER_STATE.lock().unwrap();
                    if let Some(data) = guard.as_mut() {
                        data.display = display;
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                0
            }
            WM_SYSKEYUP | WM_KEYUP => 0,
            WM_LBUTTONUP => {
                let raw_x = (lparam & 0xFFFF) as i32;
                let raw_y = ((lparam >> 16) & 0xFFFF) as i32;
                let dpi = GetDpiForWindow(hwnd).max(96);
                let x = unscale_dpi(raw_x, dpi);
                let y = unscale_dpi(raw_y, dpi);
                if y >= 126 && y <= 158 {
                    if x >= 24 && x <= 122 {
                        save_recorded_hotkey(hwnd);
                        return 0;
                    } else if x >= 132 && x <= 230 {
                        let mut guard = RECORDER_STATE.lock().unwrap();
                        if let Some(data) = guard.as_mut() {
                            data.mods = MOD_CONTROL | MOD_ALT;
                            data.vk = 0x4B; // 'K'
                            data.display = "Ctrl + Alt + K".to_string();
                        }
                        drop(guard);
                        save_recorded_hotkey(hwnd);
                        return 0;
                    } else if x >= 240 && x <= 338 {
                        DestroyWindow(hwnd);
                        return 0;
                    }
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn save_recorded_hotkey(hwnd: HWND) {
        let (parent_hwnd, mods, vk, display) = {
            let guard = RECORDER_STATE.lock().unwrap();
            match guard.as_ref() {
                Some(d) => (d.parent_hwnd as HWND, d.mods, d.vk, d.display.clone()),
                None => return,
            }
        };

        if mods == 0 || vk == 0 || display.ends_with("...") {
            MessageBoxW(
                hwnd,
                to_wstring("请在键盘上按下完整的快捷键组合 (需包含修饰键与触发键)！").as_ptr(),
                to_wstring("提示").as_ptr(),
                MB_OK | MB_ICONWARNING | MB_TOPMOST,
            );
            return;
        }

        let ok = apply_hotkey(parent_hwnd, &HotkeyConfig {
            enabled: true,
            modifiers: mods,
            vk,
            display: display.clone(),
        });

        if !ok {
            let old_cfg = {
                let state_guard = STATE.lock().unwrap();
                state_guard
                    .as_ref()
                    .map(|s| s.prefs.hotkey.clone())
                    .unwrap_or_default()
            };
            apply_hotkey(parent_hwnd, &old_cfg);

            MessageBoxW(
                hwnd,
                to_wstring(&format!("快捷键「{}」已被系统或其他正在运行的软件占用，请尝试其他组合！", display)).as_ptr(),
                to_wstring("快捷键冲突").as_ptr(),
                MB_OK | MB_ICONWARNING | MB_TOPMOST,
            );
            return;
        }

        {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.prefs.hotkey.enabled = true;
                state.prefs.hotkey.modifiers = mods;
                state.prefs.hotkey.vk = vk;
                state.prefs.hotkey.display = display.clone();
                state.prefs.save();
                state.status_message = Some(format!("一键清理快捷键已更新为: {}", display));
                state.status_timestamp = Some(Instant::now());
            }
        }

        DestroyWindow(hwnd);
    }

    unsafe fn show_shortcut_recorder_dialog(parent_hwnd: HWND) {
        let cur_hk = {
            let state_guard = STATE.lock().unwrap();
            state_guard
                .as_ref()
                .map(|s| s.prefs.hotkey.clone())
                .unwrap_or_default()
        };

        {
            let mut guard = RECORDER_STATE.lock().unwrap();
            *guard = Some(RecorderData {
                parent_hwnd: parent_hwnd as isize,
                mods: cur_hk.modifiers,
                vk: cur_hk.vk,
                display: cur_hk.display.clone(),
            });
        }

        let h_instance = GetModuleHandleW(std::ptr::null());
        let class_name = to_wstring("TaskCleanerShortcutRecorder");

        if !RECORDER_CLASS_REGISTERED.load(Ordering::SeqCst) {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_DROPSHADOW,
                lpfnWndProc: Some(recorder_wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: 0 as HICON,
                hCursor: LoadCursorW(0 as HINSTANCE, IDC_ARROW),
                hbrBackground: 0 as HBRUSH,
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: 0 as HICON,
            };
            RegisterClassExW(&wc);
            RECORDER_CLASS_REGISTERED.store(true, Ordering::SeqCst);
        }

        let dpi = GetDpiForWindow(parent_hwnd).max(96);
        let screen_w = GetSystemMetrics(0 /* SM_CXSCREEN */);
        let screen_h = GetSystemMetrics(1 /* SM_CYSCREEN */);
        let dlg_w = scale_dpi(380, dpi);
        let dlg_h = scale_dpi(210, dpi);
        let x = (screen_w - dlg_w) / 2;
        let y = (screen_h - dlg_h) / 2;

        let dlg_hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name.as_ptr(),
            to_wstring("录制一键清理快捷键 - Task Cleaner").as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x,
            y,
            dlg_w,
            dlg_h,
            parent_hwnd,
            0 as HMENU,
            h_instance,
            std::ptr::null(),
        );

        let corner_preference = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            dlg_hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner_preference as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        EnableWindow(parent_hwnd, 0);
        ShowWindow(dlg_hwnd, SW_SHOW);
        SetForegroundWindow(dlg_hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while IsWindow(dlg_hwnd) != 0 && GetMessageW(&mut msg, 0 as HWND, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        EnableWindow(parent_hwnd, 1);
        SetForegroundWindow(parent_hwnd);
        InvalidateRect(parent_hwnd, std::ptr::null(), 1);
    }

    unsafe fn build_fluent_tiered_menu(hwnd: HWND) -> Vec<FluentMenuItem> {
        let (lang, prefs, targets, protected) = {
            let state_guard = STATE.lock().unwrap();
            let s = match state_guard.as_ref() {
                Some(s) => s,
                None => return Vec::new(),
            };
            (s.active_language, s.prefs.clone(), s.targets.clone(), s.protected.clone())
        };

        let mut items = Vec::new();

        // 1. 一键退出全部待清理任务 (含快捷键提示)
        let clean_all_shortcut = if prefs.hotkey.enabled {
            prefs.hotkey.display.clone()
        } else {
            String::new()
        };
        if !clean_all_shortcut.is_empty() {
            items.push(FluentMenuItem::action_with_shortcut(
                IDM_CLEAN_ALL,
                tr(I18nKey::BtnTerminate, lang),
                Some('\u{E74D}'),
                clean_all_shortcut,
            ));
        } else {
            items.push(FluentMenuItem::action(
                IDM_CLEAN_ALL,
                tr(I18nKey::BtnTerminate, lang),
                Some('\u{E74D}'),
            ));
        }

        // 清理执行策略子菜单
        let clean_options = vec![
            FluentMenuItem::action(IDM_ACTION_GRACEFUL, tr(I18nKey::ActionCleanGraceful, lang), Some('\u{E74D}')),
            FluentMenuItem::action(IDM_ACTION_FORCE, tr(I18nKey::ActionCleanForce, lang), Some('\u{E711}')),
            FluentMenuItem::separator(),
            FluentMenuItem::action(IDM_ACTION_PURGE, tr(I18nKey::ActionCleanPurge, lang), Some('\u{E894}')),
        ];
        items.push(FluentMenuItem::submenu("清理执行策略", Some('\u{E894}'), clean_options));

        // 2. 待结束应用分级子菜单 (Killable Apps Submenu)
        let mut target_subitems = Vec::new();
        if targets.is_empty() {
            target_subitems.push(FluentMenuItem {
                text: "当前无待结束任务".to_string(),
                icon: None,
                shortcut: None,
                kind: MenuItemKind::Action(0),
                enabled: false,
            });
        } else {
            for (i, t) in targets.iter().take(32).enumerate() {
                let label = format!("{} ({:.1} MB)", t.name, t.memory_mb());
                let row_actions = vec![
                    FluentMenuItem::action(IDM_KILL_TARGET_BASE + i, "结束该任务", Some('\u{E711}')),
                    FluentMenuItem::action(IDM_PROTECT_TARGET_BASE + i, "加入白名单保护", Some('\u{EA18}')),
                    FluentMenuItem::action(IDM_REVEAL_TARGET_BASE + i, "在资源管理器中定位", Some('\u{ED25}')),
                    FluentMenuItem::action(IDM_PURGE_TARGET_BASE + i, "释放工作集内存", Some('\u{E894}')),
                ];
                target_subitems.push(FluentMenuItem::submenu(label, Some('\u{E71D}'), row_actions));
            }
            if targets.len() > 32 {
                target_subitems.push(FluentMenuItem {
                    text: format!("... 及其余 {} 个任务", targets.len() - 32),
                    icon: None,
                    shortcut: None,
                    kind: MenuItemKind::Action(0),
                    enabled: false,
                });
            }
        }
        items.push(FluentMenuItem::submenu(
            format!("待结束任务 ({})", targets.len()),
            Some('\u{E71D}'),
            target_subitems,
        ));

        // 3. 白名单保护应用分级子菜单 (Protected Apps Submenu)
        let mut protected_subitems = Vec::new();
        if protected.is_empty() {
            protected_subitems.push(FluentMenuItem {
                text: "当前无受保护应用".to_string(),
                icon: None,
                shortcut: None,
                kind: MenuItemKind::Action(0),
                enabled: false,
            });
        } else {
            for (i, (t, _)) in protected.iter().take(32).enumerate() {
                let label = format!("{} ({:.1} MB)", t.name, t.memory_mb());
                let row_actions = vec![
                    FluentMenuItem::action(IDM_UNPROTECT_BASE + i, "解除保护", Some('\u{E711}')),
                    FluentMenuItem::action(IDM_REVEAL_PROTECTED_BASE + i, "在资源管理器中定位", Some('\u{ED25}')),
                ];
                protected_subitems.push(FluentMenuItem::submenu(label, Some('\u{EA18}'), row_actions));
            }
            if protected.len() > 32 {
                protected_subitems.push(FluentMenuItem {
                    text: format!("... 及其余 {} 个受保护应用", protected.len() - 32),
                    icon: None,
                    shortcut: None,
                    kind: MenuItemKind::Action(0),
                    enabled: false,
                });
            }
        }
        items.push(FluentMenuItem::submenu(
            format!("受保护应用 ({})", protected.len()),
            Some('\u{EA18}'),
            protected_subitems,
        ));

        items.push(FluentMenuItem::separator());

        // 4. 开机自动启动开关
        let autostart_on = is_autostart_enabled();
        items.push(FluentMenuItem::toggle(
            IDM_CFG_STARTUP,
            tr(I18nKey::MenuLaunchAtLogin, lang),
            autostart_on,
            None,
        ));

        // 5. 全局清理快捷键子菜单
        let mut hk_items = Vec::new();
        hk_items.push(FluentMenuItem::toggle(
            IDM_HOTKEY_TOGGLE_ENABLE,
            "启用全局清理快捷键",
            prefs.hotkey.enabled,
            None,
        ));
        hk_items.push(FluentMenuItem::separator());
        for (i, p) in PRESETS.iter().enumerate() {
            let is_matched = prefs.hotkey.enabled && prefs.hotkey.modifiers == p.modifiers && prefs.hotkey.vk == p.vk;
            hk_items.push(FluentMenuItem::radio(IDM_HOTKEY_PRESET_BASE + i, p.display, is_matched));
        }
        hk_items.push(FluentMenuItem::separator());
        hk_items.push(FluentMenuItem::action(
            IDM_HOTKEY_CUSTOM_RECORDER,
            "自定义快捷键录制...",
            Some('\u{E765}'),
        ));
        let cur_hk_summary = if prefs.hotkey.enabled {
            prefs.hotkey.display.as_str()
        } else {
            "已禁用"
        };
        items.push(FluentMenuItem::submenu(
            format!("{}: {}", tr(I18nKey::MenuGlobalShortcut, lang), cur_hk_summary),
            Some('\u{E765}'),
            hk_items,
        ));

        // 6. 任务排序方式子菜单
        let sort_items = vec![
            FluentMenuItem::radio(IDM_SORT_COMPOSITE, tr(I18nKey::SortComposite, lang), prefs.sort_mode == SortMode::Composite),
            FluentMenuItem::radio(IDM_SORT_MEMORY, tr(I18nKey::SortMemory, lang), prefs.sort_mode == SortMode::Memory),
            FluentMenuItem::radio(IDM_SORT_CPU, tr(I18nKey::SortCpu, lang), prefs.sort_mode == SortMode::Cpu),
            FluentMenuItem::radio(IDM_SORT_WINDOWS, tr(I18nKey::SortWindows, lang), prefs.sort_mode == SortMode::Windows),
            FluentMenuItem::radio(IDM_SORT_DEFAULT, tr(I18nKey::SortDefault, lang), prefs.sort_mode == SortMode::Default),
        ];
        items.push(FluentMenuItem::submenu(
            format!("{}: {}", tr(I18nKey::MenuSortBy, lang), prefs.sort_mode.label()),
            Some('\u{E8CB}'),
            sort_items,
        ));

        // 7. 界面显示偏好子菜单
        let disp_items = vec![
            FluentMenuItem::toggle(IDM_CFG_TOGGLE_DETAILED_METRICS, tr(I18nKey::MenuShowDetailedMetrics, lang), prefs.show_detailed_metrics, None),
            FluentMenuItem::toggle(IDM_CFG_TOGGLE_APP_ID, tr(I18nKey::MenuShowAppIdentifier, lang), prefs.show_app_identifier, None),
            FluentMenuItem::toggle(IDM_CFG_TOGGLE_SORT_BTN, tr(I18nKey::MenuShowSortButton, lang), prefs.show_sort_button, None),
        ];
        items.push(FluentMenuItem::submenu("界面显示偏好", Some('\u{E790}'), disp_items));

        // 8. CLI 命令行工具 (mtc) 子菜单
        let installed_path = CliManager::find_installed_cli();
        let bundled_path = CliManager::get_bundled_cli_path();
        let cli_installed = installed_path.is_some();
        let cli_has_any = cli_installed || bundled_path.is_some();

        let mut cli_items = Vec::new();
        let cli_status_str = if cli_installed {
            format!("{}: 全局已就绪", tr(I18nKey::CliStatusInstalled, lang))
        } else if bundled_path.is_some() {
            format!("{}: 开发就绪 (免安装可用)", tr(I18nKey::CliStatusInstalled, lang))
        } else {
            format!("{}: 未安装", tr(I18nKey::CliStatusNotInstalled, lang))
        };
        cli_items.push(FluentMenuItem {
            text: cli_status_str,
            icon: Some('\u{E946}'),
            shortcut: None,
            kind: MenuItemKind::Action(0),
            enabled: false,
        });
        cli_items.push(FluentMenuItem::separator());
        if !cli_installed {
            cli_items.push(FluentMenuItem::action(IDM_CLI_INSTALL_USER, tr(I18nKey::CliMenuInstallUser, lang), Some('\u{E756}')));
        }
        if cli_has_any {
            cli_items.push(FluentMenuItem::action(IDM_CLI_TEST_TERMINAL, tr(I18nKey::CliMenuTest, lang), Some('\u{E756}')));
            cli_items.push(FluentMenuItem::action(IDM_CLI_REVEAL, tr(I18nKey::CliMenuReveal, lang), Some('\u{ED25}')));
        }
        if cli_installed {
            cli_items.push(FluentMenuItem::separator());
            cli_items.push(FluentMenuItem::action(IDM_CLI_UNINSTALL, tr(I18nKey::CliMenuUninstall, lang), Some('\u{E711}')));
        }
        items.push(FluentMenuItem::submenu(tr(I18nKey::MenuCliTools, lang), Some('\u{E756}'), cli_items));

        // 9. 语言偏好设置子菜单
        let mut lang_items = Vec::new();
        lang_items.push(FluentMenuItem::radio(IDM_LANG_AUTO, tr(I18nKey::LangAuto, lang), prefs.language_pref == LanguagePreference::Auto));
        lang_items.push(FluentMenuItem::separator());
        for (idx, &l) in Language::ALL.iter().enumerate() {
            let checked = prefs.language_pref == LanguagePreference::Specific(l);
            lang_items.push(FluentMenuItem::radio(IDM_LANG_BASE + idx, l.display_name(), checked));
        }
        items.push(FluentMenuItem::submenu(
            format!("语言 / Language ({})", lang.display_name()),
            Some('\u{E774}'),
            lang_items,
        ));

        items.push(FluentMenuItem::separator());

        // 10. 详细控制面板与系统设置
        items.push(FluentMenuItem::action(IDM_OPEN, "打开详细控制面板", Some('\u{E737}')));
        items.push(FluentMenuItem::action(IDM_REFRESH, tr(I18nKey::HeaderRefreshHelp, lang), Some('\u{E72C}')));
        items.push(FluentMenuItem::action(IDM_CFG_RELOAD, "重新加载配置与白名单", Some('\u{E72C}')));
        items.push(FluentMenuItem::action(IDM_CFG_OPEN_FILE, tr(I18nKey::MenuOpenConfigFile, lang), Some('\u{E8A5}')));
        items.push(FluentMenuItem::action(IDM_CFG_OPEN_DIR, tr(I18nKey::MenuOpenConfigDir, lang), Some('\u{ED25}')));
        items.push(FluentMenuItem::action(IDM_CFG_GITHUB, tr(I18nKey::MenuGithubRepo, lang), Some('\u{E71B}')));
        items.push(FluentMenuItem::action(IDM_CFG_ABOUT, tr(I18nKey::BtnAbout, lang), Some('\u{E946}')));

        items.push(FluentMenuItem::separator());

        // 11. 退出
        items.push(FluentMenuItem::action(IDM_QUIT, tr(I18nKey::BtnQuit, lang), Some('\u{E711}')));

        items
    }

    unsafe fn handle_menu_command(hwnd: HWND, cmd: usize) {
        match cmd {
            IDM_OPEN => {
                refresh_scan();
                position_window(hwnd);
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                IS_VISIBLE.store(true, Ordering::SeqCst);
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_REFRESH => {
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLEAN_ALL | IDM_ACTION_GRACEFUL => {
                execute_clean_all(TerminationMode::Standard);
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ACTION_FORCE => {
                execute_clean_all(TerminationMode::ForceImmediate);
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ACTION_PURGE => {
                execute_clean_all(TerminationMode::PurgeWorkingSet);
                refresh_scan_silent();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_QUIT => {
                PostQuitMessage(0);
            }
            IDM_CFG_STARTUP => {
                let now_enabled = toggle_autostart();
                let status = if now_enabled {
                    "已开启开机自动启动"
                } else {
                    "已关闭开机自动启动"
                };
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some(status.to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_HOTKEY_TOGGLE_ENABLE => {
                let (new_enabled, cfg) = {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.prefs.hotkey.enabled = !state.prefs.hotkey.enabled;
                        let enabled = state.prefs.hotkey.enabled;
                        let cfg = state.prefs.hotkey.clone();
                        state.prefs.save();
                        (enabled, cfg)
                    } else {
                        (false, HotkeyConfig::default())
                    }
                };
                apply_hotkey(hwnd, &cfg);
                let status = if new_enabled {
                    format!("一键清理快捷键已开启: {}", cfg.display)
                } else {
                    "一键清理快捷键已关闭".to_string()
                };
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some(status);
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_HOTKEY_CUSTOM_RECORDER => {
                show_shortcut_recorder_dialog(hwnd);
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            cmd if (IDM_HOTKEY_PRESET_BASE..IDM_HOTKEY_PRESET_BASE + PRESETS.len()).contains(&cmd) => {
                let idx = cmd - IDM_HOTKEY_PRESET_BASE;
                let preset = &PRESETS[idx];
                let new_cfg = HotkeyConfig {
                    enabled: true,
                    modifiers: preset.modifiers,
                    vk: preset.vk,
                    display: match preset.name {
                        "CtrlAltK" => "Ctrl + Alt + K".to_string(),
                        "CtrlShiftK" => "Ctrl + Shift + K".to_string(),
                        "AltShiftK" => "Alt + Shift + K".to_string(),
                        "CtrlAltX" => "Ctrl + Alt + X".to_string(),
                        "WinAltK" => "Win + Alt + K".to_string(),
                        "WinShiftK" => "Win + Shift + K".to_string(),
                        _ => preset.display.to_string(),
                    },
                };
                let ok = apply_hotkey(hwnd, &new_cfg);
                if ok {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.prefs.hotkey = new_cfg.clone();
                        state.prefs.save();
                        state.status_message = Some(format!("一键清理快捷键已设为: {}", new_cfg.display));
                        state.status_timestamp = Some(Instant::now());
                    }
                } else {
                    let old_cfg = {
                        let state_guard = STATE.lock().unwrap();
                        state_guard.as_ref().map(|s| s.prefs.hotkey.clone()).unwrap_or_default()
                    };
                    apply_hotkey(hwnd, &old_cfg);
                    MessageBoxW(
                        hwnd,
                        to_wstring(&format!("快捷键「{}」已被系统或其他正在运行的软件占用，请选择其他预设或自定义录制！", new_cfg.display)).as_ptr(),
                        to_wstring("快捷键冲突").as_ptr(),
                        MB_OK | MB_ICONWARNING | MB_TOPMOST,
                    );
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_RELOAD => {
                let fresh_prefs = GuiPreferences::load();
                apply_hotkey(hwnd, &fresh_prefs.hotkey);
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.whitelist = WhitelistManager::new();
                    state.prefs = fresh_prefs;
                    state.status_message = Some("配置与白名单规则已重新加载".to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_TOGGLE_DETAILED_METRICS => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.prefs.show_detailed_metrics = !state.prefs.show_detailed_metrics;
                    state.prefs.save();
                    let s = if state.prefs.show_detailed_metrics { "已开启详细指标" } else { "已关闭详细指标" };
                    state.status_message = Some(s.to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_TOGGLE_APP_ID => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.prefs.show_app_identifier = !state.prefs.show_app_identifier;
                    state.prefs.save();
                    let s = if state.prefs.show_app_identifier { "已开启进程标识" } else { "已关闭进程标识" };
                    state.status_message = Some(s.to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_TOGGLE_SORT_BTN => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.prefs.show_sort_button = !state.prefs.show_sort_button;
                    state.prefs.save();
                    let s = if state.prefs.show_sort_button { "已显示排序按钮" } else { "已隐藏排序按钮" };
                    state.status_message = Some(s.to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_SORT_COMPOSITE | IDM_SORT_MEMORY | IDM_SORT_CPU | IDM_SORT_WINDOWS | IDM_SORT_DEFAULT => {
                let new_mode = match cmd {
                    IDM_SORT_COMPOSITE => SortMode::Composite,
                    IDM_SORT_MEMORY => SortMode::Memory,
                    IDM_SORT_CPU => SortMode::Cpu,
                    IDM_SORT_WINDOWS => SortMode::Windows,
                    _ => SortMode::Default,
                };
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.prefs.sort_mode = new_mode;
                    state.prefs.save();
                    sort_targets(&mut state.targets, new_mode);
                    sort_protected(&mut state.protected, new_mode);
                    state.status_message = Some(format!("{}: {}", tr(I18nKey::MenuSortBy, state.active_language), new_mode.label()));
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLI_INSTALL_USER => {
                let res = CliManager::install();
                let status = match res {
                    Ok(p) => format!("CLI 已成功安装至: {}", p.display()),
                    Err(e) => format!("CLI 安装失败: {}", e),
                };
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some(status);
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLI_TEST_TERMINAL => {
                CliManager::test_in_terminal();
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some("正在终端中启动 mtc...".to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLI_REVEAL => {
                CliManager::reveal_in_explorer();
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some("正在文件资源管理器中定位 mtc.exe...".to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLI_UNINSTALL => {
                let res = CliManager::uninstall();
                let status = match res {
                    Ok(_) => "CLI 工具已从用户路径移除".to_string(),
                    Err(e) => format!("CLI 卸载失败: {}", e),
                };
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some(status);
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_OPEN_FILE => {
                let path = WhitelistManager::get_config_path();
                if !path.exists() {
                    let dir = WhitelistManager::get_config_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(&path, "# Task Cleaner Configuration\n");
                }
                let _ = std::process::Command::new("notepad.exe").arg(&path).spawn();
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some("已在记事本中打开配置文件".to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_OPEN_DIR => {
                let dir = WhitelistManager::get_config_dir();
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.status_message = Some("已在资源管理器中打开配置目录".to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_GITHUB => {
                let wverb = to_wstring("open");
                let wurl = to_wstring("https://github.com/macos-task-cleaner/windows-task-cleaner-gui");
                ShellExecuteW(0 as HWND, wverb.as_ptr(), wurl.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL as i32);
            }
            IDM_CFG_ABOUT => {
                let cur_shortcut = {
                    let state_guard = STATE.lock().unwrap();
                    state_guard
                        .as_ref()
                        .map(|s| {
                            if s.prefs.hotkey.enabled {
                                s.prefs.hotkey.display.clone()
                            } else {
                                "已禁用".to_string()
                            }
                        })
                        .unwrap_or_else(|| "Ctrl + Alt + K".to_string())
                };
                let caption = to_wstring("Task Cleaner");
                let msg = to_wstring(&format!("Task Cleaner for Windows 11\n版本: 1.0.0 (Rust Native Fluent 2.0)\n全局清理快捷键: {} (一键退出全部未保护任务)\n\n轻量优雅的一体化前台任务管理、白名单保护与内存工作集深度释放套件。\n100% 独立原生 Rust 二进制，零外部重型依赖。", cur_shortcut));
                MessageBoxW(
                    hwnd,
                    msg.as_ptr(),
                    caption.as_ptr(),
                    MB_OK | MB_ICONINFORMATION | MB_TOPMOST,
                );
            }
            IDM_LANG_AUTO => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.prefs.language_pref = LanguagePreference::Auto;
                    state.active_language = detect_system_language();
                    state.prefs.save();
                    state.status_message = Some("语言已设置为跟随系统".to_string());
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            cmd if (IDM_LANG_BASE..IDM_LANG_BASE + Language::ALL.len()).contains(&cmd) => {
                let target_lang = Language::ALL[cmd - IDM_LANG_BASE];
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.prefs.language_pref = LanguagePreference::Specific(target_lang);
                    state.active_language = target_lang;
                    state.prefs.save();
                    state.status_message = Some(format!("语言已切换为: {}", target_lang.display_name()));
                    state.status_timestamp = Some(Instant::now());
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            cmd if (IDM_KILL_TARGET_BASE..IDM_KILL_TARGET_BASE + 100).contains(&cmd) => {
                let idx = cmd - IDM_KILL_TARGET_BASE;
                let app = {
                    let state = STATE.lock().unwrap();
                    state.as_ref().and_then(|s| s.targets.get(idx).cloned())
                };
                if let Some(app) = app {
                    if is_explorer(&app.name) || is_explorer(&app.bundle_id) {
                        let mut state_guard = STATE.lock().unwrap();
                        if let Some(state) = state_guard.as_mut() {
                            state.status_message = Some("系统资源管理器严禁强制终止".to_string());
                            state.status_timestamp = Some(Instant::now());
                        }
                    } else {
                        #[cfg(windows)]
                        {
                            let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, app.pid) };
                            if !handle.is_null() {
                                unsafe {
                                    TerminateProcess(handle, 1);
                                    CloseHandle(handle);
                                }
                            }
                        }
                        let mut state_guard = STATE.lock().unwrap();
                        if let Some(state) = state_guard.as_mut() {
                            state.status_message = Some(format!("已结束: {}", app.name));
                            state.status_timestamp = Some(Instant::now());
                        }
                    }
                    refresh_scan();
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            cmd if (IDM_PROTECT_TARGET_BASE..IDM_PROTECT_TARGET_BASE + 100).contains(&cmd) => {
                let idx = cmd - IDM_PROTECT_TARGET_BASE;
                let app = {
                    let state = STATE.lock().unwrap();
                    state.as_ref().and_then(|s| s.targets.get(idx).cloned())
                };
                if let Some(app) = app {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.whitelist.protect(&app.name);
                        if let Some(exe_name) = std::path::Path::new(&app.exe_path).file_name().and_then(|n| n.to_str()) {
                            state.whitelist.protect(exe_name);
                        }
                        state.status_message = Some(format!("已将 {} 加入保护名单", app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                    refresh_scan();
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            cmd if (IDM_REVEAL_TARGET_BASE..IDM_REVEAL_TARGET_BASE + 100).contains(&cmd) => {
                let idx = cmd - IDM_REVEAL_TARGET_BASE;
                let path = {
                    let state = STATE.lock().unwrap();
                    state.as_ref().and_then(|s| s.targets.get(idx).map(|t| t.exe_path.clone()))
                };
                if let Some(path) = path {
                    if !path.is_empty() {
                        reveal_file_in_explorer(&path);
                    }
                }
            }
            cmd if (IDM_PURGE_TARGET_BASE..IDM_PURGE_TARGET_BASE + 100).contains(&cmd) => {
                let idx = cmd - IDM_PURGE_TARGET_BASE;
                let target = {
                    let state = STATE.lock().unwrap();
                    state.as_ref().and_then(|s| s.targets.get(idx).map(|t| (t.pid, t.name.clone())))
                };
                if let Some((pid, name)) = target {
                    purge_process_working_set(pid);
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some(format!("{}: {}", tr(I18nKey::ActionCleanPurge, state.active_language), name));
                        state.status_timestamp = Some(Instant::now());
                    }
                    refresh_scan_silent();
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            cmd if (IDM_UNPROTECT_BASE..IDM_UNPROTECT_BASE + 100).contains(&cmd) => {
                let idx = cmd - IDM_UNPROTECT_BASE;
                let app = {
                    let state = STATE.lock().unwrap();
                    state.as_ref().and_then(|s| s.protected.get(idx).map(|(t, _)| t.clone()))
                };
                if let Some(app) = app {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.whitelist.unprotect(&app.name);
                        if let Some(exe_name) = std::path::Path::new(&app.exe_path).file_name().and_then(|n| n.to_str()) {
                            state.whitelist.unprotect(exe_name);
                        }
                        state.status_message = Some(format!("已解除 {} 的保护状态", app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                    refresh_scan();
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            cmd if (IDM_REVEAL_PROTECTED_BASE..IDM_REVEAL_PROTECTED_BASE + 100).contains(&cmd) => {
                let idx = cmd - IDM_REVEAL_PROTECTED_BASE;
                let path = {
                    let state = STATE.lock().unwrap();
                    state.as_ref().and_then(|s| s.protected.get(idx).map(|(t, _)| t.exe_path.clone()))
                };
                if let Some(path) = path {
                    if !path.is_empty() {
                        reveal_file_in_explorer(&path);
                    }
                }
            }
            _ => {}
        }
    }

    unsafe fn handle_row_command(hwnd: HWND, cmd: usize, app: &AppTarget, _is_protected: bool) {
        let lang = STATE.lock().unwrap().as_ref().map(|s| s.active_language).unwrap_or(Language::En);
        match cmd {
            IDM_ROW_WHITELIST_ADD => {
                {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.whitelist.protect(&app.name);
                        if let Some(exe_name) = std::path::Path::new(&app.exe_path).file_name().and_then(|n| n.to_str()) {
                            state.whitelist.protect(exe_name);
                        }
                        state.status_message = Some(format!("已将 {} 加入保护名单", app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                }
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_WHITELIST_REMOVE => {
                {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.whitelist.unprotect(&app.name);
                        if let Some(exe_name) = std::path::Path::new(&app.exe_path).file_name().and_then(|n| n.to_str()) {
                            state.whitelist.unprotect(exe_name);
                        }
                        state.status_message = Some(format!("已解除 {} 的保护状态", app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                }
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_FORCE_KILL => {
                if is_explorer(&app.name) || is_explorer(&app.bundle_id) {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some("系统资源管理器严禁强制终止".to_string());
                        state.status_timestamp = Some(Instant::now());
                    }
                } else {
                    #[cfg(windows)]
                    {
                        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, app.pid) };
                        if !handle.is_null() {
                            unsafe {
                                TerminateProcess(handle, 1);
                                CloseHandle(handle);
                            }
                        }
                    }
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some(format!("已强制结束: {}", app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                }
                refresh_scan();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_PURGE_MEMORY => {
                purge_process_working_set(app.pid);
                {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some(format!("{}: {}", tr(I18nKey::ActionCleanPurge, lang), app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                }
                refresh_scan_silent();
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_REVEAL => {
                if !app.exe_path.is_empty() {
                    reveal_file_in_explorer(&app.exe_path);
                } else {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some("无法获取该进程的可执行文件路径".to_string());
                        state.status_timestamp = Some(Instant::now());
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            IDM_ROW_COPY_NAME => {
                copy_to_clipboard(hwnd, &app.name);
                {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some(format!("{}: {}", tr(I18nKey::RowCopyName, lang), app.name));
                        state.status_timestamp = Some(Instant::now());
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_COPY_PID => {
                copy_to_clipboard(hwnd, &format!("{}", app.pid));
                {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some(format!("{}: {}", tr(I18nKey::RowCopyPid, lang), app.pid));
                        state.status_timestamp = Some(Instant::now());
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_PROPERTIES => {
                if !app.exe_path.is_empty() {
                    show_file_properties(hwnd, &app.exe_path);
                } else {
                    let mut state_guard = STATE.lock().unwrap();
                    if let Some(state) = state_guard.as_mut() {
                        state.status_message = Some("无法获取该进程的可执行文件路径".to_string());
                        state.status_timestamp = Some(Instant::now());
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            }
            _ => {}
        }
    }

    unsafe fn show_tray_context_menu(hwnd: HWND) {
        refresh_scan_silent();
        let items = build_fluent_tiered_menu(hwnd);
        if items.is_empty() {
            return;
        }

        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);

        SetForegroundWindow(hwnd);
        IS_MENU_ACTIVE.store(true, Ordering::SeqCst);
        let cmd = track_fluent_menu(pt.x, pt.y, true, items, hwnd);
        IS_MENU_ACTIVE.store(false, Ordering::SeqCst);

        if let Some(cmd) = cmd {
            handle_menu_command(hwnd, cmd);
        }
    }

    unsafe fn show_sort_menu(hwnd: HWND) {
        let (cur_mode, lang) = {
            let state = STATE.lock().unwrap();
            let s = state.as_ref().unwrap();
            (s.prefs.sort_mode, s.active_language)
        };

        let items = vec![
            FluentMenuItem::radio(IDM_SORT_COMPOSITE, tr(I18nKey::SortComposite, lang), cur_mode == SortMode::Composite),
            FluentMenuItem::radio(IDM_SORT_MEMORY, tr(I18nKey::SortMemory, lang), cur_mode == SortMode::Memory),
            FluentMenuItem::radio(IDM_SORT_CPU, tr(I18nKey::SortCpu, lang), cur_mode == SortMode::Cpu),
            FluentMenuItem::radio(IDM_SORT_WINDOWS, tr(I18nKey::SortWindows, lang), cur_mode == SortMode::Windows),
            FluentMenuItem::radio(IDM_SORT_DEFAULT, tr(I18nKey::SortDefault, lang), cur_mode == SortMode::Default),
        ];

        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut pt = POINT { x: scale_dpi(260, dpi), y: scale_dpi(38, dpi) };
        ClientToScreen(hwnd, &mut pt);

        SetForegroundWindow(hwnd);
        IS_MENU_ACTIVE.store(true, Ordering::SeqCst);
        let cmd = track_fluent_menu(pt.x, pt.y, false, items, hwnd);
        IS_MENU_ACTIVE.store(false, Ordering::SeqCst);

        {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.hovered_btn = None;
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);

        if let Some(cmd) = cmd {
            handle_menu_command(hwnd, cmd);
        }
    }

    unsafe fn show_action_chevron_menu(hwnd: HWND) {
        let lang = STATE.lock().unwrap().as_ref().map(|s| s.active_language).unwrap_or(Language::En);
        let items = vec![
            FluentMenuItem::action(IDM_ACTION_GRACEFUL, tr(I18nKey::ActionCleanGraceful, lang), Some('\u{E74D}')),
            FluentMenuItem::action(IDM_ACTION_FORCE, tr(I18nKey::ActionCleanForce, lang), Some('\u{E711}')),
            FluentMenuItem::separator(),
            FluentMenuItem::action(IDM_ACTION_PURGE, tr(I18nKey::ActionCleanPurge, lang), Some('\u{E894}')),
        ];

        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut pt = POINT { x: scale_dpi(298, dpi), y: scale_dpi(124, dpi) };
        ClientToScreen(hwnd, &mut pt);

        SetForegroundWindow(hwnd);
        IS_MENU_ACTIVE.store(true, Ordering::SeqCst);
        let cmd = track_fluent_menu(pt.x, pt.y, false, items, hwnd);
        IS_MENU_ACTIVE.store(false, Ordering::SeqCst);

        {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.hovered_btn = None;
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);

        if let Some(cmd) = cmd {
            handle_menu_command(hwnd, cmd);
        }
    }

    unsafe fn show_row_more_menu(hwnd: HWND, actual_idx: usize) {
        let (app_info, lang) = {
            let state_guard = STATE.lock().unwrap();
            let state = match state_guard.as_ref() {
                Some(s) => s,
                None => return,
            };
            (get_item_at(state, actual_idx), state.active_language)
        };

        let (app, is_protected) = match app_info {
            Some(v) => v,
            None => return,
        };

        let mut items = Vec::new();
        if is_protected {
            items.push(FluentMenuItem::action(IDM_ROW_WHITELIST_REMOVE, tr(I18nKey::RowRemoveWhitelist, lang), Some('\u{E711}')));
            items.push(FluentMenuItem::action(IDM_ROW_PURGE_MEMORY, tr(I18nKey::RowPurgeMemory, lang), Some('\u{E894}')));
        } else {
            items.push(FluentMenuItem::action(IDM_ROW_WHITELIST_ADD, tr(I18nKey::RowAddWhitelist, lang), Some('\u{EA18}')));
            items.push(FluentMenuItem::action(IDM_ROW_FORCE_KILL, tr(I18nKey::RowForceKill, lang), Some('\u{E711}')));
            items.push(FluentMenuItem::action(IDM_ROW_PURGE_MEMORY, tr(I18nKey::RowPurgeMemory, lang), Some('\u{E894}')));
        }
        items.push(FluentMenuItem::separator());
        items.push(FluentMenuItem::action(IDM_ROW_REVEAL, tr(I18nKey::RowRevealInExplorer, lang), Some('\u{ED25}')));
        items.push(FluentMenuItem::action(IDM_ROW_COPY_NAME, tr(I18nKey::RowCopyName, lang), Some('\u{E8A5}')));
        items.push(FluentMenuItem::action(IDM_ROW_COPY_PID, tr(I18nKey::RowCopyPid, lang), Some('\u{E8A5}')));
        items.push(FluentMenuItem::separator());
        items.push(FluentMenuItem::action(IDM_ROW_PROPERTIES, tr(I18nKey::RowProperties, lang), Some('\u{E946}')));

        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);

        SetForegroundWindow(hwnd);
        IS_MENU_ACTIVE.store(true, Ordering::SeqCst);
        let cmd = track_fluent_menu(pt.x, pt.y, false, items, hwnd);
        IS_MENU_ACTIVE.store(false, Ordering::SeqCst);

        {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.hovered_btn = None;
                state.hovered_row = None;
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);

        if let Some(cmd) = cmd {
            handle_row_command(hwnd, cmd, &app, is_protected);
        }
    }

    unsafe fn show_settings_menu(hwnd: HWND) {
        let items = build_fluent_tiered_menu(hwnd);
        if items.is_empty() {
            return;
        }

        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut pt = POINT { x: scale_dpi(14, dpi), y: scale_dpi(444, dpi) };
        ClientToScreen(hwnd, &mut pt);

        SetForegroundWindow(hwnd);
        IS_MENU_ACTIVE.store(true, Ordering::SeqCst);
        let cmd = track_fluent_menu(pt.x, pt.y, true, items, hwnd);
        IS_MENU_ACTIVE.store(false, Ordering::SeqCst);

        {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.hovered_btn = None;
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);

        if let Some(cmd) = cmd {
            handle_menu_command(hwnd, cmd);
        }
    }

    unsafe fn show_language_menu(hwnd: HWND) {
        let (current_pref, lang) = {
            let state = STATE.lock().unwrap();
            let s = state.as_ref().unwrap();
            (s.prefs.language_pref, s.active_language)
        };

        let mut items = Vec::new();
        items.push(FluentMenuItem::radio(IDM_LANG_AUTO, tr(I18nKey::LangAuto, lang), current_pref == LanguagePreference::Auto));
        items.push(FluentMenuItem::separator());

        for (idx, &l) in Language::ALL.iter().enumerate() {
            let checked = current_pref == LanguagePreference::Specific(l);
            items.push(FluentMenuItem::radio(IDM_LANG_BASE + idx, l.display_name(), checked));
        }

        let dpi = GetDpiForWindow(hwnd).max(96);
        let mut pt = POINT { x: scale_dpi(222, dpi), y: scale_dpi(444, dpi) };
        ClientToScreen(hwnd, &mut pt);

        SetForegroundWindow(hwnd);
        IS_MENU_ACTIVE.store(true, Ordering::SeqCst);
        let cmd = track_fluent_menu(pt.x, pt.y, true, items, hwnd);
        IS_MENU_ACTIVE.store(false, Ordering::SeqCst);

        {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.hovered_btn = None;
            }
        }
        InvalidateRect(hwnd, std::ptr::null(), 0);

        if let Some(cmd) = cmd {
            handle_menu_command(hwnd, cmd);
        }
    }
    fn execute_clean_all(mode: TerminationMode) {
        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            let lang = state.active_language;
            if mode == TerminationMode::PurgeWorkingSet {
                let mut purged_count = 0;
                for t in &state.targets {
                    if purge_process_working_set(t.pid) {
                        purged_count += 1;
                    }
                }
                for (p, _) in &state.protected {
                    if purge_process_working_set(p.pid) {
                        purged_count += 1;
                    }
                }
                state.status_message = Some(format!(
                    "{} {} 个进程内存工作集",
                    tr(I18nKey::ActionCleanPurge, lang),
                    purged_count
                ));
                state.status_timestamp = Some(Instant::now());
                return;
            }

            if state.targets.is_empty() {
                state.status_message = Some("没有待结束的应用".to_string());
                state.status_timestamp = Some(Instant::now());
                return;
            }
            let report = tiered_terminate(&state.targets, mode, 400, &state.whitelist);
            state.status_message = Some(format!(
                "已结束 {} 个任务",
                report.terminated_graceful + report.terminated_force
            ));
            state.status_timestamp = Some(Instant::now());
        }
    }

    unsafe fn toggle_window(hwnd: HWND) {
        let cur = IS_VISIBLE.load(Ordering::SeqCst);
        if cur {
            ShowWindow(hwnd, SW_HIDE);
            IS_VISIBLE.store(false, Ordering::SeqCst);
        } else {
            refresh_scan();
            position_window(hwnd);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            IS_VISIBLE.store(true, Ordering::SeqCst);
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }

    /// 高精度 Windows 11 Fluent 2.0 拟真材质渲染管道
    unsafe fn draw_ui(hwnd: HWND, hdc: HDC) {
        let mut client_rect: RECT = std::mem::zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect);

        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, client_rect.right, client_rect.bottom);
        let old_bmp = SelectObject(mem_dc, mem_bmp);

        let dpi = GetDpiForWindow(hwnd).max(96);
        let s = |v: i32| scale_dpi(v, dpi);
        let s_rect = |l: i32, t: i32, r: i32, b: i32| RECT {
            left: scale_dpi(l, dpi),
            top: scale_dpi(t, dpi),
            right: scale_dpi(r, dpi),
            bottom: scale_dpi(b, dpi),
        };

        // 1. 底板画布
        let bg_brush = CreateSolidBrush(COLOR_CANVAS_BG);
        FillRect(mem_dc, &client_rect, bg_brush);
        DeleteObject(bg_brush);

        // 字体矩阵 (根据 DPI 精确点对点适配物理像素，确保 100% 锐利清晰无模糊)
        let font_title = create_font(s(15), FW_BOLD as i32);
        let font_card_title = create_font(s(13), FW_SEMIBOLD as i32);
        let font_body = create_font(s(12), FW_MEDIUM as i32);
        let font_sub = create_font(s(10), FW_NORMAL as i32);
        let font_badge = create_font(s(9), FW_SEMIBOLD as i32);

        SetBkMode(mem_dc, TRANSPARENT as i32);

        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            let hovered_btn = state.hovered_btn;
            let total_running = state.targets.len() + state.protected.len();
            let lang = state.active_language;

            // ----------------------------------------------------
            // 2. 顶栏 (Header)
            // ----------------------------------------------------
            // 标题: Task Cleaner
            SelectObject(mem_dc, font_title);
            SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
            let mut title_rect = s_rect(14, 12, 110, 36);
            let title_text = to_wstring("Task Cleaner");
            DrawTextW(mem_dc, title_text.as_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

            // 运行中胶囊徽章 (Pill Badge)
            let badge_rect = s_rect(112, 15, 190, 33);
            draw_rounded_box(mem_dc, &badge_rect, s(10), COLOR_BADGE_BG, None);
            SelectObject(mem_dc, font_badge);
            SetTextColor(mem_dc, COLOR_BADGE_TEXT);
            let mut b_text_rect = badge_rect;
            let badge_unit = match lang {
                Language::ZhHans => "运行中",
                Language::ZhHant => "運行中",
                Language::Ja => "実行中",
                _ => "Running",
            };
            let badge_text = to_wstring(&format!("{} {}", total_running, badge_unit));
            DrawTextW(mem_dc, badge_text.as_ptr(), -1, &mut b_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 排序按钮 [↕] (受偏好开关控制)
            if state.prefs.show_sort_button {
                let sort_rect = s_rect(236, 13, 258, 35);
                let is_sort_hover = hovered_btn == Some(HoverButton::SortMenu);
                if is_sort_hover {
                    draw_rounded_box(mem_dc, &sort_rect, s(6), COLOR_BTN_HOVER, None);
                }
                SelectObject(mem_dc, font_body);
                SetTextColor(mem_dc, if is_sort_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
                let mut s_rect_txt = sort_rect;
                let sort_icon = to_wstring("↕");
                DrawTextW(mem_dc, sort_icon.as_ptr(), -1, &mut s_rect_txt, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }

            // 刷新按钮 [↻]
            let ref_rect = s_rect(262, 13, 284, 35);
            let is_ref_hover = hovered_btn == Some(HoverButton::Refresh);
            if is_ref_hover {
                draw_rounded_box(mem_dc, &ref_rect, s(6), COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if is_ref_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut r_rect = ref_rect;
            let ref_icon = to_wstring("↻");
            DrawTextW(mem_dc, ref_icon.as_ptr(), -1, &mut r_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 最小化到托盘按钮 [×]
            let close_rect = s_rect(288, 13, 308, 35);
            let is_close_hover = hovered_btn == Some(HoverButton::CloseToTray);
            if is_close_hover {
                draw_rounded_box(mem_dc, &close_rect, s(6), COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if is_close_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
            let mut c_rect = close_rect;
            let close_icon = to_wstring("×");
            DrawTextW(mem_dc, close_icon.as_ptr(), -1, &mut c_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // ----------------------------------------------------
            // 3. 核心操作卡片 (Hero Action Card)
            // ----------------------------------------------------
            let card_rect = s_rect(12, 42, 308, 130);
            draw_rounded_box(mem_dc, &card_rect, s(10), COLOR_CARD_BG, Some(COLOR_CARD_BORDER));

            let has_targets = !state.targets.is_empty();

            // 卡片标题与副标题
            SelectObject(mem_dc, font_card_title);
            SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
            let mut card_title = s_rect(24, 50, 230, 70);
            let title_str = if has_targets {
                let unit = match lang {
                    Language::ZhHans => "个待结束应用",
                    Language::ZhHant => "個待結束應用",
                    Language::Ja => "件の終了対象",
                    _ => "Processes to Clean",
                };
                format!("{} {}", state.targets.len(), unit)
            } else {
                tr(I18nKey::EmptyTargetsTitle, lang).to_string()
            };
            let card_title_txt = to_wstring(&title_str);
            DrawTextW(mem_dc, card_title_txt.as_ptr(), -1, &mut card_title, DT_LEFT | DT_SINGLELINE);

            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
            let mut card_sub = s_rect(24, 70, 230, 86);
            let sub_str = if let Some(msg) = &state.status_message {
                msg.clone()
            } else if has_targets {
                match lang {
                    Language::ZhHans => "结束未受保护的前台应用",
                    Language::ZhHant => "結束未受保護的前台應用",
                    Language::Ja => "保護されていないアプリを終了",
                    _ => "Clean unprotected foreground applications",
                }
                .to_string()
            } else {
                tr(I18nKey::EmptyTargetsSubtitle, lang).to_string()
            };
            let card_sub_txt = to_wstring(&sub_str);
            DrawTextW(mem_dc, card_sub_txt.as_ptr(), -1, &mut card_sub, DT_LEFT | DT_SINGLELINE);

            // 右上角状态胶囊标签 (待处理 / 已就绪)
            let tag_rect = s_rect(242, 50, 298, 68);
            let tag_bg = if has_targets {
                COLOR_STATUS_PENDING_BG
            } else {
                COLOR_STATUS_READY_BG
            };
            let tag_fg = if has_targets {
                COLOR_STATUS_PENDING_TEXT
            } else {
                COLOR_STATUS_READY_TEXT
            };
            draw_rounded_box(mem_dc, &tag_rect, s(9), tag_bg, None);
            SelectObject(mem_dc, font_badge);
            SetTextColor(mem_dc, tag_fg);
            let mut tr_rect = tag_rect;
            let tag_txt = to_wstring(if has_targets {
                tr(I18nKey::BadgePending, lang)
            } else {
                tr(I18nKey::BadgeProtected, lang)
            });
            DrawTextW(mem_dc, tag_txt.as_ptr(), -1, &mut tr_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 核心操作按钮组: 结束大按钮 + 选项下拉箭头
            let btn_main_rect = s_rect(22, 90, if has_targets { 268 } else { 298 }, 122);
            let is_main_hover = hovered_btn == Some(HoverButton::HeroMain);
            let main_btn_bg = if !has_targets {
                COLOR_STATUS_PENDING_BG
            } else if is_main_hover {
                COLOR_HERO_BTN_HOVER
            } else {
                COLOR_HERO_BTN
            };
            draw_rounded_box(mem_dc, &btn_main_rect, s(6), main_btn_bg, None);

            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if has_targets { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
            let mut bm_text_rect = btn_main_rect;
            let btn_text = to_wstring(&format!(
                "{} {}",
                if has_targets { "[-]" } else { "[OK]" },
                if has_targets {
                    tr(I18nKey::BtnTerminate, lang)
                } else {
                    tr(I18nKey::StatusReady, lang)
                }
            ));
            DrawTextW(mem_dc, btn_text.as_ptr(), -1, &mut bm_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 下拉小箭头 (仅当有待结束任务时显示)
            if has_targets {
                let btn_chev_rect = s_rect(272, 90, 298, 122);
                let is_chev_hover = hovered_btn == Some(HoverButton::HeroChevron);
                let chev_bg = if is_chev_hover {
                    COLOR_HERO_BTN_HOVER
                } else {
                    COLOR_HERO_BTN
                };
                draw_rounded_box(mem_dc, &btn_chev_rect, s(6), chev_bg, None);
                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
                let mut bc_text_rect = btn_chev_rect;
                let chev_txt = to_wstring("˅");
                DrawTextW(mem_dc, chev_txt.as_ptr(), -1, &mut bc_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }

            // ----------------------------------------------------
            // 4. 分段选择器 (Segmented Tab Bar)
            // ----------------------------------------------------
            let tab_container = s_rect(12, 138, 308, 166);
            draw_rounded_box(mem_dc, &tab_container, s(8), COLOR_TAB_BG, None);

            let tab_items = [
                (tr(I18nKey::TabTargets, lang), state.targets.len()),
                (tr(I18nKey::TabProtected, lang), state.protected.len()),
                (tr(I18nKey::TabAll, lang), total_running),
            ];

            let tab_w = (296 - 4) / 3;
            for (idx, (tab_title, count)) in tab_items.iter().enumerate() {
                let tx = 14 + (idx as i32 * tab_w);
                let tab_rect = s_rect(tx, 140, tx + tab_w - 2, 164);
                let is_active = state.active_tab == idx;
                let is_tab_hover = hovered_btn == Some(HoverButton::Tab(idx));

                if is_active {
                    draw_rounded_box(mem_dc, &tab_rect, s(6), COLOR_TAB_ACTIVE, None);
                } else if is_tab_hover {
                    draw_rounded_box(mem_dc, &tab_rect, s(6), COLOR_BTN_HOVER, None);
                }

                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, if is_active { rgb(255, 255, 255) } else { COLOR_TAB_INACTIVE_TEXT });
                let mut t_text_rect = RECT {
                    left: tab_rect.left + s(4),
                    top: tab_rect.top,
                    right: tab_rect.right - s(24),
                    bottom: tab_rect.bottom,
                };
                let tw = to_wstring(tab_title);
                DrawTextW(mem_dc, tw.as_ptr(), -1, &mut t_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                let badge_pill = RECT {
                    left: tab_rect.right - s(22),
                    top: tab_rect.top + s(4),
                    right: tab_rect.right - s(4),
                    bottom: tab_rect.bottom - s(4),
                };
                let pill_bg = if is_active {
                    rgb(30, 144, 255)
                } else {
                    COLOR_TAB_BADGE_INACTIVE
                };
                draw_rounded_box(mem_dc, &badge_pill, s(6), pill_bg, None);
                SelectObject(mem_dc, font_badge);
                SetTextColor(mem_dc, if is_active { rgb(255, 255, 255) } else { COLOR_TAB_INACTIVE_TEXT });
                let mut bp_rect = badge_pill;
                let count_txt = to_wstring(&count.to_string());
                DrawTextW(mem_dc, count_txt.as_ptr(), -1, &mut bp_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }

            // ----------------------------------------------------
            // 5. 应用列表区 (List Container Card)
            // ----------------------------------------------------
            let list_container = s_rect(12, 174, 308, 434);
            draw_rounded_box(mem_dc, &list_container, s(10), COLOR_CARD_BG, Some(COLOR_CARD_BORDER));

            let items: Vec<(&AppTarget, bool)> = match state.active_tab {
                1 => state.protected.iter().map(|(a, _)| (a, true)).collect(),
                2 => state
                    .targets
                    .iter()
                    .map(|a| (a, false))
                    .chain(state.protected.iter().map(|(a, _)| (a, true)))
                    .collect(),
                _ => state.targets.iter().map(|a| (a, false)).collect(),
            };

            if items.is_empty() {
                SelectObject(mem_dc, font_title);
                SetTextColor(mem_dc, COLOR_ACCENT_BLUE);
                let mut check_rect = s_rect(12, 220, 308, 250);
                let check_txt = to_wstring("[OK]");
                DrawTextW(mem_dc, check_txt.as_ptr(), -1, &mut check_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                SelectObject(mem_dc, font_card_title);
                SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
                let mut empty_title = s_rect(12, 255, 308, 275);
                let empty_title_txt = to_wstring(tr(I18nKey::EmptyTargetsTitle, lang));
                DrawTextW(mem_dc, empty_title_txt.as_ptr(), -1, &mut empty_title, DT_CENTER | DT_SINGLELINE);

                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
                let mut empty_sub = s_rect(12, 278, 308, 295);
                let empty_sub_txt = to_wstring(tr(I18nKey::EmptyTargetsSubtitle, lang));
                DrawTextW(mem_dc, empty_sub_txt.as_ptr(), -1, &mut empty_sub, DT_CENTER | DT_SINGLELINE);

                let view_all_rect = s_rect(80, 310, 240, 334);
                let is_va_hover = hovered_btn == Some(HoverButton::ViewAllFromEmpty);
                draw_rounded_box(
                    mem_dc,
                    &view_all_rect,
                    s(12),
                    if is_va_hover { COLOR_HERO_BTN_HOVER } else { COLOR_HERO_BTN },
                    None,
                );
                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
                let mut va_text_rect = view_all_rect;
                let va_txt = to_wstring(&format!("{} ({})", tr(I18nKey::BtnViewAll, lang), total_running));
                DrawTextW(mem_dc, va_txt.as_ptr(), -1, &mut va_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            } else {
                let scroll = state.scroll_offset;
                let row_height = s(ROW_HEIGHT);
                let icon_size = s(24);

                for i in 0..VISIBLE_ROWS {
                    let actual_idx = scroll + i;
                    if actual_idx >= items.len() {
                        break;
                    }
                    let (app, is_protected) = items[actual_idx];
                    let y = s(175) + (i as i32 * row_height);
                    let row_rect = RECT {
                        left: s(14),
                        top: y,
                        right: s(306),
                        bottom: y + row_height,
                    };

                    let is_row_hover = state.hovered_row == Some(i);
                    if is_row_hover {
                        let r_bg = CreateSolidBrush(COLOR_CARD_HOVER);
                        FillRect(mem_dc, &row_rect, r_bg);
                        DeleteObject(r_bg);
                    }

                    // 1. 真实高清应用图标 (指定当前 DPI 目标物理像素，原生清晰渲染)
                    let icon_opt = get_app_icon(&app.exe_path, icon_size, &mut state.icon_cache);
                    if let Some(h_icon) = icon_opt {
                        DrawIconEx(mem_dc, s(22), y + s(8), h_icon, icon_size, icon_size, 0, 0 as HBRUSH, DI_NORMAL);
                    } else {
                        let def_rect = RECT {
                            left: s(22),
                            top: y + s(8),
                            right: s(22) + icon_size,
                            bottom: y + s(8) + icon_size,
                        };
                        draw_rounded_box(mem_dc, &def_rect, s(5), COLOR_STATUS_PENDING_BG, None);
                    }

                    // 2. 应用友好主名称
                    SelectObject(mem_dc, font_body);
                    SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
                    let mut name_rect = RECT {
                        left: s(52),
                        top: y + s(4),
                        right: s(248),
                        bottom: y + s(22),
                    };
                    let friendly_name = resolve_friendly_name(app);
                    let name_txt = to_wstring(&friendly_name);
                    DrawTextW(mem_dc, name_txt.as_ptr(), -1, &mut name_rect, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE | DT_END_ELLIPSIS);

                    // 3. 详细遥测指标
                    SelectObject(mem_dc, font_sub);
                    SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
                    let mut sub_rect = RECT {
                        left: s(52),
                        top: y + s(22),
                        right: s(248),
                        bottom: y + s(38),
                    };

                    let win_unit = tr(I18nKey::UnitWindows, lang);
                    let sub_str = if state.prefs.show_detailed_metrics && state.prefs.show_app_identifier {
                        format!(
                            "{} · {:.0} MB · {:.1}% · {} {}",
                            app.name,
                            app.memory_mb(),
                            app.cpu_percent,
                            app.window_count,
                            win_unit
                        )
                    } else if state.prefs.show_detailed_metrics {
                        format!(
                            "{:.0} MB · {:.1}% · {} {}",
                            app.memory_mb(),
                            app.cpu_percent,
                            app.window_count,
                            win_unit
                        )
                    } else if state.prefs.show_app_identifier {
                        format!("PID: {} · {}", app.pid, app.name)
                    } else {
                        format!("PID: {}", app.pid)
                    };
                    let sub_txt = to_wstring(&sub_str);
                    DrawTextW(mem_dc, sub_txt.as_ptr(), -1, &mut sub_rect, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE | DT_END_ELLIPSIS);

                    // 4. 右侧操作按钮组
                    if !is_protected {
                        let trash_rect = RECT {
                            left: s(256),
                            top: y + s(10),
                            right: s(278),
                            bottom: y + s(32),
                        };
                        let is_trash_hover = hovered_btn == Some(HoverButton::RowTrash(i));
                        if is_trash_hover {
                            draw_rounded_box(mem_dc, &trash_rect, s(4), COLOR_BTN_HOVER, None);
                        }
                        SelectObject(mem_dc, font_body);
                        SetTextColor(mem_dc, if is_trash_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
                        let mut tr_text = trash_rect;
                        let t_icon = to_wstring("×");
                        DrawTextW(mem_dc, t_icon.as_ptr(), -1, &mut tr_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                    }

                    // 拓展操作按钮 (竖三点 ⋮)
                    let more_rect = RECT {
                        left: s(282),
                        top: y + s(10),
                        right: s(304),
                        bottom: y + s(32),
                    };
                    let is_more_hover = hovered_btn == Some(HoverButton::RowMore(i));
                    if is_more_hover {
                        draw_rounded_box(mem_dc, &more_rect, s(4), COLOR_BTN_HOVER, None);
                    }
                    SelectObject(mem_dc, font_body);
                    SetTextColor(mem_dc, if is_more_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
                    let mut mr_text = more_rect;
                    let m_icon = to_wstring("⋮");
                    DrawTextW(mem_dc, m_icon.as_ptr(), -1, &mut mr_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                    // 行分割线
                    if i < VISIBLE_ROWS - 1 && actual_idx < items.len() - 1 {
                        let sep_rect = RECT {
                            left: s(52),
                            top: y + row_height - 1,
                            right: s(300),
                            bottom: y + row_height,
                        };
                        let s_brush = CreateSolidBrush(COLOR_ROW_SEP);
                        FillRect(mem_dc, &sep_rect, s_brush);
                        DeleteObject(s_brush);
                    }
                }

                // 滚动条指示器
                if items.len() > VISIBLE_ROWS {
                    let total = items.len() as f32;
                    let track_h = s(240) as f32;
                    let thumb_h = (VISIBLE_ROWS as f32 / total * track_h).max(s(20) as f32);
                    let thumb_y = s(180) as f32
                        + (scroll as f32 / (total - VISIBLE_ROWS as f32) * (track_h - thumb_h));
                    let thumb_rect = RECT {
                        left: s(304),
                        top: thumb_y as i32,
                        right: s(307),
                        bottom: (thumb_y + thumb_h) as i32,
                    };
                    draw_rounded_box(mem_dc, &thumb_rect, s(2), COLOR_BADGE_BG, None);
                }
            }

            // ----------------------------------------------------
            // 6. 底栏 (Footer Toolbar)
            // ----------------------------------------------------
            let sep_line = s_rect(12, 442, 308, 443);
            let foot_brush = CreateSolidBrush(COLOR_CARD_BORDER);
            FillRect(mem_dc, &sep_line, foot_brush);
            DeleteObject(foot_brush);

            // 配置项按钮 (左侧)
            let cfg_rect = s_rect(14, 448, 90, 472);
            let is_cfg_hover = hovered_btn == Some(HoverButton::Settings);
            if is_cfg_hover {
                draw_rounded_box(mem_dc, &cfg_rect, s(4), COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, if is_cfg_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut cr = cfg_rect;
            let cfg_txt = to_wstring(tr(I18nKey::BtnSettings, lang));
            DrawTextW(mem_dc, cfg_txt.as_ptr(), -1, &mut cr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 语言切换按钮 (中间偏右 [文/A])
            let lang_rect = s_rect(196, 448, 248, 472);
            let is_lang_hover = hovered_btn == Some(HoverButton::Language);
            if is_lang_hover {
                draw_rounded_box(mem_dc, &lang_rect, s(4), COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, if is_lang_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut lr = lang_rect;
            let lang_txt = to_wstring("[文/A]");
            DrawTextW(mem_dc, lang_txt.as_ptr(), -1, &mut lr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 退出按钮 (右侧)
            let quit_rect = s_rect(254, 448, 306, 472);
            let is_q_hover = hovered_btn == Some(HoverButton::Quit);
            if is_q_hover {
                draw_rounded_box(mem_dc, &quit_rect, s(4), COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, if is_q_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut qr = quit_rect;
            let quit_txt = to_wstring(tr(I18nKey::BtnQuit, lang));
            DrawTextW(mem_dc, quit_txt.as_ptr(), -1, &mut qr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        }

        BitBlt(hdc, 0, 0, client_rect.right, client_rect.bottom, mem_dc, 0, 0, SRCCOPY);

        SelectObject(mem_dc, old_bmp);
        DeleteObject(mem_bmp);
        DeleteDC(mem_dc);

        DeleteObject(font_title);
        DeleteObject(font_card_title);
        DeleteObject(font_body);
        DeleteObject(font_sub);
        DeleteObject(font_badge);
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAYICON => {
                let event = (lparam & 0xFFFF) as u32;
                match event {
                    WM_LBUTTONUP | WM_RBUTTONUP => {
                        show_tray_context_menu(hwnd);
                    }
                    0x0203 /* WM_LBUTTONDBLCLK */ => {
                        toggle_window(hwnd);
                    }
                    _ => {}
                }
                0
            }
            WM_TIMER => {
                if wparam == TIMER_HEARTBEAT_ID as usize {
                    let is_vis = IS_VISIBLE.load(Ordering::Relaxed);
                    let is_menu = IS_MENU_ACTIVE.load(Ordering::Relaxed);
                    if is_vis && !is_menu {
                        refresh_scan_silent();
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
                0
            }
            WM_HOTKEY => {
                if (wparam as i32) == HOTKEY_TOGGLE_ID {
                    MessageBeep(0);
                    refresh_scan();
                    execute_clean_all(TerminationMode::Standard);
                    refresh_scan();
                    let tip_text = {
                        let state = STATE.lock().unwrap();
                        state
                            .as_ref()
                            .and_then(|s| s.status_message.clone())
                            .unwrap_or_else(|| "Task Cleaner (Windows 11)".to_string())
                    };
                    update_tray_tooltip(hwnd, &format!("Task Cleaner - {}", tip_text));
                    if IS_VISIBLE.load(Ordering::SeqCst) {
                        InvalidateRect(hwnd, std::ptr::null(), 1);
                    }
                }
                0
            }
            WM_SETTINGCHANGE => {
                update_tray_icon(hwnd);
                0
            }
            WM_DPICHANGED => {
                let new_rect = lparam as *const RECT;
                if !new_rect.is_null() {
                    let r = *new_rect;
                    SetWindowPos(
                        hwnd,
                        0 as HWND,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                update_tray_icon(hwnd);
                InvalidateRect(hwnd, std::ptr::null(), 1);
                0
            }
            WM_ACTIVATE => {
                let activation = (wparam & 0xFFFF) as u32;
                if activation == 0 /* WA_INACTIVE */ {
                    if !IS_MENU_ACTIVE.load(Ordering::SeqCst) && IS_VISIBLE.load(Ordering::SeqCst) {
                        ShowWindow(hwnd, SW_HIDE);
                        IS_VISIBLE.store(false, Ordering::SeqCst);
                    }
                }
                0
            }
            WM_MOUSEMOVE => {
                let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
                tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
                tme.dwFlags = TME_LEAVE;
                tme.hwndTrack = hwnd;
                TrackMouseEvent(&mut tme);

                let raw_x = (lparam & 0xFFFF) as i32;
                let raw_y = ((lparam >> 16) & 0xFFFF) as i32;
                let dpi = GetDpiForWindow(hwnd).max(96);
                let x = unscale_dpi(raw_x, dpi);
                let y = unscale_dpi(raw_y, dpi);

                let mut new_btn = None;
                let mut new_row = None;

                let show_sort = STATE
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|s| s.prefs.show_sort_button)
                    .unwrap_or(true);

                // 顶栏按钮
                if show_sort && x >= 236 && x <= 258 && y >= 13 && y <= 35 {
                    new_btn = Some(HoverButton::SortMenu);
                } else if x >= 262 && x <= 284 && y >= 13 && y <= 35 {
                    new_btn = Some(HoverButton::Refresh);
                } else if x >= 288 && x <= 308 && y >= 13 && y <= 35 {
                    new_btn = Some(HoverButton::CloseToTray);
                }
                // 核心卡片按钮
                else if y >= 90 && y <= 122 {
                    if x >= 22 && x <= 268 {
                        new_btn = Some(HoverButton::HeroMain);
                    } else if x >= 272 && x <= 298 {
                        new_btn = Some(HoverButton::HeroChevron);
                    }
                }
                // 分段选择器 Tab 切换
                else if y >= 138 && y <= 166 {
                    let tab_w = (296 - 4) / 3;
                    for idx in 0..3 {
                        let tx = 14 + (idx as i32 * tab_w);
                        if x >= tx && x <= tx + tab_w - 2 {
                            new_btn = Some(HoverButton::Tab(idx));
                            break;
                        }
                    }
                }
                // 应用列表区
                else if y >= 175 && y <= 175 + (VISIBLE_ROWS as i32 * ROW_HEIGHT) {
                    let row_idx = ((y - 175) / ROW_HEIGHT) as usize;
                    new_row = Some(row_idx);
                    if x >= 256 && x <= 278 {
                        new_btn = Some(HoverButton::RowTrash(row_idx));
                    } else if x >= 282 && x <= 304 {
                        new_btn = Some(HoverButton::RowMore(row_idx));
                    }
                }
                // 空状态查看全部按钮
                else if x >= 80 && x <= 240 && y >= 310 && y <= 334 {
                    new_btn = Some(HoverButton::ViewAllFromEmpty);
                }
                // 底栏按钮
                else if y >= 448 && y <= 472 {
                    if x >= 14 && x <= 90 {
                        new_btn = Some(HoverButton::Settings);
                    } else if x >= 196 && x <= 248 {
                        new_btn = Some(HoverButton::Language);
                    } else if x >= 254 && x <= 306 {
                        new_btn = Some(HoverButton::Quit);
                    }
                }

                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    if state.hovered_btn != new_btn || state.hovered_row != new_row {
                        state.hovered_btn = new_btn;
                        state.hovered_row = new_row;
                        drop(state_guard);
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
                0
            }
            WM_MOUSELEAVE => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    if state.hovered_btn.is_some() || state.hovered_row.is_some() {
                        state.hovered_btn = None;
                        state.hovered_row = None;
                        drop(state_guard);
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
                0
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam >> 16) & 0xFFFF) as i16;
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    let total_items = match state.active_tab {
                        1 => state.protected.len(),
                        2 => state.targets.len() + state.protected.len(),
                        _ => state.targets.len(),
                    };
                    let max_offset = total_items.saturating_sub(VISIBLE_ROWS);
                    if delta > 0 {
                        state.scroll_offset = state.scroll_offset.saturating_sub(1);
                    } else if delta < 0 {
                        state.scroll_offset = (state.scroll_offset + 1).min(max_offset);
                    }
                    drop(state_guard);
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                0
            }
            WM_RBUTTONUP => {
                let raw_y = ((lparam >> 16) & 0xFFFF) as i32;
                let dpi = GetDpiForWindow(hwnd).max(96);
                let y = unscale_dpi(raw_y, dpi);
                if y >= 175 && y <= 175 + (VISIBLE_ROWS as i32 * ROW_HEIGHT) {
                    let visible_idx = ((y - 175) / ROW_HEIGHT) as usize;
                    let scroll = {
                        let state = STATE.lock().unwrap();
                        state.as_ref().map(|s| s.scroll_offset).unwrap_or(0)
                    };
                    let actual_idx = scroll + visible_idx;
                    show_row_more_menu(hwnd, actual_idx);
                    return 0;
                }
                0
            }
            WM_LBUTTONUP => {
                let raw_x = (lparam & 0xFFFF) as i32;
                let raw_y = ((lparam >> 16) & 0xFFFF) as i32;
                let dpi = GetDpiForWindow(hwnd).max(96);
                let x = unscale_dpi(raw_x, dpi);
                let y = unscale_dpi(raw_y, dpi);

                let show_sort = STATE
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|s| s.prefs.show_sort_button)
                    .unwrap_or(true);

                // 1. 顶栏操作
                if show_sort && x >= 236 && x <= 258 && y >= 13 && y <= 35 {
                    show_sort_menu(hwnd);
                    return 0;
                }
                if x >= 262 && x <= 284 && y >= 13 && y <= 35 {
                    refresh_scan();
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                    return 0;
                }
                if x >= 288 && x <= 308 && y >= 13 && y <= 35 {
                    ShowWindow(hwnd, SW_HIDE);
                    IS_VISIBLE.store(false, Ordering::SeqCst);
                    return 0;
                }

                // 2. 核心卡片操作
                if y >= 90 && y <= 122 {
                    if x >= 22 && x <= 268 {
                        execute_clean_all(TerminationMode::Standard);
                        refresh_scan();
                        InvalidateRect(hwnd, std::ptr::null(), 1);
                        return 0;
                    } else if x >= 272 && x <= 298 {
                        show_action_chevron_menu(hwnd);
                        return 0;
                    }
                }

                // 3. 分段选择器 Tab 切换
                if y >= 138 && y <= 166 {
                    let tab_w = (296 - 4) / 3;
                    for idx in 0..3 {
                        let tx = 14 + (idx as i32 * tab_w);
                        if x >= tx && x <= tx + tab_w - 2 {
                            if let Some(state) = STATE.lock().unwrap().as_mut() {
                                state.active_tab = idx;
                                state.scroll_offset = 0;
                            }
                            InvalidateRect(hwnd, std::ptr::null(), 1);
                            return 0;
                        }
                    }
                }

                // 4. 空状态“查看全部活动”按钮
                if x >= 80 && x <= 240 && y >= 310 && y <= 334 {
                    if let Some(state) = STATE.lock().unwrap().as_mut() {
                        state.active_tab = 2;
                        state.scroll_offset = 0;
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                    return 0;
                }

                // 5. 应用列表行操作
                if y >= 175 && y <= 175 + (VISIBLE_ROWS as i32 * ROW_HEIGHT) {
                    let visible_idx = ((y - 175) / ROW_HEIGHT) as usize;
                    let scroll = {
                        let state = STATE.lock().unwrap();
                        state.as_ref().map(|s| s.scroll_offset).unwrap_or(0)
                    };
                    let actual_idx = scroll + visible_idx;

                    // 单项垃圾桶结束
                    if x >= 256 && x <= 278 {
                        let item_to_kill = {
                            let state_guard = STATE.lock().unwrap();
                            if let Some(state) = state_guard.as_ref() {
                                get_item_at(state, actual_idx)
                            } else {
                                None
                            }
                        };
                        if let Some((app, is_prot)) = item_to_kill {
                            if !is_prot {
                                if is_explorer(&app.name) || is_explorer(&app.bundle_id) {
                                    let mut state_guard = STATE.lock().unwrap();
                                    if let Some(state) = state_guard.as_mut() {
                                        state.status_message = Some("系统资源管理器严禁终止".to_string());
                                        state.status_timestamp = Some(Instant::now());
                                    }
                                } else {
                                    let whitelist = {
                                        let state_guard = STATE.lock().unwrap();
                                        state_guard.as_ref().map(|s| s.whitelist.clone()).unwrap_or_else(WhitelistManager::new)
                                    };
                                    tiered_terminate(&[app], TerminationMode::Standard, 400, &whitelist);
                                    refresh_scan();
                                }
                                InvalidateRect(hwnd, std::ptr::null(), 1);
                                return 0;
                            }
                        }
                    }

                    // 竖三点更多菜单
                    if x >= 282 && x <= 304 {
                        show_row_more_menu(hwnd, actual_idx);
                        return 0;
                    }
                }

                // 6. 底栏操作
                if y >= 448 && y <= 472 {
                    if x >= 14 && x <= 90 {
                        show_settings_menu(hwnd);
                        return 0;
                    } else if x >= 196 && x <= 248 {
                        show_language_menu(hwnd);
                        return 0;
                    } else if x >= 254 && x <= 306 {
                        PostQuitMessage(0);
                        return 0;
                    }
                }

                0
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                draw_ui(hwnd, hdc);
                EndPaint(hwnd, &ps);
                0
            }
            WM_ERASEBKGND => 1,
            WM_DESTROY => {
                KillTimer(hwnd, TIMER_HEARTBEAT_ID);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    pub fn run_gui() {
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
        }
        let h_instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let class_name = to_wstring("TaskCleanerTrayWindow");

        let mutex_name = to_wstring("TaskCleaner_Win32_SingleInstance_Mutex_2026");
        let h_mutex = unsafe {
            CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr())
        };
        let last_err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        if last_err == 183 /* ERROR_ALREADY_EXISTS */ {
            let existing_hwnd = unsafe {
                FindWindowW(class_name.as_ptr(), std::ptr::null())
            };
            if existing_hwnd != 0 as HWND {
                unsafe {
                    ShowWindow(existing_hwnd, SW_SHOW);
                    SetForegroundWindow(existing_hwnd);
                }
            }
            if h_mutex != 0 as HANDLE {
                unsafe { CloseHandle(h_mutex); }
            }
            return;
        }

        let h_app_icon = unsafe { LoadIconW(h_instance, 1 as *const u16) };

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: h_app_icon,
            hCursor: unsafe { LoadCursorW(0 as HINSTANCE, IDC_ARROW) },
            hbrBackground: 0 as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: h_app_icon,
        };

        unsafe {
            RegisterClassExW(&wc);
        }

        let dpi = unsafe { GetDpiForSystem().max(96) };
        let win_w = scale_dpi(WINDOW_WIDTH, dpi);
        let win_h = scale_dpi(WINDOW_HEIGHT, dpi);

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                to_wstring("Task Cleaner").as_ptr(),
                WS_POPUP,
                100,
                100,
                win_w,
                win_h,
                0 as HWND,
                0 as HMENU,
                h_instance,
                std::ptr::null(),
            )
        };

        // 启用 Windows 11 原生圆角 (DWMWCP_ROUND)
        let corner_preference = DWMWCP_ROUND;
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &corner_preference as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        // 启用 2 秒心跳后台监控定时器 (失焦静默刷新)
        unsafe {
            SetTimer(hwnd, TIMER_HEARTBEAT_ID, 2000, None);
        }

        // 初始化托盘图标
        let h_icon = unsafe { create_default_tray_icon() };
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = h_icon;

        let tip = to_wstring("Task Cleaner (Windows 11)");
        let max_len = 127.min(tip.len());
        for i in 0..max_len {
            nid.szTip[i] = tip[i];
        }
        nid.szTip[max_len] = 0;

        unsafe {
            Shell_NotifyIconW(NIM_ADD, &nid);
        }

        let prefs = GuiPreferences::load();
        apply_hotkey(hwnd, &prefs.hotkey);
        let active_lang = prefs.language_pref.resolved_language();

        {
            let mut state = STATE.lock().unwrap();
            *state = Some(GuiState {
                whitelist: WhitelistManager::new(),
                targets: Vec::new(),
                protected: Vec::new(),
                prefs,
                active_language: active_lang,
                active_tab: 0,
                scroll_offset: 0,
                hovered_row: None,
                hovered_btn: None,
                icon_cache: HashMap::new(),
                status_message: None,
                status_timestamp: None,
                last_scan: Instant::now(),
            });
        }

        refresh_scan();

        // 启动时在屏幕右下角打开并前置展示主面板
        unsafe {
            position_window(hwnd);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            IS_VISIBLE.store(true, Ordering::SeqCst);
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }

        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { GetMessageW(&mut msg, 0 as HWND, 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe {
            UnregisterHotKey(hwnd, HOTKEY_TOGGLE_ID);
            Shell_NotifyIconW(NIM_DELETE, &nid);
            DestroyIcon(h_icon);
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                for &h in state.icon_cache.values() {
                    if h != 0 {
                        DestroyIcon(h as HICON);
                    }
                }
            }
            CoUninitialize();
            if h_mutex != 0 as HANDLE {
                CloseHandle(h_mutex);
            }
        }
    }
}

#[cfg(windows)]
fn main() {
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    win_gui::run_gui();
}
