// Task Cleaner - Windows 11 Native Fluent Menu Engine
// 100% Native Win32 + GDI implementation with zero external runtime bloat.
// Matches Windows 11 Fluent Design & TranslucentTB tiered menu UX.

use std::sync::Mutex;
use windows_sys::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, InvalidateRect, ReleaseDC, RoundRect,
    SelectObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY, DT_CALCRECT, DT_CENTER, DT_LEFT,
    DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, FW_NORMAL, HFONT, PAINTSTRUCT, PS_NULL,
    PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
    VK_ESCAPE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos,
    GetMessageW, IsWindow, LoadCursorW, RegisterClassExW, SetForegroundWindow,
    SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage, CS_DROPSHADOW, IDC_ARROW,
    MSG, SPI_GETWORKAREA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE,
    SW_SHOW, SW_SHOWNOACTIVATE, WA_INACTIVE, WM_ACTIVATE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCLBUTTONDOWN, WM_NCRBUTTONDOWN,
    WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const WM_MOUSELEAVE: u32 = 0x02A3;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[inline]
fn scale_dpi(val: i32, dpi: i32) -> i32 {
    ((val as i64 * dpi as i64) / 96) as i32
}

#[inline]
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn to_wstring(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn is_dark_theme() -> bool {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    };
    let subkey = to_wstring(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let val_name = to_wstring("SystemUsesLightTheme");
    let mut hkey = 0 as windows_sys::Win32::System::Registry::HKEY;
    let res = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey)
    };
    if res == 0 {
        let mut data: u32 = 0;
        let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
        let query_res = unsafe {
            RegQueryValueExW(
                hkey,
                val_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut data as *mut u32 as *mut u8,
                &mut data_size,
            )
        };
        unsafe { RegCloseKey(hkey); }
        if query_res == 0 {
            return data == 0;
        }
    }
    false
}

#[derive(Clone, Debug)]
pub enum MenuItemKind {
    Action(usize),
    Toggle(usize, bool),
    Radio(usize, bool),
    Submenu(Vec<FluentMenuItem>),
    Separator,
}

#[derive(Clone, Debug)]
pub struct FluentMenuItem {
    pub text: String,
    pub icon: Option<char>,
    pub shortcut: Option<String>,
    pub kind: MenuItemKind,
    pub enabled: bool,
}

impl FluentMenuItem {
    pub fn action(id: usize, text: impl Into<String>, icon: Option<char>) -> Self {
        Self {
            text: text.into(),
            icon,
            shortcut: None,
            kind: MenuItemKind::Action(id),
            enabled: true,
        }
    }

    pub fn action_with_shortcut(
        id: usize,
        text: impl Into<String>,
        icon: Option<char>,
        shortcut: impl Into<String>,
    ) -> Self {
        Self {
            text: text.into(),
            icon,
            shortcut: Some(shortcut.into()),
            kind: MenuItemKind::Action(id),
            enabled: true,
        }
    }

    pub fn toggle(
        id: usize,
        text: impl Into<String>,
        checked: bool,
        icon: Option<char>,
    ) -> Self {
        Self {
            text: text.into(),
            icon,
            shortcut: None,
            kind: MenuItemKind::Toggle(id, checked),
            enabled: true,
        }
    }

    pub fn radio(id: usize, text: impl Into<String>, checked: bool) -> Self {
        Self {
            text: text.into(),
            icon: None,
            shortcut: None,
            kind: MenuItemKind::Radio(id, checked),
            enabled: true,
        }
    }

    pub fn submenu(
        text: impl Into<String>,
        icon: Option<char>,
        children: Vec<FluentMenuItem>,
    ) -> Self {
        Self {
            text: text.into(),
            icon,
            shortcut: None,
            kind: MenuItemKind::Submenu(children),
            enabled: true,
        }
    }

    pub fn separator() -> Self {
        Self {
            text: String::new(),
            icon: None,
            shortcut: None,
            kind: MenuItemKind::Separator,
            enabled: false,
        }
    }
}

// 内部菜单窗口状态
struct MenuLevelState {
    hwnd: HWND,
    level: usize,
    items: Vec<FluentMenuItem>,
    hovered: Option<usize>,
    dpi: i32,
    dark_mode: bool,
    width: i32,
    height: i32,
    parent_item_idx: Option<usize>,
}

