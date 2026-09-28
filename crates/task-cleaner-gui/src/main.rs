#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    println!("Task Cleaner GUI is designed for Windows 11. Run on Windows to launch tray application.");
}

#[cfg(windows)]
mod win_gui {
    use std::collections::HashSet;
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
        BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
        CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
        GetStockObject, RoundRect, SelectObject, SetBkColor, SetBkMode, SetTextColor,
        DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD, FW_NORMAL,
        FW_SEMIBOLD, HBITMAP, HBRUSH, HDC, HFONT, PAINTSTRUCT, SRCCOPY, TRANSPARENT,
        WHITE_BRUSH,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, APPBARDATA, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD,
        NIM_DELETE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
        GetWindowRect, ICONINFO, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG,
        PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetWindowPos, ShowWindow,
        TrackPopupMenuEx, TranslateMessage, HICON, HMENU, SM_CXSMICON, SM_CYSMICON, SWP_NOACTIVATE,
        SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
        TPM_RETURNCMD, WM_ACTIVATE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_ERASEBKGND,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WM_USER,
        WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    const WM_TRAYICON: u32 = WM_USER + 101;
    const IDM_OPEN: usize = 1001;
    const IDM_REFRESH: usize = 1002;
    const IDM_CLEAN_ALL: usize = 1003;
    const IDM_QUIT: usize = 1004;

    const WINDOW_WIDTH: i32 = 360;
    const WINDOW_HEIGHT: i32 = 500;

    static IS_VISIBLE: AtomicBool = AtomicBool::new(false);

    struct GuiState {
        whitelist: WhitelistManager,
        targets: Vec<AppTarget>,
        protected: Vec<(AppTarget, WhitelistMatch)>,
        sort_mode: SortMode,
        active_tab: usize, // 0: 待结束, 1: 已保护, 2: 全部
        status_text: String,
        last_scan: Instant,
    }

    static STATE: Mutex<Option<GuiState>> = Mutex::new(None);

    fn to_wstring(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// 动态合成内存图标 (16x16 极简现代胶囊 X 几何图标，规避文件缺失)
    unsafe fn create_default_tray_icon() -> HICON {
        let cx = GetSystemMetrics(SM_CXSMICON);
        let cy = GetSystemMetrics(SM_CYSMICON);
        let hdc = CreateCompatibleDC(0 as HDC);

        let hbm_color = CreateCompatibleBitmap(hdc, cx, cy);
        let hbm_mask = CreateCompatibleBitmap(hdc, cx, cy);

        let h_old = SelectObject(hdc, hbm_color);
        let bg_brush = CreateSolidBrush(0x0078D4); // 经典深蓝点缀色
        let rect = RECT {
            left: 0,
            top: 0,
            right: cx,
            bottom: cy,
        };
        FillRect(hdc, &rect, bg_brush);
        DeleteObject(bg_brush);

        // 绘制白色小方框作为中心图案
        let fg_brush = CreateSolidBrush(0xFFFFFF);
        let inner_rect = RECT {
            left: cx / 4,
            top: cy / 4,
            right: cx * 3 / 4,
            bottom: cy * 3 / 4,
        };
        FillRect(hdc, &inner_rect, fg_brush);
        DeleteObject(fg_brush);

        SelectObject(hdc, h_old);
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
            state.last_scan = Instant::now();
            state.status_text = format!("扫描完成 (共 {} 个前台任务)", state.targets.len() + state.protected.len());
        }
    }

