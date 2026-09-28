using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using TaskCleaner.Core.Models;
using TaskCleaner.Core.Native;

namespace TaskCleaner.Core.Services
{
    public interface IProcessManager
    {
        CleaningSummary ScanProcesses(SortMode sortMode = SortMode.Composite);
        Task<bool> TerminateProcessGracefullyAsync(uint pid, int timeoutMs = 400);
        Task<int> TerminateAllTargetsAsync(IEnumerable<ProcessEntry> targets);
    }

    public class ProcessManager : IProcessManager
    {
        private readonly IWhitelistService _whitelistService;
        private readonly IProcessTelemetry _telemetry;

        public ProcessManager(IWhitelistService whitelistService, IProcessTelemetry telemetry)
        {
            _whitelistService = whitelistService;
            _telemetry = telemetry;
        }

        public CleaningSummary ScanProcesses(SortMode sortMode = SortMode.Composite)
        {
            var summary = new CleaningSummary();
            var pidToWindows = new Dictionary<uint, List<IntPtr>>();
            var pidToTitles = new Dictionary<uint, string>();

            // 1. 遍历当前桌面顶层可见窗口
            Win32PInvoke.EnumWindows((hWnd, lParam) =>
            {
                if (!Win32PInvoke.IsWindowVisible(hWnd))
                {
                    return true;
                }

                // 过滤工具栏小浮窗
                int exStyle = Win32PInvoke.GetWindowLongW(hWnd, Win32PInvoke.GWL_EXSTYLE);
                if ((exStyle & Win32PInvoke.WS_EX_TOOLWINDOW) != 0)
                {
                    return true;
                }

                // 过滤无标题后台窗口
                int titleLength = Win32PInvoke.GetWindowTextLengthW(hWnd);
                if (titleLength == 0)
                {
                    return true;
                }

                // 过滤 Windows 10/11 虚拟桌面/休眠 Cloaked 窗口
                if (Win32PInvoke.DwmGetWindowAttribute(hWnd, Win32PInvoke.DWMWA_CLOAKED, out int cloaked, sizeof(int)) == 0 && cloaked != 0)
                {
                    return true;
                }

                Win32PInvoke.GetWindowThreadProcessId(hWnd, out uint pid);
                if (pid == 0)
                {
                    return true;
                }

                if (!pidToWindows.ContainsKey(pid))
                {
                    pidToWindows[pid] = new List<IntPtr>();
                    var sb = new StringBuilder(titleLength + 1);
                    Win32PInvoke.GetWindowTextW(hWnd, sb, sb.Capacity);
                    pidToTitles[pid] = sb.ToString();
                }

                pidToWindows[pid].Add(hWnd);
                return true;
            }, IntPtr.Zero);

            var entries = new List<ProcessEntry>();

            // 2. 丰富每个独立前台进程的元数据与系统遥测指标
            foreach (var kv in pidToWindows)
            {
                uint pid = kv.Key;
                int windowCount = kv.Value.Count;
                string mainTitle = pidToTitles.TryGetValue(pid, out var title) ? title : string.Empty;

                IntPtr hProcess = Win32PInvoke.OpenProcess(
                    Win32PInvoke.PROCESS_QUERY_LIMITED_INFORMATION | Win32PInvoke.PROCESS_VM_READ,
                    false,
                    pid);

                string processName = string.Empty;
                string exePath = string.Empty;
                string displayName = string.Empty;

                if (hProcess != IntPtr.Zero)
                {
                    try
                    {
                        var sbPath = new StringBuilder(1024);
                        uint size = (uint)sbPath.Capacity;
                        if (Win32PInvoke.QueryFullProcessImageNameW(hProcess, 0, sbPath, ref size))
                        {
                            exePath = sbPath.ToString();
                            processName = Path.GetFileName(exePath);

                            // 从文件版本信息中获取用户友好的应用显示名称 (如 Google Chrome)
                            if (File.Exists(exePath))
                            {
                                var versionInfo = FileVersionInfo.GetVersionInfo(exePath);
                                if (!string.IsNullOrWhiteSpace(versionInfo.FileDescription))
                                {
                                    displayName = versionInfo.FileDescription;
                                }
                            }
                        }
                    }
                    catch
                    {
                        // 忽略权限受限的路径提取错误
                    }
                }

                if (string.IsNullOrEmpty(processName))
                {
                    try
                    {
                        using var proc = Process.GetProcessById((int)pid);
                        processName = $"{proc.ProcessName}.exe";
                    }
                    catch
                    {
                        processName = $"PID_{pid}.exe";
                    }
                }

                var entry = new ProcessEntry
                {
                    Pid = pid,
                    ProcessName = processName,
                    DisplayName = displayName,
                    MainWindowTitle = mainTitle,
                    ExecutablePath = exePath,
                    WindowCount = windowCount
                };

                // 3. 采样常驻内存与动态 CPU 占用
                if (hProcess != IntPtr.Zero)
                {
                    _telemetry.SampleTelemetry(entry, hProcess);
                    Win32PInvoke.CloseHandle(hProcess);
                }

                // 4. 白名单防御矩阵定级
                entry.Tier = _whitelistService.ClassifyProcess(pid, processName, exePath);
                entries.Add(entry);
            }

            // 5. 分流归类与排序
            foreach (var entry in entries)
            {
                summary.All.Add(entry);
                if (entry.IsProtected)
                {
                    summary.Protected.Add(entry);
                }
                else
                {
                    summary.Targets.Add(entry);
                    summary.TotalTargetMemoryBytes += entry.MemoryBytes;
                }
            }

            summary.TotalRunning = summary.All.Count;
            summary.TargetCount = summary.Targets.Count;
            summary.ProtectedCount = summary.Protected.Count;

            _telemetry.SortEntries(summary.Targets, sortMode);
            _telemetry.SortEntries(summary.Protected, sortMode);
            _telemetry.SortEntries(summary.All, sortMode);

            return summary;
        }

