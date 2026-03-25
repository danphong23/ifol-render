# Unit Coordinate System

Tài liệu kỹ thuật mô tả hệ tọa độ **World Unit** của ifol-render SDK.

> **Nguyên tắc cốt lõi**: Toàn bộ scene tồn tại trong không gian **unit**. Pixel chỉ xuất hiện
> tại thời điểm render — khi SDK flatten dữ liệu cho 1 viewport hoặc camera cụ thể.

---

## 1. World Space

### 1.1 Đơn vị Unit

Mọi entity trong scene sử dụng đơn vị **unit** cho position và size:

```
Entity {
  x: 100,          // 100 units từ gốc tọa độ
  y: 50,           // 50 units từ gốc tọa độ
  width: 200,      // chiều rộng 200 units
  height: 150,     // chiều cao 150 units
}
```

Unit **không có kích thước vật lý** — nó là đơn vị trừu tượng. 1 unit không bằng 1 pixel,
không bằng 1 cm, không bằng bất kỳ đơn vị đo lường nào. Ý nghĩa của unit phụ thuộc vào
PPU và viewport/camera đang hiển thị.

**Gốc tọa độ** `(0, 0)` nằm ở góc trên bên trái. Trục X hướng sang phải, trục Y hướng xuống.

```
(0,0)────────────► X+
  │
  │   World Space
  │   (vô hạn)
  │
  ▼ Y+
```

World space **không có biên giới** — entities có thể nằm ở bất kỳ tọa độ nào, kể cả âm.

### 1.2 PPU (Pixels Per Unit)

PPU là hệ số quy đổi **pixel của media gốc → unit trong world**, áp dụng **1 lần duy nhất**
khi import media vào scene.

```
PPU = 1 (mặc định)
  Import ảnh 1920×1080 px → entity 1920×1080 units

PPU = 2
  Import ảnh 1920×1080 px → entity 960×540 units

PPU = 0.5
  Import ảnh 1920×1080 px → entity 3840×2160 units
```

**Công thức**:
```
entityWidth  = imagePixelWidth  / PPU
entityHeight = imagePixelHeight / PPU
```

PPU là setting **cấp scene** — tất cả media dùng chung 1 PPU. Thay đổi PPU ảnh hưởng
kích thước unit của tất cả media khi import (không ảnh hưởng media đã import).

**Khi nào dùng PPU ≠ 1?**
- Game: sprite 32×32px, PPU=32 → mỗi sprite = 1×1 unit → dễ snap vào grid
- Hi-DPI: PPU=2 → media Retina hiển thị đúng kích thước

**Trong video editor**: PPU=1 là chuẩn. 1 pixel gốc = 1 unit.

---

## 2. Viewport

Viewport là **cửa sổ nhìn vào world** — nó quyết định phần nào của world hiển thị
trên màn hình.

### 2.1 Thuộc tính

```
Viewport {
  // Kích thước màn hình thực (DOM pixels)
  screenWidth: 1000,      // px
  screenHeight: 800,      // px

  // Vùng world đang nhìn
  centerX: 960,           // units — tâm viewport
  centerY: 540,           // units
  zoom: 1.0,              // 1.0 = mặc định, 2.0 = zoom in 2×

  // Hiệu suất
  renderScale: 1.0,       // 0.0–1.0, phần trăm phân giải render
}
```

### 2.2 Vùng World hiển thị

Từ viewport, SDK tính được vùng world đang nhìn thấy:

```
visibleWidth  = screenWidth  / (PPU × zoom)
visibleHeight = screenHeight / (PPU × zoom)

left = centerX - visibleWidth  / 2
top  = centerY - visibleHeight / 2
```

**Ví dụ**: viewport 1000×800px, PPU=1, zoom=1:
```
visibleWidth  = 1000 / (1 × 1) = 1000 units
visibleHeight = 800  / (1 × 1) = 800 units
```

