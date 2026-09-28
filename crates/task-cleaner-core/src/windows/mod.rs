#![cfg(windows)]

pub mod app;
pub mod signal;
pub mod telemetry;

pub use app::{get_process_memory_bytes, scan_foreground_apps};
pub use signal::{get_caller_lineage, is_explorer, is_process_alive, tiered_terminate};
pub use telemetry::{calculate_composite_score, sort_targets};
