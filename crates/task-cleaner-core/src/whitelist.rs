use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::i18n::Language;
use crate::model::AppTarget;

/// 四级白名单分级定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhitelistTier {
    /// L1: 系统核心层 (Core OS) - 保护 Windows 核心图形与系统关键进程
    L1CoreOs,
    /// L2: 会话终端层 (Context Shell) - 保护当前命令执行宿主、终端与 IDE
    L2ContextShell,
    /// L3: 基础设施与效率工具 (Persistent Utilities) - 保护系统托盘、安全中心、驱动组件
    L3PersistentUtilities,
    /// L4: 用户自定义配置 (User Config) - 从 config.toml 读取
    L4UserConfig,
    /// L4: 命令行临时追加 (CLI Override) - 通过 -k / --keep 临时指定
    L4CliOverride,
}

impl WhitelistTier {
    pub fn label(&self) -> &'static str {
        self.localized_label(Language::ZhHans)
    }

    pub fn localized_label(&self, lang: Language) -> &'static str {
        match lang {
            Language::ZhHans => match self {
                Self::L1CoreOs => "系统核心",
                Self::L2ContextShell => "会话终端",
                Self::L3PersistentUtilities => "系统设施",
                Self::L4UserConfig => "用户规则",
                Self::L4CliOverride => "临时保留",
            },
            Language::ZhHant => match self {
                Self::L1CoreOs => "系統核心",
                Self::L2ContextShell => "會話終端",
                Self::L3PersistentUtilities => "系統設施",
                Self::L4UserConfig => "用戶規則",
                Self::L4CliOverride => "臨時保留",
            },
            _ => match self {
                Self::L1CoreOs => "Core OS",
                Self::L2ContextShell => "Context Shell",
                Self::L3PersistentUtilities => "Utilities",
                Self::L4UserConfig => "User Config",
                Self::L4CliOverride => "CLI Keep",
            },
        }
    }
}

/// 白名单命中结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistMatch {
    pub tier: WhitelistTier,
    pub tier_label: String,
    pub matched_rule: String,
}

/// 用户配置文件结构定义
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskCleanerConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub whitelist: WhitelistSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_grace_period")]
    pub grace_period_ms: u64,
    #[serde(default = "default_dry_run")]
    pub default_dry_run: bool,
    #[serde(default)]
    pub language: Option<String>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            grace_period_ms: default_grace_period(),
            default_dry_run: default_dry_run(),
            language: None,
        }
    }
}

fn default_grace_period() -> u64 {
    400
}

fn default_dry_run() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WhitelistSection {
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default)]
    pub disabled_rules: Vec<String>,
}

/// 白名单矩阵管理器
pub struct WhitelistManager {
    // L1: 系统核心层
    l1_names: HashSet<String>,

    // L2: 会话终端与编辑器
    l2_names: HashSet<String>,

    // L3: 基础设施与系统服务
    l3_names: HashSet<String>,

    // L4: 用户配置文件
    l4_user_names: HashSet<String>,

    // L4: CLI 命令行临时指定
    cli_keep_rules: HashSet<String>,

    // 禁用规则集合
    disabled_rules: HashSet<String>,

    pub loaded_config_path: Option<PathBuf>,
}

impl Default for WhitelistManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WhitelistManager {
    pub fn new() -> Self {
        // L1: Windows 核心系统进程
        let l1 = [
            "explorer.exe",
            "dwm.exe",
            "csrss.exe",
            "smss.exe",
            "lsass.exe",
            "services.exe",
            "svchost.exe",
            "SearchHost.exe",
            "StartMenuExperienceHost.exe",
            "ShellExperienceHost.exe",
            "conhost.exe",
            "sihost.exe",
            "fontdrvhost.exe",
            "winlogon.exe",
            "taskhostw.exe",
            "ctfmon.exe",
            "TextInputHost.exe",
            "System",
        ];

        // L2: 终端、Shell 与开发环境
        let l2 = [
            "WindowsTerminal.exe",
            "wt.exe",
            "powershell.exe",
            "pwsh.exe",
            "cmd.exe",
            "Code.exe",
            "devenv.exe",
            "idea64.exe",
            "rust-analyzer.exe",
            "TaskCleaner.exe",
            "mtc.exe",
        ];

        // L3: 基础设施、安全与驱动辅助
        let l3 = [
            "MsMpEng.exe",
            "SecurityHealthSystray.exe",
            "SecurityHealthService.exe",
            "OneDrive.exe",
            "NVIDIA Share.exe",
            "NVDisplay.Container.exe",
            "RadeonSoftware.exe",
            "PowerToys.exe",
            "Everything.exe",
        ];

        let mut mgr = Self {
            l1_names: l1.iter().map(|s| s.to_lowercase()).collect(),
            l2_names: l2.iter().map(|s| s.to_lowercase()).collect(),
            l3_names: l3.iter().map(|s| s.to_lowercase()).collect(),
            l4_user_names: HashSet::new(),
            cli_keep_rules: HashSet::new(),
            disabled_rules: HashSet::new(),
            loaded_config_path: None,
        };

        mgr.load_user_config();
        mgr
    }

    pub fn set_cli_keep_rules(&mut self, rules: &[String]) {
        self.cli_keep_rules = rules.iter().map(|s| s.to_lowercase()).collect();
    }

