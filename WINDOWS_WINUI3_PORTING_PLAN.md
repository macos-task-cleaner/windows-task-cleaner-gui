# Windows 11 (WinUI 3) 任务清理工具移植工程实施计划

[文档类型] 平台移植与系统级架构工程计划书  
[目标平台] Windows 11 (22H2 / 23H2+，x64 与 ARM64 双架构支持)  
[技术架构] C# 12 / .NET 8 (LTS) + WinUI 3 (Windows App SDK 1.5+)  
[源工程基底] macOS Task Cleaner (SwiftUI / AppKit / Rust Core)  
[创建日期] 2026-09-28  

---

## 1. 工程背景与移植宗旨

macOS Task Cleaner (MTC) 在 macOS 平台通过纯原生运行时桥接、4 级白名单防御矩阵、Liquid Glass 拟真材质设计与 5 维实时遥测排序，成功打造了高口碑、零开销的前台任务管理体验。

针对 Windows 11 环境，现有系统级“任务管理器 (Task Manager)”功能虽然全面但过于沉重，用户在遇到前台程序卡死、遗留无用后台、多窗口杂乱时，缺乏一个驻留于任务栏通知区 (System Tray)、可一键轻量清理、且具备 Windows 11 Fluent Design 原生质感的敏捷实用工具。

本项目旨在将 MTC 的核心心智模型与架构完整平移至 Windows 11，打造第一方水准的 WinUI 3 原生任务托盘面板。

---

## 2. 技术选型与技术栈决策矩阵

经过对 Windows 现代桌面开发技术栈的深度评估，确立如下核心选型：

| 维度 | 选定方案 | 备选对比 (弃用原因) |
| :--- | :--- | :--- |
| **应用与 UI 框架** | **WinUI 3 + Windows App SDK (WASDK)** | WPF (技术较旧、Mica 材质需 Hack)、Electron (内存开销 150MB+，违背轻量初衷)、Flutter (非原生 Windows 控件体系) |
| **宿主运行时** | **.NET 8 (LTS) / C# 12** | C++ / WinRT (开发往返耗时过长，XAML 编译复杂，难以快速迭代) |
| **架构模式** | **MVVM (CommunityToolkit.Mvvm)** | 原生 Code-Behind (高耦合难以维护) |
| **系统托盘集成** | **H.NotifyIcon.WinUI** | 自写 Win32 Shell_NotifyIconW 消息泵 (维护成本高，易出现托盘图标重影与 DPI 缩放闪烁) |
| **Win32 底层互操作** | **Microsoft.Windows.CsWin32 (源码生成器)** | 手工编写 `[DllImport]` P/Invoke (类型安全度低，易发生结构体对齐 Bug) |
| **打包与分发形式** | **非打包免安装单文件 (Unpackaged Single-File) + 可选 MSIX** | 纯粹 MSIX (缺少安装权限的企业机环境无法启动，调试繁琐) |

---

## 3. macOS 与 Windows 11 核心子系统映射表

| 核心功能点 | macOS 原生实现 (`macos-task-cleaner`) | Windows 11 对应实现 (`windows-task-cleaner-gui`) |
| :--- | :--- | :--- |
| **前台应用发现** | `NSWorkspace.runningApplications` (匹配 `activationPolicy == .regular`) | `EnumWindows` 遍历顶层窗口 + `IsWindowVisible` + `GetWindowLong(GWL_EXSTYLE) != WS_EX_TOOLWINDOW` |
| **进程 PID 提取** | `NSRunningApplication.processIdentifier` | `GetWindowThreadProcessId(hWnd, out pid)` |
| **进程映像路径** | `NSRunningApplication.bundleURL` | `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW` |
| **应用名称与图标** | `NSRunningApplication.localizedName` + `icon` | `FileVersionInfo.FileDescription` + `PrivateExtractIconsW` / `SHGetFileInfoW` 提取 32x32 与 48x48 高清图标 |
| **物理常驻内存** | Darwin `libproc` (`proc_pidinfo` / `pti_resident_size`) | Win32 PSAPI: `GetProcessMemoryInfo` (`PROCESS_MEMORY_COUNTERS.WorkingSetSize`) |
| **CPU 占用率差分采样** | `proc_taskinfo` (`pti_total_user + pti_total_system`) 纳秒差分 | `GetProcessTimes` (核算 `UserTime + KernelTime` 64 位时钟滴答增量 / 采样间隔) |
| **窗口数量感知** | `CGWindowListCopyWindowInfo` (Layer 0 在屏计数) | 在 `EnumWindows` 扫描过程中累加同一 PID 的可见桌面顶层主窗口计数 |
| **安全两阶段退出** | AppKit `.terminate()` -> 400ms 轮询 -> `libc::kill(SIGKILL)` | 向主窗口派发 `PostMessage(hWnd, WM_CLOSE, 0, 0)` -> 400ms 等待 -> `TerminateProcess(hProcess, 1)` |
| **特殊外壳保护** | Finder (`com.apple.finder` 特殊退出防崩溃重启) | Explorer (`explorer.exe` 桌面与任务栏宿主，严禁强制终止导致黑屏，仅允许优雅退出) |
| **全局快捷键** | Carbon `RegisterEventHotKey` + `AXIsProcessTrusted` 状态感知 | Win32 `RegisterHotKey(hWnd, id, fsModifiers, vk)` + 消息循环分发 `WM_HOTKEY` |
| **开机自启** | macOS 13+ `SMAppService.mainApp` | Windows 注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 或 StartupTask |
| **材质设计语言** | Liquid Glass (`.thinMaterial` / NSVisualEffectView) | Windows 11 Fluent Design (`Mica Alt` / `DesktopAcrylic` 背景材质 + DWM 8px 圆角) |

