@echo off
echo ================================
echo  ifol-render SDK Test Environment
echo ================================
echo.

echo [1/2] Starting Asset Server on port 8000...
start "Asset Server" cmd /k "cd /d %~dp0 && python asset-server.py"

echo [2/2] Starting Vite Dev Server...
timeout /t 2 >nul
start "Vite Dev" cmd /k "cd /d %~dp0 && npx vite --host"

timeout /t 3 >nul
echo.
echo Ready! Open http://localhost:5173/test-sdk.html
echo.
start http://localhost:5173/test-sdk.html
