@echo off
setlocal
set "SCRIPT_DIR=%~dp0"
echo =======================================================
echo    ifol-render ECS Editor (V4 Pipeline)
echo =======================================================
echo.

echo [1/2] Compiling WASM backend...
call wasm-pack build "%SCRIPT_DIR%crates\wasm" --target web --out-dir "%SCRIPT_DIR%crates\wasm\pkg"

echo.
echo [2/2] Starting Development Server...
echo The browser will open automatically.
cd /d "%SCRIPT_DIR%web"
call npm run dev
