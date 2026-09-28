use crate::model::{AppTarget, SortMode};

/// 计算 5 维综合负载评分 (严格对齐 macOS 版权重)
/// - 内存权重 40% (以 100MB 为基准单位)
/// - CPU 权重 40% (每个 1% 算 2.0 分)
/// - 窗口数量权重 20% (每个可见主窗口算 5.0 分)
pub fn calculate_composite_score(memory_mb: f64, cpu_percent: f64, window_count: usize) -> f64 {
    (memory_mb / 100.0) * 0.4 + (cpu_percent * 2.0) * 0.4 + (window_count as f64 * 5.0) * 0.2
}

/// 按照指定模式进行就地排序
pub fn sort_targets(targets: &mut [AppTarget], mode: SortMode) {
    match mode {
        SortMode::Composite => {
            targets.sort_by(|a, b| {
                b.composite_score
                    .partial_cmp(&a.composite_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortMode::Memory => {
            targets.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        }
        SortMode::Cpu => {
            targets.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortMode::Windows => {
            targets.sort_by(|a, b| b.window_count.cmp(&a.window_count));
        }
        SortMode::Default => {
            targets.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.pid.cmp(&b.pid))
            });
        }
    }
}
