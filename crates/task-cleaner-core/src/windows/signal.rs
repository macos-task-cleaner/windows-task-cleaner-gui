#![cfg(windows)]

use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, BOOL, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, GetExitCodeProcess, OpenProcess, TerminateProcess, WaitForSingleObject,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, SYNCHRONIZE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
};

use crate::i18n::{localize_status_code, Language};
use crate::model::{
    AppTarget, ProcessTerminationRecord, TerminationMode, TerminationReport, TerminationStatusCode,
};
use crate::whitelist::{WhitelistManager, WhitelistTier};

const STILL_ACTIVE: u32 = 259;

/// 递归解析调用者进程的完整父系祖先 PID (Toolhelp32 Process Tree)
/// 严格保护 PowerShell、CMD、Windows Terminal、VS Code 及宿主环境不被误杀
pub fn get_caller_lineage() -> HashSet<u32> {
    let mut lineage = HashSet::new();
    let current_pid = unsafe { GetCurrentProcessId() };
    lineage.insert(current_pid);

    let snapshot: HANDLE = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return lineage;
    }

    let mut parent_map = HashMap::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    if unsafe { Process32FirstW(snapshot, &mut entry) } != 0 {
        loop {
            parent_map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(snapshot) };

    // 沿祖先链向上回溯直至根节点或检测到回环
    let mut curr = current_pid;
    let mut visited = HashSet::new();
    visited.insert(curr);

    while let Some(&ppid) = parent_map.get(&curr) {
        if ppid == 0 || visited.contains(&ppid) {
            break;
        }
        lineage.insert(ppid);
        visited.insert(ppid);
        curr = ppid;
    }

    lineage
}

/// 检查指定 PID 进程当前是否仍然存活
pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    let handle: HANDLE = unsafe {
        OpenProcess(
            SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return false;
    }

    let wait_res = unsafe { WaitForSingleObject(handle, 0) };
    let mut exit_code: u32 = 0;
    let code_ok = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
    unsafe { CloseHandle(handle) };

    if wait_res == WAIT_OBJECT_0 || (code_ok != 0 && exit_code != STILL_ACTIVE) {
        false
    } else {
        true
    }
}

pub fn is_explorer(identifier: &str) -> bool {
    let clean = identifier.trim().to_lowercase();
    clean == "explorer.exe" || clean == "explorer"
}

struct EnumCloseContext {
    target_pid: u32,
    closed_any: bool,
}

unsafe extern "system" fn enum_close_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam as *mut EnumCloseContext) };
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };

    if pid == ctx.target_pid {
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        ctx.closed_any = true;
    }
    1
}