    /// 根据当前任务栏托盘位置，自动锚定吸附在托盘正上方
    unsafe fn position_above_tray(hwnd: HWND) {
        let mut data: APPBARDATA = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<APPBARDATA>() as u32;

        let res = windows_sys::Win32::UI::Shell::SHAppBarMessage(
            windows_sys::Win32::UI::Shell::ABM_GETTASKBARPOS,
            &mut data,
        );

        let (x, y) = if res != 0 {
            // uEdge: 0 = Left, 1 = Top, 2 = Right, 3 = Bottom (Win11 常见)
            match data.uEdge {
                1 => (data.rc.right - WINDOW_WIDTH - 16, data.rc.bottom + 12),
                0 => (data.rc.right + 12, data.rc.bottom - WINDOW_HEIGHT - 12),
                2 => (data.rc.left - WINDOW_WIDTH - 12, data.rc.bottom - WINDOW_HEIGHT - 12),
                _ => (data.rc.right - WINDOW_WIDTH - 16, data.rc.top - WINDOW_HEIGHT - 12),
            }
        } else {
            (1920 - WINDOW_WIDTH - 20, 1080 - WINDOW_HEIGHT - 60)
        };

        SetWindowPos(
            hwnd,
            0 as HWND,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }

    unsafe fn show_tray_context_menu(hwnd: HWND) {
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);

        let menu: HMENU = CreatePopupMenu();
        let open_text = to_wstring("打开 Task Cleaner");
        let refresh_text = to_wstring("扫描刷新");
        let clean_text = to_wstring("一键清理待结束任务");
        let quit_text = to_wstring("退出");

        AppendMenuW(menu, MF_STRING, IDM_OPEN, open_text.as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_REFRESH, refresh_text.as_ptr());
        AppendMenuW(menu, MF_STRING, IDM_CLEAN_ALL, clean_text.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, IDM_QUIT, quit_text.as_ptr());

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
                toggle_window(hwnd);
            }
            IDM_REFRESH => {
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_CLEAN_ALL => {
                execute_clean_all();
                refresh_scan();
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
            }
            IDM_QUIT => {
                PostQuitMessage(0);
            }
            _ => {}
        }
    }

    fn execute_clean_all() {
        let mut state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            if state.targets.is_empty() {
                return;
            }
            let report = tiered_terminate(
                &state.targets,
                TerminationMode::Standard,
                400,
                &state.whitelist,
            );
            state.status_text = format!(
                "已结束 {} 个任务 (释放内存)",
                report.terminated_graceful + report.terminated_force
            );
        }
    }

    unsafe fn toggle_window(hwnd: HWND) {
        let cur = IS_VISIBLE.load(Ordering::SeqCst);
        if cur {
            ShowWindow(hwnd, SW_HIDE);
            IS_VISIBLE.store(false, Ordering::SeqCst);
        } else {
            refresh_scan();
            position_above_tray(hwnd);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            IS_VISIBLE.store(true, Ordering::SeqCst);
            windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }

    unsafe fn draw_ui(hwnd: HWND, hdc: HDC) {
        let mut client_rect: RECT = std::mem::zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect);

        // 双缓冲内存 DC 渲染，彻底消除高频闪烁
        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, client_rect.right, client_rect.bottom);
        let old_bmp = SelectObject(mem_dc, mem_bmp);

        // 1. 绘制 Windows 11 Fluent 浅灰/亚克力背景
        let bg_brush = CreateSolidBrush(0xFAFAFA);
        FillRect(mem_dc, &client_rect, bg_brush);
        DeleteObject(bg_brush);

        let font_title = CreateFontW(
            17, 0, 0, 0, FW_SEMIBOLD as i32, 0, 0, 0, 0, 0, 0, 0, 0,
            to_wstring("Segoe UI Variable Display").as_ptr(),
        );
        let font_body = CreateFontW(
            14, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, 0, 0, 0, 0, 0,
            to_wstring("Segoe UI Variable Text").as_ptr(),
        );
        let font_small = CreateFontW(
            12, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, 0, 0, 0, 0, 0,
            to_wstring("Segoe UI Variable Text").as_ptr(),
        );

        SetBkMode(mem_dc, TRANSPARENT as i32);

        let state_guard = STATE.lock().unwrap();
        if let Some(state) = state_guard.as_ref() {
            // 2. 顶栏: 标题与状态徽章
            SelectObject(mem_dc, font_title);
            SetTextColor(mem_dc, 0x1A1A1A);
            let mut title_rect = RECT {
                left: 16,
                top: 14,
                right: 180,
                bottom: 38,
            };
            let title_text = to_wstring("Task Cleaner");
            DrawTextW(mem_dc, title_text.as_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

            // 徽章
            SelectObject(mem_dc, font_small);
            let badge_bg = CreateSolidBrush(0xF0E4D4);
            let badge_rect = RECT {
                left: 130,
                top: 16,
                right: 215,
                bottom: 36,
            };
            RoundRect(mem_dc, badge_rect.left, badge_rect.top, badge_rect.right, badge_rect.bottom, 10, 10);
            DeleteObject(badge_bg);
            SetTextColor(mem_dc, 0xD47800);
            let badge_text = to_wstring(&format!("{} 运行中", state.targets.len() + state.protected.len()));
            let mut b_text_rect = badge_rect;
            DrawTextW(mem_dc, badge_text.as_ptr(), -1, &mut b_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 3. 核心卡片 (Action Card)
            let card_rect = RECT {
                left: 14,
                top: 48,
                right: WINDOW_WIDTH - 14,
                bottom: 114,
            };
            let card_brush = CreateSolidBrush(0xFFFFFF);
            SelectObject(mem_dc, card_brush);
            RoundRect(mem_dc, card_rect.left, card_rect.top, card_rect.right, card_rect.bottom, 12, 12);
            DeleteObject(card_brush);

            SelectObject(mem_dc, font_body);
            SetTextColor(mem_dc, 0x222222);
            let mut card_title = RECT {
                left: 26,
                top: 58,
                right: 230,
                bottom: 78,
            };
            let card_title_txt = to_wstring(&format!("{} 个待清理任务", state.targets.len()));
            DrawTextW(mem_dc, card_title_txt.as_ptr(), -1, &mut card_title, DT_LEFT | DT_SINGLELINE);

            SelectObject(mem_dc, font_small);
            SetTextColor(mem_dc, 0x777777);
            let mut card_sub = RECT {
                left: 26,
                top: 80,
                right: 230,
                bottom: 98,
            };
            let total_mem: f64 = state.targets.iter().map(|t| t.memory_mb()).sum();
            let card_sub_txt = to_wstring(&format!("释放约 {:.0} MB 内存", total_mem));
            DrawTextW(mem_dc, card_sub_txt.as_ptr(), -1, &mut card_sub, DT_LEFT | DT_SINGLELINE);

            // 一键清理按钮
            let btn_rect = RECT {
                left: WINDOW_WIDTH - 110,
                top: 62,
                right: WINDOW_WIDTH - 24,
                bottom: 100,
            };
            let btn_brush = CreateSolidBrush(if state.targets.is_empty() { 0xCCCCCC } else { 0x0078D4 });
            SelectObject(mem_dc, btn_brush);
            RoundRect(mem_dc, btn_rect.left, btn_rect.top, btn_rect.right, btn_rect.bottom, 10, 10);
            DeleteObject(btn_brush);
            SetTextColor(mem_dc, 0xFFFFFF);
            SelectObject(mem_dc, font_body);
            let mut btn_txt_rect = btn_rect;
            let btn_text = to_wstring(if state.targets.is_empty() { "无需清理" } else { "一键结束" });
            DrawTextW(mem_dc, btn_text.as_ptr(), -1, &mut btn_txt_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // 4. 分段选项卡 (待结束 | 已保护 | 全部)
            SelectObject(mem_dc, font_small);
            let tabs = ["待结束", "已保护", "全部"];
            for (idx, &tab_label) in tabs.iter().enumerate() {
                let tab_x = 16 + (idx as i32 * 68);
                let tab_rect = RECT {
                    left: tab_x,
                    top: 122,
                    right: tab_x + 62,
                    bottom: 144,
                };
                let is_active = state.active_tab == idx;
                let t_brush = CreateSolidBrush(if is_active { 0xEAEAEA } else { 0xFAFAFA });
                SelectObject(mem_dc, t_brush);
                RoundRect(mem_dc, tab_rect.left, tab_rect.top, tab_rect.right, tab_rect.bottom, 8, 8);
                DeleteObject(t_brush);
                SetTextColor(mem_dc, if is_active { 0x0078D4 } else { 0x555555 });
                let mut tr = tab_rect;
                let tw = to_wstring(tab_label);
                DrawTextW(mem_dc, tw.as_ptr(), -1, &mut tr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            }

            // 5. 应用列表区
            let items: Vec<&AppTarget> = match state.active_tab {
                1 => state.protected.iter().map(|(a, _)| a).collect(),
                2 => state.targets.iter().chain(state.protected.iter().map(|(a, _)| a)).collect(),
                _ => state.targets.iter().collect(),
            };

            let mut y = 152;
            let max_show = 6;
            for (i, app) in items.iter().take(max_show).enumerate() {
                let row_rect = RECT {
                    left: 14,
                    top: y,
                    right: WINDOW_WIDTH - 14,
                    bottom: y + 46,
                };
                let row_brush = CreateSolidBrush(0xFFFFFF);
                SelectObject(mem_dc, row_brush);
                RoundRect(mem_dc, row_rect.left, row_rect.top, row_rect.right, row_rect.bottom, 8, 8);
                DeleteObject(row_brush);

                // 名称
                SelectObject(mem_dc, font_body);
                SetTextColor(mem_dc, 0x222222);
                let mut name_rect = RECT {
                    left: 24,
                    top: y + 5,
                    right: 210,
                    bottom: y + 24,
                };
                let name_txt = to_wstring(&app.name);
                DrawTextW(mem_dc, name_txt.as_ptr(), -1, &mut name_rect, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE);

                // 指标
                SelectObject(mem_dc, font_small);
                SetTextColor(mem_dc, 0x888888);
                let mut metric_rect = RECT {
                    left: 24,
                    top: y + 25,
                    right: 210,
                    bottom: y + 42,
                };
                let metric_txt = to_wstring(&app.metrics_display());
                DrawTextW(mem_dc, metric_txt.as_ptr(), -1, &mut metric_rect, DT_LEFT | DT_NOPREFIX | DT_SINGLELINE);

                // 单项结束按钮
                let row_btn = RECT {
                    left: WINDOW_WIDTH - 64,
                    top: y + 10,
                    right: WINDOW_WIDTH - 24,
                    bottom: y + 36,
                };
                let r_btn_brush = CreateSolidBrush(0xF0F0F0);
                SelectObject(mem_dc, r_btn_brush);
                RoundRect(mem_dc, row_btn.left, row_btn.top, row_btn.right, row_btn.bottom, 6, 6);
                DeleteObject(r_btn_brush);
                SetTextColor(mem_dc, 0x333333);
                let mut r_txt_rect = row_btn;
                let r_btn_txt = to_wstring("结束");
                DrawTextW(mem_dc, r_btn_txt.as_ptr(), -1, &mut r_txt_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

                y += 52;
            }

            // 6. 底栏状态
            SelectObject(mem_dc, font_small);
            SetTextColor(mem_dc, 0x888888);
            let mut status_rect = RECT {
                left: 16,
                top: WINDOW_HEIGHT - 32,
                right: WINDOW_WIDTH - 16,
                bottom: WINDOW_HEIGHT - 10,
            };
            let status_txt = to_wstring(&state.status_text);
            DrawTextW(mem_dc, status_txt.as_ptr(), -1, &mut status_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        }

        // 将内存双缓冲 BitBlt 到屏幕上
        BitBlt(hdc, 0, 0, client_rect.right, client_rect.bottom, mem_dc, 0, 0, SRCCOPY);

        SelectObject(mem_dc, old_bmp);
        DeleteObject(mem_bmp);
        DeleteDC(mem_dc);

        DeleteObject(font_title);
        DeleteObject(font_body);
        DeleteObject(font_small);
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
            WM_ACTIVATE => {
                let state = (wparam & 0xFFFF) as u32;
                if state == 0 {
                    // WA_INACTIVE: 失焦自动隐退
                    ShowWindow(hwnd, SW_HIDE);
                    IS_VISIBLE.store(false, Ordering::SeqCst);
                }
                0
            }
            WM_LBUTTONUP => {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;

                // 检查一键结束按钮点击
                if x >= WINDOW_WIDTH - 110 && x <= WINDOW_WIDTH - 24 && y >= 62 && y <= 100 {
                    execute_clean_all();
                    refresh_scan();
                    windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                    return 0;
                }

                // 检查分段 Tab 点击
                for idx in 0..3 {
                    let tab_x = 16 + (idx * 68);
                    if x >= tab_x && x <= tab_x + 62 && y >= 122 && y <= 144 {
                        if let Some(state) = STATE.lock().unwrap().as_mut() {
                            state.active_tab = idx as usize;
                        }
                        windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                        return 0;
                    }
                }

                // 检查单项任务的结束点击
                let mut row_y = 152;
                for i in 0..6 {
                    if x >= WINDOW_WIDTH - 64 && x <= WINDOW_WIDTH - 24 && y >= row_y + 10 && y <= row_y + 36 {
                        let mut state_guard = STATE.lock().unwrap();
                        if let Some(state) = state_guard.as_mut() {
                            if i < state.targets.len() {
                                let target = state.targets[i].clone();
                                drop(state_guard);
                                let report = tiered_terminate(&[target], TerminationMode::Standard, 400, &WhitelistManager::new());
                                refresh_scan();
                                windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, std::ptr::null(), 1);
                                return 0;
                            }
                        }
                    }
                    row_y += 52;
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
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: 0 as HICON,
            hCursor: unsafe { windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW(0 as HINSTANCE, 32512 as *const u16) },
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

        // 启用 Windows 11 原生圆角
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

        // 初始化应用数据
        {
            let mut state = STATE.lock().unwrap();
            *state = Some(GuiState {
                whitelist: WhitelistManager::new(),
                targets: Vec::new(),
                protected: Vec::new(),
                sort_mode: SortMode::Composite,
                active_tab: 0,
                status_text: "就绪".to_string(),
                last_scan: Instant::now(),
            });
        }

        refresh_scan();

        // Win32 标准消息循环
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { GetMessageW(&mut msg, 0 as HWND, 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 清理托盘图标
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &nid);
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(h_icon);
        }
    }
}

#[cfg(windows)]
fn main() {
    win_gui::run_gui();
}
