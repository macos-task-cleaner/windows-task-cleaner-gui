use serde::{Deserialize, Serialize};

use crate::model::TerminationStatusCode;

/// 支持的国际化语言枚举 (与 macOS 端保持 100% 一致，支持 24 种语言)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    En,
    ZhHans,
    ZhHant,
    Ja,
    Ko,
    Fr,
    De,
    Es,
    Pt,
    It,
    Ru,
    Nl,
    Pl,
    Tr,
    Ar,
    Th,
    Vi,
    Id,
    Sv,
    Da,
    Nb,
    Fi,
    Cs,
    Uk,
}

impl Language {
    pub const ALL: [Language; 24] = [
        Language::En,
        Language::ZhHans,
        Language::ZhHant,
        Language::Ja,
        Language::Ko,
        Language::Fr,
        Language::De,
        Language::Es,
        Language::Pt,
        Language::It,
        Language::Ru,
        Language::Nl,
        Language::Pl,
        Language::Tr,
        Language::Ar,
        Language::Th,
        Language::Vi,
        Language::Id,
        Language::Sv,
        Language::Da,
        Language::Nb,
        Language::Fi,
        Language::Cs,
        Language::Uk,
    ];

    pub fn from_locale_str(s: &str) -> Self {
        let s = s.to_lowercase().replace('_', "-");
        if s.starts_with("zh-hant")
            || s.starts_with("zh-tw")
            || s.starts_with("zh-hk")
            || s.starts_with("zh-mo")
            || s.starts_with("zh-cht")
        {
            Self::ZhHant
        } else if s.starts_with("zh") {
            Self::ZhHans
        } else if s.starts_with("ja") {
            Self::Ja
        } else if s.starts_with("ko") {
            Self::Ko
        } else if s.starts_with("fr") {
            Self::Fr
        } else if s.starts_with("de") {
            Self::De
        } else if s.starts_with("es") {
            Self::Es
        } else if s.starts_with("pt") {
            Self::Pt
        } else if s.starts_with("it") {
            Self::It
        } else if s.starts_with("ru") {
            Self::Ru
        } else if s.starts_with("nl") {
            Self::Nl
        } else if s.starts_with("pl") {
            Self::Pl
        } else if s.starts_with("tr") {
            Self::Tr
        } else if s.starts_with("ar") {
            Self::Ar
        } else if s.starts_with("th") {
            Self::Th
        } else if s.starts_with("vi") {
            Self::Vi
        } else if s.starts_with("id") {
            Self::Id
        } else if s.starts_with("sv") {
            Self::Sv
        } else if s.starts_with("da") {
            Self::Da
        } else if s.starts_with("nb") || s.starts_with("no") {
            Self::Nb
        } else if s.starts_with("fi") {
            Self::Fi
        } else if s.starts_with("cs") {
            Self::Cs
        } else if s.starts_with("uk") {
            Self::Uk
        } else {
            Self::En
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhHans => "zh-Hans",
            Self::ZhHant => "zh-Hant",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Es => "es",
            Self::Pt => "pt",
            Self::It => "it",
            Self::Ru => "ru",
            Self::Nl => "nl",
            Self::Pl => "pl",
            Self::Tr => "tr",
            Self::Ar => "ar",
            Self::Th => "th",
            Self::Vi => "vi",
            Self::Id => "id",
            Self::Sv => "sv",
            Self::Da => "da",
            Self::Nb => "nb",
            Self::Fi => "fi",
            Self::Cs => "cs",
            Self::Uk => "uk",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::En => "English",
            Self::ZhHans => "简体中文",
            Self::ZhHant => "繁體中文",
            Self::Ja => "日本語",
            Self::Ko => "한국어",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Es => "Español",
            Self::Pt => "Português",
            Self::It => "Italiano",
            Self::Ru => "Русский",
            Self::Nl => "Nederlands",
            Self::Pl => "Polski",
            Self::Tr => "Türkçe",
            Self::Ar => "العربية",
            Self::Th => "ไทย",
            Self::Vi => "Tiếng Việt",
            Self::Id => "Bahasa Indonesia",
            Self::Sv => "Svenska",
            Self::Da => "Dansk",
            Self::Nb => "Norsk Bokmål",
            Self::Fi => "Suomi",
            Self::Cs => "Čeština",
            Self::Uk => "Українська",
        }
    }
}

