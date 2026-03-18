# Getting Started

## Prerequisites

1. **Rust** 1.85+ — install from [rustup.rs](https://rustup.rs/)
2. **GPU** with Vulkan (Linux/Windows), DX12 (Windows), or Metal (macOS) support
3. **FFmpeg** (optional) — required only for video export

### FFmpeg Setup

Download from [ffmpeg.org](https://ffmpeg.org/download.html) and either:
- Add to your system PATH, or
- Place in `tools/ffmpeg` in the project directory and configure the path in the studio

---

## Running the Studio

```bash
git clone https://github.com/nicengi/ifol-render.git
cd ifol-render
cargo run -p ifol-render-studio
```

### Studio Workflow

1. **New Scene** — Studio opens with an empty scene
2. **Add Entities** — Click `+ Add` in the Entity List to add Color Solids or Image Layers
3. **Edit Properties** — Select an entity to view/edit Transform, Color, Timeline in the Properties panel
4. **Timeline** — Click/drag the ruler to scrub the playhead; play with `Space` or `▶ Run`
5. **Save** — `File > Save` or `Ctrl+S` to save as JSON
6. **Export** — `⋮ > Export Video...` to export via FFmpeg

### Viewport Overlays

- Click **▦** to toggle the rule-of-thirds grid
- Click **◻** to toggle broadcast safe zones (action 90%, title 80%)

### FFmpeg Path (Studio)

Configure in `⋮ > FFmpeg Path` — type the path or click Browse.

---

## CLI Usage

```bash
# Build the CLI
cargo build -p ifol-render-cli

# Scene info
cargo run -p ifol-render-cli -- info -s examples/test_render.json

# Preview a single frame
cargo run -p ifol-render-cli -- preview -s examples/test_render.json -t 2.5 -o frame.png

# Export video
cargo run -p ifol-render-cli -- export -s examples/test_render.json -o output.mp4

# Export with options
cargo run -p ifol-render-cli -- export \
  -s examples/test_phase5_9.json \
  -o output.webm \
  -c vp9 \
  --crf 28 \
  --ffmpeg path/to/ffmpeg
```

### CLI Options

| Flag | Description |
|------|-------------|
| `-s, --scene` | Path to scene JSON file |
| `-o, --output` | Output file path |
| `-c, --codec` | Video codec: h264, h265, vp9, prores, png |
| `--crf` | Quality (0=lossless, 51=worst; default 18) |
| `--fps` | Override scene FPS |
| `-w, --width` | Override width |
| `-h, --height` | Override height |
| `--ffmpeg` | Path to FFmpeg binary |

---

## Creating a Scene File

Scene files are JSON documents following the `SceneDescription` format:

```json
{
  "version": "1.0",
  "settings": {
    "width": 1920,
    "height": 1080,
    "fps": 30,
    "duration": 10.0
  },
  "entities": [
    {
      "id": "unique_name",
      "components": {
        "colorSource": { "color": { "r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0 } },
        "timeline": { "startTime": 0.0, "duration": 10.0, "layer": 0 },
        "transform": {
          "position": { "x": 0.0, "y": 0.0 },
          "scale": { "x": 0.5, "y": 0.5 },
          "rotation": 0.0
        },
        "opacity": 0.8,
        "parent": "other_entity_id"
      }
    }
  ]
}
```

### Available Components

| Component | Fields |
|-----------|--------|
| `colorSource` | `color: { r, g, b, a }` |
| `imageSource` | `path: string` |
| `videoSource` | `path, trimStart, trimEnd, playbackRate` |
| `textSource` | `content, font, fontSize, color, bold, italic` |
| `timeline` | `startTime, duration, layer` |
| `transform` | `position, scale, rotation, anchor` |
| `opacity` | `float (0.0–1.0)` |
| `animation` | `keyframes: [{ time, property, value, easing }]` |
| `parent` | `entity_id (string)` |
| `color` | `brightness, contrast, saturation, hue, temperature` |
| `effects` | `[{ type, params: {} }]` |

### Easing Types

- `"linear"` — constant rate
- `"easeIn"` — accelerate (cubic)
- `"easeOut"` — decelerate (cubic)
- `"easeInOut"` — smooth both ends
- `{ "cubicBezier": [x1, y1, x2, y2] }` — custom bezier curve
