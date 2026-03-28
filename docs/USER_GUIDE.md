# ifol-render User Guide

> **⚠️ Note:** The current primary development target is the **Web** platform via WASM + WebGPU.
> Use `web/v4-test.html` as the test editor. The Studio GUI app described below is **legacy** and may not reflect the latest ECS architecture.

Welcome to the `ifol-render` engine! This guide covers environment setup, the web test editor, and headless CLI export.

## 1. Prerequisites (CRITICAL)

The engine delegates hardware video decoding/encoding and audio playback to FFmpeg.
**You MUST have FFmpeg installed on your system.**

### Windows
1. Download a pre-compiled Windows build (e.g., from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/)).
2. Extract the ZIP file and copy the contents of the `bin` folder (`ffmpeg.exe`, `ffprobe.exe`) into your system's `PATH`.
3. Verify by opening a command prompt and typing `ffmpeg -version`. If it throws "program not found", the installation has failed.

### MacOS / Linux
- **MacOS**: `brew install ffmpeg`
- **Linux (Ubuntu)**: `sudo apt install ffmpeg`

---

## 2. Using the Studio App (GUI)

The Studio App is a real-time developer previewer built with `egui` and `wgpu`. 
It allows you to visualize JSON Scene files, scrub the timeline synchronously with audio, and execute hardware-accelerated batch exports.

### Running the Studio
```bash
cargo run -p ifol-render-studio
```

### Viewing a Scene
1. Click **File → Open Scene** in the top bar.
2. Select a valid JSON benchmark/scene file (e.g., `examples/full_movie_test.json`).
3. Note: Large scenes are parsed asynchronously to prevent locking the UI. You will see a spinner while the background thread loads the timeline.

### Playback Controls
- **Spacebar**: Toggle play/pause.
- **Timeline Scrubbing**: Drag the slider at the bottom panel to instantly jump to a specific frame. The audio decoder will hot-seek to match the visual frame perfectly using native FFmpeg byte-seeking. Note that frame-seeking heavily depends on codec keyframes.

### Background Video Textures (Missing Video Issue)
If your video background fails to render (appears as a **blank white screen**):
1. Check that FFmpeg is installed and accessible in the system PATH.
2. If FFmpeg is installed in a custom location, click the **Export** button and fill in the absolute path to your `ffmpeg.exe` binary in the configuration window.

### Exporting Video
1. Click the **Export** button.
2. **FFmpeg Path**: If your system PATH is configured, leave as `ffmpeg`. Alternatively, paste the exact absolute path (e.g., `C:\ffmpeg\bin\ffmpeg.exe`).
3. **Preset**: Dictates the balance between Export Time and Output Size.
   - `ultrafast`: Maximum speed, huge file size (Warning: file can be extremely large).
   - `medium` (default): Optimal balance.
   - `slow`: Best compression, lowest speed.
   The engine uses a heavily optimized Multi-Threaded MPSC `sync_channel` that decouples the GPU and CPU FFmpeg encoder. Expect 100%+ speedup compared to synchronous writing.

---

## 3. Using the CLI (Headless)

The `ifol-render-cli` crate operates entirely headless via terminal commands and is ideal for server farm orchestration or CI/CD testing.

### Basic Export Command
```bash
cargo run --release -p ifol-render-cli -- export --scene path/to/scene.json --output exported.mp4
```

### Single Frame Render
```bash
cargo run --release -p ifol-render-cli -- frame-render --frame path/to/frame.json --output exported.png
```

See [CLI_GUIDE.md](CLI_GUIDE.md) for full command reference.

## 4. Troubleshooting
- **wgpu Panic / Validation Error**: Check that your graphics drivers support Vulkan 1.2+ or DirectX 12. If running on ancient hardware, set `WGPU_BACKEND=gl` to fallback to OpenGL, though Zero-Copy pass capabilities may be degraded.
- **Lag On Startup**: The massive `full_movie_test.json` generates ~10,000+ hierarchical blocks of data. This is normal and is handled cleanly via our async parser string loader.
- **Audio Stutter**: `Rodio` dynamically streams `.wav` pipes parsed by the ffmpeg audio engine out to your speakers. Extreme system load might desync streams. Consider pausing and hot-reloading if desync cascades.
