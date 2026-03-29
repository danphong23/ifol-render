const wasm = require('./crates/wasm/pkg/ifol_render_wasm.js');

const jsonPayload = JSON.stringify({
    assets: {
        "blur_shader": {
            type: "shader",
            url: "blur.wgsl"
        }
    },
    entities: []
});

console.log("Exports:", Object.keys(wasm));
if (wasm.IfolRenderWeb.parse_v2_json) {
    console.log("Result:", wasm.IfolRenderWeb.parse_v2_json(jsonPayload));
} else {
    console.log("parse_v2_json not found!");
}
