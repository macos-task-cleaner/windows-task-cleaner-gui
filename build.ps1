# Task Cleaner (WinUI 3) - Windows 11 Build & Publish Script
# Dual-licensed under GNU AGPLv3 and Commercial License.
param (
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64",
    [string]$OutputDir = "publish\win-x64"
)

# Force terminal and child process streams to UTF-8
chcp 65001 | Out-Null
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

# Silence .NET first-run telemetry and logo
$env:DOTNET_CLI_TELEMETRY_OPTOUT = "1"
$env:DOTNET_NOLOGO = "1"

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
 Task Cleaner (WinUI 3) - Standalone Publish Log
 Start Time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 Log File: $logFilePath
 Project Root: $projectRoot
 Target Runtime: $Runtime
 Configuration: $Configuration
 Output Dir: $OutputDir
========================================================
"@

Write-Host $header
$header | Out-File -FilePath $logFilePath -Encoding utf8

$script:buildExitCode = 0

& {
    Write-Host "[INFO] Checking .NET SDK environment..."
    $sdkVersion = dotnet --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] .NET SDK not detected. Please install .NET 8 SDK from https://dotnet.microsoft.com/download" -ForegroundColor Red
        $script:buildExitCode = 1
        return
    }
    Write-Host "[INFO] Current .NET SDK Version: $sdkVersion"

    Write-Host "[INFO] Publishing $Runtime standalone single-file executable..."
    $projectPath = "src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj"
    
    dotnet publish $projectPath -c $Configuration -r $Runtime --self-contained true -p:PublishSingleFile=true -o $OutputDir -v minimal
    $script:buildExitCode = $LASTEXITCODE

    if ($script:buildExitCode -ne 0) {
        Write-Host "[ERROR] Publish build failed with code: $($script:buildExitCode)" -ForegroundColor Red
    } else {
        $exePath = Join-Path $OutputDir "TaskCleaner.WinUI.exe"
        Write-Host "[SUCCESS] Publish successful! Standalone exe located at: $exePath" -ForegroundColor Green
    }
} 2>&1 | ForEach-Object {
    Write-Host $_
    $_ | Out-File -FilePath $logFilePath -Append -Encoding utf8
}

$exitCode = $script:buildExitCode

$footer = @"

========================================================
 End Time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 Status: $(if ($exitCode -eq 0) { 'SUCCESS' } else { "FAILED (Code: $exitCode)" })
 Log File: $logFilePath
 Latest Copy: $latestLogPath
========================================================
"@

Write-Host $footer
$footer | Out-File -FilePath $logFilePath -Append -Encoding utf8

try {
    Copy-Item -Path $logFilePath -Destination $latestLogPath -Force
} catch {
}

exit $exitCode