/// 语言偏好设置 (跟随系统或固定指定语言)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LanguagePreference {
    #[default]
    Auto,
    Specific(Language),
}

impl LanguagePreference {
    pub fn resolved_language(&self) -> Language {
        match self {
            Self::Auto => detect_system_language(),
            Self::Specific(l) => *l,
        }
    }
}

/// 自动探测当前系统显示语言
#[cfg(windows)]
pub fn detect_system_language() -> Language {
    let lang_id = unsafe {
        windows_sys::Win32::Globalization::GetUserDefaultUILanguage()
    };
    match lang_id & 0x03FF {
        0x04 => {
            if lang_id == 0x0804 || lang_id == 0x1004 {
                Language::ZhHans
            } else {
                Language::ZhHant
            }
        }
        0x11 => Language::Ja,
        0x12 => Language::Ko,
        0x0c => Language::Fr,
        0x07 => Language::De,
        0x0a => Language::Es,
        0x16 => Language::Pt,
        0x10 => Language::It,
        0x19 => Language::Ru,
        0x13 => Language::Nl,
        0x15 => Language::Pl,
        0x1f => Language::Tr,
        0x01 => Language::Ar,
        0x1e => Language::Th,
        0x2a => Language::Vi,
        0x21 => Language::Id,
        0x1d => Language::Sv,
        0x06 => Language::Da,
        0x14 => Language::Nb,
        0x0b => Language::Fi,
        0x05 => Language::Cs,
        0x22 => Language::Uk,
        _ => Language::En,
    }
}

#[cfg(not(windows))]
pub fn detect_system_language() -> Language {
    if let Ok(lang) = std::env::var("LANG") {
        Language::from_locale_str(&lang)
    } else {
        Language::En
    }
}

/// GUI 与交互界面全栈国际化键值
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum I18nKey {
    HeaderRunning,
    HeaderRefreshHelp,
    BadgePending,
    BadgeProtected,
    StatusReady,
    StatusWorking,
    BtnTerminate,
    BtnViewAll,
    TabTargets,
    TabProtected,
    TabAll,
    EmptyTargetsTitle,
    EmptyTargetsSubtitle,
    ActionCleanGraceful,
    ActionCleanForce,
    ActionCleanPurge,
    RowAddWhitelist,
    RowRemoveWhitelist,
    RowForceKill,
    RowPurgeMemory,
    RowRevealInExplorer,
    RowCopyName,
    RowCopyPid,
    RowProperties,
    BtnSettings,
    BtnQuit,
    MenuLaunchAtLogin,
    MenuGlobalShortcut,
    MenuCliTools,
    MenuSortBy,
    SortComposite,
    SortMemory,
    SortCpu,
    SortWindows,
    SortDefault,
    MenuShowDetailedMetrics,
    MenuShowAppIdentifier,
    MenuShowSortButton,
    MenuOpenConfigFile,
    MenuOpenConfigDir,
    MenuGithubRepo,
    BtnAbout,
    LangAuto,
    CliStatusInstalled,
    CliStatusNotInstalled,
    CliMenuInstallUser,
    CliMenuTest,
    CliMenuReveal,
    CliMenuUninstall,
    UnitWindows,
    UnitWindowSingular,
}

