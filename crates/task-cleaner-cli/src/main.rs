use std::env;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use task_cleaner_core::{
    get_caller_lineage, scan_foreground_apps, sort_targets, tiered_terminate, AppTarget,
    SortMode, TerminationMode, TerminationReport, WhitelistManager, WhitelistMatch,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default, Debug, Clone)]
struct CliArgs {
    dry_run: bool,
    execute: bool,
    force: bool,
    json: bool,
    cli_keeps: Vec<String>,
    add_whitelist: Vec<String>,
    remove_whitelist: Vec<String>,
    terminate_pids: Vec<u32>,
    sort_mode: SortMode,
    show_help: bool,
    show_version: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliJsonSummary {
    pub scanned_total: usize,
    pub protected_count: usize,
    pub target_count: usize,
    pub scan_duration_ms: f64,
    pub targets: Vec<AppTarget>,
    pub protected: Vec<ProtectedJsonEntry>,
    pub total_target_memory_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedJsonEntry {
    pub target: AppTarget,
    pub tier: String,
    pub rule: String,
}

fn parse_cli_args() -> CliArgs {
    let mut args = env::args().skip(1).peekable();
    let mut cli = CliArgs::default();
    cli.dry_run = true; // 默认预览模式，避免用户误杀

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-n" | "--dry-run" | "--preview" => {
                cli.dry_run = true;
                cli.execute = false;
            }
            "-e" | "--execute" | "--clean" | "-k" if arg == "--clean" => {
                cli.execute = true;
                cli.dry_run = false;
            }
            "-f" | "--force" => {
                cli.force = true;
                cli.execute = true;
                cli.dry_run = false;
            }
            "--json" => {
                cli.json = true;
            }
            "-k" | "--keep" => {
                if let Some(val) = args.next() {
                    cli.cli_keeps.push(val);
                }
            }
            "-a" | "--add-whitelist" => {
                if let Some(val) = args.next() {
                    cli.add_whitelist.push(val);
                }
            }
            "-r" | "--remove-whitelist" => {
                if let Some(val) = args.next() {
                    cli.remove_whitelist.push(val);
                }
            }
            "-s" | "--sort" => {
                if let Some(val) = args.next() {
                    cli.sort_mode = SortMode::from_str_loose(&val);
                }
            }
            "-t" | "--terminate-pid" => {
                if let Some(val) = args.next() {
                    if let Ok(pid) = val.parse::<u32>() {
                        cli.terminate_pids.push(pid);
                        cli.execute = true;
                        cli.dry_run = false;
                    }
                }
            }
            "-h" | "--help" => {
                cli.show_help = true;
            }
            "-v" | "--version" => {
                cli.show_version = true;
            }
            _ => {}
        }
    }

    cli
}

fn print_help() {
    println!(
        r#"Task Cleaner CLI (mtc) - Windows 11 原生前台任务与进程管理工具 v{}

用法:
    mtc.exe [选项]

选项:
    -n, --dry-run               [默认] 仅预检扫描并打印待清理目标，不执行退出
    -e, --execute, --clean      正式执行未受保护前台应用的退出清理
    -f, --force                 强制直接终止 (立即 TerminateProcess，资源管理器除外)
    -s, --sort <模式>           设置结果排序模式: composite(综合), memory(内存), cpu, windows, default
    -k, --keep <进程名>         本次运行临时将指定应用列入保护
    -a, --add-whitelist <进程名> 将指定进程名永久追加至用户白名单 (~/.config/taskcleaner/config.toml)
    -r, --remove-whitelist <名> 从用户白名单中移除指定规则
    -t, --terminate-pid <PID>   指定并仅终止单个 PID 进程
        --json                  以结构化 JSON 输出结果 (第三方集成标准)
    -h, --help                  打印帮助文档
    -v, --version               打印版本信息
"#,
        VERSION
    );
}

