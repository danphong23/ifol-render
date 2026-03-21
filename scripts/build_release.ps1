<#
.SYNOPSIS
    Builds the ifol-render ecosystem for a release.

.DESCRIPTION
    This script automates the process of building the CLI, Studio, WASM, and SDK.
    It packages the Rust binaries into a zip file for GitHub Releases, and packs the SDK into a .tgz file for NPM.

.EXAMPLE
    .\scripts\build_release.ps1
#>

$ErrorActionPreference = "Stop"

$ReleaseDir = "release_builds"
if (Test-Path $ReleaseDir) {
    Remove-Item -Recurse -Force $ReleaseDir
}
New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null

Write-Host "🚀 Building ifol-render ecosystem for release..." -ForegroundColor Cyan

# 1. Build Rust Binaries (CLI and Studio)
Write-Host "📦 Building Rust Binaries (CLI & Studio) in Release mode..." -ForegroundColor Yellow
cargo build --release -p ifol-render-cli
cargo build --release -p ifol-render-studio

Write-Host "Copying binaries to release folder..." -ForegroundColor Yellow
$BinDir = "$ReleaseDir\ifol-render-windows-x64"
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Copy-Item "target\release\ifol-render-cli.exe" -Destination $BinDir
Copy-Item "target\release\ifol-render-studio.exe" -Destination $BinDir

Write-Host "Zipping binaries..." -ForegroundColor Yellow
Compress-Archive -Path "$BinDir\*" -DestinationPath "$ReleaseDir\ifol-render-windows-x64.zip" -Force

# 2. Build WASM
Write-Host "🌐 Building WASM Package..." -ForegroundColor Yellow
Set-Location "crates\wasm"
wasm-pack build --target web --release
Set-Location "..\.."

# 3. Build SDK and Pack
Write-Host "📦 Building and Packing SDK for NPM..." -ForegroundColor Yellow
Set-Location "sdk"
npm install
npm run build
npm pack --pack-destination "..\$ReleaseDir"
Set-Location ".."

Write-Host "✅ Release build complete! Artifacts are in the '$ReleaseDir' folder." -ForegroundColor Green
Write-Host " - GitHub Release: $ReleaseDir\ifol-render-windows-x64.zip"
Write-Host " - NPM Release:    $ReleaseDir\ifol-render-sdk-*.tgz"
