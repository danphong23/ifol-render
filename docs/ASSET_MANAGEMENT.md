# Asset Management Architecture

Tài liệu này định nghĩa rõ ràng **trách nhiệm (responsibilities)** và **ranh giới (boundaries)** của hệ thống quản lý tài nguyên (Assets như Image, Video, Font, Audio) trong hệ sinh thái `ifol-render`. 

Sự phân tách này là yếu tố cốt lõi giúp `ifol-render` có thể chạy trơn tru trên cả môi trường Trình duyệt (WASM/Vite) lẫn môi trường Máy chủ (CLI/Node.js/Go).

---

## 1. Cơ Chế 3 Tầng (The 3-Tier Layering)

Hệ thống xử lý Asset không phải do một cục code duy nhất đảm nhận, mà được chia thành 3 tầng độc lập hoàn toàn:

### Tầng 1: System Host (Vite / Node.js Backend / Server)
Đây là môi trường ứng dụng thực tế nơi bạn tích hợp ifol-render.
* **Quyền hạn & Trách nhiệm:** 
  * Chịu hoàn toàn trách nhiệm trong việc **thu thập (Fetch)** tài nguyên từ Internet, S3, hoặc từ Database.
  * Quyết định vị trí lưu trữ File an toàn (Cache nội bộ của trình duyệt, hoặc thư mục `/assets/` vật lý trên Server).
  * Khởi tạo file `scene.json` chứa các đường link chính xác (URL web hợp lệ với WASM, hoặc đường dẫn ổ cứng cục bộ hợp lệ với CLI).
* **Luật bất thành văn:** Không bao giờ trông chờ `ifol-render-core` tự vào internet tải file giúp bạn. Mọi file phải được chuẩn bị sẵn sàng (Pre-fetched) trước khi gọi quá trình Render.

### Tầng 2: Wrapper Bindings (`crates/wasm` và `crates/cli`)
Đây là hai module "Cầu nối" đại diện cho hai hệ điều hành khác nhau. Chúng đọc JSON và làm nhiệm vụ "Bơm" dữ liệu vào Core Engine.
* **Với Web (WASM / `ifol-wrapper.js`):** 
  Sử dụng Javascript Fetch API hoặc thẻ `<video> / <img>` của HTML5 để load nội dung nhanh nhất có thể. Hình ảnh được giải mã trơn tru qua GPU zero-copy của trình duyệt, sau đó truyền raw bytes (hoặc object handle) vào hàm WASM.
* **Với Native (CLI):** 
  Bản chất chạy trên Backend Server (Node.js spawn process). Nó dùng Rust `std::fs::read` để đọc trực tiếp các Image/Font từ đĩa cứng (Local Disk). (Chỉ tính năng debug nội bộ mới fallback sang lệnh `curl`).

### Tầng 3: Core GPU Engine (`crates/core`)
* **Đặc tính:** Core Engine được thiết kế hoàn toàn "Agnostic" (Mù với thế giới bên ngoài). Nó không biết mạng HTTP là gì, không hiểu khái niệm thư mục (Folder).
* **Nhiệm vụ:** API của nó chỉ nhận duy nhất dữ liệu Pixel thô hoặc byte nhị phân. Khi gọi `engine.load_image(key, path/bytes)`, Core Engine tự động giải mã (decode) đưa nó vào **Texture GPU (VRAM)**.
* Nó sở hữu một thuật toán tối ưu hóa bộ nhớ `Texture Cache LRU`. Nghĩa là nếu bạn đút vào quá nhiều ảnh khiến VRAM vượt mức (ví dụ: > 1GB), Core Engine tự động xóa bộ nhớ của ảnh cũ nhất không dùng tới.

---

## 2. Hướng Dẫn Tích Hợp (Integration Guide)

Tùy vào nền tảng bạn đang lập trình, hãy tuân theo quy tắc sau:

### Giao thức cho Web Developer (React / Vue / Vite)
1. Trong File JSON của bạn, thẻ `url` của Asset có thể là đường dẫn web thoải mái: `https://...` hoặc `blob://...`.
2. Giao phó hoàn toàn cho Javascript tự tải file.
3. Khi file tải xong, JS gọi thẳng `CoreEngine` WASM để cache Asset. WebGPU sẽ lo chuyện còn lại.
4. **Lưu ý hiệu suất:** Video nên dùng Element `<video>` cấp trình duyệt để kích hoạt phần cứng giải mã (Hardware Decoder) có sẵn của Chrome/Safari thay vì ép Core tự tính toán.

### Giao thức cho Backend Engineer (Node.js / Python / Go)
Nếu bạn đang làm Server hệ thống tự động sinh (Render Farm):
1. **(1) Thu thập:** Tiến trình chính (VD: Node.js worker) tải tất cả assets liên quan của video (Font, Nhạc đệm, Hình overlay) về lưu chung vào một thư mục tạm `C:/workers/task_001/assets/`.
2. **(2) Ánh xạ File:** Bạn dùng code Node.js biến đổi toàn bộ đường link từ URL tải trên web trong file JSON trở thành thư mục cục bộ `C:/workers/task_001/assets/file.png`. 
3. **(3) Khởi động render:** Gọi (Spawn) lệnh chạy file nhị phân `ifol-render-cli` và trỏ vào file JSON đã sửa trên máy. 
4. CLI lúc này chỉ việc bốc file trực tiếp từ đĩa với tốc độ I/O nhanh nhất mà không phải lo xử lý bất kì lỗi mất mạng hay Timeout nào!

---

## 3. Tổng Kết Việc Xử Lý Các Trường Hợp Cụ Thể

| Loại Asset | Môi trường Web (WASM / JS) | Môi trường Backend (CLI Native) |
|---|---|---|
| **Font chữ** | JS dùng `fetch()`, nạp dưới dạng bộ đệm (ArrayBuffer ByteArray). | CLI đọc trực tiếp file `.ttf` qua hệ điều hành. |
| **Ảnh (Image)** | Dùng constructor `new Image()` của DOM, giải mã qua Canvas/WGPU. | Thư viện Rust `image` decode định dạng gốc, upload raw byte lên GPU. |
| **Âm thanh** | Dùng API `Audio` hoặc gắn thẳng trên `<video>`. Bị ngắt khi pause trên Timeline. | Trích xuất bằng FFmpeg, sau đó thư viện `ifol-audio` tự động Mux (trộn) vào file MP4 ở bước cuối. |
