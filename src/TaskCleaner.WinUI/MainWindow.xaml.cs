using System;
using System.Runtime.InteropServices;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using TaskCleaner.Core.Models;
using TaskCleaner.WinUI.Helpers;
using TaskCleaner.WinUI.ViewModels;
using WinRT.Interop;

namespace TaskCleaner.WinUI
{
    public sealed partial class MainWindow : Window
    {
        public MainViewModel ViewModel { get; }
        private readonly IntPtr _hWnd;
        private readonly AppWindow _appWindow;

        public MainWindow(MainViewModel viewModel)
        {
            ViewModel = viewModel;
            this.InitializeComponent();

            _hWnd = WindowNative.GetWindowHandle(this);
            var windowId = Win32Interop.GetWindowIdFromWindow(_hWnd);
            _appWindow = AppWindow.GetFromWindowId(windowId);

            ConfigureWindow();
        }

        private void ConfigureWindow()
        {
            // 排除在 Alt+Tab 任务切换列表中 (作为原生系统托盘小窗存在)
            _appWindow.IsShownInSwitchers = false;

            if (_appWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.IsResizable = false;
                presenter.IsMaximizable = false;
                presenter.IsMinimizable = false;
                presenter.SetBorderAndTitleBar(true, false);
            }

            // 窗口定位到任务栏托盘上方
            PositionWindow();

            // 失焦自动隐藏
            this.Activated += MainWindow_Activated;
        }

        private void PositionWindow()
        {
            WindowPositionHelper.PositionAboveTaskbar(_hWnd, 340, 480);
        }

        private void MainWindow_Activated(object sender, WindowActivatedEventArgs args)
        {
            if (args.WindowActivationState == WindowActivationState.Deactivated)
            {
                // 点击外部自动隐退
                _appWindow.Hide();
                ViewModel.StopLiveMonitoring();
            }
            else
            {
                ViewModel.StartLiveMonitoring();
            }
        }

        public void ToggleWindowVisibility()
        {
            if (_appWindow.IsVisible)
            {
                _appWindow.Hide();
                ViewModel.StopLiveMonitoring();
            }
            else
            {
                PositionWindow();
                _appWindow.Show();
                SetForegroundWindow(_hWnd);
                ViewModel.StartLiveMonitoring();
            }
        }

        public CommunityToolkit.Mvvm.Input.IRelayCommand ToggleWindowVisibilityCommand =>
            new CommunityToolkit.Mvvm.Input.RelayCommand(ToggleWindowVisibility);

        private void OnShowFromTrayClicked(object sender, RoutedEventArgs e)
        {
            PositionWindow();
            _appWindow.Show();
            SetForegroundWindow(_hWnd);
        }

        private void OnRefreshClicked(object sender, RoutedEventArgs e)
        {
            ViewModel.Refresh();
        }

        private void OnQuitClicked(object sender, RoutedEventArgs e)
        {
            TrayIcon.Dispose();
            Application.Current.Exit();
        }

        private void OnTabPendingClicked(object sender, RoutedEventArgs e)
        {
            ViewModel.SelectTab(0);
        }

        private void OnTabProtectedClicked(object sender, RoutedEventArgs e)
        {
            ViewModel.SelectTab(1);
        }

        private void OnTabAllClicked(object sender, RoutedEventArgs e)
        {
            ViewModel.SelectTab(2);
        }

        private void OnSortSelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (sender is ComboBox cb && cb.SelectedItem is ComboBoxItem item && item.Tag is string tag)
            {
                if (Enum.TryParse<SortMode>(tag, out var mode))
                {
                    ViewModel.SetSortMode(mode);
                }
            }
        }

        private async void OnProcessActionClicked(object sender, RoutedEventArgs e)
        {
            if (sender is FrameworkElement fe && fe.DataContext is ProcessEntry entry)
            {
                if (entry.IsProtected)
                {
                    ViewModel.ToggleProtection(entry);
                }
                else
                {
                    await ViewModel.TerminateTargetAsync(entry);
                }
            }
        }

        private void OnSettingsClicked(object sender, RoutedEventArgs e)
        {
            // 设置菜单待后续添加弹窗
        }

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool SetForegroundWindow(IntPtr hWnd);
    }
}
