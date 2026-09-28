#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    println!("Task Cleaner GUI is designed for Windows 11. Run on Windows to launch tray application.");
}

#[cfg(windows)]
mod win_gui {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::time::Instant;

    use task_cleaner_core::{
        get_caller_lineage, scan_foreground_apps, sort_targets, tiered_terminate, AppTarget,
        SortMode, TerminationMode, WhitelistManager, WhitelistMatch,
    };
    use windows_sys::Win32::Foundation::{
        COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
    };
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, BitBlt, CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
        CreatePen, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
        GetDC, GetStockObject, ReleaseDC, RoundRect, SelectObject, SetBkMode, SetTextColor,
        BLACK_BRUSH, CLEARTYPE_QUALITY, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
        DT_SINGLELINE, DT_VCENTER, FW_BOLD, FW_MEDIUM, FW_NORMAL, FW_SEMIBOLD, HBRUSH, HDC,
        HFONT, PAINTSTRUCT, PS_NULL, PS_SOLID, SRCCOPY, TRANSPARENT,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    };
    use windows_sys::Win32::UI::Shell::{
        ExtractIconExW, Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE,
        NIF_TIP, NIM_ADD, NIM_DELETE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DispatchMessageW, DrawIconEx, GetCursorPos, GetMessageW,
        GetSystemMetrics, LoadCursorW, ICONINFO, IDC_ARROW, MF_CHECKED, MF_SEPARATOR,
        MF_STRING, MSG, PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetWindowPos,
        ShowWindow, SystemParametersInfoW, TrackPopupMenuEx, TranslateMessage, CS_DROPSHADOW,
        DI_NORMAL, HICON, HMENU, HWND_TOPMOST, SM_CXSMICON, SM_CYSMICON, SPI_GETWORKAREA,
        SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, TPM_BOTTOMALIGN,
        TPM_LEFTALIGN, TPM_RETURNCMD, WM_ACTIVATE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONUP,
        WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_USER, WNDCLASSEXW, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    };

    const WM_TRAYICON: u32 = WM_USER + 101;
    const WM_MOUSELEAVE: u32 = 0x02A3;

    // 菜单 ID 定义
    const IDM_OPEN: usize = 1001;
    const IDM_REFRESH: usize = 1002;
    const IDM_CLEAN_ALL: usize = 1003;
    const IDM_QUIT: usize = 1005;

    const IDM_SORT_COMPOSITE: usize = 1101;
    const IDM_SORT_MEMORY: usize = 1102;
    const IDM_SORT_CPU: usize = 1103;
    const IDM_SORT_WINDOWS: usize = 1104;
    const IDM_SORT_DEFAULT: usize = 1105;

    const IDM_ACTION_GRACEFUL: usize = 1201;
    const IDM_ACTION_FORCE: usize = 1202;

    const IDM_ROW_WHITELIST_ADD: usize = 1301;
    const IDM_ROW_WHITELIST_REMOVE: usize = 1302;
    const IDM_ROW_REVEAL: usize = 1303;
    const IDM_ROW_COPY_NAME: usize = 1304;
    const IDM_ROW_COPY_PID: usize = 1305;
    const IDM_ROW_FORCE_KILL: usize = 1306;

    const IDM_CFG_STARTUP: usize = 1401;
    const IDM_CFG_RELOAD: usize = 1402;
    const IDM_CFG_ABOUT: usize = 1403;

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

    struct GuiState {
        whitelist: WhitelistManager,
        targets: Vec<AppTarget>,
        protected: Vec<(AppTarget, WhitelistMatch)>,
        sort_mode: SortMode,
        active_tab: usize, // 0: 待结束, 1: 已保护, 2: 全部活动
        scroll_offset: usize,
        hovered_row: Option<usize>,
        hovered_btn: Option<HoverButton>,
        icon_cache: HashMap<String, isize>,
        status_message: Option<String>,
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
        Quit,
    }

    static STATE: Mutex<Option<GuiState>> = Mutex::new(None);

    fn to_wstring(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// 应用名本地化与友好解析 (告别裸露的 .exe 粗糙毛胚感)
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
                } else if let Some(stem) = std::path::Path::new(&app.name).file_stem().and_then(|s| s.to_str()) {
                    stem.to_string()
                } else {
                    app.name.clone()
                }
            }
        }
    }

    /// 提取真实高分辨率应用图标 (Win32 Shell ExtractIconEx)
    unsafe fn get_app_icon(exe_path: &str, cache: &mut HashMap<String, isize>) -> Option<HICON> {
        if exe_path.is_empty() {
            return None;
        }

        if let Some(&h) = cache.get(exe_path) {
            return if h != 0 { Some(h as HICON) } else { None };
        }

        let wpath = to_wstring(exe_path);
        let mut h_small: HICON = 0 as HICON;
        let count = ExtractIconExW(wpath.as_ptr(), 0, std::ptr::null_mut(), &mut h_small, 1);

        if count > 0 && h_small != 0 as HICON {
            cache.insert(exe_path.to_string(), h_small as isize);
            Some(h_small)
        } else {
            cache.insert(exe_path.to_string(), 0);
            None
        }
    }

    /// 动态合成现代极简胶囊 X 几何托盘图标 (无外部资源依赖，100% 独立稳定)
    unsafe fn create_default_tray_icon() -> HICON {
        let cx = GetSystemMetrics(SM_CXSMICON).max(16);
        let cy = GetSystemMetrics(SM_CYSMICON).max(16);
        let screen_dc = GetDC(0 as HWND);
        let hdc = CreateCompatibleDC(screen_dc);

        let hbm_color = CreateCompatibleBitmap(screen_dc, cx, cy);
        let hbm_mask = CreateBitmap(cx, cy, 1, 1, std::ptr::null());

        ReleaseDC(0 as HWND, screen_dc);

        // 1. 绘制彩色底板与标志 (Windows 经典 Accent 蓝底)
        let h_old = SelectObject(hdc, hbm_color);
        let bg_brush = CreateSolidBrush(COLOR_ACCENT_BLUE);
        let rect = RECT { left: 0, top: 0, right: cx, bottom: cy };
        FillRect(hdc, &rect, bg_brush);
        DeleteObject(bg_brush);

        let fg_brush = CreateSolidBrush(rgb(255, 255, 255));
        let pad = cx / 4;
        let inner_rect = RECT {
            left: pad,
            top: pad,
            right: cx - pad,
            bottom: cy - pad,
        };
        FillRect(hdc, &inner_rect, fg_brush);
        DeleteObject(fg_brush);

        SelectObject(hdc, h_old);

        // 2. 初始化 Mask 蒙版为全黑 (0 = 100% 不透明，杜绝噪点棋盘杂色)
        let mask_old = SelectObject(hdc, hbm_mask);
        let black_brush = GetStockObject(BLACK_BRUSH as i32);
        FillRect(hdc, &rect, black_brush as HBRUSH);
        SelectObject(hdc, mask_old);

        DeleteDC(hdc);

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

    /// 高精度圆角矩形渲染助手 (彻底杜绝 GDI 默认粗黑边框，打造 Apple/Fluent 2.0 质感)
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

            sort_targets(&mut targets, state.sort_mode);

            state.targets = targets;
            state.protected = protected;
            state.scroll_offset = 0;
            state.last_scan = Instant::now();
        }
    }

    /// 自动将窗口精确吸附在屏幕右下角任务栏正上方 (支持任意分辨率和任务栏位置)
    unsafe fn position_window(hwnd: HWND) {
        let mut work_area: RECT = std::mem::zeroed();
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut work_area as *mut _ as *mut _,
            0,
        );

        let screen_w = work_area.right - work_area.left;
        let screen_h = work_area.bottom - work_area.top;

        let x = if screen_w > WINDOW_WIDTH {
            work_area.right - WINDOW_WIDTH - 16
        } else {
            work_area.left
        };

        let y = if screen_h > WINDOW_HEIGHT {
            work_area.bottom - WINDOW_HEIGHT - 12
        } else {
            work_area.top
        };

        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            SWP_SHOWWINDOW,
        );
    }

    unsafe fn show_tray_context_menu(hwnd: HWND) {
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);

        let menu: HMENU = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, IDM_OPEN, to_wstring("打开 Task Cleaner").as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_REFRESH, to_wstring("重新扫描").as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_CLEAN_ALL, to_wstring("清理未受保护任务").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, IDM_QUIT, to_wstring("退出").as_ptr());

        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);

        match cmd as usize {
            IDM_OPEN => {
                refresh_scan();
                position_window(hwnd);
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                IS_VISIBLE.store(true, Ordering::SeqCst);
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_REFRESH => {
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLEAN_ALL => {
                execute_clean_all(TerminationMode::Standard);
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_QUIT => {
                PostQuitMessage(0);
            }
            _ => {}
        }
    }

    unsafe fn show_sort_menu(hwnd: HWND, x: i32, y: i32) {
        let cur_mode = STATE.lock().unwrap().as_ref().map(|s| s.sort_mode).unwrap_or_default();
        let menu: HMENU = CreatePopupMenu();

        let add_item = |m: HMENU, id: usize, title: &str, checked: bool| {
            let flags = MF_STRING | if checked { MF_CHECKED } else { 0 };
            AppendMenuW(m, flags, id, to_wstring(title).as_ptr());
        };

        add_item(menu, IDM_SORT_COMPOSITE, "综合负载", cur_mode == SortMode::Composite);
        add_item(menu, IDM_SORT_MEMORY, "内存占用", cur_mode == SortMode::Memory);
        add_item(menu, IDM_SORT_CPU, "CPU 占用", cur_mode == SortMode::Cpu);
        add_item(menu, IDM_SORT_WINDOWS, "窗口数量", cur_mode == SortMode::Windows);
        add_item(menu, IDM_SORT_DEFAULT, "默认字母", cur_mode == SortMode::Default);

        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            x,
            y,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);

        let new_mode = match cmd as usize {
            IDM_SORT_COMPOSITE => Some(SortMode::Composite),
            IDM_SORT_MEMORY => Some(SortMode::Memory),
            IDM_SORT_CPU => Some(SortMode::Cpu),
            IDM_SORT_WINDOWS => Some(SortMode::Windows),
            IDM_SORT_DEFAULT => Some(SortMode::Default),
            _ => None,
        };

        if let Some(mode) = new_mode {
            let mut state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                state.sort_mode = mode;
                sort_targets(&mut state.targets, mode);
            }
            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }

    unsafe fn show_action_chevron_menu(hwnd: HWND, x: i32, y: i32) {
        let menu: HMENU = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, IDM_ACTION_GRACEFUL, to_wstring("常规结束 (标准模式)").as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_ACTION_FORCE, to_wstring("强制结束 (彻底清理)").as_ptr());

        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            x,
            y,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);

        match cmd as usize {
            IDM_ACTION_GRACEFUL => {
                execute_clean_all(TerminationMode::Standard);
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ACTION_FORCE => {
                execute_clean_all(TerminationMode::ForceImmediate);
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            _ => {}
        }
    }

    unsafe fn show_row_more_menu(hwnd: HWND, x: i32, y: i32, actual_idx: usize, is_protected_tab: bool) {
        let mut app_info = None;
        {
            let state_guard = STATE.lock().unwrap();
            if let Some(state) = state_guard.as_ref() {
                if is_protected_tab {
                    if actual_idx < state.protected.len() {
                        app_info = Some((state.protected[actual_idx].0.clone(), true));
                    }
                } else {
                    if actual_idx < state.targets.len() {
                        app_info = Some((state.targets[actual_idx].clone(), false));
                    }
                }
            }
        }

        let (app, is_protected) = match app_info {
            Some(v) => v,
            None => return,
        };

        let menu: HMENU = CreatePopupMenu();
        if is_protected {
            AppendMenuW(menu, MF_STRING, IDM_ROW_WHITELIST_REMOVE, to_wstring("移出保护名单").as_ptr());
        } else {
            AppendMenuW(menu, MF_STRING, IDM_ROW_WHITELIST_ADD, to_wstring("加入保护名单").as_ptr());
            AppendMenuW(menu, MF_STRING, IDM_ROW_FORCE_KILL, to_wstring("强制结束任务").as_ptr());
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, IDM_ROW_REVEAL, to_wstring("在资源管理器中显示").as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_ROW_COPY_NAME, to_wstring("复制进程名称").as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_ROW_COPY_PID, to_wstring("复制 PID").as_ptr());

        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            x,
            y,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);

        match cmd as usize {
            IDM_ROW_WHITELIST_ADD => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.whitelist.add_user_rule(&app.name);
                }
                drop(state_guard);
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_WHITELIST_REMOVE => {
                let mut state_guard = STATE.lock().unwrap();
                if let Some(state) = state_guard.as_mut() {
                    state.whitelist.remove_user_rule(&app.name);
                }
                drop(state_guard);
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_FORCE_KILL => {
                tiered_terminate(&[app], TerminationMode::ForceImmediate, 0, &WhitelistManager::new());
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_ROW_REVEAL => {
                if !app.exe_path.is_empty() {
                    let _ = std::process::Command::new("explorer.exe")
                        .arg(format!("/select,{}", app.exe_path))
                        .spawn();
                }
            }
            IDM_ROW_COPY_NAME => {
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!("Set-Clipboard -Value '{}'", app.name)])
                    .spawn();
            }
            IDM_ROW_COPY_PID => {
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!("Set-Clipboard -Value '{}'", app.pid)])
                    .spawn();
            }
            _ => {}
        }
    }

    unsafe fn show_settings_menu(hwnd: HWND, x: i32, y: i32) {
        let menu: HMENU = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, IDM_CFG_STARTUP, to_wstring("开机自启动设置").as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_CFG_RELOAD, to_wstring("重新加载白名单规则").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, IDM_CFG_ABOUT, to_wstring("关于 Task Cleaner (Windows 11)").as_ptr());

        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            x,
            y,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);

        match cmd as usize {
            IDM_CFG_RELOAD => {
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CFG_ABOUT => {
                let caption = to_wstring("Task Cleaner");
                let msg = to_wstring("Task Cleaner for Windows 11\n版本: 1.0.0 (Rust Native Fluent 2.0)\n\n轻量优雅的一体化前台任务管理与内存释放套件。");
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    hwnd,
                    msg.as_ptr(),
                    caption.as_ptr(),
                    windows_sys::Win32::UI::WindowsAndMessaging::MB_OK | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION,
                );
            }
            _ => {}
        }
    }

    fn execute_clean_all(mode: TerminationMode) {
        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            if state.targets.is_empty() {
                return;
            }
            let report = tiered_terminate(
                &state.targets,
                mode,
                400,
                &state.whitelist,
            );
            state.status_message = Some(format!(
                "已结束 {} 个任务",
                report.terminated_graceful + report.terminated_force
            ));
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
            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }

    /// 高精度 Windows 11 Fluent 2.0 拟真材质渲染管道 (100% 对齐 macOS 极简美学)
    unsafe fn draw_ui(hwnd: HWND, hdc: HDC) {
        let mut client_rect: RECT = std::mem::zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect);

        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, client_rect.right, client_rect.bottom);
        let old_bmp = SelectObject(mem_dc, mem_bmp);

        // 1. 底板画布: 极简纯净的银灰底板
        let bg_brush = CreateSolidBrush(COLOR_CANVAS_BG);
        FillRect(mem_dc, &client_rect, bg_brush);
        DeleteObject(bg_brush);

        // 字体矩阵: 采用 Windows 11 Segoe UI Variable 家族与 ClearType
        let font_title = create_font(15, FW_BOLD as i32);
        let font_card_title = create_font(13, FW_SEMIBOLD as i32);
        let font_body = create_font(12, FW_MEDIUM as i32);
        let font_sub = create_font(10, FW_NORMAL as i32);
        let font_badge = create_font(9, FW_SEMIBOLD as i32);

        SetBkMode(mem_dc, TRANSPARENT as i32);

        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            let hovered_btn = state.hovered_btn;
            let total_running = state.targets.len() + state.protected.len();

            // ----------------------------------------------------
            // 2. 顶栏 (Header)
            // ----------------------------------------------------
            // 标题: Task Cleaner
            SelectObject(mem_dc, font_title);
            SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
            let mut title_rect = RECT { left: 14, top: 12, right: 110, bottom: 36 };
            let title_text = to_wstring("Task Cleaner");
            DrawTextW(mem_dc, title_text.as_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

            // 运行中胶囊徽章 (Pill Badge: [9 运行中])
            let badge_rect = RECT { left: 112, top: 15, right: 182, bottom: 33 };
            draw_rounded_box(mem_dc, &badge_rect, 10, COLOR_BADGE_BG, None);
            SelectObject(mem_dc, font_badge);
            SetTextColor(mem_dc, COLOR_BADGE_TEXT);
            let mut b_text_rect = badge_rect;
            let badge_text = to_wstring(&format!("{} 运行中", total_running));
            DrawTextW(mem_dc, badge_text.as_ptr(), -1, &mut b_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 排序按钮 [↕]
            let sort_rect = RECT { left: 236, top: 13, right: 258, bottom: 35 };
            let is_sort_hover = hovered_btn == Some(HoverButton::SortMenu);
            if is_sort_hover {
                draw_rounded_box(mem_dc, &sort_rect, 6, COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if is_sort_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut s_rect = sort_rect;
            let sort_icon = to_wstring("↕");
            DrawTextW(mem_dc, sort_icon.as_ptr(), -1, &mut s_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 刷新按钮 [↻]
            let ref_rect = RECT { left: 262, top: 13, right: 284, bottom: 35 };
            let is_ref_hover = hovered_btn == Some(HoverButton::Refresh);
            if is_ref_hover {
                draw_rounded_box(mem_dc, &ref_rect, 6, COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if is_ref_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut r_rect = ref_rect;
            let ref_icon = to_wstring("↻");
            DrawTextW(mem_dc, ref_icon.as_ptr(), -1, &mut r_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 最小化到托盘按钮 [×]
            let close_rect = RECT { left: 288, top: 13, right: 308, bottom: 35 };
            let is_close_hover = hovered_btn == Some(HoverButton::CloseToTray);
            if is_close_hover {
                draw_rounded_box(mem_dc, &close_rect, 6, COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if is_close_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
            let mut c_rect = close_rect;
            let close_icon = to_wstring("×");
            DrawTextW(mem_dc, close_icon.as_ptr(), -1, &mut c_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // ----------------------------------------------------
            // 3. 核心卡片 (Hero Action Card)
            // ----------------------------------------------------
            let card_rect = RECT { left: 12, top: 42, right: 308, bottom: 130 };
            draw_rounded_box(mem_dc, &card_rect, 10, COLOR_CARD_BG, Some(COLOR_CARD_BORDER));

            let has_targets = !state.targets.is_empty();

            // 卡片标题与副标题
            SelectObject(mem_dc, font_card_title);
            SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
            let mut card_title = RECT { left: 24, top: 50, right: 230, bottom: 70 };
            let title_str = if has_targets {
                format!("{} 个待结束应用", state.targets.len())
            } else {
                "全部应用已保护".to_string()
            };
            let card_title_txt = to_wstring(&title_str);
            DrawTextW(mem_dc, card_title_txt.as_ptr(), -1, &mut card_title, DT_LEFT | DT_SINGLELINE);

            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
            let mut card_sub = RECT { left: 24, top: 70, right: 230, bottom: 86 };
            let sub_str = if let Some(msg) = &state.status_message {
                msg.clone()
            } else if has_targets {
                "结束未受保护的前台应用".to_string()
            } else {
                "当前前台无待清理的任务".to_string()
            };
            let card_sub_txt = to_wstring(&sub_str);
            DrawTextW(mem_dc, card_sub_txt.as_ptr(), -1, &mut card_sub, DT_LEFT | DT_SINGLELINE);

            // 右上角状态胶囊标签 (待处理 / 已就绪)
            let tag_rect = RECT { left: 248, top: 50, right: 296, bottom: 68 };
            let tag_bg = if has_targets { COLOR_STATUS_PENDING_BG } else { COLOR_STATUS_READY_BG };
            let tag_fg = if has_targets { COLOR_STATUS_PENDING_TEXT } else { COLOR_STATUS_READY_TEXT };
            draw_rounded_box(mem_dc, &tag_rect, 9, tag_bg, None);
            SelectObject(mem_dc, font_badge);
            SetTextColor(mem_dc, tag_fg);
            let mut tr_rect = tag_rect;
            let tag_txt = to_wstring(if has_targets { "待处理" } else { "已就绪" });
            DrawTextW(mem_dc, tag_txt.as_ptr(), -1, &mut tr_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 核心操作按钮组: 结束大按钮 + 选项下拉箭头
            let btn_main_rect = RECT { left: 22, top: 90, right: if has_targets { 268 } else { 298 }, bottom: 122 };
            let is_main_hover = hovered_btn == Some(HoverButton::HeroMain);
            let main_btn_bg = if !has_targets {
                COLOR_STATUS_PENDING_BG
            } else if is_main_hover {
                COLOR_HERO_BTN_HOVER
            } else {
                COLOR_HERO_BTN
            };
            draw_rounded_box(mem_dc, &btn_main_rect, 6, main_btn_bg, None);

            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, if has_targets { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
            let mut bm_text_rect = btn_main_rect;
            let btn_text = to_wstring(if has_targets { "⊗ 结束" } else { "✓ 就绪" });
            DrawTextW(mem_dc, btn_text.as_ptr(), -1, &mut bm_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 下拉小箭头 (仅当有待结束任务时显示)
            if has_targets {
                let btn_chev_rect = RECT { left: 272, top: 90, right: 298, bottom: 122 };
                let is_chev_hover = hovered_btn == Some(HoverButton::HeroChevron);
                let chev_bg = if is_chev_hover { COLOR_HERO_BTN_HOVER } else { COLOR_HERO_BTN };
                draw_rounded_box(mem_dc, &btn_chev_rect, 6, chev_bg, None);
                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
                let mut bc_text_rect = btn_chev_rect;
                let chev_txt = to_wstring("˅");
                DrawTextW(mem_dc, chev_txt.as_ptr(), -1, &mut bc_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }

            // ----------------------------------------------------
            // 4. 分段选择器 (Segmented Tab Bar)
            // ----------------------------------------------------
            let tab_container = RECT { left: 12, top: 138, right: 308, bottom: 166 };
            draw_rounded_box(mem_dc, &tab_container, 8, COLOR_TAB_BG, None);

            let tab_items = [
                ("待结束", state.targets.len()),
                ("已保护", state.protected.len()),
                ("全部活动", total_running),
            ];

            let tab_w = (296 - 4) / 3;
            for (idx, (tab_title, count)) in tab_items.iter().enumerate() {
                let tx = 14 + (idx as i32 * tab_w);
                let tab_rect = RECT {
                    left: tx,
                    top: 140,
                    right: tx + tab_w - 2,
                    bottom: 164,
                };
                let is_active = state.active_tab == idx;
                let is_tab_hover = hovered_btn == Some(HoverButton::Tab(idx));

                if is_active {
                    draw_rounded_box(mem_dc, &tab_rect, 6, COLOR_TAB_ACTIVE, None);
                } else if is_tab_hover {
                    draw_rounded_box(mem_dc, &tab_rect, 6, COLOR_BTN_HOVER, None);
                }

                // 绘制 Tab 标题文本
                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, if is_active { rgb(255, 255, 255) } else { COLOR_TAB_INACTIVE_TEXT });
                let mut t_text_rect = RECT {
                    left: tab_rect.left + 4,
                    top: tab_rect.top,
                    right: tab_rect.right - 24,
                    bottom: tab_rect.bottom,
                };
                let tw = to_wstring(tab_title);
                DrawTextW(mem_dc, tw.as_ptr(), -1, &mut t_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                // 绘制 Tab 数量小胶囊徽章
                let badge_pill = RECT {
                    left: tab_rect.right - 22,
                    top: tab_rect.top + 4,
                    right: tab_rect.right - 4,
                    bottom: tab_rect.bottom - 4,
                };
                let pill_bg = if is_active {
                    rgb(30, 144, 255) // 高亮白色半透感淡蓝
                } else {
                    COLOR_TAB_BADGE_INACTIVE
                };
                draw_rounded_box(mem_dc, &badge_pill, 6, pill_bg, None);
                SelectObject(mem_dc, font_badge);
                SetTextColor(mem_dc, if is_active { rgb(255, 255, 255) } else { COLOR_TAB_INACTIVE_TEXT });
                let mut bp_rect = badge_pill;
                let count_txt = to_wstring(&count.to_string());
                DrawTextW(mem_dc, count_txt.as_ptr(), -1, &mut bp_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }

            // ----------------------------------------------------
            // 5. 应用列表区 (List Container Card)
            // ----------------------------------------------------
            let list_container = RECT { left: 12, top: 174, right: 308, bottom: 434 };
            draw_rounded_box(mem_dc, &list_container, 10, COLOR_CARD_BG, Some(COLOR_CARD_BORDER));

            let items: Vec<(&AppTarget, bool)> = match state.active_tab {
                1 => state.protected.iter().map(|(a, _)| (a, true)).collect(),
                2 => state.targets.iter().map(|a| (a, false)).chain(state.protected.iter().map(|(a, _)| (a, true))).collect(),
                _ => state.targets.iter().map(|a| (a, false)).collect(),
            };

            if items.is_empty() {
                // 空状态优雅占位展示
                SelectObject(mem_dc, font_title);
                SetTextColor(mem_dc, COLOR_ACCENT_BLUE);
                let mut check_rect = RECT { left: 12, top: 220, right: 308, bottom: 250 };
                let check_txt = to_wstring("✓");
                DrawTextW(mem_dc, check_txt.as_ptr(), -1, &mut check_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                SelectObject(mem_dc, font_card_title);
                SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
                let mut empty_title = RECT { left: 12, top: 255, right: 308, bottom: 275 };
                let empty_title_txt = to_wstring("无需清理");
                DrawTextW(mem_dc, empty_title_txt.as_ptr(), -1, &mut empty_title, DT_CENTER | DT_SINGLELINE);

                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
                let mut empty_sub = RECT { left: 12, top: 278, right: 308, bottom: 295 };
                let empty_sub_txt = to_wstring("所有前台应用均受规则保护");
                DrawTextW(mem_dc, empty_sub_txt.as_ptr(), -1, &mut empty_sub, DT_CENTER | DT_SINGLELINE);

                // 查看全部活动胶囊按钮
                let view_all_rect = RECT { left: 100, top: 310, right: 220, bottom: 334 };
                let is_va_hover = hovered_btn == Some(HoverButton::ViewAllFromEmpty);
                draw_rounded_box(mem_dc, &view_all_rect, 12, if is_va_hover { COLOR_HERO_BTN_HOVER } else { COLOR_HERO_BTN }, None);
                SelectObject(mem_dc, font_sub);
                SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
                let mut va_text_rect = view_all_rect;
                let va_txt = to_wstring(&format!("查看全部活动 ({})", total_running));
                DrawTextW(mem_dc, va_txt.as_ptr(), -1, &mut va_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            } else {
                let scroll = state.scroll_offset;
                for i in 0..VISIBLE_ROWS {
                    let actual_idx = scroll + i;
                    if actual_idx >= items.len() {
                        break;
                    }
                    let (app, is_protected) = items[actual_idx];
                    let y = 175 + (i as i32 * ROW_HEIGHT);
                    let row_rect = RECT {
                        left: 14,
                        top: y,
                        right: 306,
                        bottom: y + ROW_HEIGHT,
                    };

                    let is_row_hover = state.hovered_row == Some(i);
                    if is_row_hover {
                        let r_bg = CreateSolidBrush(COLOR_CARD_HOVER);
                        FillRect(mem_dc, &row_rect, r_bg);
                        DeleteObject(r_bg);
                    }

                    // 1. 真实高清应用图标
                    let icon_opt = get_app_icon(&app.exe_path, &mut state.icon_cache);
                    if let Some(h_icon) = icon_opt {
                        DrawIconEx(mem_dc, 22, y + 8, h_icon, 24, 24, 0, 0 as HBRUSH, DI_NORMAL);
                    } else {
                        let def_rect = RECT { left: 22, top: y + 8, right: 46, bottom: y + 32 };
                        draw_rounded_box(mem_dc, &def_rect, 5, COLOR_STATUS_PENDING_BG, None);
                    }

                    // 2. 应用主名称 (解析后的友好名称)
                    SelectObject(mem_dc, font_body);
                    SetTextColor(mem_dc, COLOR_TEXT_PRIMARY);
                    let mut name_rect = RECT { left: 52, top: y + 4, right: 248, bottom: y + 22 };
                    let friendly_name = resolve_friendly_name(app);
                    let name_txt = to_wstring(&friendly_name);
                    DrawTextW(mem_dc, name_txt.as_ptr(), -1, &mut name_rect, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE | DT_END_ELLIPSIS);

                    // 3. 进程详细指标 (chrome.exe · 154 MB · 2 窗口)
                    SelectObject(mem_dc, font_sub);
                    SetTextColor(mem_dc, COLOR_TEXT_SECONDARY);
                    let mut sub_rect = RECT { left: 52, top: y + 22, right: 248, bottom: y + 38 };
                    let sub_txt = to_wstring(&format!(
                        "{} · {:.0} MB · {} 窗口",
                        app.name,
                        app.memory_mb(),
                        app.window_count
                    ));
                    DrawTextW(mem_dc, sub_txt.as_ptr(), -1, &mut sub_rect, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE | DT_END_ELLIPSIS);

                    // 4. 右侧操作按钮组
                    if !is_protected {
                        // 结束按钮 (垃圾桶小图标)
                        let trash_rect = RECT { left: 256, top: y + 10, right: 278, bottom: y + 32 };
                        let is_trash_hover = hovered_btn == Some(HoverButton::RowTrash(i));
                        if is_trash_hover {
                            draw_rounded_box(mem_dc, &trash_rect, 4, COLOR_BTN_HOVER, None);
                        }
                        SelectObject(mem_dc, font_body);
                        SetTextColor(mem_dc, if is_trash_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
                        let mut tr_text = trash_rect;
                        let t_icon = to_wstring("×");
                        DrawTextW(mem_dc, t_icon.as_ptr(), -1, &mut tr_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                    }

                    // 拓展操作按钮 (竖三点 ⋮)
                    let more_rect = RECT { left: 282, top: y + 10, right: 304, bottom: y + 32 };
                    let is_more_hover = hovered_btn == Some(HoverButton::RowMore(i));
                    if is_more_hover {
                        draw_rounded_box(mem_dc, &more_rect, 4, COLOR_BTN_HOVER, None);
                    }
                    SelectObject(mem_dc, font_body);
                    SetTextColor(mem_dc, if is_more_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_MUTED });
                    let mut mr_text = more_rect;
                    let m_icon = to_wstring("⋮");
                    DrawTextW(mem_dc, m_icon.as_ptr(), -1, &mut mr_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                    // 行分割线
                    if i < VISIBLE_ROWS - 1 && actual_idx < items.len() - 1 {
                        let sep_rect = RECT { left: 52, top: y + ROW_HEIGHT - 1, right: 300, bottom: y + ROW_HEIGHT };
                        let s_brush = CreateSolidBrush(COLOR_ROW_SEP);
                        FillRect(mem_dc, &sep_rect, s_brush);
                        DeleteObject(s_brush);
                    }
                }

                // 滚动条指示器 (当条目超过 6 项时展示)
                if items.len() > VISIBLE_ROWS {
                    let total = items.len() as f32;
                    let track_h = 240.0;
                    let thumb_h = (VISIBLE_ROWS as f32 / total * track_h).max(20.0);
                    let thumb_y = 180.0 + (scroll as f32 / (total - VISIBLE_ROWS as f32) * (track_h - thumb_h));
                    let thumb_rect = RECT {
                        left: 304,
                        top: thumb_y as i32,
                        right: 307,
                        bottom: (thumb_y + thumb_h) as i32,
                    };
                    draw_rounded_box(mem_dc, &thumb_rect, 2, COLOR_BADGE_BG, None);
                }
            }

            // ----------------------------------------------------
            // 6. 底栏 (Footer Toolbar)
            // ----------------------------------------------------
            let sep_line = RECT { left: 12, top: 442, right: 308, bottom: 443 };
            let foot_brush = CreateSolidBrush(COLOR_CARD_BORDER);
            FillRect(mem_dc, &sep_line, foot_brush);
            DeleteObject(foot_brush);

            // 配置项按钮 (左侧)
            let cfg_rect = RECT { left: 14, top: 448, right: 86, bottom: 472 };
            let is_cfg_hover = hovered_btn == Some(HoverButton::Settings);
            if is_cfg_hover {
                draw_rounded_box(mem_dc, &cfg_rect, 4, COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, if is_cfg_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut cr = cfg_rect;
            let cfg_txt = to_wstring("⚙ 配置项");
            DrawTextW(mem_dc, cfg_txt.as_ptr(), -1, &mut cr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 退出按钮 (右侧)
            let quit_rect = RECT { left: 254, top: 448, right: 306, bottom: 472 };
            let is_q_hover = hovered_btn == Some(HoverButton::Quit);
            if is_q_hover {
                draw_rounded_box(mem_dc, &quit_rect, 4, COLOR_BTN_HOVER, None);
            }
            SelectObject(mem_dc, font_sub);
            SetTextColor(mem_dc, if is_q_hover { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY });
            let mut qr = quit_rect;
            let q_txt = to_wstring("🌐 退出");
            DrawTextW(mem_dc, q_txt.as_ptr(), -1, &mut qr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
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

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_TRAYICON => {
                let event = (lparam & 0xFFFF) as u32;
                match event {
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP => {
                        toggle_window(hwnd);
                    }
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP => {
                        show_tray_context_menu(hwnd);
                    }
                    _ => {}
                }
                0
            }
            WM_ACTIVATE => 0,
            WM_MOUSEMOVE => {
                let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
                tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
                tme.dwFlags = TME_LEAVE;
                tme.hwndTrack = hwnd;
                TrackMouseEvent(&mut tme);

                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;

                let mut new_btn = None;
                let mut new_row = None;

                // 顶栏按钮 (排序 / 刷新 / 最小化到托盘)
                if x >= 236 && x <= 258 && y >= 13 && y <= 35 {
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
                else if x >= 100 && x <= 220 && y >= 310 && y <= 334 {
                    new_btn = Some(HoverButton::ViewAllFromEmpty);
                }
                // 底栏按钮
                else if y >= 448 && y <= 472 {
                    if x >= 14 && x <= 86 {
                        new_btn = Some(HoverButton::Settings);
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
                        windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 0);
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
                        windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 0);
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
                    windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                0
            }
            WM_LBUTTONUP => {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;

                // 1. 顶栏操作
                if x >= 236 && x <= 258 && y >= 13 && y <= 35 {
                    show_sort_menu(hwnd, x, y);
                    return 0;
                }
                if x >= 262 && x <= 284 && y >= 13 && y <= 35 {
                    refresh_scan();
                    windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
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
                        windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                        return 0;
                    } else if x >= 272 && x <= 298 {
                        show_action_chevron_menu(hwnd, x, y);
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
                            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                            return 0;
                        }
                    }
                }

                // 4. 空状态“查看全部活动”按钮
                if x >= 100 && x <= 220 && y >= 310 && y <= 334 {
                    if let Some(state) = STATE.lock().unwrap().as_mut() {
                        state.active_tab = 2;
                        state.scroll_offset = 0;
                    }
                    windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                    return 0;
                }

                // 5. 应用列表行操作
                if y >= 175 && y <= 175 + (VISIBLE_ROWS as i32 * ROW_HEIGHT) {
                    let visible_idx = ((y - 175) / ROW_HEIGHT) as usize;
                    let (scroll, active_tab) = {
                        let state = STATE.lock().unwrap();
                        let s = state.as_ref().unwrap();
                        (s.scroll_offset, s.active_tab)
                    };
                    let actual_idx = scroll + visible_idx;

                    // 单项垃圾桶结束
                    if x >= 256 && x <= 278 {
                        let mut target_to_kill = None;
                        {
                            let state_guard = STATE.lock().unwrap();
                            if let Some(state) = state_guard.as_ref() {
                                if active_tab == 0 && actual_idx < state.targets.len() {
                                    target_to_kill = Some(state.targets[actual_idx].clone());
                                } else if active_tab == 2 {
                                    if actual_idx < state.targets.len() {
                                        target_to_kill = Some(state.targets[actual_idx].clone());
                                    }
                                }
                            }
                        }
                        if let Some(target) = target_to_kill {
                            tiered_terminate(&[target], TerminationMode::Standard, 400, &WhitelistManager::new());
                            refresh_scan();
                            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                            return 0;
                        }
                    }

                    // 竖三点更多菜单
                    if x >= 282 && x <= 304 {
                        show_row_more_menu(hwnd, x, y, actual_idx, active_tab == 1);
                        return 0;
                    }
                }

                // 6. 底栏操作
                if y >= 448 && y <= 472 {
                    if x >= 14 && x <= 86 {
                        show_settings_menu(hwnd, x, y);
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
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    pub fn run_gui() {
        let h_instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let class_name = to_wstring("TaskCleanerTrayWindow");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DROPSHADOW, // 开启 Windows 11 原生悬浮投影
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: 0 as HICON,
            hCursor: unsafe { LoadCursorW(0 as HINSTANCE, IDC_ARROW) },
            hbrBackground: 0 as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: 0 as HICON,
        };

        unsafe {
            RegisterClassExW(&wc);
        }

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                to_wstring("Task Cleaner").as_ptr(),
                WS_POPUP,
                100,
                100,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
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
        let len = tip.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip[..len]);

        unsafe {
            Shell_NotifyIconW(NIM_ADD, &nid);
        }

        {
            let mut state = STATE.lock().unwrap();
            *state = Some(GuiState {
                whitelist: WhitelistManager::new(),
                targets: Vec::new(),
                protected: Vec::new(),
                sort_mode: SortMode::Composite,
                active_tab: 0,
                scroll_offset: 0,
                hovered_row: None,
                hovered_btn: None,
                icon_cache: HashMap::new(),
                status_message: None,
                last_scan: Instant::now(),
            });
        }

        refresh_scan();

        // 启动时直接在屏幕右下角打开并前置展示 Fluent 2.0 主面板！
        unsafe {
            position_window(hwnd);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            IS_VISIBLE.store(true, Ordering::SeqCst);
            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
        }
        while unsafe { GetMessageW(&mut msg, 0 as HWND, 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe {
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
        }
    }
}

#[cfg(windows)]
fn main() {
    win_gui::run_gui();
}