unsafe impl Send for MenuLevelState {}

static MENU_STACK: Mutex<Vec<MenuLevelState>> = Mutex::new(Vec::new());
static SELECTED_COMMAND: Mutex<Option<usize>> = Mutex::new(None);
static IS_MODAL_ACTIVE: Mutex<bool> = Mutex::new(false);

const MENU_CLASS_NAME: &str = "TaskCleanerFluentMenuClass";

unsafe fn ensure_menu_class_registered(h_instance: HINSTANCE) {
    static REGISTERED: Mutex<bool> = Mutex::new(false);
    let mut reg = REGISTERED.lock().unwrap();
    if *reg {
        return;
    }

    let class_w = to_wstring(MENU_CLASS_NAME);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DROPSHADOW,
        lpfnWndProc: Some(menu_window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: h_instance,
        hIcon: 0 as _,
        hCursor: LoadCursorW(0 as _, IDC_ARROW),
        hbrBackground: 0 as _,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_w.as_ptr(),
        hIconSm: 0 as _,
    };
    RegisterClassExW(&wc);
    *reg = true;
}

unsafe fn create_fluent_font(name: &str, size_pt: i32, weight: i32) -> HFONT {
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
        to_wstring(name).as_ptr(),
    )
}

fn compute_menu_dimensions(
    items: &[FluentMenuItem],
    dpi: i32,
    hwnd_ref: HWND,
) -> (i32, i32) {
    let mut total_h = scale_dpi(8, dpi); // 上下外边距各 4px
    let mut max_content_w = scale_dpi(120, dpi);

    let hdc = unsafe { windows_sys::Win32::Graphics::Gdi::GetDC(hwnd_ref) };
    let font_text = unsafe { create_fluent_font("Segoe UI Variable Text", scale_dpi(10, dpi), FW_NORMAL as i32) };
    let font_shortcut = unsafe { create_fluent_font("Segoe UI Variable Text", scale_dpi(9, dpi), FW_NORMAL as i32) };

    let old_font = unsafe { SelectObject(hdc, font_text) };

    for item in items {
        match &item.kind {
            MenuItemKind::Separator => {
                total_h += scale_dpi(7, dpi);
            }
            _ => {
                total_h += scale_dpi(34, dpi);

                let text_w = to_wstring(&item.text);
                let mut rc_text: RECT = unsafe { std::mem::zeroed() };
                unsafe {
                    DrawTextW(
                        hdc,
                        text_w.as_ptr(),
                        text_w.len() as i32 - 1,
                        &mut rc_text,
                        DT_CALCRECT | DT_NOPREFIX | DT_SINGLELINE,
                    );
                }
                let mut w = rc_text.right - rc_text.left;

                if let Some(sc) = &item.shortcut {
                    unsafe { SelectObject(hdc, font_shortcut); }
                    let sc_w = to_wstring(sc);
                    let mut rc_sc: RECT = unsafe { std::mem::zeroed() };
                    unsafe {
                        DrawTextW(
                            hdc,
                            sc_w.as_ptr(),
                            sc_w.len() as i32 - 1,
                            &mut rc_sc,
                            DT_CALCRECT | DT_NOPREFIX | DT_SINGLELINE,
                        );
                    }
                    w += (rc_sc.right - rc_sc.left) + scale_dpi(16, dpi);
                    unsafe { SelectObject(hdc, font_text); }
                } else if matches!(item.kind, MenuItemKind::Submenu(_)) {
                    w += scale_dpi(22, dpi); // 右侧箭头预留宽度
                }

                if w > max_content_w {
                    max_content_w = w;
                }
            }
        }
    }

    unsafe {
        SelectObject(hdc, old_font);
        DeleteObject(font_text);
        DeleteObject(font_shortcut);
        ReleaseDC(hwnd_ref, hdc);
    }

    // 左侧图标槽位 (28px) + 左文字内边距 (8px) + 文字宽度 + 右内边距 (16px) + 窗体左右外边距 (8px)
    let total_w = (scale_dpi(28 + 8 + 16 + 8, dpi) + max_content_w)
        .clamp(scale_dpi(220, dpi), scale_dpi(360, dpi));

    (total_w, total_h)
}

