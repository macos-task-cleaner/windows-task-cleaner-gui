# Windows Task Cleaner GUI - Agent Guidelines & Engineering Constraints

This document defines the architectural conventions, engineering rules, and hard constraints for AI coding agents operating on the `windows-task-cleaner-gui` codebase.

---

## 1. Global Operating Policies

1. **Strict No-Emoji Policy**:
   * Never output Unicode emojis in code, comments, Git commit messages, logs, UI strings, documentation, or responses.
   * Use plain text prefixes for emphasis or status (e.g., `[INFO]`, `[WARN]`, `[SUCCESS]`, `*`, `-`).

2. **Workspace Delivery Principle**:
   * All deliverables, source code modifications, scripts, and documentation must physically persist within the local workspace directory.
   * Never leave deliverables exclusively in hidden cache directories.

3. **Clickable File Links**:
   * All file paths and symbol references in explanations must use the `file://` scheme (e.g. `file:///Users/don/work/git/windows-task-cleaner-gui/crates/task-cleaner-gui/src/main.rs`).

4. **Proactive Git Push upon Milestone Delivery**:
   * While intermediate micro-debugging loops avoid redundant commit noise, the agent MUST proactively stage, commit, and execute `git push origin main` upon completing any user-requested feature, asset deliverable, bug fix, or CI/CD workflow milestone.
   * Never leave completed deliverables uncommitted or unpushed in the workspace waiting for the user to prompt.
   * Commit messages must strictly adhere to the No-Emoji policy and follow Conventional Commits (e.g., `feat(ci): ...`, `fix(gui): ...`).

---

## 2. Architecture Overview

`TaskCleaner.exe` is a lightweight, high-performance Windows 11 system tray utility and process cleaner built with 100% native Rust, Win32 API, and GDI, free from C#/.NET, WinUI 3, or WebView2 runtime bloat.

### Workspace Structure

* `crates/task-cleaner-core/`:
  * Core cross-process detection, foreground window scanning, process telemetry sampling (CPU, Private Working Set Memory, window counts), whitelist management, composite scoring, and multi-tier termination logic (`TerminationMode`).
  * `src/whitelist.rs`: Persistent whitelist rules (`~/.config/task-cleaner/whitelist.toml`) and 3-tier safety barriers (L1 Core Windows components, L2 Caller lineage protection, L3 User whitelist).
  * `src/i18n.rs`: 24-language internationalization dictionary and runtime locale resolution (`Language`, `LanguagePreference`, `I18nKey`, `tr`).
  * `src/windows/`: Win32 native APIs for process memory working set purging (`EmptyWorkingSet`), process enumeration, and token privileges.
* `crates/task-cleaner-gui/`:
  * Native Win32 popup window with Fluent 2.0 aesthetics and system tray integration.
  * `src/main.rs`: High-precision GDI rendering pipeline, Per-Monitor V2 High-DPI scaling, custom pill-X tray icon synthesis, event loop, dynamic shortcut recorder dialog, and secondary context menus.
* `crates/task-cleaner-cli/`:
  * Standalone CLI binary (`mtc.exe`) providing direct terminal commands (`-e`, `-l`, `-w`, `-s`, `-p`, `--json`).
* `run.ps1` & `build.ps1`:
  * Optimized PowerShell developer entry points supporting automatic local VM caching (`C:\Temp\taskcleaner-target`), process lifecycle management, and UTF-8 console output.

---

## 3. High-DPI & Win32 GDI Hard Constraints (Lessons Learned)

### A. Per-Monitor V2 DPI Awareness (`SetProcessDpiAwarenessContext`)
* **Problem**: Without explicit DPI awareness context, Windows Desktop Window Manager (DWM) treats the application as 96 DPI and applies bilinear stretching on high-resolution displays (125%, 150%, 200%), causing severe blurriness across text, borders, and icons.
* **Rule**:
  * Must call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at the absolute beginning of `main()` and `run_gui()` before any window creation or Win32 UI call.
  * Always handle `WM_DPICHANGED` in `window_proc` to reposition and resize the window to `lparam` suggestions, update the tray icon, and trigger full invalidation.