/// 分级安全终止核心执行引擎 (两阶段 WM_CLOSE -> 轮询宽限期 -> TerminateProcess 强制兜底)
pub fn tiered_terminate(
    targets: &[AppTarget],
    mode: TerminationMode,
    grace_period_ms: u64,
    whitelist: &WhitelistManager,
) -> TerminationReport {
    let start_time = Instant::now();
    let caller_lineage = get_caller_lineage();
    let mut report = TerminationReport::default();
    report.total_targets = targets.len();

    let lang = Language::ZhHans;

    for target in targets {
        // 1. 调用者会话链路主动防御
        if caller_lineage.contains(&target.pid) {
            report.records.push(ProcessTerminationRecord {
                app: target.clone(),
                status: localize_status_code(TerminationStatusCode::SkippedCallerLineage, lang).to_string(),
                status_code: TerminationStatusCode::SkippedCallerLineage,
                exit_signal: None,
                error_msg: Some("调用者终端会话保护，禁止自杀".to_string()),
            });
            continue;
        }

        // 2. 检查白名单矩阵 (L1 系统守护绝对豁免)
        if let Some(matched) = whitelist.classify(target, &caller_lineage) {
            if matched.tier == WhitelistTier::L1CoreOs || is_explorer(&target.name) || is_explorer(&target.bundle_id) {
                report.records.push(ProcessTerminationRecord {
                    app: target.clone(),
                    status: localize_status_code(TerminationStatusCode::SkippedCriticalDaemon, lang).to_string(),
                    status_code: TerminationStatusCode::SkippedCriticalDaemon,
                    exit_signal: None,
                    error_msg: Some(format!("命中白名单 [{}]: {}", matched.tier_label, matched.matched_rule)),
                });
                continue;
            }
        }

        // 3. 执行终止逻辑
        match mode {
            TerminationMode::ForceImmediate => {
                if is_explorer(&target.name) || is_explorer(&target.bundle_id) {
                    report.failed += 1;
                    report.records.push(ProcessTerminationRecord {
                        app: target.clone(),
                        status: localize_status_code(TerminationStatusCode::SkippedCriticalDaemon, lang).to_string(),
                        status_code: TerminationStatusCode::SkippedCriticalDaemon,
                        exit_signal: None,
                        error_msg: Some("资源管理器严禁强制终止".to_string()),
                    });
                    continue;
                }

                let handle: HANDLE = unsafe { OpenProcess(PROCESS_TERMINATE, 0, target.pid) };
                if !handle.is_null() {
                    let ok = unsafe { TerminateProcess(handle, 1) };
                    unsafe { CloseHandle(handle) };
                    if ok != 0 {
                        report.terminated_force += 1;
                        report.records.push(ProcessTerminationRecord {
                            app: target.clone(),
                            status: localize_status_code(TerminationStatusCode::SuccessForceTerminate, lang).to_string(),
                            status_code: TerminationStatusCode::SuccessForceTerminate,
                            exit_signal: Some("TerminateProcess".to_string()),
                            error_msg: None,
                        });
                    } else {
                        report.failed += 1;
                        report.records.push(ProcessTerminationRecord {
                            app: target.clone(),
                            status: localize_status_code(TerminationStatusCode::FailedKill, lang).to_string(),
                            status_code: TerminationStatusCode::FailedKill,
                            exit_signal: None,
                            error_msg: Some("强制终止失败".to_string()),
                        });
                    }
                } else {
                    report.failed += 1;
                    report.records.push(ProcessTerminationRecord {
                        app: target.clone(),
                        status: localize_status_code(TerminationStatusCode::FailedPermissionDenied, lang).to_string(),
                        status_code: TerminationStatusCode::FailedPermissionDenied,
                        exit_signal: None,
                        error_msg: Some("无法获取进程句柄 (权限不足)".to_string()),
                    });
                }
            }
            TerminationMode::Standard => {
                // 第一阶段：向该应用所有顶层窗口投递 WM_CLOSE 消息 (优雅退出)
                let mut close_ctx = EnumCloseContext {
                    target_pid: target.pid,
                    closed_any: false,
                };
                unsafe {
                    EnumWindows(
                        Some(enum_close_proc),
                        &mut close_ctx as *mut _ as LPARAM,
                    );
                }

                // 第二阶段：宽限期轮询探测
                let deadline = Instant::now() + Duration::from_millis(grace_period_ms);
                let mut exited = false;

                while Instant::now() < deadline {
                    if !is_process_alive(target.pid) {
                        exited = true;
                        break;
                    }
                    thread::sleep(Duration::from_millis(40));
                }

                if exited {
                    report.terminated_graceful += 1;
                    report.records.push(ProcessTerminationRecord {
                        app: target.clone(),
                        status: localize_status_code(TerminationStatusCode::SuccessGraceful, lang).to_string(),
                        status_code: TerminationStatusCode::SuccessGraceful,
                        exit_signal: Some("WM_CLOSE".to_string()),
                        error_msg: None,
                    });
                } else {
                    // 超时未退出，实施强制终止 (针对 explorer.exe 特殊豁免)
                    if is_explorer(&target.name) || is_explorer(&target.bundle_id) {
                        report.records.push(ProcessTerminationRecord {
                            app: target.clone(),
                            status: "优雅退出超时 (资源管理器豁免强制终止)".to_string(),
                            status_code: TerminationStatusCode::SkippedCriticalDaemon,
                            exit_signal: None,
                            error_msg: None,
                        });
                        continue;
                    }

                    let handle: HANDLE = unsafe { OpenProcess(PROCESS_TERMINATE, 0, target.pid) };
                    if !handle.is_null() {
                        let ok = unsafe { TerminateProcess(handle, 1) };
                        unsafe { CloseHandle(handle) };
                        if ok != 0 {
                            report.terminated_force += 1;
                            report.records.push(ProcessTerminationRecord {
                                app: target.clone(),
                                status: localize_status_code(TerminationStatusCode::SuccessForceTerminate, lang).to_string(),
                                status_code: TerminationStatusCode::SuccessForceTerminate,
                                exit_signal: Some("TerminateProcess (Grace Timeout)".to_string()),
                                error_msg: None,
                            });
                        } else {
                            report.failed += 1;
                            report.records.push(ProcessTerminationRecord {
                                app: target.clone(),
                                status: localize_status_code(TerminationStatusCode::FailedKill, lang).to_string(),
                                status_code: TerminationStatusCode::FailedKill,
                                exit_signal: None,
                                error_msg: Some("强制终止调用失败".to_string()),
                            });
                        }
                    } else {
                        report.failed += 1;
                        report.records.push(ProcessTerminationRecord {
                            app: target.clone(),
                            status: localize_status_code(TerminationStatusCode::FailedPermissionDenied, lang).to_string(),
                            status_code: TerminationStatusCode::FailedPermissionDenied,
                            exit_signal: None,
                            error_msg: Some("权限不足，无法执行 TerminateProcess".to_string()),
                        });
                    }
                }
            }
        }
    }

    report.duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    report
}
