# Task Cleaner (Windows 11) - Rust Native Run and Dev Script
# Dual-licensed under GNU AGPLv3 and Commercial License.
param (
    [string]$Mode = "gui",
    [switch]$Release
)

# Force terminal stream to UTF-8
chcp 65001 | Out-Null
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Continue"
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

# Ensure logs directory
$logsDir = Join-Path $projectRoot "logs"
if (-not (Test-Path $logsDir)) {
    New-Item -ItemType Directory -Path $logsDir -Force | Out-Null
}

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$logFilePath = Join-Path $logsDir "run_$timestamp.log"
$latestLogPath = Join-Path $logsDir "run_latest.log"

$header = @"
========================================================
 Task Cleaner (Rust Native Windows 11)
 Session Log: $timestamp
 Project Root: $projectRoot
 Mode: $Mode
========================================================
"@
Write-Host $header
$header | Out-File -FilePath $logFilePath -Encoding utf8

# Check cargo path
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoHome) {
        $env:PATH = "$cargoHome;" + $env:PATH
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $msg = "[ERROR] Cargo (Rust) not found. Please install Rust by running:`nwinget install Rustlang.Rustup`nor visit https://rustup.rs"
    Write-Host $msg -ForegroundColor Red
    $msg | Out-File -FilePath $logFilePath -Append -Encoding utf8
    exit 1
}

$cargoVer = cargo --version
Write-Host "[INFO] $cargoVer"
"[INFO] $cargoVer" | Out-File -FilePath $logFilePath -Append -Encoding utf8

# Optimize VM shared folder build speed by using local temp target directory
if (Test-Path "C:\") {
    $localTarget = "C:\Temp\taskcleaner-target"
    if (-not (Test-Path $localTarget)) {
        New-Item -ItemType Directory -Path $localTarget -Force | Out-Null
    }
    $env:CARGO_TARGET_DIR = $localTarget
    Write-Host "[INFO] Local VM Target Cache: $localTarget (High Performance I/O)"
}

$configArg = ""
$configName = "Debug"
if ($Release) {
    $configArg = "--release"
    $configName = "Release"
}

$binName = "TaskCleaner"
if ($Mode -eq "cli" -or $Mode -eq "mtc") {
    $binName = "mtc"
}

Write-Host "[INFO] Compiling $binName ($configName)..."
if ($configArg) {
    cargo build --bin $binName --release
} else {
    cargo build --bin $binName
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Build failed with exit code $LASTEXITCODE" -ForegroundColor Red
    exit $LASTEXITCODE
}

$subDir = "debug"
if ($Release) {
    $subDir = "release"
}

$targetBase = Join-Path $projectRoot "target"
if ($env:CARGO_TARGET_DIR) {
    $targetBase = $env:CARGO_TARGET_DIR
}
$exePath = Join-Path $targetBase "$subDir\$binName.exe"

if (-not (Test-Path $exePath)) {
    Write-Host "[ERROR] Compiled executable not found at: $exePath" -ForegroundColor Red
    exit 1
}

Write-Host "[SUCCESS] Launching: $exePath" -ForegroundColor Green
if ($binName -eq "mtc") {
    & $exePath @args
} else {
    Start-Process -FilePath $exePath
    Write-Host "[INFO] Task Cleaner Tray Application is now active in the system tray (bottom-right)." -ForegroundColor Cyan
}

try {
    Copy-Item -Path $logFilePath -Destination $latestLogPath -Force
} catch {
}
