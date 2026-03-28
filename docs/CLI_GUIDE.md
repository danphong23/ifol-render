# CLI Guide — ifol-render

The `ifol-render` CLI provides headless GPU rendering and video export from the command line. It is designed for backend servers, CI/CD pipelines, and batch processing.

## Installation

Download `ifol-render.exe` from [GitHub Releases](https://github.com/danphong23/ifol-render/releases).

### Prerequisites
- **FFmpeg** — required for video export. Install via:
  - Windows: `winget install ffmpeg` or download from [ffmpeg.org](https://ffmpeg.org/download.html)
  - Linux: `sudo apt install ffmpeg`
  - macOS: `brew install ffmpeg`

---

## Commands

### `export` — Render scene to video

```bash
ifol-render export --scene scene.json --output video.mp4
```

**Full options:**
```bash
ifol-render export \
  --scene scene.json \
  --output video.mp4 \
  --ffmpeg /path/to/ffmpeg \
  --codec h264 \
  --crf 23 \
  --preset medium \
  --pixel-format yuv420p \
  --width 1920 \
  --height 1080
```

| Flag | Default | Description |
|------|---------|-------------|
| `--scene` | *(required)* | Path to scene JSON file |
| `--output` | `output.mp4` | Output video path |
| `--ffmpeg` | System PATH | Path to FFmpeg binary |
| `--codec` | `h264` | Video codec: `h264`, `h265`, `vp9`, `prores`, `png` |
| `--crf` | `23` | Quality (0–51, lower = better) |
| `--preset` | `medium` | Speed/quality: `ultrafast` to `veryslow` |
| `--pixel-format` | `yuv420p` | Pixel format for FFmpeg |
| `--width` | From scene | Override output width |
| `--height` | From scene | Override output height |

### `frame-render` — Render a single frame to PNG

```bash
ifol-render frame-render --frame frame.json --output preview.png
```

### `render-test` — GPU diagnostic tests

```bash
ifol-render render-test --test basic --output test.png --width 800 --height 600
```

Available tests: `basic`, `blend`, `shapes`, `gradients`, `resize`, `masking`, `text`, `effects`, `perf`

---

## Scene JSON Format

The CLI uses the same **V4 ECS scene JSON** format as the web:

```json
{
  "assets": {
    "bg_img": { "image": { "url": "/data/images/hero.png" } },
    "music":  { "audio": { "url": "/data/audio/bg.mp3" } }
  },
  "entities": [
    {
      "id": "main_cam",
      "camera": { "resolutionWidth": 1920, "resolutionHeight": 1080 },
      "rect": { "width": 1920, "height": 1080 },
      "transform": { "x": 0, "y": 0, "rotation": 0, "scaleX": 1, "scaleY": 1, "anchorX": 0, "anchorY": 0 },
      "lifespan": { "start": 0, "end": 30 }
    },
    {
      "id": "photo",
      "imageSource": { "assetId": "bg_img", "intrinsicWidth": 800, "intrinsicHeight": 600 },
      "rect": { "width": 1920, "height": 1080, "fitMode": "cover" },
      "transform": { "x": 960, "y": 540, "rotation": 0, "scaleX": 1, "scaleY": 1, "anchorX": 0.5, "anchorY": 0.5 },
      "lifespan": { "start": 0, "end": 10 },
      "layer": 1
    }
  ]
}
```

> **Note:** On server/CLI, the asset resolver reads files from the filesystem using the `url` paths directly. The `image` crate handles decoding (PNG, JPEG, etc.).

---

## Audio Processing

When the scene JSON contains `audio_clips`, the CLI automatically:
1. Renders video frames (GPU) → temporary video file
2. Mixes all audio clips (FFmpeg decode + in-memory mix)
3. Exports mixed audio → temporary WAV
4. Muxes video + audio → final output file
5. Cleans up temporary files

---

## Examples

**Export a 30fps video with background music:**
```bash
ifol-render export --scene project.json --output final.mp4 --codec h264 --crf 18 --preset slow
```

**High-quality ProRes export for editing:**
```bash
ifol-render export --scene project.json --output raw.mov --codec prores
```

**Quick preview of a single frame:**
```bash
ifol-render frame-render --frame keyframe.json --output preview.png
```

**GPU performance benchmark:**
```bash
ifol-render render-test --test perf --width 1920 --height 1080
```
