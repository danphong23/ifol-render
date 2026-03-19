#!/usr/bin/env python3
"""Generate a comprehensive test scene JSON for ifol-render studio.

Features:
- Video playback (38.mp4) with shake effect at start and end
- Image overlay (#cmt_0.png) fading in
- Text with custom font (multi-line, centered)
- Moving shapes (circle, rectangle)
- Gradient background entity
- Vignette effect pass
"""

import json
import math
import os

# ── Config ──
FPS = 30.0
DURATION = 10.0  # seconds
WIDTH = 1920
HEIGHT = 1080
TOTAL_FRAMES = int(FPS * DURATION)

# Asset paths (absolute)
BASE = os.path.dirname(os.path.abspath(__file__))
EXAMPLES = os.path.join(BASE, "..", "examples")
TOOL = os.path.join(BASE, "..", "tool")
VIDEO_PATH = os.path.join(EXAMPLES, "38.mp4").replace("\\", "/")
IMAGE_PATH = os.path.join(EXAMPLES, "#cmt_0.png").replace("\\", "/")
FONT_PATH = os.path.join(EXAMPLES, "NotoSans-Regular.ttf").replace("\\", "/")
FFMPEG_PATH = os.path.join(TOOL, "ffmpeg.exe").replace("\\", "/")

def lerp(a, b, t):
    return a + (b - a) * max(0.0, min(1.0, t))

def ease_in_out(t):
    return t * t * (3 - 2 * t)

def shake(t, intensity=8.0, freq=15.0):
    """Camera shake: returns (dx, dy) offset."""
    dx = intensity * math.sin(t * freq * 2 * math.pi) * math.cos(t * freq * 1.3 * math.pi)
    dy = intensity * math.cos(t * freq * 1.7 * math.pi) * math.sin(t * freq * 0.9 * math.pi)
    return dx, dy

