#![cfg(not(windows))]

use std::collections::HashSet;
use crate::model::{AppTarget, TerminationMode, TerminationReport};
use crate::whitelist::WhitelistManager;

pub fn scan_foreground_apps() -> Vec<AppTarget> {
    vec![
        AppTarget {
            pid: 1001,
            name: "chrome.exe".to_string(),
            bundle_id: "chrome.exe".to_string(),
            title: "Google Chrome".to_string(),
            exe_path: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".to_string(),
            memory_bytes: 420 * 1024 * 1024,
            cpu_percent: 1.5,
            window_count: 2,
            composite_score: 3.2,
        },
        AppTarget {
            pid: 1002,
            name: "notepad.exe".to_string(),
            bundle_id: "notepad.exe".to_string(),
            title: "Untitled - Notepad".to_string(),
            exe_path: "C:\\Windows\\System32\\notepad.exe".to_string(),
            memory_bytes: 35 * 1024 * 1024,
            cpu_percent: 0.1,
            window_count: 1,
            composite_score: 0.8,
        },
    ]
}

pub fn get_process_memory_bytes(_pid: u32) -> u64 {
    50 * 1024 * 1024
}

pub fn is_process_alive(pid: u32) -> bool {
    pid > 0
}

pub fn is_explorer(identifier: &str) -> bool {
    let clean = identifier.trim().to_lowercase();
    clean == "explorer.exe" || clean == "explorer"
}

pub fn get_caller_lineage() -> HashSet<u32> {
    let mut s = HashSet::new();
    s.insert(std::process::id());
    s
}

pub fn purge_process_working_set(_pid: u32) -> bool {
    true
}


pub fn tiered_terminate(

    targets: &[AppTarget],
    _mode: TerminationMode,
    _grace_period_ms: u64,
    _whitelist: &WhitelistManager,
) -> TerminationReport {
    TerminationReport {
        total_targets: targets.len(),
        terminated_graceful: targets.len(),
        terminated_force: 0,
        purged: 0,
        failed: 0,
        duration_ms: 50.0,
        records: vec![],
    }
}


pub fn calculate_composite_score(memory_mb: f64, cpu_percent: f64, window_count: usize) -> f64 {
    (memory_mb / 100.0) * 0.4 + (cpu_percent * 2.0) * 0.4 + (window_count as f64 * 5.0) * 0.2
}

pub fn sort_targets(targets: &mut [AppTarget], mode: crate::model::SortMode) {
    match mode {
        crate::model::SortMode::Composite => {
            targets.sort_by(|a, b| {
                b.composite_score
                    .partial_cmp(&a.composite_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        crate::model::SortMode::Memory => {
            targets.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        }
        crate::model::SortMode::Cpu => {
            targets.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        crate::model::SortMode::Windows => {
            targets.sort_by(|a, b| b.window_count.cmp(&a.window_count));
        }
        crate::model::SortMode::Default => {
            targets.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.pid.cmp(&b.pid))
            });
        }
    }
}
