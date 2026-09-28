# Task Cleaner (WinUI 3) - Windows 11 运行与日志捕获脚本
# Dual-licensed under GNU AGPLv3 and Commercial License.
param (
    [string]$Configuration = "Debug"
)

$ErrorActionPreference = "Continue"
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

$logsDir = Join-Path $projectRoot "logs"
if (-not (Test-Path $logsDir)) {
    New-Item -ItemType Directory -Path $logsDir -Force | Out-Null
}

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$logFileName = "run_$timestamp.log"
$logFilePath = Join-Path $logsDir $logFileName
$latestLogPath = Join-Path $logsDir "run_latest.log"

$header = @"
========================================================
 Task Cleaner (WinUI 3) - 运行会话日志
 启动时间: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 日志文件: $logFilePath
 工作目录: $projectRoot
 编译配置: $Configuration
========================================================
"@

Write-Host $header
$header | Out-File -FilePath $logFilePath -Encoding utf8

$script:runExitCode = 0

& {
    Write-Host "[INFO] 检查 .NET SDK 环境..."
    $sdkVersion = dotnet --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] 未检测到 .NET SDK，请访问 https://dotnet.microsoft.com/download 安装 .NET 8 SDK。" -ForegroundColor Red
        $script:runExitCode = 1
        return
    }
    Write-Host "[INFO] 当前 .NET SDK 版本: $sdkVersion"

    Write-Host "[INFO] 正在编译并启动 WinUI 3 托盘应用程序 ($Configuration)..."
    $projectPath = "src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj"
    
    dotnet run --project $projectPath -c $Configuration
    $script:runExitCode = $LASTEXITCODE

    if ($script:runExitCode -ne 0) {
        Write-Host "[ERROR] 应用程序退出，退出代码: $($script:runExitCode)" -ForegroundColor Red
    } else {
        Write-Host "[SUCCESS] 应用程序已正常退出。" -ForegroundColor Green
    }
} 2>&1 | Tee-Object -FilePath $logFilePath -Append

$exitCode = $script:runExitCode

$footer = @"

========================================================
 运行结束时间: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 最终退出代码: $exitCode
 日志已持久化: $logFilePath
 最新日志副本: $latestLogPath
========================================================
"@

Write-Host $footer
$footer | Out-File -FilePath $logFilePath -Append -Encoding utf8

try {
    Copy-Item -Path $logFilePath -Destination $latestLogPath -Force
} catch {
}

exit $exitCode
