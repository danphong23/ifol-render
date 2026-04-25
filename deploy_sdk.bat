@echo off
setlocal

:: ── CẤU HÌNH ĐƯỜNG DẪN ──
:: Tên package của SDK nằm trong file Cargo.toml
set WASM_DIR=crates\wasm
set PKG_DIR=%WASM_DIR%\pkg

:: TODO: Sửa TARGET_APP_DIR thành đường dẫn tuyệt đối tới dự án App chính của bạn
:: (Ví dụ: C:\Users\abc\Desktop\ifol-video-editor\node_modules\ifol-render-wasm)
set TARGET_APP_DIR=C:\PATH_DEN_DU_AN_CUA_BAN\node_modules\ifol-render-wasm

echo ==============================================
echo 1. CLEANING OLD BUILD...
echo ==============================================
if exist "%PKG_DIR%" rd /s /q "%PKG_DIR%"

echo.
echo ==============================================
echo 2. BUILDING WASM SDK...
echo ==============================================
cmd /c "wasm-pack build --target web --out-dir pkg %WASM_DIR%"
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Wasm-pack build failed!
    exit /b %ERRORLEVEL%
)

echo.
echo ==============================================
echo 3. DEPLOYING TO MAIN APP...
echo ==============================================
if not exist "%TARGET_APP_DIR%" (
    echo [INFO] Target directory doesn't exist, creating: %TARGET_APP_DIR%
    mkdir "%TARGET_APP_DIR%"
)

:: Copy toàn bộ package đã build sang dự án chính
xcopy "%PKG_DIR%\*" "%TARGET_APP_DIR%\" /E /Y /I /Q

if %ERRORLEVEL% equ 0 (
    echo.
    echo ==============================================
    echo [SUCCESS] SDK DEPLOYED SUCCESSFULLY TO APP!
    echo ==============================================
) else (
    echo.
    echo [ERROR] Failed to copy SDK files!
)

endlocal
pause