pub fn tr(key: I18nKey, lang: Language) -> &'static str {
    match lang {
        Language::ZhHans => match key {
            I18nKey::HeaderRunning => "Task Cleaner",
            I18nKey::HeaderRefreshHelp => "重新扫描前台任务",
            I18nKey::BadgePending => "待结束",
            I18nKey::BadgeProtected => "已保护",
            I18nKey::StatusReady => "已就绪",
            I18nKey::StatusWorking => "处理中...",
            I18nKey::BtnTerminate => "结束",
            I18nKey::BtnViewAll => "查看全部活动任务",
            I18nKey::TabTargets => "待结束",
            I18nKey::TabProtected => "已保护",
            I18nKey::TabAll => "全部活动",
            I18nKey::EmptyTargetsTitle => "全部前台应用均受白名单保护",
            I18nKey::EmptyTargetsSubtitle => "无需执行任务清理操作",
            I18nKey::ActionCleanGraceful => "常规结束 (标准模式)",
            I18nKey::ActionCleanForce => "强制结束 (彻底清理)",
            I18nKey::ActionCleanPurge => "深度释放 (工作集整理)",
            I18nKey::RowAddWhitelist => "加入保护名单",
            I18nKey::RowRemoveWhitelist => "移出保护名单",
            I18nKey::RowForceKill => "强制结束任务",
            I18nKey::RowPurgeMemory => "深度释放内存",
            I18nKey::RowRevealInExplorer => "在文件资源管理器中显示",
            I18nKey::RowCopyName => "复制进程名称",
            I18nKey::RowCopyPid => "复制 PID",
            I18nKey::RowProperties => "查看进程属性",
            I18nKey::BtnSettings => "配置项",
            I18nKey::BtnQuit => "退出",
            I18nKey::MenuLaunchAtLogin => "开机自动启动",
            I18nKey::MenuGlobalShortcut => "全局快捷键",
            I18nKey::MenuCliTools => "命令行工具 (mtc)",
            I18nKey::MenuSortBy => "排序方式",
            I18nKey::SortComposite => "综合负载",
            I18nKey::SortMemory => "内存占用",
            I18nKey::SortCpu => "CPU 占用",
            I18nKey::SortWindows => "窗口数量",
            I18nKey::SortDefault => "默认字母",
            I18nKey::MenuShowDetailedMetrics => "显示详细遥测指标",
            I18nKey::MenuShowAppIdentifier => "显示进程可执行文件名",
            I18nKey::MenuShowSortButton => "显示顶栏排序按钮",
            I18nKey::MenuOpenConfigFile => "打开配置文件",
            I18nKey::MenuOpenConfigDir => "打开配置目录",
            I18nKey::MenuGithubRepo => "GitHub 仓库主页",
            I18nKey::BtnAbout => "关于 Task Cleaner",
            I18nKey::LangAuto => "跟随系统语言",
            I18nKey::CliStatusInstalled => "已加入系统命令路径",
            I18nKey::CliStatusNotInstalled => "未安装在命令路径中",
            I18nKey::CliMenuInstallUser => "安装至当前用户目录 (推荐)",
            I18nKey::CliMenuTest => "在终端中测试运行 (mtc -n)",
            I18nKey::CliMenuReveal => "在资源管理器中定位 mtc.exe",
            I18nKey::CliMenuUninstall => "卸载命令行工具",
            I18nKey::UnitWindows => "窗口",
            I18nKey::UnitWindowSingular => "窗口",
        },
        Language::ZhHant => match key {
            I18nKey::HeaderRunning => "Task Cleaner",
            I18nKey::HeaderRefreshHelp => "重新掃描前台任務",
            I18nKey::BadgePending => "待結束",
            I18nKey::BadgeProtected => "已保護",
            I18nKey::StatusReady => "已就緒",
            I18nKey::StatusWorking => "處理中...",
            I18nKey::BtnTerminate => "結束",
            I18nKey::BtnViewAll => "查看全部活動任務",
            I18nKey::TabTargets => "待結束",
            I18nKey::TabProtected => "已保護",
            I18nKey::TabAll => "全部活動",
            I18nKey::EmptyTargetsTitle => "全部前台應用均受白名單保護",
            I18nKey::EmptyTargetsSubtitle => "無需執行任務清理操作",
            I18nKey::ActionCleanGraceful => "常規結束 (標準模式)",
            I18nKey::ActionCleanForce => "強制結束 (徹底清理)",
            I18nKey::ActionCleanPurge => "深度釋放 (工作集整理)",
            I18nKey::RowAddWhitelist => "加入保護名單",
            I18nKey::RowRemoveWhitelist => "移出保護名單",
            I18nKey::RowForceKill => "強制結束任務",
            I18nKey::RowPurgeMemory => "深度釋放內存",
            I18nKey::RowRevealInExplorer => "在檔案總管中顯示",
            I18nKey::RowCopyName => "複製進程名稱",
            I18nKey::RowCopyPid => "複製 PID",
            I18nKey::RowProperties => "查看進程屬性",
            I18nKey::BtnSettings => "配置項",
            I18nKey::BtnQuit => "退出",
            I18nKey::MenuLaunchAtLogin => "開機自動啟動",
            I18nKey::MenuGlobalShortcut => "全局快捷鍵",
            I18nKey::MenuCliTools => "命令行工具 (mtc)",
            I18nKey::MenuSortBy => "排序方式",
            I18nKey::SortComposite => "綜合負載",
            I18nKey::SortMemory => "記憶體佔用",
            I18nKey::SortCpu => "CPU 佔用",
            I18nKey::SortWindows => "視窗數量",
            I18nKey::SortDefault => "默認字母",
            I18nKey::MenuShowDetailedMetrics => "顯示詳細遙測指標",
            I18nKey::MenuShowAppIdentifier => "顯示進程執行檔名稱",
            I18nKey::MenuShowSortButton => "顯示頂欄排序按鈕",
            I18nKey::MenuOpenConfigFile => "打開配置文件",
            I18nKey::MenuOpenConfigDir => "打開配置目錄",
            I18nKey::MenuGithubRepo => "GitHub 倉庫主頁",
            I18nKey::BtnAbout => "關於 Task Cleaner",
            I18nKey::LangAuto => "跟隨系統語言",
            I18nKey::CliStatusInstalled => "已加入系統命令路徑",
            I18nKey::CliStatusNotInstalled => "未安裝在命令路徑中",
            I18nKey::CliMenuInstallUser => "安裝至當前使用者目錄 (推薦)",
            I18nKey::CliMenuTest => "在終端機中測試運行 (mtc -n)",
            I18nKey::CliMenuReveal => "在檔案總管中定位 mtc.exe",
            I18nKey::CliMenuUninstall => "卸載命令行工具",
            I18nKey::UnitWindows => "視窗",
            I18nKey::UnitWindowSingular => "視窗",
        },
        Language::Ja => match key {
            I18nKey::HeaderRunning => "Task Cleaner",
            I18nKey::HeaderRefreshHelp => "フォアグラウンドタスクを再スキャン",
            I18nKey::BadgePending => "終了待機",
            I18nKey::BadgeProtected => "保護中",
            I18nKey::StatusReady => "準備完了",
            I18nKey::StatusWorking => "処理中...",
            I18nKey::BtnTerminate => "終了",
            I18nKey::BtnViewAll => "すべてのアクティブタスクを表示",
            I18nKey::TabTargets => "終了対象",
            I18nKey::TabProtected => "保護済み",
            I18nKey::TabAll => "すべて",
            I18nKey::EmptyTargetsTitle => "すべてのアプリが保護されています",
            I18nKey::EmptyTargetsSubtitle => "終了するタスクはありません",
            I18nKey::ActionCleanGraceful => "通常終了 (標準モード)",
            I18nKey::ActionCleanForce => "強制終了 (完全終了)",
            I18nKey::ActionCleanPurge => "ディープ解放 (ワーキングセット圧縮)",
            I18nKey::RowAddWhitelist => "保護リストに追加",
            I18nKey::RowRemoveWhitelist => "保護リストから削除",
            I18nKey::RowForceKill => "タスクを強制終了",
            I18nKey::RowPurgeMemory => "メモリをディープ解放",
            I18nKey::RowRevealInExplorer => "エクスプローラーで表示",
            I18nKey::RowCopyName => "プロセス名をコピー",
            I18nKey::RowCopyPid => "PID をコピー",
            I18nKey::RowProperties => "プロパティを表示",
            I18nKey::BtnSettings => "設定",
            I18nKey::BtnQuit => "終了",
            I18nKey::MenuLaunchAtLogin => "ログイン時に自動起動",
            I18nKey::MenuGlobalShortcut => "グローバルショートカット",
            I18nKey::MenuCliTools => "コマンドラインツール (mtc)",
            I18nKey::MenuSortBy => "並び替え",
            I18nKey::SortComposite => "総合負荷",
            I18nKey::SortMemory => "メモリ使用量",
            I18nKey::SortCpu => "CPU 使用率",
            I18nKey::SortWindows => "ウィンドウ数",
            I18nKey::SortDefault => "名前順",
            I18nKey::MenuShowDetailedMetrics => "詳細メトリクスを表示",
            I18nKey::MenuShowAppIdentifier => "実行ファイル名を表示",
            I18nKey::MenuShowSortButton => "ヘッダーのソートボタンを表示",
            I18nKey::MenuOpenConfigFile => "設定ファイルを開く",
            I18nKey::MenuOpenConfigDir => "設定フォルダを開く",
            I18nKey::MenuGithubRepo => "GitHub リポジトリ",
            I18nKey::BtnAbout => "Task Cleaner について",
            I18nKey::LangAuto => "システム言語に従う",
            I18nKey::CliStatusInstalled => "コマンドパスにインストール済み",
            I18nKey::CliStatusNotInstalled => "パスに未インストール",
            I18nKey::CliMenuInstallUser => "ユーザーディレクトリにインストール (推奨)",
            I18nKey::CliMenuTest => "ターミナルでテスト (mtc -n)",
            I18nKey::CliMenuReveal => "エクスプローラーで表示",
            I18nKey::CliMenuUninstall => "CLI ツールをアンインストール",
            I18nKey::UnitWindows => "ウィンドウ",
            I18nKey::UnitWindowSingular => "ウィンドウ",
        },
        _ => match key {
            I18nKey::HeaderRunning => "Task Cleaner",
            I18nKey::HeaderRefreshHelp => "Rescan foreground processes",
            I18nKey::BadgePending => "Pending",
            I18nKey::BadgeProtected => "Protected",
            I18nKey::StatusReady => "Ready",
            I18nKey::StatusWorking => "Processing...",
            I18nKey::BtnTerminate => "Clean",
            I18nKey::BtnViewAll => "View All Active Processes",
            I18nKey::TabTargets => "To Clean",
            I18nKey::TabProtected => "Protected",
            I18nKey::TabAll => "All Active",
            I18nKey::EmptyTargetsTitle => "All Foreground Apps Protected",
            I18nKey::EmptyTargetsSubtitle => "No pending processes to clean",
            I18nKey::ActionCleanGraceful => "Standard Terminate (WM_CLOSE)",
            I18nKey::ActionCleanForce => "Force Terminate (TerminateProcess)",
            I18nKey::ActionCleanPurge => "Deep Working Set Purge (K32EmptyWorkingSet)",
            I18nKey::RowAddWhitelist => "Add to Whitelist",
            I18nKey::RowRemoveWhitelist => "Remove from Whitelist",
            I18nKey::RowForceKill => "Force Terminate Process",
            I18nKey::RowPurgeMemory => "Purge Process Memory",
            I18nKey::RowRevealInExplorer => "Reveal in File Explorer",
            I18nKey::RowCopyName => "Copy Process Name",
            I18nKey::RowCopyPid => "Copy Process ID (PID)",
            I18nKey::RowProperties => "File Properties...",
            I18nKey::BtnSettings => "Settings",
            I18nKey::BtnQuit => "Quit",
            I18nKey::MenuLaunchAtLogin => "Launch at Startup",
            I18nKey::MenuGlobalShortcut => "Global Shortcut",
            I18nKey::MenuCliTools => "Command-Line Tool (mtc)",
            I18nKey::MenuSortBy => "Sort By",
            I18nKey::SortComposite => "Composite Load",
            I18nKey::SortMemory => "Memory Usage",
            I18nKey::SortCpu => "CPU Usage",
            I18nKey::SortWindows => "Window Count",
            I18nKey::SortDefault => "Alphabetical",
            I18nKey::MenuShowDetailedMetrics => "Show Detailed Metrics",
            I18nKey::MenuShowAppIdentifier => "Show Process Executable Name",
            I18nKey::MenuShowSortButton => "Show Header Sort Button",
            I18nKey::MenuOpenConfigFile => "Open Configuration File",
            I18nKey::MenuOpenConfigDir => "Open Configuration Folder",
            I18nKey::MenuGithubRepo => "GitHub Repository",
            I18nKey::BtnAbout => "About Task Cleaner",
            I18nKey::LangAuto => "Auto (System Default)",
            I18nKey::CliStatusInstalled => "Installed in System PATH",
            I18nKey::CliStatusNotInstalled => "Not Installed in PATH",
            I18nKey::CliMenuInstallUser => "Install to User Bin (Recommended)",
            I18nKey::CliMenuTest => "Test in Terminal (mtc -n)",
            I18nKey::CliMenuReveal => "Reveal mtc.exe in Explorer",
            I18nKey::CliMenuUninstall => "Uninstall CLI Tool",
            I18nKey::UnitWindows => "windows",
            I18nKey::UnitWindowSingular => "window",
        },
    }
}

