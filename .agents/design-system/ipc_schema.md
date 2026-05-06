---
trigger: always_on
glob:
description: Quy chuẩn giao tiếp dữ liệu giữa các interface (UI, MCP, CLI) và Rust Core
---

# Giao tiếp IPC & Data Schema

## 1. Kiến trúc Multi-Interface / Single Core

Tất cả các interface đều giao tiếp với **cùng một Rust Core** duy nhất thông qua Event Bus:

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Tauri UI    │  │  MCP Server  │  │     CLI      │
│  (Svelte+TS) │  │  (AI Agent)  │  │  (Headless)  │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │  Tauri IPC      │  rmcp/stdio      │  Direct call
       │                 │                  │
       ▼                 ▼                  ▼
┌─────────────────────────────────────────────────────┐
│                    Event Bus                         │
│  (Receive commands, dispatch state changes)          │
├─────────────────────────────────────────────────────┤
│                 ECS World (Rust)                     │
│  Single Source of Truth — project state lives here   │
├─────────────────────────────────────────────────────┤
│              wgpu Render Engine                      │
│  Native viewport rendering                           │
└─────────────────────────────────────────────────────┘
```

**Nguyên tắc**: Dù bạn kéo timeline trên UI, hay AI gửi lệnh qua MCP, hay CLI chạy export — tất cả đều bắn Event vào Event Bus → Event Bus update ECS World → Mọi interface đều nhận trạng thái mới.

## 2. Single Source of Truth (Rust Owns All Types)
- Mọi Type Definition (Interface dữ liệu) đều bắt nguồn từ **Rust Structs**.
- Frontend (TypeScript) **KHÔNG ĐƯỢC** tự định nghĩa interface cho dữ liệu lõi (ECS Entity, Scene State, Render Config, Frame Data).

## 3. Quy trình đồng bộ Type (Bắt buộc dùng `ts-rs`)
- Trong Rust Backend, mọi Struct truyền qua Tauri IPC phải được gắn macro `#[derive(TS)]` và `#[ts(export)]` sử dụng crate `ts-rs`.
- Khi build Rust, các file `.ts` tương ứng sẽ được tự động sinh ra trong thư mục TypeScript bindings.
- Frontend Svelte bắt buộc phải `import` các type/interface từ thư mục bindings này. Điều này đảm bảo Type Safety tuyệt đối qua IPC Boundary.

## 4. Quy chuẩn gọi lệnh (Tauri Commands)
Mọi hàm `invoke` từ TypeScript gọi xuống Rust phải tuân thủ naming convention:
- Bắt đầu bằng động từ hành động rõ ràng: `get_`, `set_`, `update_`, `delete_`, `trigger_`.
- Ví dụ: `invoke("update_entity_transform", { payload: TransformData })`.
- Rust bắt buộc phải trả về kiểu `Result<T, String>` để Frontend xử lý thông báo lỗi.

## 5. Không Hard-Code, Dùng Config
- Các giá trị cấu hình (đường dẫn ffmpeg, thư mục mặc định, FPS mặc định, resolution mặc định) phải nằm trong file config (ví dụ: `config.toml` hoặc Tauri config).
- Không hard-code đường dẫn tuyệt đối, magic numbers, hay giá trị cấu hình trực tiếp trong code.
- Config phải có cơ chế override: Default config → User config → Runtime args.
