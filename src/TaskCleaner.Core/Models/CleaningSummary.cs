using System.Collections.Generic;

namespace TaskCleaner.Core.Models
{
    public class CleaningSummary
    {
        public int TotalRunning { get; set; }
        public int TargetCount { get; set; }
        public int ProtectedCount { get; set; }
        public ulong TotalTargetMemoryBytes { get; set; }

        public List<ProcessEntry> Targets { get; set; } = new();
        public List<ProcessEntry> Protected { get; set; } = new();
        public List<ProcessEntry> All { get; set; } = new();

        public double TotalTargetMemoryMb => TotalTargetMemoryBytes / (1024.0 * 1024.0);

        public string FormattedTargetMemory => TotalTargetMemoryMb >= 1024
            ? $"{TotalTargetMemoryMb / 1024.0:F1} GB"
            : $"{TotalTargetMemoryMb:F0} MB";
    }
}
