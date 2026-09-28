using System;
using System.IO;

namespace TaskCleaner.Core.Services
{
    public static class AppLogger
    {
        private static readonly object _lock = new();
        private static string? _logFilePath;
        private static string? _latestLogFilePath;

        public static string LogFilePath
        {
            get
            {
                if (_logFilePath == null)
                {
                    InitializeLogPath();
                }
                return _logFilePath!;
            }
        }

        private static void InitializeLogPath()
        {
            string baseDir = AppDomain.CurrentDomain.BaseDirectory;
            string logsDir = Path.Combine(baseDir, "logs");

            // 若当前执行路径上层存在 logs 目录 (开发环境根目录)，优先写入开发日志区
            string parentLogsDir = Path.Combine(baseDir, "..", "..", "..", "..", "logs");
            if (Directory.Exists(parentLogsDir))
            {
                logsDir = Path.GetFullPath(parentLogsDir);
            }
            else if (!Directory.Exists(logsDir))
            {
                try
                {
                    Directory.CreateDirectory(logsDir);
                }
                catch
                {
                    // 若无写入权限，回退至 %APPDATA%\TaskCleaner\logs
                    var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
                    logsDir = Path.Combine(appData, "TaskCleaner", "logs");
                    Directory.CreateDirectory(logsDir);
                }
            }

            string timeStr = DateTime.Now.ToString("yyyyMMdd_HHmmss");
            _logFilePath = Path.Combine(logsDir, $"app_{timeStr}.log");
            _latestLogFilePath = Path.Combine(logsDir, "app_latest.log");

            try
            {
                File.WriteAllText(_latestLogFilePath, $"[SESSION START: {DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}]" + Environment.NewLine);
            }
            catch
            {
            }
        }

        public static void Info(string message) => Log("INFO", message);
        public static void Warn(string message) => Log("WARN", message);
        public static void Error(string message, Exception? ex = null)
        {
            string fullMsg = ex != null ? $"{message}\n{ex}" : message;
            Log("ERROR", fullMsg);
        }

        private static void Log(string level, string message)
        {
            lock (_lock)
            {
                try
                {
                    string path = LogFilePath;
                    string time = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff");
                    string line = $"[{time}] [{level}] {message}";

                    File.AppendAllText(path, line + Environment.NewLine);
                    if (!string.IsNullOrEmpty(_latestLogFilePath))
                    {
                        File.AppendAllText(_latestLogFilePath, line + Environment.NewLine);
                    }
                }
                catch
                {
                    // 保证日志模块绝不抛出未捕获异常中断主程序
                }
            }
        }
    }
}