fn main() {
    let args = parse_cli_args();

    if args.show_help {
        print_help();
        return;
    }

    if args.show_version {
        println!("Task Cleaner CLI (mtc) v{}", VERSION);
        return;
    }

    let mut whitelist = WhitelistManager::new();

    // 处理白名单增删配置
    if !args.add_whitelist.is_empty() {
        for rule in &args.add_whitelist {
            if whitelist.add_user_rule(rule) {
                println!("[SUCCESS] 已将 \"{}\" 添加到用户保护白名单", rule);
            }
        }
        return;
    }

    if !args.remove_whitelist.is_empty() {
        for rule in &args.remove_whitelist {
            if whitelist.remove_user_rule(rule) {
                println!("[SUCCESS] 已将 \"{}\" 从用户白名单中移除", rule);
            }
        }
        return;
    }

    whitelist.set_cli_keep_rules(&args.cli_keeps);

    let start = Instant::now();
    let caller_lineage = get_caller_lineage();
    let mut scanned_apps = scan_foreground_apps();
    let scan_duration = start.elapsed().as_secs_f64() * 1000.0;

    let mut protected_list: Vec<(AppTarget, WhitelistMatch)> = Vec::new();
    let mut target_list: Vec<AppTarget> = Vec::new();

    for app in scanned_apps.drain(..) {
        if !args.terminate_pids.is_empty() {
            if args.terminate_pids.contains(&app.pid) {
                target_list.push(app);
            } else {
                // 不在指定列表内的均视同保护
                protected_list.push((
                    app,
                    WhitelistMatch {
                        tier: task_cleaner_core::WhitelistTier::L4CliOverride,
                        tier_label: "未被选定".to_string(),
                        matched_rule: "Explicit PID filter".to_string(),
                    },
                ));
            }
            continue;
        }

        if let Some(matched) = whitelist.classify(&app, &caller_lineage) {
            protected_list.push((app, matched));
        } else {
            target_list.push(app);
        }
    }

    sort_targets(&mut target_list, args.sort_mode);

    let total_target_mem_mb: f64 = target_list.iter().map(|t| t.memory_mb()).sum();

    // 1. JSON 格式输出
    if args.json {
        let json_summary = CliJsonSummary {
            scanned_total: protected_list.len() + target_list.len(),
            protected_count: protected_list.len(),
            target_count: target_list.len(),
            scan_duration_ms: scan_duration,
            targets: target_list.clone(),
            protected: protected_list
                .iter()
                .map(|(t, m)| ProtectedJsonEntry {
                    target: t.clone(),
                    tier: m.tier_label.clone(),
                    rule: m.matched_rule.clone(),
                })
                .collect(),
            total_target_memory_mb: total_target_mem_mb,
        };
        if let Ok(j) = serde_json::to_string_pretty(&json_summary) {
            println!("{}", j);
        }
        return;
    }

    // 2. 终端预览模式 (Dry-Run)
    if args.dry_run {
        println!("================================================================================");
        println!(
            " Task Cleaner CLI (Windows 11) v{} - 前台任务预检扫描",
            VERSION
        );
        println!("================================================================================");
        println!(
            "已扫描: {} 个应用  |  已保护: {} 个  |  待清理: {} 个  |  耗时: {:.1} ms",
            protected_list.len() + target_list.len(),
            protected_list.len(),
            target_list.len(),
            scan_duration
        );
        println!("排序模式: {}  |  预计释放物理常驻内存: {:.1} MB", args.sort_mode.label(), total_target_mem_mb);
        println!("--------------------------------------------------------------------------------");

        if target_list.is_empty() {
            println!("\n[INFO] 当前所有前台应用均受白名单保护，无需清理。\n");
        } else {
            println!("\n[待清理的前台任务目标]:");
            println!(
                "{:<8} {:<24} {:<12} {:<8} {:<6} {}",
                "PID", "进程名", "常驻内存", "CPU%", "窗口", "主窗口标题"
            );
            println!("{}", "-".repeat(80));
            for t in &target_list {
                println!(
                    "{:<8} {:<24} {:<12} {:<8} {:<6} {}",
                    t.pid,
                    truncate_str(&t.name, 22),
                    format!("{:.0} MB", t.memory_mb()),
                    format!("{:.1}%", t.cpu_percent),
                    t.window_count,
                    truncate_str(&t.title, 26)
                );
            }
        }

        if !protected_list.is_empty() {
            println!("\n[受保护的应用 (白名单豁免)]: ");
            for (p, m) in &protected_list {
                println!(
                    "  * PID {:<6} {:<22} [{}] (命中: {})",
                    p.pid,
                    truncate_str(&p.name, 20),
                    m.tier_label,
                    m.matched_rule
                );
            }
        }

        println!("\n[提示] 确认清理以上待结束任务，请使用执行参数: mtc --clean 或 mtc -e");
        return;
    }

    // 3. 执行终止模式
    if args.execute {
        println!("================================================================================");
        println!(
            " Task Cleaner CLI (Windows 11) - 正在执行任务清理 (共 {} 个目标)...",
            target_list.len()
        );
        println!("================================================================================");

        let mode = if args.force {
            TerminationMode::ForceImmediate
        } else {
            TerminationMode::Standard
        };

        let report: TerminationReport = tiered_terminate(&target_list, mode, 400, &whitelist);

        println!(
            "清理结果: 成功退出 {} 个 (优雅退出: {}, 强制终止: {}), 失败 {} 个, 耗时 {:.1} ms",
            report.terminated_graceful + report.terminated_force,
            report.terminated_graceful,
            report.terminated_force,
            report.failed,
            report.duration_ms
        );

        println!("{}", "-".repeat(80));
        for rec in &report.records {
            println!(
                "  PID {:<6} {:<24} => {} {}",
                rec.app.pid,
                truncate_str(&rec.app.name, 22),
                rec.status,
                rec.exit_signal.as_deref().unwrap_or("")
            );
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(2)).collect();
        format!("{}..", truncated)
    }
}
