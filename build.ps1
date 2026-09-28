# Task Cleaner (Windows 11) - Release Build Script
# Dual-licensed under GNU AGPLv3 and Commercial License.

chcp 65001 | Out-Null
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "Continue"
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $projectRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoHome) {
        $env:PATH = "$cargoHome;" + $env:PATH
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[ERROR] Cargo (Rust) not found. Run: winget install Rustlang.Rustup" -ForegroundColor Red
    exit 1
}

if (Test-Path "C:\") {
    $localTarget = "C:\Temp\taskcleaner-target"
    if (-not (Test-Path $localTarget)) {
        New-Item -ItemType Directory -Path $localTarget -Force | Out-Null
    }
    $env:CARGO_TARGET_DIR = $localTarget
}

Write-Host "[INFO] Building Task Cleaner Suite in Release mode..."
cargo build --release --workspace

if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Build failed." -ForegroundColor Red
    exit $LASTEXITCODE
}

$publishDir = Join-Path $projectRoot "publish"
if (-not (Test-Path $publishDir)) {
    New-Item -ItemType Directory -Path $publishDir -Force | Out-Null
}

$targetBase = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot "target" }

Copy-Item -Path (Join-Path $targetBase "release\mtc.exe") -Destination $publishDir -Force
Copy-Item -Path (Join-Path $targetBase "release\TaskCleaner.exe") -Destination $publishDir -Force

$mtcSize = (Get-Item (Join-Path $publishDir "mtc.exe")).Length / 1MB
$guiSize = (Get-Item (Join-Path $publishDir "TaskCleaner.exe")).Length / 1MB

Write-Host "========================================================" -ForegroundColor Green
Write-Host " [SUCCESS] Build Complete! Published to: $publishDir" -ForegroundColor Green
Write-Host "  * mtc.exe (CLI):          $([math]::Round($mtcSize, 2)) MB" -ForegroundColor Green
Write-Host "  * TaskCleaner.exe (Tray): $([math]::Round($guiSize, 2)) MB" -ForegroundColor Green
Write-Host "========================================================" -ForegroundColor Green