### B. Decoupled Coordinate System (Forward Scaling vs. Inverse Mapping)
* **Rule**:
  * **Rendering**: Use `scale_dpi(val, dpi)` for all visual dimensions (fonts, padding, card bounds, badges, icons). All ClearType fonts must be instantiated with scaled heights (`-scale_dpi(pt, dpi)`).
  * **Hit Testing & Mouse Messages**: Windows delivers physical pixel coordinates in `lparam` under Per-Monitor V2. In `WM_MOUSEMOVE`, `WM_LBUTTONUP`, and `WM_RBUTTONUP`, immediately map raw coordinates back to 96-DPI logical coordinates using `unscale_dpi(raw, dpi)`.
  * This guarantees that all hit bounds (e.g. `x >= 236 && x <= 258`) remain invariant and pixel-perfect across any monitor DPI factor.

### C. Native High-Resolution Icon Extraction (`PrivateExtractIconsW`)
* **Rule**:
  * Never rely solely on `ExtractIconExW` with small icon handles (16x16), which leads to blurred upscaling on high-DPI displays.
  * Always compute target physical pixels (`icon_size = scale_dpi(24, dpi)`).
  * Prefer `PrivateExtractIconsW` requesting the exact physical target size from the PE binary resource group. Fall back to large icons (`h_large`, 32x32+) downsampled with `DI_NORMAL`.

### D. System Tray Icon Synthesis & Dark Mode Adaptation
* **Rule**:
  * Dynamically fetch small icon metrics for current system DPI via `GetSystemMetricsForDpi(SM_CXSMICON, dpi)`.
  * Synthesize 32-bit ARGB DIBSection with premultiplied alpha and 4x4 subpixel anti-aliasing matching the macOS Pill-X specification.
  * Query `SystemUsesLightTheme` under `Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` to automatically adapt icon luminance for dark vs. light taskbars on `WM_SETTINGCHANGE`.

### E. GDI Resource Leak Prevention
* **Rule**:
  * Every created GDI object (`CreateSolidBrush`, `CreatePen`, `CreateFontW`, `CreateCompatibleBitmap`, `CreateDIBSection`, `CreateBitmap`) MUST have a corresponding `DeleteObject` call once deselected from the device context.
  * Every `CreateCompatibleDC` must be released via `DeleteDC`.
  * Every `GetDC` must be matched with `ReleaseDC`.

---

## 4. Build & Verification Tiering (CRITICAL EFFICIENCY RULE)

1. **Daily Development & Micro-adjustments (Level 1 - STRICT)**:
   * For syntax check, type safety, and logic verification, **ONLY execute `cargo check --workspace`** or `cargo test --workspace`.
   * Compilation finishes incrementally in under 2 seconds.
   * **Strictly Prohibited**: Never run full multi-target packaging or long-running release builds on iterative tweaks.

2. **Milestone / Release Packaging (Level 3 - EXPLICIT ONLY)**:
   * Only run `cargo build --workspace --release` or `build.ps1 -Release` when the user explicitly requests a production binary or release distribution.

3. **Anti-Polling Policy**:
   * Never poll `manage_task {"Action": "status"}` in a tight loop. Rely on the system's reactive wakeup mechanism when tasks finish.

---

## 5. Packaging & Distribution Engineering Standards

