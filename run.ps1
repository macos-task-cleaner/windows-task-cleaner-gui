# Task Cleaner (WinUI 3) - Windows 11 Run and Log Script
# Dual-licensed under GNU AGPLv3 and Commercial License.
param (
    [string]$Configuration = "Debug"
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
$logFileName = "run_$timestamp.log"
$logFilePath = Join-Path $logsDir $logFileName
$latestLogPath = Join-Path $logsDir "run_latest.log"

$header = @"
========================================================
 Task Cleaner (WinUI 3) - Execution Session Log
 Start Time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 Log File: $logFilePath
 Working Dir: $projectRoot
 Build Config: $Configuration
========================================================
"@

Write-Host $header
$header | Out-File -FilePath $logFilePath -Encoding utf8

$script:runExitCode = 0

& {
    Write-Host "[INFO] Checking .NET SDK environment..."
    $sdkVersion = dotnet --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] .NET SDK not detected. Please install .NET 8 SDK from https://dotnet.microsoft.com/download" -ForegroundColor Red
        $script:runExitCode = 1
        return
    }
    Write-Host "[INFO] Current .NET SDK Version: $sdkVersion"

    $platform = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
    $arch = "win-$platform"
    Write-Host "[INFO] Detected Platform: $platform, Architecture: $arch"
    Write-Host "[INFO] Building and starting WinUI 3 Tray App ($Configuration, $arch)..."
    $projectPath = "src\TaskCleaner.WinUI\TaskCleaner.WinUI.csproj"
    
    dotnet run --project $projectPath -c $Configuration -r $arch -p:Platform=$platform -v minimal
    $script:runExitCode = $LASTEXITCODE

    if ($script:runExitCode -ne 0) {
        Write-Host "[ERROR] Application exited with code: $($script:runExitCode)" -ForegroundColor Red
    } else {
        Write-Host "[SUCCESS] Application exited normally." -ForegroundColor Green
    }
} 2>&1 | ForEach-Object {
    Write-Host $_
    $_ | Out-File -FilePath $logFilePath -Append -Encoding utf8
}

$exitCode = $script:runExitCode

$footer = @"

========================================================
 End Time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')
 Exit Code: $exitCode
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
