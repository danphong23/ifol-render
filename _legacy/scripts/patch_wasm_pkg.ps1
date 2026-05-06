<#
.SYNOPSIS
    Patches the wasm-pack generated package.json to use the scoped NPM name.
.DESCRIPTION
    wasm-pack generates pkg/package.json from the Cargo crate name.
    This script patches it with the correct scoped name, version, and author
    so it can be published to NPM under the @danphong23 scope.
.EXAMPLE
    .\scripts\patch_wasm_pkg.ps1
#>

$PkgPath = "crates\wasm\pkg\package.json"

if (-not (Test-Path $PkgPath)) {
    Write-Host "ERROR: $PkgPath not found. Run 'wasm-pack build' first." -ForegroundColor Red
    exit 1
}

$pkg = Get-Content $PkgPath -Raw | ConvertFrom-Json

# Patch fields
$pkg.name = "@danphong23/ifol-render-wasm"
$pkg.version = "0.2.0"
$pkg | Add-Member -NotePropertyName "author" -NotePropertyValue "danphong23" -Force
$pkg | Add-Member -NotePropertyName "repository" -NotePropertyValue @{
    type = "git"
    url  = "https://github.com/nicengi/ifol-render"
    directory = "crates/wasm"
} -Force
$pkg | Add-Member -NotePropertyName "keywords" -NotePropertyValue @("wasm", "webgpu", "rendering", "2d", "video-editor") -Force

$pkg | ConvertTo-Json -Depth 10 | Set-Content $PkgPath -Encoding UTF8

Write-Host "✅ Patched $PkgPath → $($pkg.name)@$($pkg.version)" -ForegroundColor Green
