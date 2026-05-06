#!/usr/bin/env python3
"""Generate a full length benchmark test scene JSON for ifol-render studio.

Features:
- Video playback (38.mp4) running its full ~353.6s duration
- Loops image overlays, texts, and moving shapes every 10 seconds to rigorously test GPU load over time.
"""

import json
import math
import os
import struct

# ── Config ──
FPS = 30.0
DURATION = 353.6  # approx duration of 38.mp4
WIDTH = 1920
HEIGHT = 1080
TOTAL_FRAMES = int(FPS * DURATION)

# Asset paths (absolute, forward slashes for JSON)
BASE = os.path.dirname(os.path.abspath(__file__))
EXAMPLES = os.path.join(BASE, "..", "examples")
VIDEO_PATH = os.path.join(EXAMPLES, "38.mp4").replace("\\", "/")
IMAGE_PATH = os.path.join(EXAMPLES, "#cmt_0.png").replace("\\", "/")
FONT_PATH = os.path.join(EXAMPLES, "NotoSans-Regular.ttf").replace("\\", "/")

def get_png_dimensions(path):
    with open(path, "rb") as f:
        f.read(16)
        w = struct.unpack(">I", f.read(4))[0]
        h = struct.unpack(">I", f.read(4))[0]
    return w, h

IMG_W, IMG_H = get_png_dimensions(IMAGE_PATH)

def lerp(a, b, t):
    return a + (b - a) * max(0.0, min(1.0, t))

def ease_in_out(t):
    return t * t * (3 - 2 * t)

def shake(t, intensity=8.0, freq=15.0):
    dx = intensity * math.sin(t * freq * 2 * math.pi) * math.cos(t * freq * 1.3 * math.pi)
    dy = intensity * math.cos(t * freq * 1.7 * math.pi) * math.sin(t * freq * 0.9 * math.pi)
    return dx, dy

