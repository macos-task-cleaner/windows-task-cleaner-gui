using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using TaskCleaner.Core.Models;
using TaskCleaner.Core.Native;

namespace TaskCleaner.Core.Services
{
    public interface IProcessTelemetry
    {
        void SampleTelemetry(ProcessEntry entry, IntPtr hProcess);
        void SortEntries(List<ProcessEntry> entries, SortMode mode);
        double CalculateCompositeScore(double memoryMb, double cpuPercent, int windowCount);
    }

    public class ProcessTelemetry : IProcessTelemetry
    {
        private class CpuSample
        {
            public long TimestampTicks { get; set; }
            public long TotalCpuTimeTicks { get; set; }
        }

        private readonly ConcurrentDictionary<uint, CpuSample> _previousCpuSamples = new();
        private readonly int _processorCount = Environment.ProcessorCount;

        public void SampleTelemetry(ProcessEntry entry, IntPtr hProcess)
        {
            if (hProcess == IntPtr.Zero) return;

            // 1. 物理常驻内存 (Working Set) 提取
            if (Win32PInvoke.GetProcessMemoryInfo(hProcess, out var memCounters, (uint)Marshal.SizeOf<Win32PInvoke.PROCESS_MEMORY_COUNTERS_EX>()))
            {
                entry.MemoryBytes = (ulong)memCounters.WorkingSetSize.ToUInt64();
            }

            // 2. CPU 使用率差分采样 (GetProcessTimes)
            if (Win32PInvoke.GetProcessTimes(hProcess, out _, out _, out var kernelTime, out var userTime))
            {
                long kTime = ToLong(kernelTime);
                long uTime = ToLong(userTime);
                long totalCpuTime = kTime + uTime;
                long nowTicks = Stopwatch.GetTimestamp();

                double cpuPercent = 0.0;
                if (_previousCpuSamples.TryGetValue(entry.Pid, out var previous))
                {
                    double timeElapsedNanos = (nowTicks - previous.TimestampTicks) * (1_000_000_000.0 / Stopwatch.Frequency);
                    double cpuTimeNanos = (totalCpuTime - previous.TotalCpuTimeTicks) * 100.0; // 100-nanosecond units to nanos

                    if (timeElapsedNanos > 0 && cpuTimeNanos >= 0)
                    {
                        // 归一化多核心 CPU 使用率
                        cpuPercent = (cpuTimeNanos / (timeElapsedNanos * _processorCount)) * 100.0;
                        cpuPercent = Math.Clamp(cpuPercent, 0.0, 100.0 * _processorCount);
                    }
                }

                _previousCpuSamples[entry.Pid] = new CpuSample
                {
                    TimestampTicks = nowTicks,
                    TotalCpuTimeTicks = totalCpuTime
                };

                entry.CpuPercent = cpuPercent;
            }

            // 3. 计算 5 维综合负载评分
            entry.CompositeScore = CalculateCompositeScore(entry.MemoryMb, entry.CpuPercent, entry.WindowCount);
        }

        public double CalculateCompositeScore(double memoryMb, double cpuPercent, int windowCount)
        {
            // 算法权重严格对齐 macOS 版:
            // - 内存权重 40% (以 100MB 为基准单位)
            // - CPU 权重 40% (每个 1% 算 2.0 分)
            // - 窗口数量权重 20% (每个可见主窗口算 5.0 分)
            return (memoryMb / 100.0) * 0.4 + (cpuPercent * 2.0) * 0.4 + (windowCount * 5.0) * 0.2;
        }

        public void SortEntries(List<ProcessEntry> entries, SortMode mode)
        {
            switch (mode)
            {
                case SortMode.Composite:
                    entries.Sort((a, b) => b.CompositeScore.CompareTo(a.CompositeScore));
                    break;
                case SortMode.Memory:
                    entries.Sort((a, b) => b.MemoryBytes.CompareTo(a.MemoryBytes));
                    break;
                case SortMode.Cpu:
                    entries.Sort((a, b) => b.CpuPercent.CompareTo(a.CpuPercent));
                    break;
                case SortMode.Windows:
                    entries.Sort((a, b) => b.WindowCount.CompareTo(a.WindowCount));
                    break;
                case SortMode.Default:
                default:
                    entries.Sort((a, b) => string.Compare(a.DisplayTitle, b.DisplayTitle, StringComparison.CurrentCultureIgnoreCase));
                    break;
            }
        }

        private static long ToLong(System.Runtime.InteropServices.ComTypes.FILETIME ft)
        {
            return ((long)ft.dwHighDateTime << 32) | (uint)ft.dwLowDateTime;
        }
    }
}