unsafe fn create_level_window(
    h_instance: HINSTANCE,
    parent_hwnd: HWND,
    level: usize,
    items: Vec<FluentMenuItem>,
    x: i32,
    y: i32,
    dpi: i32,
    dark_mode: bool,
    parent_item_idx: Option<usize>,
) -> HWND {
    ensure_menu_class_registered(h_instance);

    let (w, h) = compute_menu_dimensions(&items, dpi, parent_hwnd);
    let class_w = to_wstring(MENU_CLASS_NAME);

    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        class_w.as_ptr(),
        std::ptr::null(),
        WS_POPUP,
        x,
        y,
        w,
        h,
        parent_hwnd,
        0 as _,
        h_instance,
        std::ptr::null(),
    );

    let corner_pref = DWMWCP_ROUND;
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
        &corner_pref as *const _ as *const _,
        std::mem::size_of::<u32>() as u32,
    );

    let dark_val = if dark_mode { 1u32 } else { 0u32 };
    DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark_val as *const _ as *const _,
        std::mem::size_of::<u32>() as u32,
    );

    {
        let mut stack = MENU_STACK.lock().unwrap();
        stack.push(MenuLevelState {
            hwnd,
            level,
            items,
            hovered: None,
            dpi,
            dark_mode,
            width: w,
            height: h,
            parent_item_idx,
        });
    }

    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    InvalidateRect(hwnd, std::ptr::null(), 0);
    hwnd
}

unsafe fn is_point_in_any_menu(pt: POINT) -> bool {
    let stack = MENU_STACK.lock().unwrap();
    for s in stack.iter() {
        let mut rc: RECT = std::mem::zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(s.hwnd, &mut rc);
        if pt.x >= rc.left && pt.x <= rc.right && pt.y >= rc.top && pt.y <= rc.bottom {
            return true;
        }
    }
    false
}

unsafe fn close_menus_above_level(level: usize) {
    let to_close: Vec<HWND> = {
        let mut stack = MENU_STACK.lock().unwrap();
        let mut closed = Vec::new();
        while let Some(top) = stack.last() {
            if top.level > level {
                closed.push(top.hwnd);
                stack.pop();
            } else {
                break;
            }
        }
        closed
    };

    for h in to_close {
        DestroyWindow(h);
    }
}

pub unsafe fn close_all_fluent_menus() {
    let to_close: Vec<HWND> = {
        let mut stack = MENU_STACK.lock().unwrap();
        let list: Vec<HWND> = stack.iter().map(|s| s.hwnd).collect();
        stack.clear();
        list
    };

    for h in to_close {
        DestroyWindow(h);
    }
}

unsafe fn open_submenu(parent_level: usize, item_idx: usize) {
    let (h_instance, parent_hwnd, sub_items, parent_item_y, parent_dpi, parent_dark, parent_rect) = {
        let stack = MENU_STACK.lock().unwrap();
        let current = match stack.iter().find(|s| s.level == parent_level) {
            Some(s) => s,
            None => return,
        };

        let sub_items = match &current.items.get(item_idx) {
            Some(FluentMenuItem {
                kind: MenuItemKind::Submenu(children),
                enabled: true,
                ..
            }) => children.clone(),
            _ => return,
        };

        let mut item_y = scale_dpi(4, current.dpi);
        for (i, it) in current.items.iter().enumerate() {
            if i == item_idx {
                break;
            }
            item_y += match &it.kind {
                MenuItemKind::Separator => scale_dpi(7, current.dpi),
                _ => scale_dpi(34, current.dpi),
            };
        }

        let mut win_rc: RECT = std::mem::zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(current.hwnd, &mut win_rc);

        let h_inst = GetModuleHandleW(std::ptr::null());
        (
            h_inst,
            current.hwnd,
            sub_items,
            item_y,
            current.dpi,
            current.dark_mode,
            win_rc,
        )
    };

    // 关闭同级及更深级的现有子菜单
    close_menus_above_level(parent_level);

    let (child_w, child_h) = compute_menu_dimensions(&sub_items, parent_dpi, parent_hwnd);

    let mut work_area: RECT = std::mem::zeroed();
    SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        &mut work_area as *mut _ as *mut _,
        0,
    );

    // 智能边界检测：如果右侧空间不足，则向左侧镜像展开（完全对齐 TranslucentTB）
    let child_x = if parent_rect.right + child_w - scale_dpi(4, parent_dpi) <= work_area.right {
        parent_rect.right - scale_dpi(4, parent_dpi)
    } else {
        parent_rect.left - child_w + scale_dpi(4, parent_dpi)
    };

    let item_screen_y = parent_rect.top + parent_item_y;
    let child_y = (item_screen_y - scale_dpi(4, parent_dpi))
        .clamp(work_area.top + scale_dpi(8, parent_dpi), (work_area.bottom - child_h - scale_dpi(8, parent_dpi)).max(work_area.top));

    create_level_window(
        h_instance,
        parent_hwnd,
        parent_level + 1,
        sub_items,
        child_x,
        child_y,
        parent_dpi,
        parent_dark,
        Some(item_idx),
    );
}

