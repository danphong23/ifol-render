# Bộ Tài Liệu iFol V4 Test Cases (TC1 - TC19)

Xin chào! Nếu bạn là kỹ sư tích hợp hoặc lập trình viên phát triển thêm tính năng cho `ifol-render`, folder `web/examples` chính là "bãi thử nghiệm" tuyệt vời nhất của dự án. 

Hệ thống cung cấp sẵn một chuỗi Test Case từ **TC1** cho đến **TC19**, được code chìm trong Javascript (`web/main.js`). Mỗi Test Case đại diện cho một mảng tính năng rải rác từ từ cơ bản đến nâng cao để kiểm tra sự ổn định của Core Engine (Rust ECS).

Dưới đây là danh sách và mục đích cụ thể của từng bài kiểm tra. Khi bạn thay đổi Core Engine, bạn *bắt buộc* phải chạy lần lượt các TC này và đối chiếu để đảm bảo không có tính năng nào bị hỏng (Regression).

---

## 🟢 Nhóm 1: Cơ Bản & Nòng Cốt (TC1 - TC8)
Kiểm tra khả năng tính toán Tọa độ (Transform), ECS Tree, và Animation Tracks.

* **TC1: Shapes (Vẽ Hình Cơ Bản)**
  * Hiển thị các block hình học thuần Vector: `Rectangle` (Vectơ Vuông), `Ellipse` (Vectơ Tròn).
  * Mục đích: Đảm bảo shader mặc định `shapes.wgsl` vẽ ra được pixel solid.
* **TC2: Hierarchy (Cây Gia Phả Parent-Child)**
  * Kiểm tra Parent kéo theo Child. Khi cha xoay (Rotation) hoặc thu phóng (Scale), con phải đi theo bằng thuật toán ma trận `Affine Transform 3x3`.
* **TC3: Keyframe (Nội Suy Khung Hình)**
  * Kiểm tra Linear Interpolation. Gắn `keyframes` vào property (VD: `transformX` từ 0 -> 400). Đảm bảo vật thể trượt mượt mà theo TimeState.
* **TC4: Loop (Vòng Lặp)**
  * Kiểm tra vòng lặp biến thiên (LoopMode) của Animation Track. Ví dụ: Chạy từ trái sang phải, xong tự động giật lùi hoặc lặp lại.
* **TC5: Opacity (Minh Mạch / Trong Suốt)**
  * Trộn màu Alpha Blend. Khẳng định thuật toán nhân `opacity` từ cha lan truyền xuống con hoạt động đúng.
* **TC6: Camera (Hệ Trục Trực Giao)**
  * Khởi tạo `main_cam`. Kiểm tra việc tịnh tiến Camera nghịch đảo với toàn thế giới (VD: Camera dịch qua phải = Đẩy toàn bộ thế giới sang trái). Đồng thời kiểm tra tỉ lệ Pixel/World.
* **TC7: Morph (Biến Thể Hình Dáng)**
  * (Tính năng đặc trưng) Mô phỏng keyframe đổi từ hình Vuông thành hình Tròn mượt mà bằng bán kính cắt (Corner Radius).
* **TC8: Lifespan (Khoảng Thời Gian Tồn Tại)**
  * Kiểm tra culling: Vật thể không được render nếu Thời Gian Hành Trình (Playback Time) nằm ngoài khung `Lifespan [start..end]`. Hệ thống sẽ tự tắt DrawCall để tiết kiệm GPU.

---

## 🟡 Nhóm 2: Composition & Thời Gian (TC9 - TC11)
Kiểm tra hệ thống `Composition` - trái tim của After Effect / Video Editor. Cây timeline phức tạp với time-stretching và trim.

* **TC9: Comp Timeline (Trục Thời Gian Local)**
  * Đảm bảo một `Composition` độc lập có trục thời gian chạy chậm hơn / nhanh hơn (Track Speed) so với `Root`. 
* **TC10: Nested Comp (Comp lồng Comp)**
  * Scale thời gian lồng chéo. Ví dụ: Root (đang chạy giây số 10) -> Cha (speed 0.5, chạy giây 5) -> Con (speed 2.0, chạy giây 10). Mọi thứ vẫn phải di chuyển trơn tru trên mọi hệ quy chiếu.
* **TC11: Easings (Gia Tốc phi tuyến tính)**
  * Áp dụng các thuật toán Bezier Curve / Sine / Quad (`easeInOutSine`, `easeOutQuad`). Đảm bảo trượt animation mượt và không bị khựng giống y hệt Web CSS.

