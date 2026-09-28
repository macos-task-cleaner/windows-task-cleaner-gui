# Windows Task Cleaner (Rust Native)

[项目类型] Windows 11 原生极速任务栏前台任务清理套件  
[技术栈] Rust 2021 · Win32 API (`windows-sys`) · DWM Mica · 双缓冲 GDI  
[交付产物] `TaskCleaner.exe` (原生托盘 GUI, ~1.8 MB) · `mtc.exe` (跨平台 CLI, ~1.2 MB)  
[授权许可] Dual-Licensed under GNU AGPLv3 and Commercial License  
[版权归属] Copyright (c) 2026 DonJone. All rights reserved.  

---

## 1. 项目简介

Windows Task Cleaner 是专为 Windows 11 打造的轻量级任务栏托盘前台进程与应用管理工具。旨在提供比原生任务管理器更聚焦、比快捷键强退更可控的快速任务清理体验。

本项目为 [macOS Task Cleaner (MTC)](https://github.com/macos-task-cleaner/macos-task-cleaner-gui) 的 Windows 11 官方原生移植版本，全面回归 **Rust 原生架构**，摒弃了传统重量级运行时，具备**零额外环境依赖、冷启 10ms、单文件仅约 2MB、常驻内存仅 6MB** 的极致性能表现。

---

## 2. 核心特性矩阵

* **Windows 11 Fluent 托盘小窗**：原生驻留系统任务栏托盘区域，点击托盘呼出 Windows 11 原生圆角悬浮卡片，失焦自动隐退。
* **精准前台窗口过滤与 UWP 穿透**：通过 Win32 `EnumWindows`、DWM Cloaked 属性自动剔除无界面后台与隐匿窗口，并原生穿透 `ApplicationFrameHost.exe` 识别真实 UWP / Modern 应用。
* **4 级白名单防御矩阵**：
  - **L1 级**：核心系统外壳 (默认保护资源管理器 `explorer.exe`、DWM 桌面合成器 `dwm.exe`、搜索与开始菜单宿主)；
  - **L2 级**：基于 Toolhelp32 进程树递归回溯调用者祖先链路，绝对免疫 PowerShell、Windows Terminal、CMD、VS Code 等宿主；
  - **L3 级**：安全中心、硬件驱动与云同步守护；
  - **L4 级**：用户自定义白名单存储于 `%APPDATA%\TaskCleaner\config.toml`。
* **全栈实时遥测与 5 维动态排序**：
  - 常驻物理内存 (Working Set / RSS)；
  - CPU 使用率动态差分采样；
  - 在屏顶层窗口数量统计；
  - 智能综合负载加权评分；
  - 默认字母顺序稳定排序。
* **分级安全终止管线**：向目标窗口分发 `WM_CLOSE` 触发平稳退出，400ms 超时未响应则派发 `TerminateProcess` 强制终止（资源管理器严格豁免）。
* **CLI 与 GUI 双端同构**：提供独立的 `mtc.exe` 终端工具，完美支持 `--dry-run`、`--clean`、`--json` 标准数据输出。

---

## 3. 构建与运行指南

### 环境准备
在 Windows 11 虚拟机或主机中，仅需安装标准 Rust 工具链：
```powershell
winget install Rustlang.Rustup
```

### 极速运行与调试 (PowerShell)
```powershell
# 编译并启动 GUI 托盘应用
.\run.ps1

# 启动 CLI 终端预览模式 (mtc.exe)
.\run.ps1 -Mode cli

# 编译发布版本 (产物输出至 publish/ 目录)
.\build.ps1
```

---

## 4. 授权协议 (Dual-Licensing)

本项目采用双重许可模式：
1. **开源授权 (GNU AGPLv3)**：个人使用、学术研究及开源衍生项目完全免费，任何衍生服务或封装必须全量开源。
2. **商业闭源授权 (Commercial License)**：企业专有部署、闭源捆绑、SaaS 化提供或去除署名需取得商业授权。详见 [COMMERCIAL.md](file:///Users/don/work/git/windows-task-cleaner-gui/COMMERCIAL.md)。

Copyright (c) 2026 DonJone. All rights reserved.