unsafe extern "system" fn menu_window_proc(
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

            let (items, hovered, dpi, dark_mode, w, h, parent_item_idx) = {
                let stack = MENU_STACK.lock().unwrap();
                if let Some(s) = stack.iter().find(|s| s.hwnd == hwnd) {
                    (
                        s.items.clone(),
                        s.hovered,
                        s.dpi,
                        s.dark_mode,
                        s.width,
                        s.height,
                        s.parent_item_idx,
                    )
                } else {
                    EndPaint(hwnd, &ps);
                    return 0;
                }
            };

            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bmp = CreateCompatibleBitmap(hdc, w, h);
            let old_bmp = SelectObject(mem_dc, mem_bmp);

            // 1. 背景色与边框色绘制
            let (bg_color, border_color, hover_bg, text_color, secondary_text, accent_color, sep_color) = if dark_mode {
                (
                    rgb(32, 32, 32),    // 现代暗黑背景
                    rgb(56, 56, 56),    // 微弱边框
                    rgb(56, 56, 56),    // 悬停胶囊色
                    rgb(243, 243, 243), // 主文字
                    rgb(160, 160, 160), // 次级快捷键/箭头文字
                    rgb(76, 194, 255),  // Win11 亚克力暗色勾选蓝
                    rgb(48, 48, 48),    // 分割线
                )
            } else {
                (
                    rgb(255, 255, 255), // 浅色纯净背景
                    rgb(229, 229, 229), // 浅色边框
                    rgb(238, 238, 238), // 浅色悬停胶囊色
                    rgb(26, 26, 26),    // 主文字
                    rgb(118, 118, 118), // 次级快捷键/箭头文字
                    rgb(0, 103, 192),   // Win11 官方强调蓝
                    rgb(235, 235, 235), // 分割线
                )
            };

            let bg_brush = CreateSolidBrush(bg_color);
            let border_pen = CreatePen(PS_SOLID, 1, border_color);
            let old_brush = SelectObject(mem_dc, bg_brush);
            let old_pen = SelectObject(mem_dc, border_pen);

            RoundRect(mem_dc, 0, 0, w, h, scale_dpi(8, dpi), scale_dpi(8, dpi));

            SelectObject(mem_dc, old_pen);
            SelectObject(mem_dc, old_brush);
            DeleteObject(border_pen);
            DeleteObject(bg_brush);

            SetBkMode(mem_dc, TRANSPARENT as i32);

            let font_text = create_fluent_font("Segoe UI Variable Text", scale_dpi(10, dpi), FW_NORMAL as i32);
            let font_icon = create_fluent_font("Segoe Fluent Icons", scale_dpi(11, dpi), FW_NORMAL as i32);
            let font_shortcut = create_fluent_font("Segoe UI Variable Text", scale_dpi(9, dpi), FW_NORMAL as i32);

            let mut cur_y = scale_dpi(4, dpi);

            for (idx, item) in items.iter().enumerate() {
                match &item.kind {
                    MenuItemKind::Separator => {
                        let sep_y = cur_y + scale_dpi(3, dpi);
                        let sep_pen = CreatePen(PS_SOLID, 1, sep_color);
                        let old_pen = SelectObject(mem_dc, sep_pen);
                        windows_sys::Win32::Graphics::Gdi::MoveToEx(mem_dc, scale_dpi(12, dpi), sep_y, std::ptr::null_mut());
                        windows_sys::Win32::Graphics::Gdi::LineTo(mem_dc, w - scale_dpi(12, dpi), sep_y);
                        SelectObject(mem_dc, old_pen);
                        DeleteObject(sep_pen);
                        cur_y += scale_dpi(7, dpi);
                    }
                    _ => {
                        let item_h = scale_dpi(34, dpi);
                        let is_hovered = hovered == Some(idx);

                        // 绘制悬停胶囊 (Hover Pill)
                        if is_hovered && item.enabled {
                            let pill_brush = CreateSolidBrush(hover_bg);
                            let null_pen = CreatePen(PS_NULL, 0, 0);
                            let old_brush = SelectObject(mem_dc, pill_brush);
                            let old_pen = SelectObject(mem_dc, null_pen);

                            let left = scale_dpi(4, dpi);
                            let top = cur_y + scale_dpi(2, dpi);
                            let right = w - scale_dpi(4, dpi);
                            let bottom = cur_y + scale_dpi(32, dpi);
                            RoundRect(mem_dc, left, top, right, bottom, scale_dpi(4, dpi), scale_dpi(4, dpi));

                            SelectObject(mem_dc, old_pen);
                            SelectObject(mem_dc, old_brush);
                            DeleteObject(null_pen);
                            DeleteObject(pill_brush);
                        }

                        // 绘制左侧图标/勾选/单选
                        let icon_left = scale_dpi(8, dpi);
                        let icon_w = scale_dpi(20, dpi);
                        let mut icon_rc = RECT {
                            left: icon_left,
                            top: cur_y,
                            right: icon_left + icon_w,
                            bottom: cur_y + item_h,
                        };

                        SelectObject(mem_dc, font_icon);

                        match &item.kind {
                            MenuItemKind::Toggle(_, true) => {
                                SetTextColor(mem_dc, accent_color);
                                let check_str = to_wstring("\u{E73E}");
                                DrawTextW(
                                    mem_dc,
                                    check_str.as_ptr(),
                                    1,
                                    &mut icon_rc,
                                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                                );
                            }
                            MenuItemKind::Radio(_, true) => {
                                SetTextColor(mem_dc, accent_color);
                                let radio_str = to_wstring("\u{F137}");
                                DrawTextW(
                                    mem_dc,
                                    radio_str.as_ptr(),
                                    1,
                                    &mut icon_rc,
                                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                                );
                            }
                            _ => {
                                if let Some(glyph) = item.icon {
                                    SetTextColor(mem_dc, if item.enabled { secondary_text } else { rgb(120, 120, 120) });
                                    let mut s = String::new();
                                    s.push(glyph);
                                    let glyph_w = to_wstring(&s);
                                    DrawTextW(
                                        mem_dc,
                                        glyph_w.as_ptr(),
                                        1,
                                        &mut icon_rc,
                                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                                    );
                                }
                            }
                        }

                        // 绘制文字
                        SelectObject(mem_dc, font_text);
                        SetTextColor(mem_dc, if item.enabled { text_color } else { secondary_text });
                        let text_w = to_wstring(&item.text);
                        let mut text_rc = RECT {
                            left: scale_dpi(34, dpi),
                            top: cur_y,
                            right: w - scale_dpi(32, dpi),
                            bottom: cur_y + item_h,
                        };
                        DrawTextW(
                            mem_dc,
                            text_w.as_ptr(),
                            text_w.len() as i32 - 1,
                            &mut text_rc,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                        );

                        // 绘制右侧快捷键或子菜单箭头
                        if matches!(item.kind, MenuItemKind::Submenu(_)) {
                            SelectObject(mem_dc, font_icon);
                            SetTextColor(mem_dc, secondary_text);
                            let chevron_str = to_wstring("\u{E76C}");
                            let mut chevron_rc = RECT {
                                left: w - scale_dpi(24, dpi),
                                top: cur_y,
                                right: w - scale_dpi(6, dpi),
                                bottom: cur_y + item_h,
                            };
                            DrawTextW(
                                mem_dc,
                                chevron_str.as_ptr(),
                                1,
                                &mut chevron_rc,
                                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                            );
                        } else if let Some(sc) = &item.shortcut {
                            SelectObject(mem_dc, font_shortcut);
                            SetTextColor(mem_dc, secondary_text);
                            let sc_w = to_wstring(sc);
                            let mut sc_rc = RECT {
                                left: w - scale_dpi(100, dpi),
                                top: cur_y,
                                right: w - scale_dpi(14, dpi),
                                bottom: cur_y + item_h,
                            };
                            DrawTextW(
                                mem_dc,
                                sc_w.as_ptr(),
                                sc_w.len() as i32 - 1,
                                &mut sc_rc,
                                DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                            );
                        }

                        cur_y += item_h;
                    }
                }
            }

            BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bmp);
            DeleteObject(mem_bmp);
            DeleteDC(mem_dc);
            DeleteObject(font_text);
            DeleteObject(font_icon);
            DeleteObject(font_shortcut);

            EndPaint(hwnd, &ps);
            0
        }
        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let (level, items, dpi, current_hovered) = {
                let stack = MENU_STACK.lock().unwrap();
                if let Some(s) = stack.iter().find(|s| s.hwnd == hwnd) {
                    (s.level, s.items.clone(), s.dpi, s.hovered)
                } else {
                    return 0;
                }
            };

            let mut cur_y = scale_dpi(4, dpi);
            let mut hovered_item = None;

            for (idx, item) in items.iter().enumerate() {
                let h = match &item.kind {
                    MenuItemKind::Separator => scale_dpi(7, dpi),
                    _ => scale_dpi(34, dpi),
                };

                if y >= cur_y && y < cur_y + h {
                    if !matches!(item.kind, MenuItemKind::Separator) && item.enabled {
                        hovered_item = Some(idx);
                    }
                    break;
                }
                cur_y += h;
            }

            if hovered_item != current_hovered {
                {
                    let mut stack = MENU_STACK.lock().unwrap();
                    if let Some(s) = stack.iter_mut().find(|s| s.hwnd == hwnd) {
                        s.hovered = hovered_item;
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);

                if let Some(idx) = hovered_item {
                    if matches!(&items[idx].kind, MenuItemKind::Submenu(_)) {
                        open_submenu(level, idx);
                    } else {
                        close_menus_above_level(level);
                    }
                }
            }

            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tme);

            0
        }
        WM_MOUSELEAVE => {
            let mut has_child_for_this = false;
            {
                let stack = MENU_STACK.lock().unwrap();
                if let Some(current) = stack.iter().find(|s| s.hwnd == hwnd) {
                    if stack.iter().any(|s| s.level == current.level + 1) {
                        has_child_for_this = true;
                    }
                }
            }

            if !has_child_for_this {
                {
                    let mut stack = MENU_STACK.lock().unwrap();
                    if let Some(s) = stack.iter_mut().find(|s| s.hwnd == hwnd) {
                        s.hovered = None;
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let (items, dpi, level) = {
                let stack = MENU_STACK.lock().unwrap();
                if let Some(s) = stack.iter().find(|s| s.hwnd == hwnd) {
                    (s.items.clone(), s.dpi, s.level)
                } else {
                    return 0;
                }
            };

            let mut cur_y = scale_dpi(4, dpi);
            for (idx, item) in items.iter().enumerate() {
                let h = match &item.kind {
                    MenuItemKind::Separator => scale_dpi(7, dpi),
                    _ => scale_dpi(34, dpi),
                };

                if y >= cur_y && y < cur_y + h {
                    if item.enabled {
                        match &item.kind {
                            MenuItemKind::Action(id)
                            | MenuItemKind::Toggle(id, _)
                            | MenuItemKind::Radio(id, _) => {
                                let mut cmd = SELECTED_COMMAND.lock().unwrap();
                                *cmd = Some(*id);
                                close_all_fluent_menus();
                            }
                            MenuItemKind::Submenu(_) => {
                                open_submenu(level, idx);
                            }
                            MenuItemKind::Separator => {}
                        }
                    }
                    break;
                }
                cur_y += h;
            }

            0
        }
        WM_ACTIVATE => {
            let active = (wparam & 0xFFFF) as u32;
            if active == WA_INACTIVE {
                let target_hwnd = lparam as HWND;
                let stack = MENU_STACK.lock().unwrap();
                let is_sibling = stack.iter().any(|s| s.hwnd == target_hwnd);
                if !is_sibling && *IS_MODAL_ACTIVE.lock().unwrap() {
                    drop(stack);
                    close_all_fluent_menus();
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 弹出并模态跟踪 Fluent 分级菜单
pub unsafe fn track_fluent_menu(
    anchor_x: i32,
    anchor_y: i32,
    align_bottom: bool,
    items: Vec<FluentMenuItem>,
    parent_hwnd: HWND,
) -> Option<usize> {
    let h_instance = GetModuleHandleW(std::ptr::null());
    let dpi = GetDpiForWindow(parent_hwnd).max(96) as i32;
    let dark_mode = is_dark_theme();

    close_all_fluent_menus();
    {
        let mut cmd = SELECTED_COMMAND.lock().unwrap();
        *cmd = None;
    }

    let (w, h) = compute_menu_dimensions(&items, dpi, parent_hwnd);

    let mut work_area: RECT = std::mem::zeroed();
    SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        &mut work_area as *mut _ as *mut _,
        0,
    );

    let x = if anchor_x + w <= work_area.right {
        anchor_x
    } else {
        (work_area.right - w - scale_dpi(8, dpi)).max(work_area.left)
    };

    let y = if align_bottom {
        if anchor_y - h >= work_area.top {
            anchor_y - h
        } else {
            anchor_y
        }
    } else {
        if anchor_y + h <= work_area.bottom {
            anchor_y
        } else {
            anchor_y - h
        }
    }
    .clamp(work_area.top + scale_dpi(4, dpi), (work_area.bottom - h - scale_dpi(4, dpi)).max(work_area.top));

    let root_hwnd = create_level_window(
        h_instance,
        parent_hwnd,
        0,
        items,
        x,
        y,
        dpi,
        dark_mode,
        None,
    );

    SetForegroundWindow(root_hwnd);

    {
        let mut active = IS_MODAL_ACTIVE.lock().unwrap();
        *active = true;
    }

    let mut msg: MSG = std::mem::zeroed();
    while *IS_MODAL_ACTIVE.lock().unwrap() && GetMessageW(&mut msg, 0 as _, 0, 0) > 0 {
        // 检测全局点击（失焦关闭）
        if msg.message == WM_LBUTTONDOWN
            || msg.message == WM_RBUTTONDOWN
            || msg.message == WM_NCLBUTTONDOWN
            || msg.message == WM_NCRBUTTONDOWN
        {
            let mut pt = POINT {
                x: (msg.lParam & 0xFFFF) as i16 as i32,
                y: ((msg.lParam >> 16) & 0xFFFF) as i16 as i32,
            };
            windows_sys::Win32::Graphics::Gdi::ClientToScreen(msg.hwnd, &mut pt);

            if !is_point_in_any_menu(pt) {
                close_all_fluent_menus();
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
                break;
            }
        }

        // 按 ESC 逐层退出
        if msg.message == WM_KEYDOWN && msg.wParam == VK_ESCAPE as usize {
            let stack_len = MENU_STACK.lock().unwrap().len();
            if stack_len > 1 {
                close_menus_above_level(stack_len - 2);
                continue;
            } else {
                close_all_fluent_menus();
                break;
            }
        }

        TranslateMessage(&msg);
        DispatchMessageW(&msg);

        // 如果用户选定了某个指令项
        if SELECTED_COMMAND.lock().unwrap().is_some() {
            break;
        }

        // 窗口全都被关闭
        if MENU_STACK.lock().unwrap().is_empty() {
            break;
        }
    }

    {
        let mut active = IS_MODAL_ACTIVE.lock().unwrap();
        *active = false;
    }

    close_all_fluent_menus();
    let result = SELECTED_COMMAND.lock().unwrap().take();
    result
}
