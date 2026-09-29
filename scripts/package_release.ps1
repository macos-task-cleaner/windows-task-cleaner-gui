# Task Cleaner Windows - Packaging & Release Utility
# Dual-licensed under GNU AGPLv3 and Commercial License.
param(
    [string]$Version = "1.0.0",
    [switch]$SkipBuild,
    [switch]$BuildArm64,
    [switch]$BuildX86
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$projectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $projectRoot

Write-Host "========================================================" -ForegroundColor Cyan
Write-Host " Task Cleaner (Windows 11) - Release Packaging Pipeline" -ForegroundColor Cyan
Write-Host " Version: $Version | Project Root: $projectRoot" -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan

$distDir = Join-Path $projectRoot "dist"
$publishDir = Join-Path $projectRoot "publish"
$publishX64 = Join-Path $publishDir "x64"
$publishX86 = Join-Path $publishDir "x86"
$publishArm64 = Join-Path $publishDir "arm64"

if (-not (Test-Path $distDir)) { New-Item -ItemType Directory -Path $distDir -Force | Out-Null }
if (-not (Test-Path $publishX64)) { New-Item -ItemType Directory -Path $publishX64 -Force | Out-Null }
if ($BuildX86 -and -not (Test-Path $publishX86)) { New-Item -ItemType Directory -Path $publishX86 -Force | Out-Null }
if ($BuildArm64 -and -not (Test-Path $publishArm64)) { New-Item -ItemType Directory -Path $publishArm64 -Force | Out-Null }

# 1. 编译步骤
if (-not $SkipBuild) {
    # 1.1 x86_64 (64位 AMD/Intel 标准版，Windows 10/11 主流)
    Write-Host "[1/4] Compiling Windows x86_64 (x64) Release Suite..." -ForegroundColor Yellow
    cargo build --release --workspace --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] x86_64 build failed." -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Copy-Item "target\x86_64-pc-windows-msvc\release\TaskCleaner.exe" $publishX64 -Force
    Copy-Item "target\x86_64-pc-windows-msvc\release\mtc.exe" $publishX64 -Force

    # 1.2 i686 (32位 x86 兼容版)
    if ($BuildX86) {
        Write-Host "[1.1/4] Compiling Windows i686 (32-bit x86) Release Suite..." -ForegroundColor Yellow
        cargo build --release --workspace --target i686-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  [ERROR] 32-bit x86 target compilation failed with exit code $LASTEXITCODE." -ForegroundColor Red
            exit $LASTEXITCODE
        }
        Copy-Item "target\i686-pc-windows-msvc\release\TaskCleaner.exe" $publishX86 -Force
        Copy-Item "target\i686-pc-windows-msvc\release\mtc.exe" $publishX86 -Force
        Write-Host "  * 32-bit x86 binaries compiled successfully." -ForegroundColor Green
    }

    # 1.3 aarch64 (ARM64 架构版)
    if ($BuildArm64) {
        Write-Host "[1.2/4] Compiling Windows aarch64 (ARM64) Release Suite..." -ForegroundColor Yellow
        cargo build --release --workspace --target aarch64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  [ERROR] ARM64 target compilation failed with exit code $LASTEXITCODE." -ForegroundColor Red
            exit $LASTEXITCODE
        }
        Copy-Item "target\aarch64-pc-windows-msvc\release\TaskCleaner.exe" $publishArm64 -Force
        Copy-Item "target\aarch64-pc-windows-msvc\release\mtc.exe" $publishArm64 -Force
        Write-Host "  * ARM64 binaries compiled successfully." -ForegroundColor Green
    }
}

# 2. 制作便携版 (Portable ZIP)
Write-Host "[2/4] Packaging Portable ZIP archives..." -ForegroundColor Yellow

function Create-PortableZip($srcDir, $archName, $zipBaseName) {
    $tempFolder = Join-Path $distDir "TaskCleaner-Windows-$archName-portable"
    if (Test-Path $tempFolder) { Remove-Item $tempFolder -Recurse -Force }
    New-Item -ItemType Directory -Path $tempFolder -Force | Out-Null

    Copy-Item (Join-Path $srcDir "TaskCleaner.exe") $tempFolder -Force
    Copy-Item (Join-Path $srcDir "mtc.exe") $tempFolder -Force
    Copy-Item "README.md" $tempFolder -Force
    Copy-Item "LICENSE" $tempFolder -Force
    Copy-Item "COMMERCIAL.md" $tempFolder -Force
    Copy-Item "assets\app.ico" $tempFolder -Force

    $destZip = Join-Path $distDir "$zipBaseName.zip"
    if (Test-Path $destZip) { Remove-Item $destZip -Force }
    Compress-Archive -Path "$tempFolder\*" -DestinationPath $destZip -CompressionLevel Optimal
    Remove-Item $tempFolder -Recurse -Force

    Write-Host "  * Portable ZIP created: $destZip" -ForegroundColor Green
    return $destZip
}

Create-PortableZip $publishX64 "x64" "TaskCleaner-Windows-x64-Portable"
Create-PortableZip $publishX64 "x64" "TaskCleaner-Windows-x64"

if ($BuildX86 -and (Test-Path (Join-Path $publishX86 "TaskCleaner.exe"))) {
    Create-PortableZip $publishX86 "x86" "TaskCleaner-Windows-x86-Portable"
}

if ($BuildArm64 -and (Test-Path (Join-Path $publishArm64 "TaskCleaner.exe"))) {
    Create-PortableZip $publishArm64 "arm64" "TaskCleaner-Windows-arm64-Portable"
}

# 3. 制作安装包 (Inno Setup Setup.exe)
Write-Host "[3/4] Building Inno Setup installer..." -ForegroundColor Yellow

$isccPath = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $isccPath)) {
    $isccCmd = Get-Command "iscc" -ErrorAction SilentlyContinue
    if ($isccCmd) { $isccPath = $isccCmd.Source }
}

if (Test-Path $isccPath) {
    & $isccPath "/DMyAppVersion=$Version" "installer.iss"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  [ERROR] Inno Setup compilation failed with exit code $LASTEXITCODE." -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "  * Setup Exe created: $distDir\TaskCleaner-Windows-x64-Setup.exe" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Inno Setup compiler (ISCC.exe) not found." -ForegroundColor Yellow
    Write-Host "  To build Setup.exe locally: winget install JRSoftware.InnoSetup" -ForegroundColor Yellow
}

# 4. 生成 SHA-256 校验和清单
Write-Host "[4/4] Generating SHA-256 Checksums..." -ForegroundColor Yellow
Get-ChildItem -Path $distDir -File | Where-Object { $_.Extension -in ".exe", ".zip" } | ForEach-Object {
    $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
    $hashLine = "$hash  $($_.Name)"
    $hashFile = "$($_.FullName).sha256"
    Set-Content -Path $hashFile -Value $hashLine -Encoding UTF8
    Write-Host "  * $($_.Name): $hash" -ForegroundColor Gray
}

Write-Host "========================================================" -ForegroundColor Green
Write-Host " [SUCCESS] All distribution assets ready in: $distDir" -ForegroundColor Green
Write-Host "========================================================" -ForegroundColor Green