---

## 🟠 Nhóm 3: Tài Nguyên & Layout (TC12 - TC13b)
Kiểm tra sức mạnh của Hệ Thống Nạp Tài Nguyên (Load Asset) và Xử Lý Font chữ.

* **TC12: Image (Tải Ảnh, Cắt Ảnh, Render Texture)**
  * Nạp file PNG. Quan trọng: Kiểm tra tính năng `FitMode` (`Stretch`, `Cover`, `Contain`). Đặc biệt test Cover và Contain luôn ép bức ảnh vào khung vuông tỷ lệ được vẽ trên Editor thay vì méo xệch. (Đã fix lỗi tràn đường bờ `ClampToEdge`).
* **TC13: Alignment (Ngân Hàng Neo - Anchor)**
  * Điểm neo `Anchor X/Y`. Transform 0.5 (Tâm), 0.0 (Góc trái), 1.0 (Góc phải). Xác nhận phép xoay sẽ lấy Anchor làm rốn xoay.
* **TC13b: Text (Hiển Thị Văn Bản & Tải Font)**
  * Tải font `.ttf` trên mạng thông qua Font Cache của SDK. Render thành Texture siêu mượt (Text Rasterization). Tự động dãn khoảng cách Width/Height của `Rect` dựa trên giới hạn text box.

---

## 🔴 Nhóm 4: Pipeline Đồ Hoạ Nâng Cao (V2) (TC14 - TC18)
Đánh giá độ sâu của công nghệ Post processing (Hiệu ứng hậu kỳ) thông qua Off-screen Framebuffer.

* **TC14: Blur / Glow (Chói lóa và Mờ ảnh)**
  * Gọi Shader Material. Kiểm tra Vùng Đệm (Padded Scope) — đảm bảo `Glow` phát sáng ra ngoài ranh giới hình Vuông/Chữ ban đầu bằng cách nới viền Drawcall rộng thêm `effect_padding` pixel trước khi render lên.
* **TC15: Drop Shadow (Đổ Bóng Padded)**
  * Test tính gắn kết. Một Comp con vừa có ảnh, chữ; khi gắn Drop Shadow vào Comp cha thì bóng của *toàn bộ hệ thống lồng nhau (flatten)* phải in xuống một lớp duy nhất bên dưới đáy thay vì in thành nhiều bóng đè lên nhau.
* **TC16: Multi Effect (Cộng Dồn Hiệu Ứng)**
  * Pipeline Ping-Pong Buffer. Thực hiện chuỗi nối tiếp: `Render Thô -> làm mờ Blur (pass 1) -> Tăng Sáng Color Grade (pass 2) -> Blend lên màn hình`.
* **TC17: AV Sync (Đồng Bộ Video & Audio Track)**
  * Bật một Video MP4 (`38.mp4`). Tách lớp Video lên Texture GPU, đồng thời gọi thư viện `ifol-audio` mix âm thanh (Web Audio API/FFMPEG). Đảm bảo Video mượt FPS, âm khớp miệng nhân vật kể cả khi tua (Seek).
* **TC18: Blur Cases (Masking & Shader Tranh Chấp)**
  * Kiểm tra cực độ: Vừa áp dụng `Mask` (Cắt Lớp/Che hình), vừa áp Blur đổ sáng. Xem Mask có gọt mất tia sáng không? Đây là TC quan trọng để hiểu ý đồ cắt ghép layer theo chuẩn Alpha Matte.

---

## 🟣 Nhóm 5: Tối Hậu Tuyệt Kỹ (TC19)
Bai test tổng hợp lớn nhất (Super Stress Test).

* **TC19: Backend TC (Export Tổng Lực V2)**
  * **Chức năng:** Kết hợp 100% tính năng kể trên. Chạy trên màn hình 1280x720. Bao gồm ảnh quay ngoắt, bóng mờ, camera tịnh tiến giật lắc, Mask gradient quét qua màn hình liên tục...
  * **Ứng Dụng:** Được dùng làm chuẩn mực để gỡ rối `ifol-render-cli` trên tiến trình Server. Nếu TC19 xuất Video Native (`export.mp4`) thành công, với số FPS 120 không tụt, và hiển thị TRÙNG KHỚP từng pixel 100% so với Render WASM thực tế trên Web thì **phiên bản ifol-render đã sẵn sàng để Ship (Release)**.
