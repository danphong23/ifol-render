@echo off
echo =======================================================
echo    Khởi động Môi trường WebGPU iFol-Render WASM
echo =======================================================
echo.
echo 1. Khởi động Local Asset Server (Python) ở cổng 8000...
start "iFol Asset Server" cmd /k "python server.py"

echo 2. Khởi động Web Frontend (Vite) ở cổng 5173...
start "iFol Vite Server" cmd /k "npm run dev"

echo.
echo Cả 2 server đang chạy ngầm trên 2 cửa sổ mới!
echo Hãy mở trình duyệt truy cập: http://localhost:5173
echo =======================================================
pause
