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

The scene JSON file contains everything needed for export:

```json
{
  "settings": {
    "width": 1920,
    "height": 1080,
    "fps": 30
  },
  "frames": [
    {
      "clear_color": [0, 0, 0, 1],
      "passes": [
        {
          "type": "entities",
          "entities": [
            {
              "pipeline": "composite",
              "uniforms": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 0],
              "textures": []
            }
          ]
        }
      ]
    }
  ],
  "audio_clips": [
    {
      "path": "music.mp3",
      "start_time": 0.0,
      "volume": 0.8,
      "fade_in": 1.0,
      "fade_out": 2.0,
      "offset": 0.0
    }
  ]
}
```

> **Tip:** Use the SDK's `buildExportPayload()` function to generate this JSON programmatically instead of writing it by hand.

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