---

## 4. 4 级白名单防御矩阵 (Windows 11 定制版)

为了保证任务清理过程绝对不破坏 Windows 系统的稳定性，构建针对 Windows 特性的分级白名单体系：

### L1 级：核心系统守护 (绝对保护，不可突破)
* `explorer.exe` (Windows 资源管理器/任务栏外壳，默认保护，移出保护时必须采用 `WM_CLOSE` 优雅退出)
* `dwm.exe` (桌面窗口管理器 Desktop Window Manager)
* `csrss.exe` (客户机/服务器运行时子系统)
* `smss.exe` (会话管理子系统)
* `lsass.exe` (本地安全机构服务)
* `services.exe` (Windows 服务控制管理器)
* `svchost.exe` (通用服务宿主)
* `System` (PID 4 操作系统内核)
* `SearchHost.exe` / `StartMenuExperienceHost.exe` / `ShellExperienceHost.exe` (Windows 11 开始菜单与搜索宿主)

### L2 级：当前调用者与开发上下文血缘回溯
* 正在执行本工具的宿主进程自身 (`TaskCleaner.exe`)；
* 用户的交互终端与开发会话：`WindowsTerminal.exe`、`cmd.exe`、`powershell.exe`、`pwsh.exe`、`Code.exe` (VS Code)、`devenv.exe` (Visual Studio)；
* 检查调用进程 PID 与父进程 PPID，严禁误杀当前活跃的终端或 IDE。

### L3 级：持久化系统后台与杀毒设施
* Windows Defender / 安全中心 (`MsMpEng.exe`, `SecurityHealthSystray.exe`)；
* 核心硬件驱动管理面板 (NVIDIA Control Panel, AMD Radeon Software, Realtek Audio Console)；
* 云存储守护进程 (`OneDrive.exe`)。

### L4 级：用户自定义白名单
* 持久化保存于用户目录：`%APPDATA%\TaskCleaner\config.json`；
* 支持按可执行文件名 (`chrome.exe`) 或完整路径进行前台移出/加入保护。

---

## 5. UI/UX 规范：Windows 11 Fluent 原生美学

### 5.1 窗口几何与托盘交互
* **锚定位置**：在任务栏通知区图标点击时呼出，根据 Windows 任务栏位置（默认位于底部）动态计算坐标，弹出于任务栏托盘正上方；
* **尺寸规范**：固定宽度 340 epx，高度根据内容自适应（最高 480 epx），挂载与 macOS 版一致的高度自适应收缩机制；
* **失焦关闭**：监听窗口 `Deactivated` 事件，点击外部区域时平滑隐退；
* **材质应用**：窗口背景全面启用 Windows 11 `Mica Alt` 材质，搭配细致的 `SystemDropShadow` 阴影与 8px 原生系统圆角 (`DWMWA_WINDOW_CORNER_PREFERENCE`)。

### 5.2 视觉模块层次 (对齐 macOS 原版设计)
1. **顶栏 (Header)**：应用标题 `Task Cleaner`、状态微型徽章 (`%d 运行中`)、刷新按钮；
2. **状态指示横条**：在快捷键冲突或异常时展示极简 24pt 原生胶囊指示条；
3. **操作卡片 (Action Card)**：一等公民的 Fluent 材质卡片，展示当前待结束进程数量，提供一键“结束”高亮按钮（拒绝高饱和度刺眼警告色，使用 Windows 系统主题 Accent 色）；
4. **分段选择器 (Segmented)**：提供「待结束」、「已保护」、「全部」三个选项卡，右侧集成排序方式下拉选择；
5. **动态遥测进程行 (Process Row)**：
   * 左侧：高清晰度应用程序图标、应用名称；
   * 右侧：轻量化遥测指标徽章 (`415 MB · 1.2% · 1 窗口`)，随当前激活的排序模式智能高亮对应指标；
   * 操作手势：单进程垃圾桶清理按钮、右键 WinUI 3 原生 `MenuFlyout` (打开所在目录、复制路径、复制 PID、加入白名单)。
