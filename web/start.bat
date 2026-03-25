@echo off
echo =======================================================
echo    ifol-render ECS Editor Test
echo =======================================================
echo.
echo 1. Starting Asset Server (Python) on port 8000...
start "iFol Asset Server" cmd /k "cd /d %~dp0 && python server.py"

echo 2. Starting Web Frontend (Vite) on port 5173...
start "iFol Vite Server" cmd /k "cd /d %~dp0 && npm run dev"

echo.
echo Both servers are running in new windows!
echo Open your browser to: http://localhost:5173
echo =======================================================
timeout /t 3 /nobreak >nul
start http://localhost:5173
