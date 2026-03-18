# ifol-render-cli — Headless CLI Tool

## Role

Headless command-line tool cho rendering và export. Dùng để:
- **Test render** trực tiếp trên terminal
- **Export video** final output
- **Preview frame** xem kết quả bằng file PNG
- **CI/CD** automated rendering

CLI = 1 consumer của core + render, build thành **1 file exe**.

## Usage

```bash
# Xem thông tin scene
ifol-render-cli info --scene scene.json

# Render 1 frame → PNG
ifol-render-cli preview --scene scene.json --time 2.5 --output frame.png

# Export video
ifol-render-cli export --scene scene.json --output video.mp4 --codec h264 --crf 18
```

## Subcommands

### `info`

Hiển thị metadata scene:

```bash
ifol-render-cli info --scene scene.json
```

Output:
```
Resolution: 1920×1080
FPS: 30
Duration: 10.0s
Entities: 12
Total frames: 300
```

### `preview`

Render 1 frame tĩnh tại thời điểm cụ thể:

```bash
ifol-render-cli preview --scene scene.json --time 2.5 --output frame.png
```

| Flag | Default | Description |
|------|---------|-------------|
| `--scene` | required | Path to scene JSON |
| `--time` | 0.0 | Time in seconds |
| `--output` | preview.png | Output file path |

### `export`

Export video qua FFmpeg:

```bash
ifol-render-cli export --scene scene.json --output video.mp4 \
  --codec h264 --crf 18 --ffmpeg /usr/bin/ffmpeg
```

| Flag | Default | Description |
|------|---------|-------------|
| `--scene` | required | Path to scene JSON |
| `--output` | required | Output video path |
| `--codec` | h264 | h264/h265/vp9/prores/png |
| `--crf` | 23 | Quality (0=lossless, 51=worst) |
| `--ffmpeg` | "ffmpeg" | Path to FFmpeg binary |

## Architecture

```
┌──────────────────────────────────────────┐
│  CLI binary                               │
│  ├── Parse args (clap)                    │
│  ├── Load scene JSON                      │
│  ├── Create Renderer (GPU)                │
│  ├── Call core pipeline                   │
│  │   ├── ECS systems                      │
│  │   ├── Build DrawCommand[]              │
│  │   └── renderer.render_frame()          │
│  └── Output (PNG file or FFmpeg pipe)     │
└──────────────────────────────────────────┘
```

CLI chỉ là **glue code** — parse args, gọi core + render, xuất kết quả.

## Testing Workflow

1. Tạo scene JSON tay hoặc từ studio
2. `ifol-render-cli preview --scene scene.json --output test.png`
3. Mở `test.png` xem kết quả
4. Sửa shader/code → build → test lại
5. Khi ổn → `ifol-render-cli export` để xuất video
