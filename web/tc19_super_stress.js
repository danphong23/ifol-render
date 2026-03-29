export function createTC19Json(v_dur) {
    const make_loop_x = (start_val, end_val, dur, num_bounces) => {
        let keys = [];
        let step = dur / num_bounces;
        for(let i=0; i<=num_bounces; i++) {
            keys.push({ time: i * step, value: i % 2 === 0 ? start_val : end_val });
        }
        return { keyframes: keys, easing: "easeInOutSine" };
    };
    
    // Simulate high frequency camera shake over an axis
    const make_shake = (amplitude, dur, freq) => {
        let keys = [];
        let step = dur / freq;
        for(let i=0; i<=freq; i++) {
            let val = (i % 2 === 0 ? amplitude : -amplitude) * (Math.random() * 0.5 + 0.5);
            keys.push({ time: i * step, value: i === 0 || i === freq ? 0 : val });
        }
        return { keyframes: keys, easing: "linear" };
    };

    return {
        settings: { width: 1280, height: 720, fps: 60 },
        assets: {
            "inter": { type: "font", url: "https://fonts.gstatic.com/s/inter/v13/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuGKYMZhrib2Bg-4.ttf" },
            "test_video": { type: "video", url: "http://localhost:5173/examples/38.mp4" },
            "test_img": { type: "image", url: "http://localhost:5173/examples/cmt_0.png" }
        },
        entities: [
            // 1. Root Camera (Exactly matching video length to prevent black tail exports)
            // Enhanced with high-frequency shaking behavior to test constant re-projection.
            {
                id: "main_cam",
                camera: { 
                    postEffects: [
                        {
                            shader_id: "vignette", scope: "padded",
                            float_uniforms: {
                                u0_radius: { keyframes: [{time:0, value:0.8}, {time:v_dur/2, value:0.4}, {time:v_dur, value:0.8}], easing: "easeInOutSine" },
                                u1_softness: { keyframes: [{time:0, value:0.3}] }
                            }
                        },
                        {
                            shader_id: "chromatic_aberration", scope: "padded",
                            float_uniforms: {
                                u0_amount: { keyframes: [{time:0, value:0.005}, {time:v_dur/3, value:0.02}, {time:v_dur, value:0.005}] }
                            }
                        },
                        {
                            shader_id: "color_grade", scope: "padded",
                            vec4_uniforms: {
                                u0_tint: { keyframes: [{time:0, value:[1,1,1,1]}, {time:v_dur/2, value:[1,0.9,0.8,1]}, {time:v_dur, value:[1,1,1,1]}] }
                            },
                            float_uniforms: {
                                u1_contrast: { keyframes: [{time:0, value:1.0}, {time:v_dur/10, value:1.2}, {time:v_dur, value:1.0}] },
                                u2_saturation: { keyframes: [{time:0, value:1.0}, {time:v_dur/2, value:1.3}, {time:v_dur, value:1.0}] },
                                u3_brightness: { keyframes: [{time:0, value:1.0}] }
                            }
                        }
                    ] 
                },
                rect: { width: 1280, height: 720 },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0, anchorY: 0 },
                animation: { floatTracks: [
                    { target: "transformX", track: make_shake(15.0, v_dur, 200) },
                    { target: "transformY", track: make_shake(10.0, v_dur, 150) }
                ]},
                lifespan: { start: 0, end: v_dur }
            },
            
            // LAYER 0: Background Video wrapped in a robust Composition
            {
                id: "comp_bg",
                composition: { duration: {manual: v_dur}, speed: 1.0, loopMode: "once", trimStart: 0.0 },
                transform: { x: 640, y: 360, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                lifespan: { start: 0, end: v_dur },
                layer: 0
            },
            {
                id: "video_track",
                parentId: "comp_bg",
                videoSource: { assetId: "test_video", duration: v_dur },
                rect: { width: 1280, height: 720, fitMode: "contain" },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                lifespan: { start: 0, end: v_dur },
                animation: { floatTracks: [
                    { target: "playbackTime", track: { keyframes: [ { time: 0, value: 0 }, { time: v_dur, value: v_dur } ]}}
                ]},
                layer: 0
            },
            
            // LAYER 1: Deep Nested Components (Test Hierarchy & Matrices)
            {
                id: "comp_nested",
                composition: { duration: {manual: v_dur}, speed: 1.0, loopMode: "once", trimStart: 0.0 },
                transform: { x: 640, y: 360, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                lifespan: { start: 5.0, end: v_dur - 10.0 },
                layer: 1
            },
            
            // Image 1: TEST FIT MODE COVER
            {
                id: "anim_image_cover",
                parentId: "comp_nested",
                imageSource: { assetId: "test_img" },
                rect: { width: 400, height: 400, fitMode: "cover" }, 
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                visual: { opacity: 1.0, blendMode: "normal" },
                animation: { floatTracks: [ 
                    { target: "opacity", track: { keyframes: [{time:0, value:0}, {time:5, value:1}, {time:v_dur-20, value:1}, {time:v_dur-15, value:0}] } },
                    { target: "transformX", track: make_loop_x(-400, 400, v_dur, 20) },
                    { target: "transformY", track: { keyframes: [{time:0, value:0}, {time:v_dur/2, value:300}, {time:v_dur, value:-200}], easing: "easeInOutSine" } },
                    { target: "transformRotation", track: { keyframes: [{time:0, value:0}, {time:v_dur, value:6.28 * 10}], easing: "linear" } }
                ]},
                materials: [
                    {
                        shader_id: "drop_shadow", scope: "padded",
                        float_uniforms: { u0_dx: { keyframes: [{time:0, value:25.0}] }, u1_dy: { keyframes: [{time:0, value:25.0}] }, u2_blur: { keyframes: [{time:0, value:40.0}] }, u3_opacity: { keyframes: [{time:0, value:0.9}] } },
                        vec4_uniforms: { u0_color: { keyframes: [{time:0, value:[0,0,0,1]}] } }
                    }
                ],
                lifespan: { start: 0.0, end: v_dur },
                layer: 0
            },
            
            // Image 2: TEST FIT MODE CONTAIN (Stretched wildly to see boundaries)
            {
                id: "anim_image_contain",
                parentId: "comp_nested",
                imageSource: { assetId: "test_img" },
                rect: { width: 200, height: 800, fitMode: "contain" }, 
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                visual: { opacity: 0.8, blendMode: "normal" },
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:-200}, {time:10, value:-600}, {time:v_dur, value:800}], easing: "easeInOutQuad" } },
                    { target: "transformRotation", track: { keyframes: [{time:0, value:-2.0}, {time:v_dur, value:2.0}], easing: "easeInOutSine" } }
                ]},
                lifespan: { start: 15.0, end: v_dur },
                layer: 1
            },

            // Image 3: TEST FIT MODE STRETCH
            {
                id: "anim_image_stretch",
                parentId: "comp_nested",
                imageSource: { assetId: "test_img" },
                rect: { width: 800, height: 100, fitMode: "stretch" }, 
                transform: { x: 200, y: -250, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                materials: [
                    {
                        shader_id: "glow", scope: "padded",
                        float_uniforms: { u0_radius: { keyframes: [{time:0, value:15.0}] }, u1_intensity: { keyframes: [{time:0, value:2.0}] } },
                        vec4_uniforms: { u0_color: { keyframes: [{time:0, value:[1,0,0,1]}] } }
                    }
                ],
                lifespan: { start: 30.0, end: 150.0 }, // Exists briefly
                layer: 2
            },
            
            // LAYER 2: Text and Complex Masking Array
            {
                id: "comp_masking",
                composition: { duration: {manual: v_dur}, speed: 1.0, loopMode: "once", trimStart: 0.0 },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0, anchorY: 0 },
                lifespan: { start: 10.0, end: v_dur - 20.0 },
                layer: 2
            },
            {
                id: "glow_text_1",
                parentId: "comp_masking",
                textSource: { content: "ULTIMATE STABILITY TEST: " + v_dur.toFixed(1) + "s", fontSize: 60, font: "inter", color: [1,1,1,1] },
                rect: { width: 1200, height: 100, fitMode: "contain" },
                transform: { x: 640, y: 150, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                animation: { floatTracks: [
                    { target: "transformScaleX", track: { keyframes: [{time:0, value:0.1}, {time:2, value:1.0}, {time:10, value:1.0}, {time:12, value:0.1}] } },
                    { target: "transformScaleY", track: { keyframes: [{time:0, value:0.1}, {time:2, value:1.0}, {time:10, value:1.0}, {time:12, value:0.1}] } }
                ]},
                materials: [
                    {
                        shader_id: "glow", scope: "padded",
                        float_uniforms: {
                            u0_radius: { keyframes: [{time:0, value:10.0}, {time:v_dur/10, value:50.0}, {time:v_dur/5, value:10.0}] },
                            u1_intensity: { keyframes: [{time:0, value:2.0}] }
                        },
                        vec4_uniforms: {
                            u0_color: { keyframes: [{time:0, value:[0.1, 0.4, 1.0, 1.0]}, {time:v_dur/10, value:[1.0, 0.2, 0.5, 1.0]}, {time:v_dur, value:[0.1, 0.8, 0.3, 1.0]}] }
                        }
                    }
                ],
                lifespan: { start: 0.0, end: v_dur },
                layer: 0
            },
            {
                // Mask Parent
                id: "mask_comp",
                parentId: "comp_masking",
                composition: { duration: {manual: v_dur}, speed: 1.0, loopMode: "once", trimStart: 0.0 },
                transform: { x: 200, y: 500, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                lifespan: { start: 0, end: v_dur },
                layer: 1
            },
            {
                id: "mask_shape",
                parentId: "mask_comp",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 1.0, 1.0, 1.0] },
                rect: { width: 250, height: 250 },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                lifespan: { start: 0, end: v_dur },
                layer: 0
            },
            {
                id: "masked_gradient",
                parentId: "mask_comp",
                shapeSource: { kind: "rectangle", fillColor: [1.0, 1.0, 1.0, 1.0] }, // Will apply gradient material
                rect: { width: 500, height: 500 },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                animation: { floatTracks: [
                    { target: "transformX", track: make_loop_x(-150, 150, v_dur, 50) }
                ]},
                materials: [
                    {
                        shader_id: "gradient", scope: "clipped",
                        vec4_uniforms: { u0_color1: { keyframes:[{time:0, value:[1,0,0,1]}] }, u1_color2: { keyframes:[{time:0, value:[0,0,1,1]}] } },
                        float_uniforms: { u0_angle: { keyframes: [{time:0, value:0}, {time:v_dur, value:6.28}] } }
                    }
                ],
                visual: { blendMode: "mask_in", opacity: 0.9 }, // Blend over the mask shape
                lifespan: { start: 0, end: v_dur },
                layer: 1
            },
            
            // LAYER 3: A Complicated Looped Composition for Shapes (Duration: 5s, loops continuously)
            {
                id: "comp_looped_shapes",
                composition: { duration: {manual: 5.0}, speed: 1.0, loopMode: "loop", trimStart: 0.0 },
                transform: { x: 640, y: 360, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                lifespan: { start: 0, end: v_dur },
                layer: 3
            },
            
            // Fast animating circle (Linear)
            {
                id: "shape_circle_linear",
                parentId: "comp_looped_shapes",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 0.2, 0.5, 0.8] },
                rect: { width: 100, height: 100 },
                transform: { x: 0, y: -200, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:-400}, {time:5.0, value:400}], easing: "linear" } }
                ]},
                materials: [
                    {
                        shader_id: "glow", scope: "padded",
                        float_uniforms: { u0_radius: { keyframes:[{time:0, value:30.0}] }, u1_intensity: { keyframes:[{time:0, value:1.8}] } },
                        vec4_uniforms: { u0_color: { keyframes:[{time:0, value:[1.0, 0.2, 0.5, 1.0]}] } }
                    }
                ],
                lifespan: { start: 0, end: 5.0 },
                layer: 0
            },
            
            // Slow easing circle (easeInOutQuad)
            {
                id: "shape_circle_quad",
                parentId: "comp_looped_shapes",
                shapeSource: { kind: "ellipse", fillColor: [0.2, 0.8, 1.0, 0.8] },
                rect: { width: 150, height: 150 },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:400}, {time:2.5, value:-400}, {time:5.0, value:400}], easing: "easeInOutQuad" } }
                ]},
                materials: [
                    {
                        shader_id: "drop_shadow", scope: "padded",
                        float_uniforms: { u0_dx: { keyframes:[{time:0, value:10.0}] }, u1_dy: { keyframes:[{time:0, value:10.0}] }, u2_blur: { keyframes:[{time:0, value:15.0}] }, u3_opacity: { keyframes:[{time:0, value:1.0}] } },
                        vec4_uniforms: { u0_color: { keyframes:[{time:0, value:[0.0, 0.0, 0.5, 1.0]}] } }
                    }
                ],
                lifespan: { start: 0, end: 5.0 },
                layer: 1
            },
            
            // Dashed orbiting circle
            {
                id: "shape_dashed_rotate",
                parentId: "comp_looped_shapes",
                shapeSource: { kind: "rectangle", fillColor: [0.0, 0.0, 0.0, 0.0], strokeColor: [1.0, 1.0, 0.2, 1.0], strokeWidth: 10.0 },
                rect: { width: 300, height: 300 },
                transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                animation: { floatTracks: [
                    { target: "transformRotation", track: { keyframes: [{time:0, value:0}, {time:5.0, value:3.14 * 2}], easing: "easeInOutSine" } }
                ]},
                lifespan: { start: 0, end: 5.0 },
                layer: 2
            },
            
            // Floating text inside loop
            {
                id: "loop_text",
                parentId: "comp_looped_shapes",
                textSource: { content: "LOOPED COMP SYNC", fontSize: 40, font: "inter", color: [1,1,1,1] },
                rect: { width: 500, height: 80, fitMode: "contain" },
                transform: { x: 0, y: 250, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
                animation: { floatTracks: [
                    { target: "transformScaleX", track: { keyframes: [{time:0, value:0.5}, {time:2.5, value:1.5}, {time:5.0, value:0.5}], easing: "easeInOutSine" } },
                    { target: "transformScaleY", track: { keyframes: [{time:0, value:0.5}, {time:2.5, value:1.5}, {time:5.0, value:0.5}], easing: "easeInOutSine" } }
                ]},
                materials: [
                    {
                        shader_id: "glow", scope: "padded",
                        float_uniforms: { u0_radius: { keyframes: [{time:0, value:5.0}] }, u1_intensity: { keyframes: [{time:0, value:2.0}] } },
                        vec4_uniforms: { u0_color: { keyframes: [{time:0, value:[0.8, 0.2, 0.8, 1.0]}] } }
                    }
                ],
                lifespan: { start: 0.0, end: 5.0 },
                layer: 3
            }
        ]
    };
}
