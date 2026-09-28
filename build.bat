@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"

set "DOTNET_CLI_TELEMETRY_OPTOUT=1"
set "DOTNET_NOLOGO=1"

where powershell >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
    set "EXIT_CODE=%ERRORLEVEL%"
    goto :HandleExit
)

if not exist "logs" mkdir "logs"
set "TIMESTAMP=%date:~0,4%%date:~5,2%%date:~8,2%_%time:~0,2%%time:~3,2%%time:~6,2%"
set "TIMESTAMP=%TIMESTAMP: =0%"
set "TIMESTAMP=%TIMESTAMP::=%"
set "TIMESTAMP=%TIMESTAMP:/=%"
set "TIMESTAMP=%TIMESTAMP:-=%"
set "LOG_FILE=logs\build_%TIMESTAMP%.log"
set "LATEST_LOG=logs\build_latest.log"

echo ========================================================
echo  Task Cleaner (WinUI 3) - Standalone Publish (CMD Fallback)
echo  Build Log: %LOG_FILE%
echo ========================================================
echo.
echo [INFO] Publishing win-x64 standalone executable...

dotnet publish src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -o publish\win-x64 -v minimal > "%LOG_FILE%" 2>&1
set "EXIT_CODE=%ERRORLEVEL%"
type "%LOG_FILE%"
copy /y "%LOG_FILE%" "%LATEST_LOG%" >nul 2>&1

:HandleExit
if %EXIT_CODE% NEQ 0 (
    echo.
    echo [ERROR] Publish failed with exit code: %EXIT_CODE%. Check logs for details.
) else (
    echo.
    echo [SUCCESS] Publish complete! Standalone exe located at: publish\win-x64\TaskCleaner.WinUI.exe
)
pause

exit /b %EXIT_CODE%