6. **底栏工具栏 (Footer)**：全局快捷键提示标签、配置管理齿轮菜单、退出按钮。

---

## 6. 工程实施与里程碑规划 (Roadmap)

### 里程碑 1：Win32 进程遥测与白名单核心引擎 (Engine Core)
* [ ] 搭建 C# 控制台验证工程，通过 CsWin32 接入 `EnumWindows` 顶层前台窗口筛选；
* [ ] 实现 L1-L4 白名单过滤算法与进程元数据提取 (名称、路径、高清图标)；
* [ ] 接入 PSAPI `GetProcessMemoryInfo` 物理内存与 `GetProcessTimes` CPU% 差分采样；
* [ ] 实现基于 `WM_CLOSE` + 400ms 轮询等待 + `TerminateProcess` 的两阶段平稳终止管道。

### 里程碑 2：WinUI 3 托盘常驻与原生 Fluent 界面 (UI Framework)
* [ ] 初始化 Windows App SDK 1.5+ WinUI 3 工程，配置非打包 (Unpackaged) 单文件发布流水线；
* [ ] 集成 `H.NotifyIcon.WinUI` 托管任务栏图标，实现左键呼出、右键菜单与位置精准锚定；
* [ ] 接入 Windows 11 `Mica Alt` 材质与 Fluent 控件样式；
* [ ] 移植主面板：Header、ActionCard、SegmentedSwitcher、ProcessListView。

### 里程碑 3：交互细节与多维排序系统 (Telemetry & Interactions)
* [ ] 实现 5 维动态排序算法 (综合负载、内存占用、CPU%、窗口数、默认名称)；
* [ ] 移植指标详情显示开关与可执行文件名/路径显示开关；
* [ ] 接入 WinUI 3 `MenuFlyout` 完整原生右键上下文菜单。

### 里程碑 4：系统级深度集成与发布打包 (Distribution & Polish)
* [ ] Win32 `RegisterHotKey` 全局快捷键注册与自定义按键捕获面板；
* [ ] 开机自启动注册表与引导配置；
* [ ] 国际化多语言方案 (Resw 资源文件，完整接入 24 语种字典)；
* [ ] CI/CD 自动化构建：通过 GitHub Actions 自动化编译 `win-x64` 与 `win-arm64` 独立可执行安装包。

---

## 7. 仓库目录结构规范

```text
windows-task-cleaner-gui/
├── .github/
│   └── workflows/
│       └── build.yml               # GitHub Actions 多架构自动化编译
├── src/
│   ├── TaskCleaner.WinUI/          # WinUI 3 主工程 (.NET 8 / WASDK)
│   │   ├── Assets/                 # 应用程序图标与托盘图标资源 (.ico / .png)
│   │   ├── Controls/               # 原生卡片与遥测指示微件
│   │   ├── Helpers/                # Win32 互操作、位置计算、DPI 缩放支持
│   │   ├── Models/                 # 进程实体、白名单规则模型
│   │   ├── Services/               # 进程枚举服务、遥测采样服务、配置存储服务
│   │   ├── Strings/                # 24 语种 .resw 国际化字典
│   │   ├── ViewModels/             # 主面板与设置子菜单 ViewModel
│   │   ├── Views/                  # MainWindow (托盘弹窗)、ShortcutDialog
│   │   ├── App.xaml / App.xaml.cs
│   │   └── TaskCleaner.WinUI.csproj
│   └── TaskCleaner.Core/           # 纯 C# 无 UI 进程操作引擎库 (方便单测与复用)
├── docs/
│   └── WINDOWS_WINUI3_PORTING_PLAN.md
├── LICENSE                         # GNU AGPLv3 许可证
├── COMMERCIAL.md                   # 商业授权声明 (与 macOS 版严格保持一致)
└── README.md                       # Windows 版项目介绍与构建指南
```

---

## 8. 开源与商业双重授权条款 (Strict Dual-Licensing)

与 macOS 原版项目保持完全一致的双重许可结构：
1. **开源协议**：遵循 **GNU AGPLv3**，任何第三方修改、衍生项目或二次封装的服务端必须保持 AGPLv3 开源；
2. **商业闭源授权**：企业内部分发、白标定制或商用闭源捆绑分发，必须向原作者取得商业授权；
3. **版权归属**：所有文件必须保留统一版权标识：  
   `Copyright (c) 2026 DonJone. All rights reserved.`
