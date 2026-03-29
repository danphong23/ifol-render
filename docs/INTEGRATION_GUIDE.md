# Hướng Dẫn Tích Hợp (Integration Guide)

Tài liệu này hướng dẫn các developer (Frontend & Backend) cách import và sử dụng `ifol-render` vào dự án thực tế của mình (Vite/React/Vue hoặc Node.js Server).

---

## 1. Tích Hợp Frontend (Browser / WebGPU / WASM)
Vào môi trường trình duyệt, lõi C++ / Rust (`ifol-render-core`) được biên dịch thành dạng `.wasm`. Bạn sẽ tương tác với nó thông qua Javascript.

### 1.1 Khởi tạo dự án Vite
Bạn có thể cài đặt package WebAssembly thông qua module npm nội bộ:
```bash
npm install file:../crates/wasm/pkg
```
Hoặc cấu hình `vite.config.js` để cho phép serve file WASM:
```javascript
import { defineConfig } from 'vite';
export default defineConfig({
  server: { fs: { strict: false } }
});
```

### 1.2 Đoạn code khởi tạo chuẩn (Javascript)
Mọi tương tác với Engine đều thông qua class `IfolRenderWeb`.

```javascript
import init, { IfolRenderWeb } from 'ifol-render-wasm';

async function setupEngine() {
    // 1. Tải core WASM nhị phân vào Chrome/Edge
    await init();
    
    // 2. Lấy thẻ canvas ở giao diện
    const canvas = document.getElementById('myCanvas');
    
    // 3. Khởi tạo engine (Canvas, Width, Height, FPS)
    const engine = await new IfolRenderWeb(canvas, 1280, 720, 60);
    
    // 4. Bắt buộc: Nạp các Shader mặc định (Shapes, Blur, Glow)
    engine.setup_builtins();
    
    console.log("WebGPU Engine Ready!");
    return engine;
}
```

### 1.3 Quy trình Load Asset & Render
Tham khảo nguyên tắc hệ thống tại `ASSET_MANAGEMENT.md`. Bạn phải tự fetch file và quẳng vào Engine.

```javascript
// A. Nạp Font chữ
const fontResp = await fetch("https://fonts.gstatic.com/s/inter/...");
const fontBytes = await fontResp.arrayBuffer();
engine.load_font_bytes("MyInterFont", new Uint8Array(fontBytes));

// B. Nạp Hình Ảnh
const img = new Image();
img.src = "./assets/bg.jpg";
await img.decode();
// Tận dụng OffscreenCanvas để biến ảnh thành ArrayBuffer RGBA thô
// ... sau đó gọi:
engine.cache_image("./assets/bg.jpg", rgbaData, img.width, img.height);

// C. Dựng Scene và Vòng lặp Render
const myScene = { entities: [...] };
engine.load_scene_v2(JSON.stringify(myScene));

function loop(timeMs) {
    const timeSec = timeMs / 1000.0;
    // Render khung hình tương ứng với giây `timeSec`
    // Tham số true = Editor Mode (cho phép bạn zoom/pan xung quanh thế giới)
    engine.render_frame_v2(timeSec, "main_cam", true, 0, 0, 1280, 720);
    requestAnimationFrame(loop);
}
requestAnimationFrame(loop);
```

---

## 2. Tích Hợp Backend (Node.js Render Farm / Server Worker)

Đối với Backend, chúng ta không dùng WebGPU hay Canvas. Lõi render sẽ chạy Native dưới dạng tiến trình máy trạm (Command-line Interface `ifol-render-cli.exe` hoặc Linux ELF) và sử dụng tài nguyên đĩa cứng.

### 2.1 Chuẩn bị Môi Trường
Máy chủ (Ubuntu/Windows) **bắt buộc** phải cài đặt hệ thống đồ họa hỗ trợ Vulkan/DX12 (tức là cần GPU rời hoặc CPU có GPU onboard mạnh) và cài `FFmpeg` ở System Path.

### 2.2 Quy trình Code (Node.js Ví dụ)
Là Backend Dev, bạn sẽ xử lý API từ người dùng, download asset về đĩa cứng, và dùng `child_process` để gọi `ifol-render-cli`.

```javascript
const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// 1. Bạn nhận được Scene cấu hình từ Frontend gửi lên API
const scenePayload = request.body.scene;

// 2. TẠO THƯ MỤC TẠM CHO JOB NÀY (Cách ly an toàn)
const jobId = 'render_' + Date.now();
const workDir = path.join(__dirname, 'tmp', jobId);
fs.mkdirSync(workDir, { recursive: true });

// 3. (Giả lập) Bạn tự tải các ảnh, font từ S3/Network về folder `workDir`
// và CHỈNH SỬA các đường link bằng đường dẫn tuyệt đối (C:/.../bg.png)
scenePayload.entities.forEach(ent => {
    if (ent.imageSource) {
        ent.imageSource.assetId = path.join(workDir, 'bg.png'); // Trỏ ổ cứng
    }
});

// 4. Lưu .json ra ổ
const jsonPath = path.join(workDir, 'scene.json');
const outPath = path.join(workDir, 'output.mp4');
fs.writeFileSync(jsonPath, JSON.stringify(scenePayload));

// 5. Spawn tiến trình Native để Render
try {
    console.log("Bắt đầu render Native...");
    execFileSync('ifol-render-cli', [
        'export',
        '--scene', jsonPath,
        '--output', outPath,
        '--codec', 'h264',   // Có thể dùng h265 hoặc prores cho chất lượng cực cao
        '--preset', 'medium',
        '--crf', '23',
        '--fps', '60'
    ], { stdio: 'inherit' });
    
    console.log("Render thành công tại: " + outPath);
} catch (e) {
    console.error("Lỗi Render Engine!", e);
}
```

### 2.3 Tham Số Tối Ưu Backend
Lệnh CLI của ifol-render hỗ trợ nhận diện card đồ họa mặc định. Nếu máy chủ có Nvidia, FFmpeg sẽ tự động được mồi tham số `h264_nvenc` để render Video bay siêu tốc bằng Hardware Acceleration. Tương tự với Intel QSV hay Apple VideoToolbox.
