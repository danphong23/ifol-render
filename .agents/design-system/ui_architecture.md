---
trigger: always_on
glob:
description: Hệ thống thiết kế (Design System) và thẩm mỹ UI của ifol-render
---

# Design System & Aesthetics (ifol-render)

## 1. Mục tiêu thị giác
- Giao diện phải mang cảm giác **Premium, Hiện đại, Thân thiện** — hướng đến trải nghiệm giống các phần mềm đồ họa chuyên nghiệp (After Effects, DaVinci Resolve, CapCut Pro, Linear app).
- **Màu sắc**: Dark Mode làm mặc định, nhưng hệ thống **phải hỗ trợ chuyển đổi Light/Dark Mode**. Sử dụng CSS Variables (hoặc TailwindCSS dark: variant) để theme có thể toggle.
- **Không dùng các màu generic** (đỏ tươi, xanh lam chói). Sử dụng dải màu HSL có chủ đích. Áp dụng hiệu ứng glassmorphism (kính mờ) ở các panel nổi khi phù hợp.
- **Typography**: Sử dụng font chữ hiện đại (Inter, Roboto, hoặc Outfit). Cấm dùng font mặc định của hệ thống/trình duyệt.
- **Animation**: Các tương tác (hover, click, mở panel, dropdown) phải có micro-animation mượt mà, tạo cảm giác ứng dụng "sống động".

## 2. Quy trình thiết kế giao diện
- Khi thiết kế giao diện mới hoặc redesign, **bắt buộc sử dụng Stitch MCP Server** (đã được kết nối sẵn) để tạo mockup/prototype trước khi code.
- Quy trình: Thiết kế trên Stitch → Trình người dùng duyệt → Khi được chấp thuận → Triển khai bằng Svelte.
- Mọi thay đổi giao diện lớn phải được người dùng phê duyệt trước khi commit.

## 3. Thư viện Component (Component Library)
- **Cho phép** sử dụng thư viện UI headless (shadcn-svelte, Bits UI) hoặc tự build component library riêng, tùy theo quyết định của người dùng.
- Nếu tự build component library: phải đảm bảo đồng bộ về design tokens (color, spacing, border-radius, typography) và hỗ trợ accessibility cơ bản.
- Các component chức năng đặc thù và phức tạp (Timeline Track, Keyframe Editor, Curve Editor) luôn được phép custom code.
