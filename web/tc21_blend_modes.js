export function createTC21Json() {
    const modes = [
        "normal", "multiply", "screen", "overlay", "add", "subtract", 
        "darken", "lighten", "soft_light", "hard_light", "difference",
        "mask_in", "mask_out"
    ];

    const entities = [
        // Background Image/Video
        {
            id: "bg",
            imageSource: { assetId: "bg_img" },
            rect: { width: 1280, height: 720, fitMode: "cover" },
            transform: { x: 640, y: 360, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
            lifespan: { start: 0, end: 100 },
            layer: 0
        },
        // Main camera
        {
            id: "main_cam",
            camera: { width: 1280, height: 720, cullingMask: 4294967295 },
            transform: { x: 640, y: 360, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
            lifespan: { start: 0, end: 100 }
        }
    ];

    const cols = 4;
    const rows = Math.ceil(modes.length / cols);
    const cellW = 1280 / cols;
    const cellH = 720 / rows;

    modes.forEach((mode, i) => {
        const col = i % cols;
        const row = Math.floor(i / cols);
        const cx = col * cellW + cellW / 2;
        const cy = row * cellH + cellH / 2;

        // Overlay element testing the blend mode
        entities.push({
            id: `overlay_${mode}`,
            shapeSource: { kind: "rectangle", fillColor: [1.0, 0.5, 0.2, 1.0] },
            rect: { width: cellW * 0.6, height: cellH * 0.6 },
            transform: { x: cx, y: cy, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
            visual: { blendMode: mode, opacity: 0.9 },
            animation: { floatTracks: [
                { target: "transformRotation", track: { keyframes: [{time:0, value:0}, {time:10, value:3.14*2}] } }
            ]},
            lifespan: { start: 0, end: 100 },
            layer: i + 1
        });

        // Label
        entities.push({
            id: `label_${mode}`,
            textSource: { content: mode.toUpperCase(), fontSize: 24, font: "inter", color: [1,1,1,1] },
            rect: { width: cellW, height: 30, fitMode: "contain" },
            transform: { x: cx, y: cy + cellH * 0.35, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
            visual: { blendMode: "normal", opacity: 1.0 },
            lifespan: { start: 0, end: 100 },
            layer: 99
        });
    });

    return {
        settings: { width: 1280, height: 720, fps: 30 },
        assets: {
            "inter": { type: "font", url: "https://fonts.gstatic.com/s/inter/v13/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuGKYMZhrib2Bg-4.ttf" },
            "bg_img": { type: "image", url: "http://localhost:5173/examples/cmt_0.png" }
        },
        entities: entities
    };
}
