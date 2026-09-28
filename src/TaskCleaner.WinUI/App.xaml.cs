using System;
using Microsoft.UI.Xaml;
using TaskCleaner.Core.Services;
using TaskCleaner.WinUI.ViewModels;

namespace TaskCleaner.WinUI
{
    public partial class App : Application
    {
        public static MainWindow? MainWindowInstance { get; private set; }
        public static MainViewModel? ViewModelInstance { get; private set; }

        public App()
        {
            this.InitializeComponent();

            this.UnhandledException += (sender, e) =>
            {
                AppLogger.Error($"[未捕获全局异常] {e.Message}", e.Exception);
            };
        }

        protected override void OnLaunched(LaunchActivatedEventArgs args)
        {
            AppLogger.Info("Task Cleaner (WinUI 3) 应用程序正在启动...");

            try
            {
                var whitelistService = new WhitelistService();
                var telemetry = new ProcessTelemetry();
                var processManager = new ProcessManager(whitelistService, telemetry);

                ViewModelInstance = new MainViewModel(processManager, whitelistService);

                MainWindowInstance = new MainWindow(ViewModelInstance);
                MainWindowInstance.Activate();

                AppLogger.Info("主窗口与托盘组件初始化成功。");
            }
            catch (Exception ex)
            {
                AppLogger.Error("启动初始化失败", ex);
                throw;
            }
        }
    }
}