pub fn localize_status_code(code: TerminationStatusCode, lang: Language) -> &'static str {
    match lang {
        Language::ZhHans => match code {
            TerminationStatusCode::Unknown => "未知",
            TerminationStatusCode::SuccessGraceful => "正常退出",
            TerminationStatusCode::SuccessForceTerminate => "强制终止",
            TerminationStatusCode::SuccessPurgeWorkingSet => "内存工作集深度释放成功",
            TerminationStatusCode::SkippedCallerLineage => "跳过 (调用者会话链路)",
            TerminationStatusCode::SkippedCriticalDaemon => "跳过 (系统关键进程)",
            TerminationStatusCode::FailedPermissionDenied => "失败 (权限不足)",
            TerminationStatusCode::FailedDispatch => "失败 (消息发送失败)",
            TerminationStatusCode::FailedTimeout => "失败 (超时无响应)",
            TerminationStatusCode::FailedKill => "失败 (无法强制终止)",
        },
        Language::ZhHant => match code {
            TerminationStatusCode::Unknown => "未知",
            TerminationStatusCode::SuccessGraceful => "正常退出",
            TerminationStatusCode::SuccessForceTerminate => "強制終止",
            TerminationStatusCode::SuccessPurgeWorkingSet => "記憶體工作集深度釋放成功",
            TerminationStatusCode::SkippedCallerLineage => "跳過 (調用者會話鏈路)",
            TerminationStatusCode::SkippedCriticalDaemon => "跳過 (系統關鍵進程)",
            TerminationStatusCode::FailedPermissionDenied => "失敗 (權限不足)",
            TerminationStatusCode::FailedDispatch => "失敗 (訊息發送失敗)",
            TerminationStatusCode::FailedTimeout => "失敗 (超時無響應)",
            TerminationStatusCode::FailedKill => "失敗 (無法強制終止)",
        },
        _ => match code {
            TerminationStatusCode::Unknown => "Unknown",
            TerminationStatusCode::SuccessGraceful => "Graceful exit",
            TerminationStatusCode::SuccessForceTerminate => "Force terminated",
            TerminationStatusCode::SuccessPurgeWorkingSet => "Working set purged",
            TerminationStatusCode::SkippedCallerLineage => "Skipped (caller lineage)",
            TerminationStatusCode::SkippedCriticalDaemon => "Skipped (critical daemon)",
            TerminationStatusCode::FailedPermissionDenied => "Failed (permission denied)",
            TerminationStatusCode::FailedDispatch => "Failed (message dispatch failed)",
            TerminationStatusCode::FailedTimeout => "Failed (timeout)",
            TerminationStatusCode::FailedKill => "Failed (force kill failed)",
        },
    }
}
