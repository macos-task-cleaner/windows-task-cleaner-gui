# Windows Task Cleaner (WinUI 3)

[项目类型] Windows 11 原生高性能任务栏前台任务清理工具  
[技术栈] C# 12 / .NET 8 (LTS) · WinUI 3 · Windows App SDK (WASDK) · Fluent Design  
[授权许可] Dual-Licensed under GNU AGPLv3 and Commercial License  
[版权归属] Copyright (c) 2026 DonJone. All rights reserved.  

---

## 1. 项目简介

Windows Task Cleaner 是专为 Windows 11 打造的轻量级任务栏托盘前台进程与应用管理工具。旨在提供比原生任务管理器更聚焦、比快捷键强退更可控的快速任务清理体验。

本项目为 [macOS Task Cleaner (MTC)](https://github.com/macos-task-cleaner/macos-task-cleaner-gui) 的 Windows 11 官方原生移植版本，采用 WinUI 3 与 Windows 11 Fluent Design (Mica Alt 材质、原生圆角与动态阴影) 构建。

---

## 2. 核心特性矩阵

* **Windows 11 Fluent 原生美学**：基于 Mica Alt 深度材质与 Segoe UI Variable 字体打造，自适应 Windows 11 浅色/深色主题，驻留系统任务栏托盘区域。
* **精准前台窗口过滤**：通过 Win32 `EnumWindows` 与桌面可见性算法筛选用户真正交互的前台活动应用，自动剔除无界面后台与系统隐匿窗口。
* **4 级白名单安全防御**：
  - **L1 级**：核心系统外壳 (默认保护资源管理器 `explorer.exe`、DWM 桌面合成器 `dwm.exe`、搜索与开始菜单宿主)；
  - **L2 级**：当前终端与开发会话回溯保护 (Windows Terminal, PowerShell, CMD, VS Code)；
  - **L3 级**：安全中心、硬件面板与云同步守护；
  - **L4 级**：用户自定义白名单存储于 `%APPDATA%\TaskCleaner\config.json`。
* **全栈实时遥测与 5 维动态排序**：
  - 常驻物理内存 (Working Set / RSS)；
  - CPU 使用率动态差分采样；
  - 在屏顶层窗口数量统计；
  - 智能综合负载加权评分；
  - 默认字母顺序稳定排序。
* **分级安全终止管线**：向目标窗口分发 `WM_CLOSE` 触发平稳退出，400ms 超时未响应则派发 `TerminateProcess` 强制终止。
* **全局快捷键与单文件免安装分发**：支持全局热键监听 (`RegisterHotKey`) 与独立单文件便携运行。

---

## 3. 架构与工程实施计划

完整的系统级 API 对齐、白名单矩阵与阶段路线图，详见项目工程计划书：
* [WINDOWS_WINUI3_PORTING_PLAN.md](file:///Users/don/work/git/windows-task-cleaner-gui/WINDOWS_WINUI3_PORTING_PLAN.md)

---

## 4. 授权协议 (Dual-Licensing)

本项目采用双重许可模式：
1. **开源授权 (GNU AGPLv3)**：个人使用、学术研究及开源衍生项目完全免费，任何衍生服务或封装必须全量开源。
2. **商业闭源授权 (Commercial License)**：企业专有部署、闭源捆绑、SaaS 化提供或去除署名需取得商业授权。详见 [COMMERCIAL.md](file:///Users/don/work/git/windows-task-cleaner-gui/COMMERCIAL.md)。

Copyright (c) 2026 DonJone. All rights reserved.
