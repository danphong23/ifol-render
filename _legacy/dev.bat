@echo off
set PATH=%USERPROFILE%\.cargo\bin;%PATH%

if "%1"=="" goto editor
if "%1"=="editor" goto editor
if "%1"=="info" goto info
if "%1"=="preview" goto preview
if "%1"=="build" goto build
if "%1"=="test" goto test

echo Usage: dev.bat [editor^|info^|preview^|build^|test]
goto :eof

:editor
echo Starting ifol-render studio...
cargo run -p ifol-render-studio
goto :eof

:info
echo Scene info:
cargo run -p ifol-render-cli -- info --scene examples/simple_scene.json
goto :eof

:preview
echo Rendering preview at 2.0s...
cargo run -p ifol-render-cli -- preview --scene examples/simple_scene.json --timestamp 2.0 --output preview.png
goto :eof

:build
echo Building release...
cargo build --release --workspace --exclude ifol-render-wasm
echo Done! Binaries in target\release\
goto :eof

:test
echo Running tests...
cargo test --workspace --exclude ifol-render-wasm
goto :eof
