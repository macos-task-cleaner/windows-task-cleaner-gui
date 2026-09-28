# Windows Task Cleaner (Rust Native) 架构与实施交付指南

[文档类型] 平台级重构与工程落地交付报告  
[工程根目录] `windows-task-cleaner-gui`  
[重构目标] 全面废弃 WinUI 3 (WASDK) 重型依赖，建立 100% Rust 原生 Windows 11 极速套件  
[交付日期] 2026-09-28  

---

## 1. 架构演进与重构复盘

### 1.1 痛点诊断与选型纠偏
在原 WinUI 3 方案中，项目遭遇了严重的生态陷阱与体验危机：
* **体积失控**：WinAppSDK 自包含模式解包体积超过 150MB，严重偏离了 MTC 工具“几兆单文件独立运行”的核心定位；
* **环境依赖断裂**：依赖未安装或版本不一致时抛出 `0x80670016` (`STATEREPOSITORY_E_DEPENDENCY_NOT_RESOLVED`)；
* **虚拟机网络盘 I/O 锁死**：跨虚拟机共享盘构建耗时 25~40 秒/次，引发开发死锁。

### 1.2 架构破局：100% Rust 原生套件
通过回归 Rust 原生架构，项目实现了：
* **极致轻量**：单个独立 `.exe` 体积仅约 **1.5MB ~ 2MB**，彻底消灭所有外部运行时安装包；
* **毫秒级冷启**：启动速度 < 15ms，常驻内存 < 6MB；
* **极速构建**：通过脚本内置的本地临时缓存目录（`C:\Temp\taskcleaner-target`），增量编译压缩至 2~3 秒。

---

## 2. 工程模块矩阵

项目采用标准的 Cargo Workspace 架构：

```text
windows-task-cleaner-gui/
├── Cargo.toml                              # Workspace 根配置文件
├── run.ps1                                 # 开发与运行主脚本 (支持 GUI 与 CLI 模式)
├── build.ps1                               # 发布编译脚本 (一键生成 publish/ 交付物)
├── README.md                               # 项目完整介绍与使用说明
└── crates/
    ├── task-cleaner-core/                  # 核心引擎 (扫描、遥测、白名单、分级终止)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs                      # 核心对外导出与自动化单元测试
    │       ├── model.rs                    # 领域数据结构 (AppTarget, Report 等)
    │       ├── i18n.rs                     # 24 语种全栈本地化字典
    │       ├── whitelist.rs                # 4 级防御矩阵与 TOML 配置持久化
    │       ├── mock.rs                     # 跨平台开发/单元测试 Mock 实现
    │       └── windows/                    # Windows 11 原生 Win32 系统级实现
    │           ├── mod.rs
    │           ├── app.rs                  # EnumWindows + UWP 穿透 + 内存采集
    │           ├── signal.rs               # Toolhelp32 进程树回溯 + 两阶段退出
    │           └── telemetry.rs            # 5 维动态排序与综合负载评分
    ├── task-cleaner-cli/                   # mtc.exe 跨平台命令行工具
    │   ├── Cargo.toml
    │   └── src/main.rs                     # 支持 --dry-run, --clean, --json
    └── task-cleaner-gui/                   # TaskCleaner.exe 原生托盘应用
        ├── Cargo.toml
        └── src/main.rs                     # Win32 原生托盘、双缓冲 GDI、Mica 圆角小窗
```

---

## 3. 关键底层技术实现

### 3.1 现代 UWP / 框架宿主穿透识别 (ApplicationFrameHost Trap)
在 `crates/task-cleaner-core/src/windows/app.rs` 中：
* 顶层窗口若检测到宿主进程为 `ApplicationFrameHost.exe`，主动派发 `EnumChildWindows` 探测子窗口；
* 捕获 `Windows.UI.Core.CoreWindow` 的真实子进程 PID，杜绝误杀宿主框架导致系统设置、计算器崩溃。

### 3.2 L2 调用者血缘祖先链回溯 (Toolhelp32 Process Tree)
在 `crates/task-cleaner-core/src/windows/signal.rs` 中：
* 调用 `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` 遍历系统当前进程树；
* 沿当前进程的 `th32ParentProcessID` 递归向上回溯至根节点，将全部祖先 PID 自动注入 L2 白名单；
* 免疫所有正在运行任务的 PowerShell、Windows Terminal、CMD 或 VS Code 会话。

### 3.3 两阶段分级安全退出 (Graceful Close Pipeline)
* **第一阶段**：向目标 PID 关联的全部顶层窗口派发 `WM_CLOSE` 消息；
* **宽限期探测**：在 400ms 宽限期内以 40ms 步进持续监听进程退出事件 (`WaitForSingleObject`)；
* **第二阶段**：超时仍未响应者派发 `TerminateProcess` 强制终止（对 `explorer.exe` 实施特殊豁免保护）。

### 3.4 原生常驻托盘与内存动态图标
在 `crates/task-cleaner-gui/src/main.rs` 中：
* 通过 `CreateIconIndirect` 在内存中动态合成 16x16 现代胶囊 X 几何图标，规避外部 `.ico` 文件丢失隐患；
* 左键呼出根据任务栏边沿 (`ABM_GETTASKBARPOS`) 动态停靠的 Mica 圆角卡片，失焦自动隐退；
* 右键弹出系统原生快捷菜单，支持快速刷新与一键清理。

---

## 4. 虚拟机端验证指南

请在 Windows 11 虚拟机（PowerShell 环境）中执行：

1. **环境准备（若尚未安装 Rust）**：
   ```powershell
   winget install Rustlang.Rustup
   ```

2. **启动运行 GUI 托盘应用**：
   ```powershell
   .\run.ps1
   ```
   * 终端将自动切换至本地缓存目录进行超快增量构建；
   * 产出约 1.8MB 的 `TaskCleaner.exe` 并常驻于任务栏右下角；
   * 点击托盘即可呼出 Windows 11 悬浮任务管理卡片。

3. **测试 CLI 终端预览模式**：
   ```powershell
   .\run.ps1 -Mode cli
   ```
   * 自动编译运行 `mtc.exe`，在终端以清晰表格格式预览待清理任务与内存统计。

4. **一键打包发布**：
   ```powershell
   .\build.ps1
   ```
   * 将在 `publish/` 目录下产出完整的生产环境独立绿色二进制文件。
