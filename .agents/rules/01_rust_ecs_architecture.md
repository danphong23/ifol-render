---
trigger: always_on
glob: "**/*.rs"
description: Quy tắc kiến trúc Backend Rust ECS và wgpu khi làm việc với code Rust trong dự án ifol-render
---

# Quy tắc Rust Backend (ECS & Render)

## 1. Triết lý Pure ECS (Kế thừa từ kiến trúc gốc)
- **Tách biệt Data và Logic**: Components chỉ chứa dữ liệu (Pure Data). Tuyệt đối cấm viết các hàm xử lý logic (ngoài builder pattern hoặc `Default`) bên trong Struct của Component.
- Mọi logic xử lý (Time, Animation, Hierarchy, Render) phải được đặt trong các **Systems**.
- Tuân thủ nghiêm ngặt 3 Phase của pipeline: **Time → Resolve (Animation, Rect, Hierarchy) → Render (Source, Draw)**. Xem `docs/ARCHITECTURE.md` để biết chi tiết.

## 2. Hệ Tọa Độ & Đơn Vị (World Units)
- **Tất cả** position, size, offset trong ECS đều tính bằng đơn vị **unit (đơn vị trừu tượng)**. 1 unit ≠ 1 pixel. Unit không có kích thước vật lý.
- **Gốc tọa độ** `(0, 0)` nằm ở góc trên bên trái. X+ sang phải, Y+ xuống dưới.
- **Góc xoay (Rotation)** trên toàn bộ pipeline tính bằng **Radians**. Không bao giờ tự ý convert sang Degrees trừ khi hiển thị trên UI.
- **Pixel chỉ xuất hiện tại thời điểm render cuối (Flatten Pipeline)**: Khi `render_sys` cần vẽ lên GPU, nó mới quy đổi từ unit sang pixel dựa trên:
  - Camera view (vùng world camera nhìn thấy, tính bằng units)
  - Render target resolution (pixel)
  - Công thức: `scale = renderW / camera.width`, `pixelX = (entity.x - camera.x) * scale`
- Xem `docs/UNIT_SYSTEM.md` để biết chi tiết đầy đủ về PPU, Viewport, Flatten Pipeline.

## 3. Cấm Web-Sys và WASM
- Đây là Native Desktop App. **Tuyệt đối không** import hay sử dụng crate `web-sys`, `js-sys`, hay `wasm-bindgen` trong Core Engine.
- Không có bất kỳ logic nào liên quan đến HTML Canvas hay DOM.

## 4. Single Source of Truth & Multi-Interface
- Toàn bộ trạng thái của một dự án video nằm gọn trong **ECS World** (một struct Rust duy nhất).
- **Nhiều interface** (Tauri UI, MCP Server, CLI) đều giao tiếp với cùng một Core Rust duy nhất thông qua **Event Bus**. Không có interface nào được phép bypass Event Bus để trực tiếp mutate ECS World.
- Khi MCP Server nhận lệnh từ AI agent, nó gửi Event vào Event Bus → ECS World cập nhật → UI Tauri nhận event và tự cập nhật.
- Khi User kéo thả trên UI, Svelte gửi Tauri IPC command → Rust bắn Event → ECS World cập nhật → MCP Server có thể poll trạng thái mới.

## 5. Không Hard-Code
- Các đường dẫn (ffmpeg binary, thư mục project, asset paths) phải đọc từ file config hoặc Tauri App config. Không bao giờ hard-code đường dẫn tuyệt đối trong code Rust.
- Các hằng số magic number (FPS, resolution mặc định, v.v.) phải được khai báo ở 1 nơi tập trung (config module hoặc constants file).
