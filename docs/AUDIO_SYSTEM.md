# ifol-render Audio System Architecture

## Overview
The `ifol-render` audio system is designed to provide sample-accurate, zero-stutter audio mixing and muxing for both Web Preview and Backend Export (CLI/Studio). It achieves this by fully decoupling the **visual frame extraction** pipeline from the **audio playback** pipeline.

## 1. The Core Backend (`ifol-audio` & `ifol-render-core`)
The backend is completely headless and engine-agnostic:
- **`ifol-audio` Workspace**: A standalone Rust module that uses `rodio` and `symphonia` (or similar) to decode any audio file format (mp3, wav, aac) and mix them in-memory.
- **Export Pipeline**: During export, `ifol-render-core` renders visual frames using `wgpu`. Simultaneously, `ifol-audio` mixes the audio tracks based on the flat JSON schema. Finally, `ffmpeg` muxes the raw H.264 video stream with the mixed audio stream into the final `.mp4`.

## 2. The Web SDK (`ifol-render-sdk`)
The Typescript SDK provides an object-oriented API to construct the flat JSON payload expected by the backend.

### `AudioScene`
The SDK now features an `AudioScene` class, acting as the audio equivalent of the visual `Scene` class.
- **`AudioTrack`**: Groups related clips (e.g., `bgm`, `sfx`, `dialogue`, `video_audio`). Each track has its own volume multiplier and mute state.
- **`AudioEntity`**: Represents a single audio event. Contains `startTime`, `duration`, `volume`, `fadeIn`, `fadeOut`, and `offset`.

```typescript
const audioScene = new AudioScene();
audioScene.autoExtractVideoAudio(visualScene); // Automatically grabs audio from video entities
audioScene.addClip({
  source: 'music.mp3',
  startTime: 0,
  volume: 0.8
}, 'bgm'); // Adds to the 'bgm' track
```

## 3. Web Preview Engine (`test-sdk.html` App Layer)
Historically, Web-based video editors suffer from heavy audio stutter when scrubbing or previewing video entities. This is because WebGL frame extractors (like our `AssetManager`) forcefully set `video.currentTime = target` every frame, which destroys the browser's audio buffer if that same `<video>` element is unmuted.

### The `AudioPreview` Solution
To achieve zero-stutter preview:
- The visual `AssetManager` manages hidden `<video>` elements muted (`muted = true`). It scrubs these aggressively for `wgpu` textures.
- The `AudioPreview` system reads the `AudioScene` and dynamically generates **independent `<audio>` tags** (clones) specifically for playback.
- When the user presses "Play" on the timeline, `AudioPreview` seeks these clean `<audio>` tags once, and lets them play naturally via the HTML5 event loop.
- Custom sounds (`bgm`, `sfx`) are decoded via the `AudioContext` Web Audio API and scheduled with sample-accuracy using `source.start()`.

## 4. The Unified Flat JSON Schema
When exporting, the SDK's `AudioScene.flattenForExport()` compiles all tracks into a simple flat struct for the Rust backend:
```json
"audio_clips": [
  {
    "path": "video.mp4",
    "start_time": 0.0,
    "duration": 5.0,
    "volume": 1.0,
    "offset": 0.0,
    "fade_in": 0.0,
    "fade_out": 0.0
  },
  {
    "path": "music.mp3",
    "start_time": 0.0,
    "volume": 0.8,
    "offset": 0.0,
    "fade_in": 1.0,
    "fade_out": 2.0
  }
]
```
This guarantees 1:1 parity between what the user hears in the Web Preview and what the FFmpeg CLI generates in the final output.