    /// 核心分级匹配算法
    pub fn classify(&self, target: &AppTarget, caller_lineage: &HashSet<u32>) -> Option<WhitelistMatch> {
        let process_name = target.name.to_lowercase();
        let bundle_name = target.bundle_id.to_lowercase();
        let exe_file = std::path::Path::new(&target.exe_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let check_match = |set: &HashSet<String>| -> Option<String> {
            if set.contains(&process_name) {
                return Some(process_name.clone());
            }
            if set.contains(&bundle_name) {
                return Some(bundle_name.clone());
            }
            if !exe_file.is_empty() && set.contains(&exe_file) {
                return Some(exe_file.clone());
            }
            None
        };

        // 1. L4 CLI 临时保留 (-k / --keep)
        if let Some(matched) = check_match(&self.cli_keep_rules) {
            return Some(WhitelistMatch {
                tier: WhitelistTier::L4CliOverride,
                tier_label: WhitelistTier::L4CliOverride.label().to_string(),
                matched_rule: matched,
            });
        }

        // 2. L1 系统核心层
        if let Some(matched) = check_match(&self.l1_names) {
            if !self.disabled_rules.contains(&matched) {
                return Some(WhitelistMatch {
                    tier: WhitelistTier::L1CoreOs,
                    tier_label: WhitelistTier::L1CoreOs.label().to_string(),
                    matched_rule: matched,
                });
            }
        }

        // 3. L2 会话终端层 (PID 血缘优先保护)
        if caller_lineage.contains(&target.pid) {
            return Some(WhitelistMatch {
                tier: WhitelistTier::L2ContextShell,
                tier_label: WhitelistTier::L2ContextShell.label().to_string(),
                matched_rule: format!("Caller Ancestry PID {}", target.pid),
            });
        }
        if let Some(matched) = check_match(&self.l2_names) {
            if !self.disabled_rules.contains(&matched) {
                return Some(WhitelistMatch {
                    tier: WhitelistTier::L2ContextShell,
                    tier_label: WhitelistTier::L2ContextShell.label().to_string(),
                    matched_rule: matched,
                });
            }
        }

        // 4. L3 基础设施与常驻工具
        if let Some(matched) = check_match(&self.l3_names) {
            if !self.disabled_rules.contains(&matched) {
                return Some(WhitelistMatch {
                    tier: WhitelistTier::L3PersistentUtilities,
                    tier_label: WhitelistTier::L3PersistentUtilities.label().to_string(),
                    matched_rule: matched,
                });
            }
        }

        // 5. L4 用户持久化规则
        if let Some(matched) = check_match(&self.l4_user_names) {
            return Some(WhitelistMatch {
                tier: WhitelistTier::L4UserConfig,
                tier_label: WhitelistTier::L4UserConfig.label().to_string(),
                matched_rule: matched,
            });
        }

        None
    }

    pub fn is_protected(&self, target: &AppTarget, caller_lineage: &HashSet<u32>) -> bool {
        self.classify(target, caller_lineage).is_some()
    }

    pub fn add_user_rule(&mut self, rule: &str) -> bool {
        let clean = rule.trim().to_lowercase();
        if clean.is_empty() {
            return false;
        }
        let added = self.l4_user_names.insert(clean);
        if added {
            let _ = self.save_user_config();
        }
        added
    }

    pub fn remove_user_rule(&mut self, rule: &str) -> bool {
        let clean = rule.trim().to_lowercase();
        let removed = self.l4_user_names.remove(&clean);
        if removed {
            let _ = self.save_user_config();
        }
        removed
    }

    pub fn get_user_rules(&self) -> Vec<String> {
        let mut list: Vec<String> = self.l4_user_names.iter().cloned().collect();
        list.sort();
        list
    }

    fn get_config_path() -> PathBuf {
        if let Ok(app_data) = std::env::var("APPDATA") {
            PathBuf::from(app_data).join("TaskCleaner").join("config.toml")
        } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
            PathBuf::from(user_profile)
                .join(".config")
                .join("taskcleaner")
                .join("config.toml")
        } else {
            PathBuf::from("config.toml")
        }
    }

    fn load_user_config(&mut self) {
        let path = Self::get_config_path();
        self.loaded_config_path = Some(path.clone());
        if !path.exists() {
            return;
        }

        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<TaskCleanerConfig>(&content) {
                for name in cfg.whitelist.process_names {
                    self.l4_user_names.insert(name.to_lowercase());
                }
                for dis in cfg.whitelist.disabled_rules {
                    self.disabled_rules.insert(dis.to_lowercase());
                }
            }
        }
    }

    fn save_user_config(&self) -> Result<(), std::io::Error> {
        let path = Self::get_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut names: Vec<String> = self.l4_user_names.iter().cloned().collect();
        names.sort();
        let mut disabled: Vec<String> = self.disabled_rules.iter().cloned().collect();
        disabled.sort();

        let cfg = TaskCleanerConfig {
            general: GeneralConfig::default(),
            whitelist: WhitelistSection {
                process_names: names,
                disabled_rules: disabled,
            },
        };

        if let Ok(toml_str) = toml::to_string_pretty(&cfg) {
            fs::write(path, toml_str)?;
        }

        Ok(())
    }
}
