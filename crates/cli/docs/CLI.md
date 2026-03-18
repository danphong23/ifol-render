# ifol-render-cli — Command-Line Rendering Tool

## Role

Headless CLI tool for rendering and exporting scenes without a GUI. Uses the same `ifol-render-core` pipeline as the studio.

## Usage

```bash
# Show scene info
ifol-render info -s scene.json

# Preview a single frame
ifol-render preview -s scene.json -t 2.5 -o frame.png

# Export video
ifol-render export -s scene.json -o output.mp4 -c h264 --crf 18

# Export with custom FFmpeg path
ifol-render export -s scene.json -o output.mp4 --ffmpeg /path/to/ffmpeg
```

## Commands

### `info`
Displays scene metadata: resolution, FPS, duration, entity count, entity list with types.

### `preview`
Renders a single frame at the specified timestamp and saves as PNG.

| Flag | Description |
|------|-------------|
| `-s, --scene` | Scene JSON path |
| `-t, --time` | Timestamp in seconds |
| `-o, --output` | Output PNG path |
| `-w, --width` | Override width |
| `-h, --height` | Override height |

### `export`
Exports the full scene as video via FFmpeg.

| Flag | Description | Default |
|------|-------------|---------|
| `-s, --scene` | Scene JSON path | required |
| `-o, --output` | Output file path | `output.mp4` |
| `-c, --codec` | Video codec | `h264` |
| `--crf` | Quality (0=best, 51=worst) | `23` |
| `--fps` | Override FPS | scene FPS |
| `--ffmpeg` | Path to FFmpeg binary | system PATH |

Codecs: `h264`, `h265`, `vp9`, `prores`, `png`

## Architecture

```
crates/cli/src/main.rs
  ├── parse CLI args (clap)
  ├── load SceneDescription from JSON
  ├── convert to World + RenderSettings
  ├── create Renderer (headless GPU)
  ├── load image textures
  └── dispatch to info/preview/export handler
```

The CLI creates a `Renderer` directly, builds the ECS `World`, and calls `pipeline::render_frame()` — the exact same pipeline the studio uses. This ensures visual consistency between studio preview and CLI export.
