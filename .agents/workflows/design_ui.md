---
description: Quy trình thiết kế giao diện UI cho ifol-render. Sử dụng khi cần tạo mới hoặc redesign giao diện.
---

# /design-ui — Quy trình thiết kế giao diện

Quy trình bắt buộc khi thiết kế bất kỳ màn hình hoặc component UI nào.

## Bước 1: Thu thập yêu cầu
- Xác nhận rõ ràng người dùng muốn giao diện gì (màn hình nào, tính năng gì).
- Tham khảo `.agents/design-system/ui_architecture.md` để nắm quy chuẩn thẩm mỹ.

## Bước 2: Thiết kế Mockup bằng Stitch MCP
- Sử dụng Stitch MCP Server để tạo mockup/prototype.
- Tạo project trên Stitch → Generate screens → Áp dụng design system phù hợp.
- Trình bày mockup cho người dùng.

## Bước 3: Chờ người dùng phê duyệt thiết kế
- **DỪNG LẠI** và chờ người dùng review mockup.
- Nếu cần chỉnh sửa: Edit trên Stitch → Trình lại.
- Lặp lại cho đến khi người dùng hài lòng.

## Bước 4: Triển khai bằng Svelte
- Chỉ bắt đầu code khi thiết kế đã được duyệt.
- Sử dụng quy trình `/dev-task` để triển khai.
- Đảm bảo giao diện code ra khớp với mockup đã duyệt.

## Bước 5: Review giao diện thực tế
- Chạy app và so sánh với mockup.
- Trình người dùng xác nhận giao diện cuối cùng.
