pub mod i18n;
pub mod model;
pub mod whitelist;

#[cfg(windows)]
pub mod windows;

#[cfg(not(windows))]
pub mod mock;

pub use model::{
    AppTarget, ProcessTerminationRecord, SortMode, TerminationMode, TerminationReport,
    TerminationStatusCode,
};
pub use whitelist::{
    GeneralConfig, TaskCleanerConfig, WhitelistManager, WhitelistMatch, WhitelistSection,
    WhitelistTier,
};
pub use i18n::{
    detect_system_language, localize_status_code, tr, I18nKey, Language, LanguagePreference,
};

#[cfg(windows)]
pub use windows::{
    calculate_composite_score, get_caller_lineage, get_process_memory_bytes, is_explorer,
    is_process_alive, purge_process_working_set, scan_foreground_apps, sort_targets,
    tiered_terminate,
};

#[cfg(not(windows))]
pub use mock::{
    calculate_composite_score, get_caller_lineage, get_process_memory_bytes, is_explorer,
    is_process_alive, purge_process_working_set, scan_foreground_apps, sort_targets,
    tiered_terminate,
};


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist_l1_core_system() {
        let mgr = WhitelistManager::new();
        let target = AppTarget::new(1234, "explorer.exe", "explorer.exe");
        let lineage = std::collections::HashSet::new();
        let m = mgr.classify(&target, &lineage);
        assert!(m.is_some());
        assert_eq!(m.unwrap().tier, WhitelistTier::L1CoreOs);
    }

    #[test]
    fn test_whitelist_l2_caller_lineage() {
        let mgr = WhitelistManager::new();
        let target = AppTarget::new(9999, "my_custom_shell.exe", "my_custom_shell.exe");
        let mut lineage = std::collections::HashSet::new();
        lineage.insert(9999);
        let m = mgr.classify(&target, &lineage);
        assert!(m.is_some());
        assert_eq!(m.unwrap().tier, WhitelistTier::L2ContextShell);
    }

    #[test]
    fn test_composite_scoring_and_sorting() {
        let mut targets = vec![
            AppTarget {
                pid: 1,
                name: "heavy.exe".to_string(),
                bundle_id: "heavy.exe".to_string(),
                title: "Heavy".to_string(),
                exe_path: "".to_string(),
                memory_bytes: 2000 * 1024 * 1024,
                cpu_percent: 15.0,
                window_count: 3,
                composite_score: calculate_composite_score(2000.0, 15.0, 3),
            },
            AppTarget {
                pid: 2,
                name: "light.exe".to_string(),
                bundle_id: "light.exe".to_string(),
                title: "Light".to_string(),
                exe_path: "".to_string(),
                memory_bytes: 50 * 1024 * 1024,
                cpu_percent: 0.1,
                window_count: 1,
                composite_score: calculate_composite_score(50.0, 0.1, 1),
            },
        ];

        sort_targets(&mut targets, SortMode::Composite);
        assert_eq!(targets[0].name, "heavy.exe");
        assert!(targets[0].composite_score > targets[1].composite_score);
    }
}
