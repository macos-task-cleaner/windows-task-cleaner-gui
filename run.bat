@echo off
echo ========================================================
echo  Task Cleaner (WinUI 3) - Windows 11 极速启动向导
echo ========================================================
echo.
echo 正在以 Debug 模式编译并启动 WinUI 3 托盘应用程序...
dotnet run --project src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj -c Debug
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [错误] 启动遇到问题，请确认已安装 .NET 8 SDK 并具备 Windows App SDK 运行环境。
    pause
)
