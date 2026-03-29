import init, { IfolRenderWeb } from './web/node_modules/ifol-render-wasm/ifol_render_wasm.js';

async function test() {
    console.log("Initializing WASM...");
    await init();
    console.log("WASM Initialized.");
    
    // We cannot instantiate IfolRenderWeb easily without canvas/WebGPU,
    // BUT we can test if `AssetDef::Shader` deserializes by directly calling a WASM function if available.
    // Wait, `load_scene_v2` requires an instance of `IfolRenderWeb`.
    // Can we instantiate `IfolRenderWeb` in Node.js? No, it expects `HTMLCanvasElement`.
    
    console.log("WASM successfully loaded. Cannot instantiate WebGPU in Node.");
}

test().catch(console.error);