def build_frame(frame_idx):
    t_global = frame_idx / FPS
    t = t_global % 10.0 # Loop animations every 10 seconds
    progress = t / 10.0

    entities = []
    texture_updates = []

    # ── 1. Video background (runs linearly, NO looping) ──
    texture_updates.append({
        "DecodeVideoFrame": {
            "key": "video_bg",
            "path": VIDEO_PATH,
            "timestamp_secs": t_global,
        }
    })

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
        "x": shake_x - 10,
        "y": shake_y - 10,
        "width": float(WIDTH + 20),
        "height": float(HEIGHT + 20),
        "opacity": 1.0,
        "color": [1.0, 1.0, 1.0, 1.0],
        "layer": 0,
        "z_index": 0.0,
    })

    # ── 1.5 Dynamic Gradient Background (Time-based shader on entity) ──
    # Shifts colors based on global time
    hue_shift = (math.sin(t_global * 0.5) + 1.0) * 0.5
    entities.insert(1, {
        "id": 2,
        "shader": "gradient",
        "textures": [],
        "x": 0.0, "y": 0.0,
        "width": float(WIDTH), "height": float(HEIGHT),
        "opacity": 0.15, # subtle overlay
        "color": [1.0, 1.0, 1.0, 1.0],
        # gradient params: color1(RGB), pad, color2(RGB), pad
        "params": [
            0.1 * hue_shift, 0.3 * (1.0-hue_shift), 0.8, 0.0,
            0.8, 0.2 * hue_shift, 0.4 * (1.0-hue_shift), 0.0
        ],
        "layer": 1,
        "z_index": 0.0,
    })

    # ── 2. Moving circle ──
    circle_x = lerp(100, WIDTH - 200, ease_in_out((math.sin(t * 0.8) + 1) / 2))
    circle_y = lerp(200, HEIGHT - 300, ease_in_out((math.cos(t * 0.6) + 1) / 2))
    circle_opacity = lerp(0.0, 0.7, min(t / 1.0, 1.0))

    entities.append({
        "id": 10,
        "shader": "shapes",
        "textures": [],
        "x": circle_x,
        "y": circle_y,
        "width": 120.0,
        "height": 120.0,
        "opacity": circle_opacity,
        "color": [0.2, 0.8, 1.0, 0.8],
        "params": [2.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "layer": 2,
        "z_index": 0.0,
    })

    # ── 3. Rotating rectangle ──
    rect_x = lerp(WIDTH - 400, 200, ease_in_out(progress))
    rect_rot = t * 0.5

    entities.append({
        "id": 11,
        "shader": "shapes",
        "textures": [],
        "x": rect_x,
        "y": 150.0,
        "width": 180.0,
        "height": 100.0,
        "rotation": rect_rot,
        "opacity": 0.6,
        "color": [1.0, 0.4, 0.2, 0.9],
        "params": [0.0, 15.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "layer": 2,
        "z_index": 1.0,
    })

    # ── 4. Image overlay ──
    if frame_idx == 0:
        texture_updates.append({
            "LoadImage": {"key": "overlay_img", "path": IMAGE_PATH}
        })

    img_opacity = 0.0
    if t < 1.0: img_opacity = 0.0
    elif t < 3.0: img_opacity = lerp(0.0, 0.9, (t - 1.0) / 2.0)
    elif t < 8.0: img_opacity = 0.9
    else: img_opacity = lerp(0.9, 0.0, (t - 8.0) / 2.0)

    if img_opacity > 0.01:
        display_w = 280.0
        display_h = display_w * (IMG_H / IMG_W)
        entities.append({
            "id": 20,
            "shader": "composite",
            "textures": ["overlay_img"],
            "x": WIDTH - display_w - 40,
            "y": HEIGHT - display_h - 80,
            "width": display_w,
            "height": display_h,
            "opacity": img_opacity,
            "color": [1.0, 1.0, 1.0, 1.0],
            "layer": 3,
            "z_index": 0.0,
        })

    # ── 5. Text ──
    if frame_idx == 0:
        texture_updates.append({"LoadFont": {"key": "noto", "path": FONT_PATH}})
        texture_updates.append({
            "RasterizeText": {
                "key": "title_text",
                "content": "iFol Render Engine\nMemory Benchmark",
                "font_size": 52.0,
                "color": [1.0, 1.0, 1.0, 1.0],
                "font_key": "noto",
                "max_width": 520.0,
                "alignment": 1,
            }
        })

    title_opacity = 0.0
    if t < 2.0: title_opacity = lerp(0.0, 1.0, ease_in_out(t / 2.0))
    elif t < 4.0: title_opacity = 1.0
    elif t < 5.0: title_opacity = lerp(1.0, 0.0, (t - 4.0) / 1.0)

    if title_opacity > 0.01:
        entities.append({
            "id": 30,
            "shader": "composite",
            "textures": ["title_text"],
            "x": (WIDTH - 520.0) / 2,
            "y": lerp(80, 60, ease_in_out(min(t / 2.0, 1.0))),
            "width": 520.0,
            "height": 130.0,
            "opacity": title_opacity,
            "color": [1.0, 1.0, 1.0, 1.0],
            "layer": 5,
            "z_index": 0.0,
        })

    # Global continuous frame counter
    texture_updates.append({
        "RasterizeText": {
            "key": "frame_counter",
            "content": f"Frame {frame_idx}/{TOTAL_FRAMES} | {t_global:.2f}s / {DURATION:.1f}s",
            "font_size": 20.0,
            "color": [0.6, 0.7, 0.8, 0.8],
            "font_key": "noto",
            "alignment": 2,
        }
    })

    entities.append({
        "id": 32,
        "shader": "composite",
        "textures": ["frame_counter"],
        "x": WIDTH - 320.0 - 20,
        "y": HEIGHT - 26.0 - 10,
        "width": 320.0,
        "height": 26.0,
        "opacity": 0.7,
        "color": [1.0, 1.0, 1.0, 1.0],
        "layer": 6,
        "z_index": 0.0,
    })

    # ── 6. Bottom bar + dots ──
    bar_opacity = lerp(0.0, 0.8, min(t / 0.5, 1.0))
    entities.append({
        "id": 40,
        "shader": "shapes",
        "textures": [],
        "x": 0.0,
        "y": float(HEIGHT - 50),
        "width": float(WIDTH),
        "height": 50.0,
        "opacity": bar_opacity,
        "color": [0.05, 0.05, 0.1, 0.9],
        "params": [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        "layer": 4,
        "z_index": 0.0,
    })

    # ── Full frame POST-PROCESSING Shaders ──

    # 1. Color Grade (Pulsing Saturation based on time)
    saturation = 1.0 + math.sin(t_global * 3.0) * 0.5 # Pulses between 0.5 and 1.5

    # 2. Chromatic Aberration (Intense during screen shakes)
    aberration = 0.0
    if t < 2.0: aberration = lerp(0.015, 0.0, t / 2.0)
    elif t > 8.0: aberration = lerp(0.0, 0.02, (t - 8.0) / 2.0)

    return {
        "texture_updates": texture_updates,
        "passes": [
            {
                "output": "scene_raw",
                "pass_type": {
                    "Entities": {
                        "entities": entities,
                        "clear_color": [0.0, 0.0, 0.0, 1.0],
                    }
                }
            },
            {
                "output": "scene_graded",
                "pass_type": {
                    "Effect": {
                        "shader": "color_grade",
                        "inputs": ["scene_raw"],
                        "params": [0.0, 1.0, saturation, 0.0] # brightness, contrast, saturation, pad
                    }
                }
            },
            {
                "output": "main",
                "pass_type": {
                    "Effect": {
                        "shader": "chromatic_aberration",
                        "inputs": ["scene_graded"],
                        "params": [aberration, 0.0, 0.0, 0.0]
                    }
                }
            },
            {
                "output": "final",
                "pass_type": {
                    "Output": {
                        "input": "main"
                    }
                }
            }
        ]
    }


def main():
    frames = []
    for i in range(TOTAL_FRAMES):
        frames.append(build_frame(i))
        if (i + 1) % 1000 == 0:
            print(f"  Generated frame {i+1}/{TOTAL_FRAMES}")

    scene = {
        "settings": {
            "width": WIDTH,
            "height": HEIGHT,
            "fps": FPS,
            "background": [0.0, 0.0, 0.0, 1.0],
        },
        "audio_clips": [{
            "path": VIDEO_PATH, 
            "start_time": 0.0,
            "volume": 0.5,
        }],
        "frames": frames,
    }

    out_path = os.path.join(EXAMPLES, "full_movie_test.json")
    print(f"  Saving {out_path} ...")
    with open(out_path, "w") as f:
        json.dump(scene, f)

    size_mb = os.path.getsize(out_path) / 1024 / 1024
    print(f"Done! Generated {TOTAL_FRAMES} frames.")
    print(f"File size: {size_mb:.1f} MB")

if __name__ == "__main__":
    main()
