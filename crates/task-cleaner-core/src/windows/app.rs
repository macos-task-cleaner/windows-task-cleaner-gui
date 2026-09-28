#![cfg(windows)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{BOOL, HANDLE, HWND, LPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_VM_READ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

use crate::model::AppTarget;
use crate::windows::telemetry::calculate_composite_score;

struct EnumContext {
    pid_windows: HashMap<u32, Vec<HWND>>,
    pid_titles: HashMap<u32, String>,
}

struct ChildEnumContext {
    parent_pid: u32,
    real_pid: Option<u32>,
}

unsafe extern "system" fn enum_child_proc(child_hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam as *mut ChildEnumContext) };
    let mut child_pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(child_hwnd, &mut child_pid) };
    if child_pid != 0 && child_pid != ctx.parent_pid {
        ctx.real_pid = Some(child_pid);
        return 0; // 找到真实业务进程，停止子窗口遍历
    }
    1
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam as *mut EnumContext) };

    // 1. 可见性过滤
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }

    // 2. 工具栏与浮动小窗口过滤
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
    if (ex_style & (WS_EX_TOOLWINDOW as i32)) != 0 {
        return 1;
    }

    // 3. 过滤无标题后台窗口
    let title_len = unsafe { GetWindowTextLengthW(hwnd) };
    if title_len == 0 {
        return 1;
    }

    // 4. 过滤 Windows 10/11 虚拟桌面/休眠 Cloaked 窗口
    let mut cloaked: u32 = 0;
    let res = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            &mut cloaked as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    if res == 0 && cloaked != 0 {
        return 1;
    }

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 {
        return 1;
    }

    // 提取窗口文本
    let mut buf = vec![0u16; (title_len + 1) as usize];
    let read_len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    let title = if read_len > 0 {
        OsString::from_wide(&buf[..read_len as usize])
            .to_string_lossy()
            .to_string()
    } else {
        String::new()
    };

    // 5. 穿透 ApplicationFrameHost.exe (UWP / 现代应用真实进程解析)
    let final_pid = resolve_real_uwp_pid(hwnd, pid);

    ctx.pid_windows.entry(final_pid).or_default().push(hwnd);
    ctx.pid_titles.entry(final_pid).or_insert(title);

    1
}

fn resolve_real_uwp_pid(hwnd: HWND, pid: u32) -> u32 {
    let proc_name = get_process_name_by_pid(pid);
    if proc_name.eq_ignore_ascii_case("ApplicationFrameHost.exe") {
        let mut child_ctx = ChildEnumContext {
            parent_pid: pid,
            real_pid: None,
        };
        unsafe {
            EnumChildWindows(
                hwnd,
                Some(enum_child_proc),
                &mut child_ctx as *mut _ as LPARAM,
            );
        }
        if let Some(real) = child_ctx.real_pid {
            return real;
        }
    }
    pid
}

fn get_process_name_by_pid(pid: u32) -> String {
    let handle: HANDLE = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return format!("PID_{}", pid);
    }

    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size) };
    unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };

    if ok != 0 && size > 0 {
        let full_path = OsString::from_wide(&buf[..size as usize]).to_string_lossy().to_string();
        Path::new(&full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string()
    } else {
        format!("PID_{}", pid)
    }
}

pub fn get_process_memory_bytes(pid: u32) -> u64 {
    let handle: HANDLE = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return 0;
    }

    let mut pmc: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

    let ok = unsafe {
        K32GetProcessMemoryInfo(
            handle,
            &mut pmc,
            pmc.cb,
        )
    };
    unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };

    if ok != 0 {
        pmc.WorkingSetSize as u64
    } else {
        0
    }
}

/// 扫描系统中所有前台可见窗口对应的应用程序
pub fn scan_foreground_apps() -> Vec<AppTarget> {
    let mut ctx = EnumContext {
        pid_windows: HashMap::new(),
        pid_titles: HashMap::new(),
    };

    unsafe {
        EnumWindows(
            Some(enum_windows_proc),
            &mut ctx as *mut _ as LPARAM,
        );
    }

    let mut targets = Vec::new();

    for (pid, windows) in ctx.pid_windows {
        let title = ctx.pid_titles.get(&pid).cloned().unwrap_or_default();
        let handle: HANDLE = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                0,
                pid,
            )
        };

        let mut exe_path = String::new();
        let mut proc_name = String::new();
        let mut memory_bytes = 0u64;
        let mut cpu_percent = 0.0;

        if !handle.is_null() {
            // 1. 获取完整路径与执行文件名
            let mut buf = vec![0u16; 1024];
            let mut size = buf.len() as u32;
            if unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size) } != 0 && size > 0 {
                exe_path = OsString::from_wide(&buf[..size as usize]).to_string_lossy().to_string();
                if let Some(file_name) = Path::new(&exe_path).file_name().and_then(|n| n.to_str()) {
                    proc_name = file_name.to_string();
                }
            }

            // 2. 提取物理常驻内存 (Working Set)
            let mut pmc: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
            pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            if unsafe { K32GetProcessMemoryInfo(handle, &mut pmc, pmc.cb) } != 0 {
                memory_bytes = pmc.WorkingSetSize as u64;
            }

            // 3. 初始 CPU 占用估算
            let mut creation = unsafe { std::mem::zeroed() };
            let mut exit = unsafe { std::mem::zeroed() };
            let mut kernel = unsafe { std::mem::zeroed() };
            let mut user = unsafe { std::mem::zeroed() };
            if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } != 0 {
                // 单次快照暂记为基础活动
                cpu_percent = 0.5;
            }

            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
        }

        if proc_name.is_empty() {
            proc_name = format!("PID_{}.exe", pid);
        }

        let mut target = AppTarget::new(pid, &proc_name, &proc_name);
        target.title = title;
        target.exe_path = exe_path;
        target.memory_bytes = memory_bytes;
        target.cpu_percent = cpu_percent;
        target.window_count = windows.len();
        target.composite_score = calculate_composite_score(
            target.memory_mb(),
            target.cpu_percent,
            target.window_count,
        );

        targets.push(target);
    }

    targets
}