### A. Setup Installer (Inno Setup)
* **Script**: [`installer.iss`](file:///Users/don/work/git/windows-task-cleaner-gui/installer.iss) at repository root.
* **Privileges**: Must enforce `PrivilegesRequired=lowest` for per-user installation (`%LOCALAPPDATA%\Programs\TaskCleaner`) to eliminate UAC elevation prompts for end users.
* **Environment PATH**: Automatically injects `{app}` into user `Environment\Path` registry key so `mtc.exe` is available terminal-wide.
* **Shortcuts & Icons**: Standard Start Menu and Desktop shortcuts using [`app.ico`](file:///Users/don/work/git/windows-task-cleaner-gui/app.ico).
* **Safe In-Place Upgrades**: Configures `CloseApplications=yes` to gracefully handle running instances during setup or upgrade.

### B. Portable Distribution (Portable ZIP)
* **Archive**: Packaged via [`scripts/package_release.ps1`](file:///Users/don/work/git/windows-task-cleaner-gui/scripts/package_release.ps1) as `TaskCleaner-Windows-x64-Portable.zip`.
* **Contents**: `TaskCleaner.exe`, `mtc.exe`, `app.ico`, `README.md`, `LICENSE`, and `COMMERCIAL.md`.

### C. Multi-Architecture CI/CD Pipeline (GitHub Actions)
* **Workflow**: [`.github/workflows/release.yml`](file:///Users/don/work/git/windows-task-cleaner-gui/.github/workflows/release.yml), strictly mirroring the macOS release matrix.
* **Supported Targets**:
  - `x86_64-pc-windows-msvc` (Standard Intel/AMD 64-bit).
  - `i686-pc-windows-msvc` (Legacy Intel/AMD 32-bit x86).
  - `aarch64-pc-windows-msvc` (Windows on ARM, Surface Pro, Snapdragon X Elite).
* **Triggers**: Weekly Sunday UTC 02:00 cron (`pre-*`), manual `workflow_dispatch` (version override, release toggle), and `v*` tag pushes.
* **Integrity**: Generates `.sha256` checksums for every `.exe` and `.zip` asset attached to Releases.

### D. High-DPI Application Icon (PE Resource Embedding)
* **Container**: 7-layer hybrid container [`assets/app.ico`](file:///Users/don/work/git/windows-task-cleaner-gui/assets/app.ico) (256x256 PNG + 128..16 32-bit BGRA DIBs).
* **PE Embedding**: Uses `winres` in [`crates/task-cleaner-gui/build.rs`](file:///Users/don/work/git/windows-task-cleaner-gui/crates/task-cleaner-gui/build.rs) to embed `app.ico` into `TaskCleaner.exe` resource section on Windows.

---

## 6. Internationalization (I18n) Decoupling & Manual Trigger (STRICT)

* **Decoupling Principle**: Full multi-lingual dictionary synchronization (`crates/task-cleaner-core/src/i18n.rs`) is strictly decoupled from daily feature development and UI prototyping.
* **Prohibited**:
  * Never proactively modify `i18n.rs` (adding new `I18nKey` enums or multi-lingual dictionary entries) during routine UI adjustments, bug fixes, or incremental feature delivery.
* **Daily Development Rule**:
  * Use inline string literals or sensible fallbacks for newly introduced labels, buttons, or status messages.
* **Manual Trigger Requirement (User-Driven)**:
  * Only synchronize and expand multi-lingual translations when the user **explicitly commands** it (e.g., "同步多语言", "更新 i18n", "做国际化").
  * When explicitly triggered, make atomic batch edits to avoid fragmented multi-slice churn.

---

## 7. Single-File Atomic Batch Editing

* **Strict Anti-Fragmentation Rule**:
  * Never perform fragmented micro-edits ("view 20 lines -> replace 3 lines -> view 20 lines -> replace 3 lines").
  * Analyze the full file context and perform all related changes in a single contiguous `replace_file_content` call.

---

## 8. Licensing & Attribution

* **Dual-Licensing Model**:
  * Open-source under **GNU AGPLv3**. Any network service or derivative software must remain AGPLv3.
  * Commercial closed-source distribution or enterprise white-labeling requires a commercial license.
* **Attribution**:
  * Preserve `LICENSE`, `COMMERCIAL.md`, and copyright notices: `Copyright (c) 2026 DonJone. All rights reserved.`.
