@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"

set "DOTNET_CLI_TELEMETRY_OPTOUT=1"
set "DOTNET_NOLOGO=1"

where powershell >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0run.ps1" %*
    set "EXIT_CODE=%ERRORLEVEL%"
    goto :HandleExit
)

if not exist "logs" mkdir "logs"
set "TIMESTAMP=%date:~0,4%%date:~5,2%%date:~8,2%_%time:~0,2%%time:~3,2%%time:~6,2%"
set "TIMESTAMP=%TIMESTAMP: =0%"
set "TIMESTAMP=%TIMESTAMP::=%"
set "TIMESTAMP=%TIMESTAMP:/=%"
set "TIMESTAMP=%TIMESTAMP:-=%"
set "LOG_FILE=logs\run_%TIMESTAMP%.log"
set "LATEST_LOG=logs\run_latest.log"

echo ========================================================
echo  Task Cleaner (WinUI 3) - Windows 11 Run (CMD Fallback)
echo  Session Log: %LOG_FILE%
echo ========================================================
if "%PROCESSOR_ARCHITECTURE%"=="ARM64" (
    set "ARCH=win-arm64"
) else (
    set "ARCH=win-x64"
)
echo [INFO] Building and starting WinUI 3 Tray App (Debug, %ARCH%)...

dotnet run --project src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj -c Debug -r %ARCH% -v minimal > "%LOG_FILE%" 2>&1
set "EXIT_CODE=%ERRORLEVEL%"
type "%LOG_FILE%"
copy /y "%LOG_FILE%" "%LATEST_LOG%" >nul 2>&1

:HandleExit
if %EXIT_CODE% NEQ 0 (
    echo.
    echo [ERROR] Launch failed with exit code: %EXIT_CODE%. Check logs for details.
    pause
)

exit /b %EXIT_CODE%
