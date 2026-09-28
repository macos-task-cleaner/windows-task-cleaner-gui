@echo off
setlocal
cd /d "%~dp0"

REM 优先调用 run.ps1 以获得精准时间戳、实时 Tee 流输出与 UTF-8 日志记录
where powershell >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0run.ps1" %*
    set "EXIT_CODE=%ERRORLEVEL%"
    goto :HandleExit
)

REM 兜底模式：若系统无 PowerShell，以纯 CMD 方式运行并输出时间戳日志
if not exist "logs" mkdir "logs"
set "TIMESTAMP=%date:~0,4%%date:~5,2%%date:~8,2%_%time:~0,2%%time:~3,2%%time:~6,2%"
set "TIMESTAMP=%TIMESTAMP: =0%"
set "TIMESTAMP=%TIMESTAMP::=%"
set "TIMESTAMP=%TIMESTAMP:/=%"
set "TIMESTAMP=%TIMESTAMP:-=%"
set "LOG_FILE=logs\run_%TIMESTAMP%.log"
set "LATEST_LOG=logs\run_latest.log"

echo ========================================================
echo  Task Cleaner (WinUI 3) - Windows 11 启动向导 (CMD 模式)
echo  会话日志: %LOG_FILE%
echo ========================================================
echo.
echo [INFO] 正在以 Debug 模式编译并启动 WinUI 3 托盘应用程序...

dotnet run --project src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj -c Debug > "%LOG_FILE%" 2>&1
set "EXIT_CODE=%ERRORLEVEL%"
type "%LOG_FILE%"
copy /y "%LOG_FILE%" "%LATEST_LOG%" >nul 2>&1

:HandleExit
if %EXIT_CODE% NEQ 0 (
    echo.
    echo [错误] 启动或运行遇到问题 (退出代码: %EXIT_CODE%)，请查看上述日志或 logs 目录。
    pause
)

exit /b %EXIT_CODE%
