using System;
using System.Collections.ObjectModel;
using System.Threading.Tasks;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using TaskCleaner.Core.Models;
using TaskCleaner.Core.Services;

namespace TaskCleaner.WinUI.ViewModels
{
    public partial class MainViewModel : ObservableObject
    {
        private readonly IProcessManager _processManager;
        private readonly IWhitelistService _whitelistService;
        private System.Threading.Timer? _monitoringTimer;

        [ObservableProperty]
        private CleaningSummary? _summary;

        [ObservableProperty]
        private ObservableCollection<ProcessEntry> _displayedProcesses = new();

        [ObservableProperty]
        private int _selectedTabIndex = 0; // 0: 待结束, 1: 已保护, 2: 全部

        [ObservableProperty]
        private SortMode _currentSortMode = SortMode.Composite;

        [ObservableProperty]
        private bool _isWorking;

        [ObservableProperty]
        private string? _statusMessage;

        [ObservableProperty]
        private bool _showDetailedMetrics = true;

        [ObservableProperty]
        private bool _showAppIdentifier = false;

        public int TotalRunning => Summary?.TotalRunning ?? 0;
        public int TargetCount => Summary?.TargetCount ?? 0;
        public int ProtectedCount => Summary?.ProtectedCount ?? 0;
        public bool HasTargets => TargetCount > 0;

        public string RunningBadgeText => $"{TotalRunning} 运行中";
        public string TargetBadgeText => $"{TargetCount} 个待结束应用";

        public string ActionButtonText => HasTargets
            ? $"结束 ({TargetCount})"
            : "无需清理";

        public MainViewModel(IProcessManager processManager, IWhitelistService whitelistService)
        {
            _processManager = processManager;
            _whitelistService = whitelistService;

            Refresh();
        }

        public void StartLiveMonitoring()
        {
            Refresh();
            _monitoringTimer?.Dispose();
            _monitoringTimer = new System.Threading.Timer(_ =>
            {
                if (!IsWorking)
                {
                    Refresh();
                }
            }, null, 2000, 2000);
        }

        public void StopLiveMonitoring()
        {
            _monitoringTimer?.Dispose();
            _monitoringTimer = null;
        }

        [RelayCommand]
        public void Refresh()
        {
            var summary = _processManager.ScanProcesses(CurrentSortMode);
            Summary = summary;

            OnPropertyChanged(nameof(TotalRunning));
            OnPropertyChanged(nameof(TargetCount));
            OnPropertyChanged(nameof(ProtectedCount));
            OnPropertyChanged(nameof(HasTargets));
            OnPropertyChanged(nameof(RunningBadgeText));
            OnPropertyChanged(nameof(TargetBadgeText));
            OnPropertyChanged(nameof(ActionButtonText));

            UpdateDisplayedProcesses();
        }

        [RelayCommand]
        public void SelectTab(int index)
        {
            SelectedTabIndex = index;
            UpdateDisplayedProcesses();
        }

        [RelayCommand]
        public void SetSortMode(SortMode mode)
        {
            CurrentSortMode = mode;
            Refresh();
        }

        [RelayCommand]
        public async Task CleanAllAsync()
        {
            if (IsWorking || Summary == null || !HasTargets) return;

            IsWorking = true;
            StatusMessage = "正在终止未保护的前台任务...";

            try
            {
                int terminated = await _processManager.TerminateAllTargetsAsync(Summary.Targets);
                StatusMessage = $"已清理 {terminated} 个前台任务";
            }
            finally
            {
                IsWorking = false;
                Refresh();

                await Task.Delay(2500);
                StatusMessage = null;
            }
        }

        [RelayCommand]
        public async Task TerminateTargetAsync(ProcessEntry entry)
        {
            if (IsWorking || entry == null) return;

            IsWorking = true;
            StatusMessage = $"正在结束 {entry.DisplayTitle}...";

            try
            {
                bool success = await _processManager.TerminateProcessGracefullyAsync(entry.Pid);
                StatusMessage = success ? $"已结束 {entry.DisplayTitle}" : $"未能终止 {entry.DisplayTitle}";
            }
            finally
            {
                IsWorking = false;
                Refresh();

                await Task.Delay(2000);
                StatusMessage = null;
            }
        }

        [RelayCommand]
        public void ToggleProtection(ProcessEntry entry)
        {
            if (entry == null) return;

            if (entry.IsProtected)
            {
                _whitelistService.RemoveUserRule(entry.ProcessName);
                StatusMessage = $"已将 {entry.DisplayTitle} 移出保护";
            }
            else
            {
                _whitelistService.AddUserRule(entry.ProcessName);
                StatusMessage = $"已将 {entry.DisplayTitle} 加入保护白名单";
            }

            Refresh();
        }

        private void UpdateDisplayedProcesses()
        {
            if (Summary == null) return;

            var list = SelectedTabIndex switch
            {
                1 => Summary.Protected,
                2 => Summary.All,
                _ => Summary.Targets
            };

            DisplayedProcesses.Clear();
            foreach (var item in list)
            {
                DisplayedProcesses.Add(item);
            }
        }
    }
}
