using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text.Json;
using TaskCleaner.Core.Models;

namespace TaskCleaner.Core.Services
{
    public interface IWhitelistService
    {
        WhitelistTier ClassifyProcess(uint pid, string processName, string exePath);
        bool IsProtected(uint pid, string processName, string exePath);
        void AddUserRule(string processName);
        void RemoveUserRule(string processName);
        IReadOnlyCollection<string> GetUserRules();
    }

    public class WhitelistService : IWhitelistService
    {
        private static readonly HashSet<string> L1CoreSystem = new(StringComparer.OrdinalIgnoreCase)
        {
            "explorer.exe",
            "dwm.exe",
            "csrss.exe",
            "smss.exe",
            "lsass.exe",
            "services.exe",
            "svchost.exe",
            "System",
            "SearchHost.exe",
            "StartMenuExperienceHost.exe",
            "ShellExperienceHost.exe",
            "conhost.exe",
            "sihost.exe",
            "fontdrvhost.exe",
            "winlogon.exe",
            "taskhostw.exe",
            "ctfmon.exe"
        };

        private static readonly HashSet<string> L2CallerContext = new(StringComparer.OrdinalIgnoreCase)
        {
            "TaskCleaner.exe",
            "TaskCleaner.WinUI.exe",
            "WindowsTerminal.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "Code.exe",
            "devenv.exe"
        };

        private static readonly HashSet<string> L3PersistentUtilities = new(StringComparer.OrdinalIgnoreCase)
        {
            "MsMpEng.exe",
            "SecurityHealthSystray.exe",
            "SecurityHealthService.exe",
            "OneDrive.exe",
            "NVIDIA Share.exe",
            "NVDisplay.Container.exe",
            "RadeonSoftware.exe"
        };

        private readonly HashSet<string> _userRules = new(StringComparer.OrdinalIgnoreCase);
        private readonly string _configFilePath;
        private readonly uint _currentProcessId;

        public WhitelistService()
        {
            _currentProcessId = (uint)Process.GetCurrentProcess().Id;
            var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            var folder = Path.Combine(appData, "TaskCleaner");
            _configFilePath = Path.Combine(folder, "config.json");

            LoadUserConfig();
        }

        public WhitelistTier ClassifyProcess(uint pid, string processName, string exePath)
        {
            if (pid == _currentProcessId)
            {
                return WhitelistTier.L2CallerContext;
            }

            var cleanName = processName.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
                ? processName
                : $"{processName}.exe";

            // L1 核心系统检查
            if (L1CoreSystem.Contains(cleanName))
            {
                return WhitelistTier.L1CoreSystem;
            }

            // L2 调用者与终端上下文
            if (L2CallerContext.Contains(cleanName))
            {
                return WhitelistTier.L2CallerContext;
            }

            // L3 持久化系统设施
            if (L3PersistentUtilities.Contains(cleanName))
            {
                return WhitelistTier.L3PersistentUtilities;
            }

            // L4 用户自定义白名单
            if (_userRules.Contains(cleanName) || (!string.IsNullOrEmpty(exePath) && _userRules.Contains(exePath)))
            {
                return WhitelistTier.L4UserConfig;
            }

            return WhitelistTier.None;
        }

        public bool IsProtected(uint pid, string processName, string exePath)
        {
            return ClassifyProcess(pid, processName, exePath) != WhitelistTier.None;
        }

        public void AddUserRule(string processName)
        {
            if (string.IsNullOrWhiteSpace(processName)) return;

            var cleanName = processName.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
                ? processName
                : $"{processName}.exe";

            if (_userRules.Add(cleanName))
            {
                SaveUserConfig();
            }
        }

        public void RemoveUserRule(string processName)
        {
            if (string.IsNullOrWhiteSpace(processName)) return;

            var cleanName = processName.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
                ? processName
                : $"{processName}.exe";

            if (_userRules.Remove(cleanName) || _userRules.Remove(processName))
            {
                SaveUserConfig();
            }
        }

        public IReadOnlyCollection<string> GetUserRules() => _userRules;

        private void LoadUserConfig()
        {
            try
            {
                if (File.Exists(_configFilePath))
                {
                    var json = File.ReadAllText(_configFilePath);
                    var list = JsonSerializer.Deserialize<List<string>>(json);
                    if (list != null)
                    {
                        foreach (var item in list)
                        {
                            _userRules.Add(item);
                        }
                    }
                }
            }
            catch
            {
                // 忽略非格式化损坏异常
            }
        }

        private void SaveUserConfig()
        {
            try
            {
                var dir = Path.GetDirectoryName(_configFilePath);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
                {
                    Directory.CreateDirectory(dir);
                }

                var json = JsonSerializer.Serialize(new List<string>(_userRules), new JsonSerializerOptions { WriteIndented = true });
                File.WriteAllText(_configFilePath, json);
            }
            catch
            {
                // 忽略 IO 异常
            }
        }
    }
}