def build_frame(frame_idx):
    t = frame_idx / FPS  # current time in seconds
    progress = frame_idx / TOTAL_FRAMES  # 0..1

    entities = []
    texture_updates = []

    # ── 1. Video background ──
    # Decode video frame at current timestamp
    texture_updates.append({
        "DecodeVideoFrame": {
            "key": "video_bg",
            "path": VIDEO_PATH,
            "timestamp_secs": t,
            "width": WIDTH,
            "height": HEIGHT,
        }
    })

    # Shake at start (0-2s) and end (8-10s)
    shake_x, shake_y = 0.0, 0.0
    if t < 2.0:
        intensity = lerp(12.0, 0.0, t / 2.0)
        shake_x, shake_y = shake(t, intensity, 12.0)
    elif t > 8.0:
        intensity = lerp(0.0, 15.0, (t - 8.0) / 2.0)
        shake_x, shake_y = shake(t, intensity, 10.0)

    entities.append({
        "id": 1,
        "shader": "composite",
        "textures": ["video_bg"],
        "x": shake_x - 10,  # slight overscan to hide edges during shake
        "y": shake_y - 10,
        "width": float(WIDTH + 20),
        "height": float(HEIGHT + 20),
        "opacity": 1.0,
        "color": [1.0, 1.0, 1.0, 1.0],
        "layer": 0,
        "z_index": 0.0,
    })

    # ── 2. Moving circle (shape) ──
    circle_x = lerp(100, WIDTH - 200, ease_in_out((math.sin(t * 0.8) + 1) / 2))
    circle_y = lerp(200, HEIGHT - 300, ease_in_out((math.cos(t * 0.6) + 1) / 2))
    circle_opacity = lerp(0.0, 0.7, min(t / 1.0, 1.0))  # fade in first 1s

    entities.append({
        "id": 10,
        "shader": "shapes",
        "textures": [],
        "x": circle_x,
        "y": circle_y,
        "width": 120.0,
        "height": 120.0,
        "opacity": circle_opacity,
        "color": [0.2, 0.8, 1.0, 0.8],  # cyan
        "params": [
            2.0,   # shape_type: circle
            0.0,   # corner_radius (unused for circle)
            2.0,   # border_width
            0.0, 0.0, 0.0, 0.0,  # border_color (unused)
            0.0,   # fill_mode
        ],
        "layer": 2,
        "z_index": 0.0,
    })

    # ── 3. Moving rectangle with rotation ──
    rect_x = lerp(WIDTH - 400, 200, ease_in_out(progress))
    rect_y = 150.0
    rect_rot = t * 0.5  # slow rotation

    entities.append({
        "id": 11,
        "shader": "shapes",
        "textures": [],
        "x": rect_x,
        "y": rect_y,
        "width": 180.0,
        "height": 100.0,
        "rotation": rect_rot,
        "opacity": 0.6,
        "color": [1.0, 0.4, 0.2, 0.9],  # orange
        "params": [
            0.0,   # shape_type: rect
            15.0,  # corner_radius
            0.0,   # border_width
            0.0, 0.0, 0.0, 0.0,
            0.0,
        ],
        "layer": 2,
        "z_index": 1.0,
    })

    # ── 4. Image overlay (fade in at 1-3s, stay, fade out at 8-10s) ──
    if frame_idx == 0:
        texture_updates.append({
            "LoadImage": {
                "key": "overlay_img",
                "path": IMAGE_PATH,
            }
        })

    img_opacity = 0.0
    if t < 1.0:
        img_opacity = 0.0
    elif t < 3.0:
        img_opacity = lerp(0.0, 0.9, (t - 1.0) / 2.0)
    elif t < 8.0:
        img_opacity = 0.9
    else:
        img_opacity = lerp(0.9, 0.0, (t - 8.0) / 2.0)

    if img_opacity > 0.01:
        # Position: bottom-right corner, with some padding
        img_w = 300.0
        img_h = 300.0
        img_x = WIDTH - img_w - 40
        img_y = HEIGHT - img_h - 80

        entities.append({
            "id": 20,
            "shader": "composite",
            "textures": ["overlay_img"],
            "x": img_x,
            "y": img_y,
            "width": img_w,
            "height": img_h,
            "opacity": img_opacity,
            "color": [1.0, 1.0, 1.0, 1.0],
            "layer": 3,
            "z_index": 0.0,
        })

    # ── 5. Text with custom font ──
    if frame_idx == 0:
        texture_updates.append({
            "LoadFont": {
                "key": "noto",
                "path": FONT_PATH,
            }
        })
        # Title text
        texture_updates.append({
            "RasterizeText": {
                "key": "title_text",
                "content": "iFol Render Engine\nComprehensive Test",
                "font_size": 52.0,
                "color": [1.0, 1.0, 1.0, 1.0],
                "font_key": "noto",
                "max_width": 800.0,
                "alignment": 1,  # center
            }
        })
        # Subtitle
        texture_updates.append({
            "RasterizeText": {
                "key": "sub_text",
                "content": "Video • Image • Text • Shapes • Effects",
                "font_size": 28.0,
                "color": [0.8, 0.9, 1.0, 0.9],
                "font_key": "noto",
                "alignment": 1,
            }
        })
        # Frame counter will be updated every frame
    
    # Update frame counter text every frame
    texture_updates.append({
        "RasterizeText": {
            "key": "frame_counter",
            "content": f"Frame {frame_idx}/{TOTAL_FRAMES}  |  {t:.2f}s / {DURATION:.1f}s",
            "font_size": 20.0,
            "color": [0.6, 0.7, 0.8, 0.8],
            "font_key": "noto",
            "alignment": 2,  # right
        }
    })

    # Title text (fade in 0-2s, stay until 4s, fade out 4-5s)
    title_opacity = 0.0
    if t < 2.0:
        title_opacity = lerp(0.0, 1.0, ease_in_out(t / 2.0))
    elif t < 4.0:
        title_opacity = 1.0
    elif t < 5.0:
        title_opacity = lerp(1.0, 0.0, (t - 4.0) / 1.0)

    if title_opacity > 0.01:
        # Title - centered top area
        title_y = lerp(80, 60, ease_in_out(min(t / 2.0, 1.0)))
        entities.append({
            "id": 30,
            "shader": "composite",
            "textures": ["title_text"],
            "x": (WIDTH - 800) / 2,
            "y": title_y,
            "width": 800.0,
            "height": 140.0,
            "opacity": title_opacity,
            "color": [1.0, 1.0, 1.0, 1.0],
            "layer": 5,
            "z_index": 0.0,
        })

        # Subtitle
        entities.append({
            "id": 31,
            "shader": "composite",
            "textures": ["sub_text"],
            "x": (WIDTH - 800) / 2,
            "y": title_y + 130,
            "width": 800.0,
            "height": 40.0,
            "opacity": title_opacity * 0.85,
            "color": [1.0, 1.0, 1.0, 1.0],
            "layer": 5,
            "z_index": 1.0,
        })

    # Frame counter - always visible, bottom-left
    entities.append({
        "id": 32,
        "shader": "composite",
        "textures": ["frame_counter"],
        "x": WIDTH - 420,
        "y": HEIGHT - 40,
        "width": 400.0,
        "height": 30.0,
        "opacity": 0.7,
        "color": [1.0, 1.0, 1.0, 1.0],
        "layer": 6,
        "z_index": 0.0,
    })

    # ── 6. Gradient bar (bottom) ──
    bar_opacity = lerp(0.0, 0.8, min(t / 0.5, 1.0))
    entities.append({
        "id": 40,
        "shader": "shapes",
        "textures": [],
        "x": 0.0,
        "y": float(HEIGHT - 60),
        "width": float(WIDTH),
        "height": 60.0,
        "opacity": bar_opacity,
        "color": [0.05, 0.05, 0.1, 0.9],
        "params": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "layer": 4,
        "z_index": 0.0,
    })

    # ── 7. Small bouncing circles ──
    for i in range(5):
        phase = i * 1.2
        bx = 200 + i * 350 + 80 * math.sin(t * 2.0 + phase)
        by = HEIGHT - 35 + 8 * math.sin(t * 3.0 + phase)
        entities.append({
            "id": 50 + i,
            "shader": "shapes",
            "textures": [],
            "x": bx,
            "y": by,
            "width": 14.0,
            "height": 14.0,
            "opacity": bar_opacity * 0.9,
            "color": [0.3 + i * 0.15, 0.7, 1.0 - i * 0.1, 1.0],
            "params": [2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "layer": 4,
            "z_index": 1.0,
        })

    # Build frame
    frame = {
        "texture_updates": texture_updates,
        "passes": [
            {
                "output": "main",
                "pass_type": {
                    "Entities": {
                        "entities": entities,
                        "clear_color": [0.0, 0.0, 0.0, 1.0],
                    }
                }
            }
        ]
    }

    return frame

def main():
    frames = []
    for i in range(TOTAL_FRAMES):
        frames.append(build_frame(i))
        if (i + 1) % 30 == 0:
            print(f"  Generated frame {i+1}/{TOTAL_FRAMES}")

    scene = {
        "settings": {
            "width": WIDTH,
            "height": HEIGHT,
            "fps": FPS,
            "background": [0.0, 0.0, 0.0, 1.0],
            "ffmpeg_path": FFMPEG_PATH,
        },
        "frames": frames,
    }

    out_path = os.path.join(EXAMPLES, "comprehensive_test.json")
    with open(out_path, "w") as f:
        json.dump(scene, f)

    size_mb = os.path.getsize(out_path) / 1024 / 1024
    print(f"Generated {out_path}")
    print(f"  {TOTAL_FRAMES} frames, {DURATION}s @ {FPS}fps")
    print(f"  File size: {size_mb:.1f} MB")

if __name__ == "__main__":
    main()