Zoom in 2×:
```
visibleWidth  = 1000 / (1 × 2) = 500 units   ← nhìn thấy ít world hơn
visibleHeight = 800  / (1 × 2) = 400 units
```

### 2.3 Pan (dịch chuyển)

Pan thay đổi `centerX`, `centerY`:
- Middle-click drag → thay đổi center theo delta mouse / scale
- Scroll horizontal → thay đổi centerX

```
// Khi user drag ΔmouseX pixels sang phải:
centerX -= ΔmouseX / scale
// (dịch viewport sang phải = world dịch sang trái tương đối)
```

### 2.4 Zoom (thu phóng)

Zoom thay đổi `zoom` value. Zoom **hướng về con trỏ chuột** (zoom-to-cursor):

```
// Mouse ở vị trí (mouseX, mouseY) trên canvas
// Tính world point dưới con trỏ TRƯỚC zoom:
worldX_before = left + mouseX / scale

// Cập nhật zoom:
zoom *= (scrollUp ? 1.1 : 1/1.1)

// Tính lại left/top sao cho worldX_before vẫn ở mouseX:
centerX = worldX_before + (visibleWidth/2 - mouseX/scale)
```

---

## 3. Flatten Pipeline

**Flatten** = chuyển đổi danh sách entity (unit coords) → Frame data (pixel coords)
cho 1 render target cụ thể.

### 3.1 Input

```
entities:    danh sách Entity (unit coords)
region:      vùng world cần render (left, top, width, height — units)
renderW:     pixel width của render target
renderH:     pixel height của render target
```

### 3.2 Tính toán

```
// Scale: 1 unit = bao nhiêu pixels trong render target
scale = renderW / region.width
// (Chỉ dùng 1 scale cho cả X và Y → uniform, không méo)

// Kiểm tra aspect ratio
// Nếu region aspect ≠ render target aspect → letterbox
scaleX = renderW / region.width
scaleY = renderH / region.height
scale  = min(scaleX, scaleY)

// Offset để center nội dung (letterbox)
offsetX = (renderW - region.width  * scale) / 2
offsetY = (renderH - region.height * scale) / 2
```

### 3.3 Quy đổi mỗi entity

```
pixelX = (entity.x - region.left) * scale + offsetX
pixelY = (entity.y - region.top)  * scale + offsetY
pixelW = entity.width  * scale
pixelH = entity.height * scale
```

### 3.4 Ví dụ minh họa

```
Scene:
  Entity A: x=100, y=50, w=200, h=150 (units)
  Entity B: x=500, y=300, w=100, h=100 (units)

Viewport: screen=1000×800px, center=(500,400), zoom=1, PPU=1
  → visible region: left=0, top=0, w=1000, h=800
  → renderW=1000, renderH=800
  → scale = 1000/1000 = 1.0
  → offset = (0, 0)

  Entity A pixel: x=100, y=50, w=200, h=150   (1:1 mapping)
  Entity B pixel: x=500, y=300, w=100, h=100

Zoom 2×:
  → visible region: left=250, top=200, w=500, h=400
  → scale = 1000/500 = 2.0

  Entity A pixel: x=(100-250)×2 = -300 (off-screen left)
  Entity B pixel: x=(500-250)×2 = 500, y=(300-200)×2 = 200
```

---

## 4. Camera

### 4.1 Camera là Entity

Camera **không phải concept đặc biệt** — nó chỉ là 1 entity trong world
với position + size tính bằng units:

```
Camera Entity {
  id: "camera",
  x: 0,            // units
  y: 0,            // units
  width: 1920,     // units — vùng world camera nhìn thấy
  height: 1080,    // units
}
```

Trong edit viewport, camera hiển thị như 1 hình chữ nhật mà user có thể drag, resize.

### 4.2 Camera View (Preview)

Camera view là 1 viewport riêng, hiển thị chính xác những gì camera nhìn thấy:

```
Flatten:
  region = { left: camera.x, top: camera.y, w: camera.width, h: camera.height }
  renderW = cameraPreviewPixelW
  renderH = cameraPreviewPixelH
  → scale = min(renderW/camera.width, renderH/camera.height)
```

