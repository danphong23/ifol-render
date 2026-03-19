# FrameData JSON Specification

> The API contract between Frontend and Core.
> Frontend builds FrameData. Core consumes it. Core never modifies it.

## JSON Schema

```json
{
  "passes": [
    {
      "output": "string — texture key for this pass output",
      "pass_type": "Entities | Effect | Output (see below)"
    }
  ],
  "texture_updates": [
    { "LoadImage": { "key": "string", "path": "string" } },
    { "UploadRgba": { "key": "string", "data": [0,0,0,255,...], "width": 100, "height": 100 } },
    { "RasterizeText": { "key": "string", "content": "string", "font_size": 48.0, "color": [1,1,1,1] } },
    { "Evict": { "key": "string" } }
  ]
}
```

---

## Minimal Example — 1 entity, no effects

```json
{
  "passes": [
    {
      "output": "main",
      "pass_type": {
        "Entities": {
          "clear_color": [0, 0, 0, 1],
          "entities": [
            {
              "id": 1,
              "x": 0.0,
              "y": 0.0,
              "width": 1920.0,
              "height": 1080.0,
              "rotation": 0.0,
              "opacity": 1.0,
              "blend_mode": 0,
              "color": [1, 1, 1, 1],
              "shader": "composite",
              "textures": ["bg"],
              "params": [],
              "layer": 0,
              "z_index": 0.0
            }
          ]
        }
      }
    },
    {
      "output": "screen",
      "pass_type": { "Output": { "input": "main" } }
    }
  ],
  "texture_updates": [
    { "LoadImage": { "key": "bg", "path": "assets/background.png" } }
  ]
}
```

---

## Multi-layer Example

```json
{
  "passes": [
    {
      "output": "layer_bg",
      "pass_type": {
        "Entities": {
          "clear_color": [0, 0, 0, 0],
          "entities": [
            {
              "id": 1,
              "x": 0, "y": 0, "width": 1920, "height": 1080,
              "rotation": 0, "opacity": 1, "blend_mode": 0,
              "color": [1, 1, 1, 1],
              "shader": "composite",
              "textures": ["bg_image"],
              "params": [], "layer": 0, "z_index": 0
            }
          ]
        }
      }
    },
    {
      "output": "layer_bg_blur",
      "pass_type": {
        "Effect": {
          "shader": "blur",
          "inputs": ["layer_bg"],
          "params": [1.0, 0.0, 4.0, 0.001]
        }
      }
    },
    {
      "output": "layer_fg",
      "pass_type": {
        "Entities": {
          "clear_color": [0, 0, 0, 0],
          "entities": [
            {
              "id": 10,
              "x": 710, "y": 290, "width": 500, "height": 500,
              "rotation": 0.15, "opacity": 0.95, "blend_mode": 0,
              "color": [1, 1, 1, 1],
              "shader": "composite",
              "textures": ["avatar"],
              "params": [], "layer": 0, "z_index": 0
            },
            {
              "id": 11,
              "x": 760, "y": 820, "width": 400, "height": 60,
              "rotation": 0, "opacity": 1, "blend_mode": 0,
              "color": [1, 1, 1, 1],
              "shader": "composite",
              "textures": ["text_name"],
              "params": [], "layer": 0, "z_index": 1
            }
          ]
        }
      }
    },
    {
      "output": "scene",
      "pass_type": {
        "Effect": {
          "shader": "composite",
          "inputs": ["layer_bg_blur", "layer_fg"],
          "params": []
        }
      }
    },
    {
      "output": "final",
      "pass_type": {
        "Effect": {
          "shader": "color_grade",
          "inputs": ["scene"],
          "params": [0.05, 1.1, 1.0, 0.0]
        }
      }
    },
    {
      "output": "screen",
      "pass_type": { "Output": { "input": "final" } }
    }
  ],
  "texture_updates": [
    { "LoadImage": { "key": "bg_image", "path": "assets/bg.jpg" } },
    { "LoadImage": { "key": "avatar", "path": "assets/avatar.png" } },
    { "RasterizeText": { "key": "text_name", "content": "Alex", "font_size": 48.0, "color": [1,1,1,1] } }
  ]
}
```

---

## FlatEntity Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | u64 | required | Unique ID (for dirty tracking) |
| `x` | f32 | required | Top-left X in pixels |
| `y` | f32 | required | Top-left Y in pixels |
| `width` | f32 | required | Render width in pixels |
| `height` | f32 | required | Render height in pixels |
| `rotation` | f32 | 0.0 | Rotation in radians (around center) |
| `opacity` | f32 | 1.0 | 0 = transparent, 1 = opaque |
| `blend_mode` | u32 | 0 | See Blend Modes table |
| `color` | [f32;4] | [1,1,1,1] | RGBA tint (multiplied with texture) |
| `shader` | string | required | Registered shader name |
| `textures` | [string] | [] | Texture keys from cache |
| `params` | [f32] | [] | Custom shader uniforms |
| `layer` | i32 | 0 | Sorting priority (ascending) |
| `z_index` | f32 | 0.0 | Secondary sort within layer |

## Blend Modes

| Value | Name | Formula |
|-------|------|---------|
| 0 | Normal | standard alpha |
| 1 | Multiply | a × b |
| 2 | Screen | 1-(1-a)(1-b) |
| 3 | Overlay | contrast |
| 4 | SoftLight | subtle |
| 5 | Add | a + b (glow) |
| 6 | Difference | |a - b| |

## Sorting Order

Entities within an `Entities` pass are sorted:

```
1st: layer (i32, ascending)    → layer 0 behind layer 1
2nd: z_index (f32, ascending)  → z 0.0 behind z 1.0
```

Entities with `layer=0, z=0` draw first (bottom/behind).
Entities with `layer=1, z=5` draw last (top/front).
