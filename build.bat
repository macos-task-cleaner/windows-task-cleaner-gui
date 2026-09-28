@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"

REM 预设 UTF-8 与静默环境变量，避免首次运行输出乱码或冗余遥测
set "DOTNET_CLI_TELEMETRY_OPTOUT=1"
set "DOTNET_NOLOGO=1"

REM 优先调用 build.ps1 以获得精准时间戳、实时 Tee 流输出与 UTF-8 日志记录
where powershell >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
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
set "LOG_FILE=logs\build_%TIMESTAMP%.log"
set "LATEST_LOG=logs\build_latest.log"

echo ========================================================
echo  Task Cleaner (WinUI 3) - 独立发布 (CMD 模式)
echo  构建日志: %LOG_FILE%
echo ========================================================
echo.
echo [INFO] 正在发布 win-x64 单文件免安装独立可执行文件...

dotnet publish src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -o publish\win-x64 > "%LOG_FILE%" 2>&1
set "EXIT_CODE=%ERRORLEVEL%"
type "%LOG_FILE%"
copy /y "%LOG_FILE%" "%LATEST_LOG%" >nul 2>&1

:HandleExit
if %EXIT_CODE% NEQ 0 (
    echo.
    echo [错误] 发布失败 (退出代码: %EXIT_CODE%)，请查看上述日志或 logs 目录。
) else (
    echo.
    echo [成功] 发布成功！单文件位于: publish\win-x64\TaskCleaner.WinUI.exe
)
pause

exit /b %EXIT_CODE%
