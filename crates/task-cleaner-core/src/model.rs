use serde::{Deserialize, Serialize};

/// 目标前台应用/进程定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTarget {
    pub pid: u32,
    pub name: String,
    pub bundle_id: String, // Windows: 进程名或执行文件名 (例如 "chrome.exe")
    pub title: String,     // 主窗口标题
    pub exe_path: String,  // 可执行文件完整路径
    #[serde(default)]
    pub memory_bytes: u64,
    #[serde(default)]
    pub cpu_percent: f64,
    #[serde(default)]
    pub window_count: usize,
    #[serde(default)]
    pub composite_score: f64,
}

impl AppTarget {
    pub fn new(pid: u32, name: impl Into<String>, bundle_id: impl Into<String>) -> Self {
        Self {
            pid,
            name: name.into(),
            bundle_id: bundle_id.into(),
            title: String::new(),
            exe_path: String::new(),
            memory_bytes: 0,
            cpu_percent: 0.0,
            window_count: 1,
            composite_score: 0.0,
        }
    }

    pub fn memory_mb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn metrics_display(&self) -> String {
        format!(
            "{:.0} MB · {:.1}% · {} 窗口",
            self.memory_mb(),
            self.cpu_percent,
            self.window_count
        )
    }
}

/// 5 维动态排序模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortMode {
    /// 综合负载: (RAM_MB / 100) * 0.4 + (CPU% * 2) * 0.4 + (窗口数 * 5) * 0.2
    #[default]
    Composite,
    /// 内存占用: 按物理常驻内存降序
    Memory,
    /// CPU 占用: 按动态处理器使用率降序
    Cpu,
    /// 窗口数量: 按在屏可见主窗口数量降序
    Windows,
    /// 默认顺序: 按进程名/标题字母稳定升序
    Default,
}

impl SortMode {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "memory" | "mem" | "ram" => Self::Memory,
            "cpu" => Self::Cpu,
            "windows" | "win" | "window" => Self::Windows,
            "default" | "name" | "alpha" => Self::Default,
            _ => Self::Composite,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Composite => "综合负载",
            Self::Memory => "内存占用",
            Self::Cpu => "CPU 占用",
            Self::Windows => "窗口数量",
            Self::Default => "默认顺序",
        }
    }
}

/// 系统清理模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TerminationMode {
    /// 标准梯次终止: 分发 WM_CLOSE -> 400ms 宽限期轮询 -> TerminateProcess 强制兜底
    #[default]
    Standard,
    /// 强制直接终止: 立即 TerminateProcess (对 explorer.exe 仍予以特殊保护)
    ForceImmediate,
}

/// 进程终止状态强类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TerminationStatusCode {
    #[default]
    Unknown,
    /// WM_CLOSE 优雅退出成功
    SuccessGraceful,
    /// TerminateProcess 强制终止成功
    SuccessForceTerminate,
    /// 调用者祖先会话链路保护，安全跳过
    SkippedCallerLineage,
    /// 系统底层核心守护进程保护 (如 explorer.exe / dwm.exe)，安全跳过
    SkippedCriticalDaemon,
    /// 信号派发权限拒绝 (Access Denied)
    FailedPermissionDenied,
    /// WM_CLOSE 消息投递失败
    FailedDispatch,
    /// 宽限期超时且仍未退出
    FailedTimeout,
    /// 强制终止失败 (TerminateProcess 失败)
    FailedKill,
}

/// 单个应用的终止处置明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTerminationRecord {
    pub app: AppTarget,
    pub status: String,
    #[serde(default)]
    pub status_code: TerminationStatusCode,
    pub exit_signal: Option<String>,
    pub error_msg: Option<String>,
}

/// 清场执行报告汇总 (完全对齐 MTC 跨平台标准 JSON 输出)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerminationReport {
    pub total_targets: usize,
    pub terminated_graceful: usize,
    pub terminated_force: usize,
    pub failed: usize,
    pub duration_ms: f64,
    pub records: Vec<ProcessTerminationRecord>,
}
