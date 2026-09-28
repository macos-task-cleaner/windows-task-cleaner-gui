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
        }

        protected override void OnLaunched(LaunchActivatedEventArgs args)
        {
            var whitelistService = new WhitelistService();
            var telemetry = new ProcessTelemetry();
            var processManager = new ProcessManager(whitelistService, telemetry);

            ViewModelInstance = new MainViewModel(processManager, whitelistService);

            MainWindowInstance = new MainWindow(ViewModelInstance);
            MainWindowInstance.Activate();
        }
    }
}