        public async Task<bool> TerminateProcessGracefullyAsync(uint pid, int timeoutMs = 400)
        {
            var windows = new List<IntPtr>();
            Win32PInvoke.EnumWindows((hWnd, lParam) =>
            {
                Win32PInvoke.GetWindowThreadProcessId(hWnd, out uint wPid);
                if (wPid == pid)
                {
                    windows.Add(hWnd);
                }
                return true;
            }, IntPtr.Zero);

            // 第一阶段：向所有相关主窗口分发 WM_CLOSE 消息 (优雅退出)
            foreach (var hWnd in windows)
            {
                Win32PInvoke.PostMessageW(hWnd, Win32PInvoke.WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
            }

            // 宽限期轮询等待进程自然退出
            IntPtr hProcess = Win32PInvoke.OpenProcess(
                Win32PInvoke.PROCESS_QUERY_LIMITED_INFORMATION | Win32PInvoke.PROCESS_TERMINATE,
                false,
                pid);

            if (hProcess == IntPtr.Zero)
            {
                return true; // 无法获取句柄或已经退出
            }

            try
            {
                var stopwatch = Stopwatch.StartNew();
                while (stopwatch.ElapsedMilliseconds < timeoutMs)
                {
                    try
                    {
                        using var proc = Process.GetProcessById((int)pid);
                        if (proc.HasExited)
                        {
                            return true;
                        }
                    }
                    catch (ArgumentException)
                    {
                        return true; // 进程已消亡
                    }
                    await Task.Delay(50);
                }

                // 第二阶段：超时仍未退出的进程执行强制终止 (针对 explorer.exe 特殊豁免)
                var pName = string.Empty;
                try
                {
                    using var proc = Process.GetProcessById((int)pid);
                    pName = proc.ProcessName;
                }
                catch { }

                if (string.Equals(pName, "explorer", StringComparison.OrdinalIgnoreCase))
                {
                    // Explorer 严禁 TerminateProcess，避免任务栏崩溃黑屏
                    return false;
                }

                return Win32PInvoke.TerminateProcess(hProcess, 1);
            }
            finally
            {
                Win32PInvoke.CloseHandle(hProcess);
            }
        }

        public async Task<int> TerminateAllTargetsAsync(IEnumerable<ProcessEntry> targets)
        {
            int terminated = 0;
            var tasks = targets.Select(async target =>
            {
                if (await TerminateProcessGracefullyAsync(target.Pid))
                {
                    Interlocked.Increment(ref terminated);
                }
            });

            await Task.WhenAll(tasks);
            return terminated;
        }
    }
}
