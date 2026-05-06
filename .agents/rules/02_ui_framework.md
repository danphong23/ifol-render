---
trigger: always_on
glob:
description: Quy tắc bắt buộc khi code Frontend UI (Svelte + TypeScript) cho ifol-render
---

# Quy tắc Frontend (Svelte 5 + TypeScript + Tauri)

## 1. Tech Stack Bắt Buộc
- **Framework**: Svelte 5 (Sử dụng Runes: `$state`, `$derived`, `$effect`).
- **Ngôn ngữ**: TypeScript 100%. Tuyệt đối không tạo file `.js`. Mọi file phải là `.ts` hoặc `.svelte` với `<script lang="ts">`.
- **Styling**: TailwindCSS.
- **Giao tiếp Backend**: Tauri IPC API (`@tauri-apps/api/core`).

## 2. Frontend là Display Layer Thuần Túy (Không chứa Logic)
Đây là nguyên tắc quan trọng nhất. Frontend hoạt động giống một **Native App UI** chứ không phải một Web App truyền thống:
- **Mọi vòng lặp chính (main loop), state management, và business logic đều nằm ở Rust Core.** Frontend KHÔNG được tự quản lý Timeline state, ECS state, hay Playback state.
- Frontend chỉ có 2 vai trò duy nhất:
  1. **Gửi lệnh** (Commands): Khi user tương tác (click, drag, nhập text), Frontend gửi lệnh xuống Rust qua Tauri IPC. Ví dụ: `invoke('update_entity_transform', { entityId, x, y })`.
  2. **Nhận và hiển thị** (Events/State): Khi Rust Core thay đổi trạng thái (seek, play, thêm entity), Rust bắn event qua Tauri IPC (`emit`). Svelte subscribe event đó và cập nhật UI. **One-way Data Flow**.
- **Tại sao?**: Vì MCP Server (AI Agent) cũng là một "frontend" khác, giao tiếp với cùng Rust Core. Nếu Frontend UI chứa logic riêng, MCP Server sẽ không thể đồng bộ. Bằng cách giữ mọi logic ở Core, cả UI lẫn MCP đều nhận được cùng một nguồn sự thật.

## 3. Hiệu năng & Animation
- Không đặt các vòng lặp `requestAnimationFrame` nặng trong Svelte component để tự render Timeline. Mọi animation liên quan đến video playback phải do Rust điều khiển.
- CSS Transform và CSS Variables nên được sử dụng cho micro-animations UI (hover, transitions).
- Tái sử dụng component và áp dụng Svelte `{#key}` hợp lý để tránh re-render toàn bộ component khi chỉ có một phần nhỏ thay đổi.
