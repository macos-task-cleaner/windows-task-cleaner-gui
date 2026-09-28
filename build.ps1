# Task Cleaner (WinUI 3) - Windows 11 单文件发布与日志捕获脚本
# Dual-licensed under GNU AGPLv3 and Commercial License.
param (
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64",
    [string]$OutputDir = "publish\win-x64"
)

$ErrorActionPreference = "Continue"
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

$logsDir = Join-Path $projectRoot "logs"
if (-not (Test-Path $logsDir)) {
    New-Item -ItemType Directory -Path $logsDir -Force | Out-Null
}

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$logFileName = "build_$timestamp.log"
$logFilePath = Join-Path $logsDir $logFileName
$latestLogPath = Join-Path $logsDir "build_latest.log"

$header = @"
========================================================
 Task Cleaner (WinUI 3) - 独立单文件发布日志
 开始时间: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 日志文件: $logFilePath
 项目根目录: $projectRoot
 目标架构: $Runtime
 编译配置: $Configuration
 输出目录: $OutputDir
========================================================
"@

Write-Host $header
$header | Out-File -FilePath $logFilePath -Encoding utf8

$script:buildExitCode = 0

& {
    Write-Host "[INFO] 检查 .NET SDK 环境..."
    $sdkVersion = dotnet --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] 未检测到 .NET SDK，请访问 https://dotnet.microsoft.com/download 安装 .NET 8 SDK。" -ForegroundColor Red
        $script:buildExitCode = 1
        return
    }
    Write-Host "[INFO] 当前 .NET SDK 版本: $sdkVersion"

    Write-Host "[INFO] 开始发布 $Runtime 独立免安装单文件可执行文件..."
    $projectPath = "src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj"
    
    dotnet publish $projectPath -c $Configuration -r $Runtime --self-contained true -p:PublishSingleFile=true -o $OutputDir
    $script:buildExitCode = $LASTEXITCODE

    if ($script:buildExitCode -ne 0) {
        Write-Host "[ERROR] 发布构建失败，退出代码: $($script:buildExitCode)" -ForegroundColor Red
    } else {
        $exePath = Join-Path $OutputDir "TaskCleaner.WinUI.exe"
        Write-Host "[SUCCESS] 发布成功！单文件位于: $exePath" -ForegroundColor Green
    }
} 2>&1 | Tee-Object -FilePath $logFilePath -Append

$exitCode = $script:buildExitCode

$footer = @"

========================================================
 发布结束时间: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 最终构建状态: $(if ($exitCode -eq 0) { '成功 (SUCCESS)' } else { "失败 (FAILED, Code: $exitCode)" })
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