Camera view **không hiển thị camera entity** (vì camera nhìn chính nó vô nghĩa).

### 4.3 Export

Khi export, user chọn **output resolution** (pixels). Camera's unit region → pixel coords:

```
Export 1920×1080 pixels:
  region = camera entity (x, y, width, height — units)
  renderW = 1920, renderH = 1080
  → Flatten pipeline giống hệt camera view
  → Mỗi frame flat → Core render → pixel output → encode MP4
```

Export resolution **độc lập** với camera size units:
- Camera 1920×1080 units, export 1920×1080px → scale=1
- Camera 1920×1080 units, export 3840×2160px → scale=2 (4K upscale)
- Camera 1920×1080 units, export 960×540px   → scale=0.5 (preview quality)

---

## 5. Resolution Scaling

### 5.1 Mục đích

Cho phép render ở phân giải thấp hơn màn hình để **tiết kiệm GPU**,
đặc biệt trên máy yếu hoặc khi scene phức tạp.

### 5.2 Cách hoạt động

```
Viewport: screenWidth=1000, screenHeight=1000, renderScale=0.5

Canvas backing size:    500×500 px   (GPU render target)
Canvas CSS size:        1000×1000 px (hiển thị trên màn hình)
→ Browser tự stretch 500→1000, hơi mờ nhưng GPU làm ít 75%
```

```ts
// Implementation
canvas.width  = screenWidth  * renderScale;  // backing (GPU)
canvas.height = screenHeight * renderScale;
canvas.style.width  = screenWidth  + 'px';   // display (CSS)
canvas.style.height = screenHeight + 'px';
```

### 5.3 Flatten với renderScale

```
renderW = screenWidth  * renderScale
renderH = screenHeight * renderScale
→ Flatten giữ nguyên logic, chỉ thay renderW/renderH
→ Entity pixel coords tự scale theo
```

### 5.4 Hiệu quả

| renderScale | Render pixels | GPU work | Chất lượng |
|-------------|---------------|----------|------------|
| 1.0         | 1000×1000     | 100%     | Full       |
| 0.75        | 750×750       | 56%      | Tốt        |
| 0.5         | 500×500       | 25%      | Chấp nhận  |
| 0.25        | 250×250       | 6%       | Thấp       |

---

## 6. Media Pipeline

### 6.1 Image

```
Developer: renderer.addImage('bg', '/photos/bg.jpg')

SDK pipeline:
  1. fetch('/photos/bg.jpg') → Response
  2. createImageBitmap(blob) → ImageBitmap (browser async decode)
  3. Draw ImageBitmap to OffscreenCanvas
  4. getImageData() → Uint8Array RGBA (w × h × 4 bytes)
  5. cache_image('bg', rgba_bytes) → WASM WebMediaBackend
  6. Entity size = (imageW / PPU) × (imageH / PPU) units

Core render:
  Frame { texture_updates: [LoadImage { key: 'bg' }], ... }
  → Core gọi read_file_bytes('bg') → tìm trong WebMediaBackend.images
  → Decode → GPU texture

Lưu ý: bước 2-4 xảy ra 1 LẦN. Sau đó Core cache texture trên GPU vĩnh viễn
cho đến khi SDK gửi Evict.
```

### 6.2 Video

```
Developer: renderer.addVideo('clip', '/videos/intro.mp4')

SDK pipeline:
  1. fetch('/videos/intro.mp4') → blob URL
  2. Tạo <video src=blobURL> (ẩn, không play)
  3. Lấy video dimensions → entity size = (videoW / PPU) × (videoH / PPU)

Mỗi frame tick (trong render loop):
  4. video.currentTime = targetTimestamp
  5. Chờ 'seeked' event
  6. Draw video frame to OffscreenCanvas (video dimensions)
  7. getImageData() → RGBA bytes
  8. cache_video_frame('clip', timestamp, rgba, w, h)

Core render:
  Frame { texture_updates: [UploadRgba { key: 'clip_t2.5', data, w, h }] }
  → Core nhận RGBA đã decode → upload thẳng lên GPU

Tối ưu:
  - Pre-seek: trong lúc render frame N, seek video cho frame N+1
  - Double canvas: 2 OffscreenCanvas luân phiên
  - Resolution match: extract ở resolution = entity pixel size trên viewport
    (không cần decode 4K nếu entity chỉ hiển thị 200×112px trên viewport)
```

