using System;

namespace TaskCleaner.Core.Models
{
    public class ProcessEntry
    {
        public uint Pid { get; set; }
        public string ProcessName { get; set; } = string.Empty;
        public string DisplayName { get; set; } = string.Empty;
        public string MainWindowTitle { get; set; } = string.Empty;
        public string ExecutablePath { get; set; } = string.Empty;

        public ulong MemoryBytes { get; set; }
        public double MemoryMb => MemoryBytes / (1024.0 * 1024.0);

        public double CpuPercent { get; set; }
        public int WindowCount { get; set; }
        public double CompositeScore { get; set; }

        public WhitelistTier Tier { get; set; } = WhitelistTier.None;
        public bool IsProtected => Tier != WhitelistTier.None;

        public string FormattedMemory => MemoryMb >= 1024
            ? $"{MemoryMb / 1024.0:F1} GB"
            : $"{MemoryMb:F0} MB";

        public string FormattedCpu => $"{CpuPercent:F1}%";

        public string FormattedWindows => $"{WindowCount} 窗口";

        public string MetricsDisplay => $"{FormattedMemory} · {FormattedCpu} · {FormattedWindows}";

        public string DisplayTitle => !string.IsNullOrWhiteSpace(DisplayName)
            ? DisplayName
            : (!string.IsNullOrWhiteSpace(ProcessName) ? ProcessName : $"PID {Pid}");

        public string Subtitle => !string.IsNullOrWhiteSpace(ProcessName)
            ? $"{ProcessName} (PID {Pid})"
            : $"PID {Pid}";
    }
}
