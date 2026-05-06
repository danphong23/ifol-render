# 🤖 AI Agent Entry Point (ifol-render)

Chào mừng AI Agent (Antigravity/Cursor/Claude). Mọi hành động sửa code, tạo file của bạn trong dự án này đều **BẮT BUỘC** phải tuân thủ các Rules và Workflows trong thư mục `.agents/`.

## 1. Bản Chất Dự Án
ifol-render là một trình Video/Motion Graphics Editor lai (Hybrid) hiệu năng cao:
- **Frontend (UI)**: Svelte 5 + TypeScript + TailwindCSS chạy trong Tauri WebView.
- **Backend (Lõi)**: Rust xử lý ECS (Entity Component System) theo kiến trúc cũ (xem `docs/ARCHITECTURE.md`).
- **Render Viewport**: Rust wgpu Native Surface (KHÔNG vẽ trong HTML Canvas/WebGL).
- **Decoder**: Native `ffmpeg.exe` subprocess (KHÔNG dùng HTML5 `<video>`).

## 2. Lệnh Cấm Tuyệt Đối (DO NOTs)
- ❌ **KHÔNG** dùng WebGL/HTML5 Canvas cho việc render video chính.
- ❌ **KHÔNG** viết logic vào các ECS Components (Phải là Pure Data).
- ❌ **KHÔNG** làm việc trực tiếp với UI state mà bỏ qua Event Bus của Rust.
- ❌ **KHÔNG** dùng Vanilla JS. Bắt buộc dùng TypeScript 100%.

## 3. Hệ Thống Tra Cứu (Nơi bạn tìm hướng dẫn)
Bạn phải đọc các file sau trong `.agents/` trước khi chạm vào mã nguồn:
- 🎨 Design System (Giao diện & Dữ liệu): Xem `.agents/design-system/`
- 🦀 Sửa code Rust/Backend: Xem `.agents/rules/01_rust_ecs_architecture.md`
- 🌐 Sửa code Svelte/Frontend: Xem `.agents/rules/02_ui_framework.md`
- 🛠️ Viết Tool cho MCP Server: Xem `.agents/rules/03_mcp_server.md`