### 6.3 Cache Lifecycle

```
addEntity('bg', {source: 'bg_img'})
  → SDK kiểm tra 'bg_img' đã decode chưa
  → Nếu chưa: fetch + decode + cache
  → Nếu rồi: skip

removeEntity('bg')
  → SDK kiểm tra: còn entity nào dùng 'bg_img' không?
  → Nếu không: gửi Evict { key: 'bg_img' } → Core xóa GPU texture

Video frames:
  → Khi seek xa (> 2s): clear_video_frames() → xóa tất cả cached frames
  → Khi play: chỉ giữ frames trong window ±1s quanh playhead
```

---

## 7. Sơ đồ tổng quan

```
┌─── Developer API ─────────────────────────────────────────────┐
│  addEntity()  addImage()  addVideo()  play()  export()        │
└───────────────────────┬───────────────────────────────────────┘
                        │
┌─── SDK ───────────────▼───────────────────────────────────────┐
│                                                               │
│  Scene Model          Media Pipeline        Flatten Engine    │
│  ┌──────────┐        ┌──────────────┐      ┌─────────────┐   │
│  │ entities │        │ fetch+decode │      │ unit→pixel  │   │
│  │ (units)  │        │ image: RGBA  │      │ per viewport│   │
│  │ camera   │        │ video: frame │      │ per camera  │   │
│  │ timeline │        │ cache track  │      │ renderScale │   │
│  └──────────┘        └──────┬───────┘      └──────┬──────┘   │
│                             │                     │           │
│                    ┌────────▼─────────────────────▼────────┐  │
│                    │        Frame (pixel coords)           │  │
│                    │  passes, texture_updates, entities     │  │
│                    └────────────────┬──────────────────────┘  │
└─────────────────────────────────────┼─────────────────────────┘
                                      │ JSON
┌─── Core WASM ───────────────────────▼─────────────────────────┐
│                                                               │
│  ┌─────────────┐    ┌──────────┐    ┌───────────────────┐     │
│  │ Texture Mgr │    │ Shader   │    │ GPU Render Target │     │
│  │ RGBA→GPU    │    │ Pipeline │    │ WebGPU Surface    │     │
│  │ Cache+Evict │    │ composite│    │ → Canvas pixels   │     │
│  └─────────────┘    │ shapes   │    └───────────────────┘     │
│                     │ mask,etc │                               │
│                     └──────────┘                               │
└───────────────────────────────────────────────────────────────┘
```

---

## 8. Ràng buộc và quy tắc

1. **Core KHÔNG biết unit** — Core chỉ nhận pixel coords. Toàn bộ quy đổi unit→pixel nằm trong SDK.

2. **SDK KHÔNG biết GPU** — SDK không truy cập WebGPU. Nó chỉ tạo Frame JSON và gọi Core methods.

3. **1 Core instance = 1 Canvas = 1 GPU device**. Nhiều viewport = nhiều Core instance.

4. **Flatten là pure function**: `flatten(entities, region, renderW, renderH) → Frame`. Không side effect. Có thể chạy trong Web Worker.

5. **PPU immutable sau khi scene tạo**. Thay đổi PPU giữa chừng sẽ khiến tất cả entity size sai.

6. **Resolution scaling chỉ ảnh hưởng canvas backing size**. Logic flatten, hit test, pan/zoom đều dùng screenWidth/screenHeight (CSS size), không dùng renderW/renderH.
