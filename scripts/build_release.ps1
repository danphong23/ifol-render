<#
.SYNOPSIS
    Builds the ifol-render ecosystem for a release.

.DESCRIPTION
    This script automates the process of building the CLI, Studio, WASM, and SDK.
    It packages the Rust binaries into a zip file for GitHub Releases,
    and packs the WASM and SDK into .tgz files for NPM.

.EXAMPLE
    .\scripts\build_release.ps1
#>

$ErrorActionPreference = "Stop"

$ReleaseDir = "release_builds"
if (Test-Path $ReleaseDir) {
    Remove-Item -Recurse -Force $ReleaseDir
}
New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null

Write-Host "=== ifol-render Release Build ===" -ForegroundColor Cyan

# ── 1. Rust Binaries (CLI & Studio) ──
Write-Host "`n[1/4] Building Rust Binaries (Release mode)..." -ForegroundColor Yellow
cargo build --release -p ifol-render-cli
cargo build --release -p ifol-render-studio

$BinDir = "$ReleaseDir\ifol-render-windows-x64"
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Copy-Item "target\release\ifol-render-cli.exe" -Destination $BinDir
Copy-Item "target\release\ifol-render-studio.exe" -Destination $BinDir
Compress-Archive -Path "$BinDir\*" -DestinationPath "$ReleaseDir\ifol-render-windows-x64.zip" -Force
Write-Host "  -> $ReleaseDir\ifol-render-windows-x64.zip" -ForegroundColor Green

# ── 2. WASM Package ──
Write-Host "`n[2/4] Building WASM Package..." -ForegroundColor Yellow
Push-Location "crates\wasm"
wasm-pack build --target web --release
Pop-Location

# Patch the generated package.json with scoped name and metadata
& ".\scripts\patch_wasm_pkg.ps1"

# Pack it
Push-Location "crates\wasm\pkg"
npm pack --pack-destination "..\..\..\$ReleaseDir"
Pop-Location
Write-Host "  -> $ReleaseDir\danphong23-ifol-render-wasm-*.tgz" -ForegroundColor Green

# ── 3. SDK Package ──
Write-Host "`n[3/4] Building SDK..." -ForegroundColor Yellow
Push-Location "sdk"
npm install
npm run build
npm pack --pack-destination "..\$ReleaseDir"
Pop-Location
Write-Host "  -> $ReleaseDir\danphong23-ifol-render-sdk-*.tgz" -ForegroundColor Green

# ── 4. Summary ──
Write-Host "`n=== Release Build Complete ===" -ForegroundColor Cyan
Write-Host "Artifacts in '$ReleaseDir\':"
Get-ChildItem $ReleaseDir | ForEach-Object {
    $size = if ($_.Length -gt 1MB) { "{0:N1} MB" -f ($_.Length / 1MB) } else { "{0:N0} KB" -f ($_.Length / 1KB) }
    Write-Host "  $($_.Name)  ($size)" -ForegroundColor Green
}

Write-Host "`nPublish order:" -ForegroundColor Yellow
Write-Host "  1. npm publish crates/wasm/pkg   (WASM first)"
Write-Host "  2. npm publish sdk               (SDK depends on WASM)"
Write-Host "  3. Upload .zip to GitHub Releases (Binaries)"
