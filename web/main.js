import init, { IfolRenderWeb } from 'ifol-render-wasm';

let renderer = null; 
let sceneJson = null;

let isPlaying = false;
let currentFrame = 0;
let totalFrames = 0;
let playLoopId = null;

const RENDER_W = 1280;
const RENDER_H = 720;

const loadBtn = document.getElementById('load-btn');
const playBtn = document.getElementById('play-btn');
const exportBtn = document.getElementById('export-btn');
const timeline = document.getElementById('timeline');
const frameCounter = document.getElementById('frame-counter');
const statusTxt = document.getElementById('status');

// ── Asset Caches ──
const cachedAssets = new Set();
const cachedVideoFrames = new Set();

// ── HTML5 Hardware Video Decoder ──
const videoPool = {};
const extractCanvas = document.createElement('canvas');
const extractCtx = extractCanvas.getContext('2d', { willReadFrequently: true });

async function ensureVideoLoaded(assetPath) {
    if (videoPool[assetPath]) return videoPool[assetPath];
    
    statusTxt.innerText = `Buffering video: ${assetPath.split('/').pop()}...`;
    const video = document.createElement('video');
    video.crossOrigin = "anonymous";
    video.preload = "auto";
    video.muted = true;
    
    const res = await fetch(`http://localhost:8000/asset?path=${encodeURIComponent(assetPath)}`);
    if (!res.ok) {
        console.warn(`Failed to fetch video: ${assetPath}`);
        return null;
    }
    const blob = await res.blob();
    video.src = URL.createObjectURL(blob);
    
    await new Promise((resolve) => {
        video.onloadeddata = resolve;
        video.onerror = () => { console.warn(`Video load error: ${assetPath}`); resolve(); };
        setTimeout(resolve, 8000);
    });
    
    videoPool[assetPath] = video;
    return video;
}

// Extract a video frame scaled to targetW x targetH — matching FFmpeg native behavior.
async function extractVideoFrame(assetPath, timeSecs, targetW, targetH) {
    const video = await ensureVideoLoaded(assetPath);
    if (!video || video.readyState < 2) return null;
    
    video.currentTime = timeSecs;
    await new Promise((resolve) => {
        video.onseeked = resolve;
        setTimeout(resolve, 200);
    });

    // CRITICAL: Scale to render target resolution (matches native FFmpeg decode)
    const w = targetW || RENDER_W;
    const h = targetH || RENDER_H;
    extractCanvas.width = w;
    extractCanvas.height = h;
    extractCtx.drawImage(video, 0, 0, w, h); // Browser GPU-scaled
    
    const imageData = extractCtx.getImageData(0, 0, w, h);
    return { data: imageData.data, width: w, height: h };
}

// ── Fetch & cache a single asset to WASM memory ──
async function fetchAndCacheAsset(path) {
    if (cachedAssets.has(path)) return;
    try {
        const res = await fetch(`http://localhost:8000/asset?path=${encodeURIComponent(path)}`);
        if (res.ok) {
            const buf = await res.arrayBuffer();
            renderer.cache_image(path, new Uint8Array(buf));
            cachedAssets.add(path);
        } else {
            console.warn(`Asset 404: ${path}`);
        }
    } catch (e) {
        console.warn(`Failed to fetch asset ${path}:`, e);
    }
}

// ── Upfront: scan entire scene JSON → pre-fetch ALL unique assets ──
async function preloadAllSceneAssets(scene) {
    const imagePaths = new Set();
    const fontPaths = new Set();
    const videoPaths = new Set();
    
    for (const frame of scene.frames) {
        for (const update of frame.texture_updates) {
            if (update.LoadImage) imagePaths.add(update.LoadImage.path);
            if (update.LoadFont) fontPaths.add(update.LoadFont.path);
            if (update.DecodeVideoFrame) videoPaths.add(update.DecodeVideoFrame.path);
        }
    }
    
    statusTxt.innerText = `Pre-loading ${imagePaths.size} images, ${fontPaths.size} fonts, ${videoPaths.size} videos...`;
    
    // Fetch all images and fonts in parallel
    const assetPromises = [];
    for (const p of imagePaths) assetPromises.push(fetchAndCacheAsset(p));
    for (const p of fontPaths) assetPromises.push(fetchAndCacheAsset(p));
    await Promise.all(assetPromises);
    
    // Buffer all videos
    for (const vp of videoPaths) {
        await ensureVideoLoaded(vp);
    }
    
    statusTxt.innerText = `Assets loaded. Ready to play.`;
}

// ── Fire-and-forget video frame decode (non-blocking) ──
// Starts decoding in background without blocking the render loop.
// Returns immediately — frame will be available for next render cycle.
function startVideoFrameDecode(frameData) {
    for (const update of frameData.texture_updates) {
        if (update.DecodeVideoFrame) {
            const path = update.DecodeVideoFrame.path;
            const time = update.DecodeVideoFrame.timestamp_secs;
            const w = update.DecodeVideoFrame.width || null;
            const h = update.DecodeVideoFrame.height || null;
            const cacheKey = `${path}@${time}`;
            if (!cachedVideoFrames.has(cacheKey)) {
                // Fire-and-forget — don't await
                extractVideoFrame(path, time, w, h).then(result => {
                    if (result) {
                        renderer.cache_video_frame(path, time, new Uint8Array(result.data.buffer), result.width, result.height);
                        cachedVideoFrames.add(cacheKey);
                    }
                }).catch(e => console.warn(`Video frame extract failed:`, e));
            }
        }
    }
}

// ── Pre-buffer upcoming frames (background decode) ──
const PREBUFFER_COUNT = 3;
function prebufferAhead() {
    for (let i = 1; i <= PREBUFFER_COUNT; i++) {
        const futureIdx = currentFrame + i;
        if (futureIdx < totalFrames) {
            startVideoFrameDecode(sceneJson.frames[futureIdx]);
        }
    }
}

// ── Blocking video frame decode (for scrubbing / single-frame viewing) ──
async function preloadFrameVideoFrames(frameData) {
    const promises = [];
    for (const update of frameData.texture_updates) {
        if (update.DecodeVideoFrame) {
            const path = update.DecodeVideoFrame.path;
            const time = update.DecodeVideoFrame.timestamp_secs;
            const w = update.DecodeVideoFrame.width || null;
            const h = update.DecodeVideoFrame.height || null;
            const cacheKey = `${path}@${time}`;
            if (!cachedVideoFrames.has(cacheKey)) {
                promises.push(
                    extractVideoFrame(path, time, w, h).then(result => {
                        if (result) {
                            renderer.cache_video_frame(path, time, new Uint8Array(result.data.buffer), result.width, result.height);
                            cachedVideoFrames.add(cacheKey);
                        }
                    }).catch(e => console.warn(`Video frame extract failed:`, e))
                );
            }
        }
    }
    await Promise.all(promises);
}

// ── Render the current frame ──
// When blocking=true (scrubbing), waits for video frames.
// When blocking=false (playback), renders immediately with whatever is cached.
async function renderCurrentFrame(blocking = true) {
    if (!renderer || !sceneJson) return;
    
    const frameData = sceneJson.frames[currentFrame];
    if (!frameData) return;

    if (blocking) {
        // Scrubbing / single-frame: wait for video frames
        await preloadFrameVideoFrames(frameData);
    } else {
        // Playback: fire-and-forget decode, render immediately
        startVideoFrameDecode(frameData);
    }
    
    // Scene export resolution (pixel coords are authored at this size)
    const sceneW = sceneJson.settings?.width || 1920;
    const sceneH = sceneJson.settings?.height || 1080;
    
    try {
        renderer.render_frame_scaled(JSON.stringify(frameData), sceneW, sceneH);
        frameCounter.innerText = `${currentFrame} / ${totalFrames - 1}`;
        timeline.value = currentFrame;
    } catch (e) {
        console.error("Render failed:", e);
        stopPlayback();
    }
}

// ── Playback Controls ──
function stopPlayback() {
    isPlaying = false;
    playBtn.innerText = "Play";
    if (playLoopId) {
        cancelAnimationFrame(playLoopId);
        playLoopId = null;
    }
}

// rAF-based playback loop with proper FPS timing
let lastFrameTime = 0;
function playLoop(timestamp) {
    if (!isPlaying) return;
    
    const fps = sceneJson.settings?.fps || 30;
    const frameDuration = 1000.0 / fps;
    
    if (timestamp - lastFrameTime >= frameDuration) {
        if (currentFrame >= totalFrames - 1) {
            stopPlayback();
            return;
        }
        currentFrame++;
        renderCurrentFrame(false); // non-blocking for smooth playback
        prebufferAhead(); // decode next frames in background
        lastFrameTime = timestamp;
    }
    
    playLoopId = requestAnimationFrame(playLoop);
}

// ── Bootstrap ──
async function bootstrap() {
    console.log("Loading WASM module...");
    await init();
    
    const canvas = document.getElementById('canvas');
    if (!navigator.gpu) {
        alert("WebGPU not supported! Please use Chrome/Edge 113+");
        return;
    }

    statusTxt.innerText = "Initializing WASM Engine...";
    try {
        // Handle devicePixelRatio to prevent zoom/crop on HiDPI displays.
        // The WebGPU surface renders at canvas.width x canvas.height pixels.
        // CSS display size must = canvas size / dpr to get 1:1 physical pixel mapping.
        const dpr = window.devicePixelRatio || 1;
        
        // Set the canvas backing store to our render resolution
        canvas.width = RENDER_W;
        canvas.height = RENDER_H;
        
        // Set CSS display size so that CSS_px * dpr = canvas.width
        // This ensures the WebGPU surface maps 1:1 to physical display pixels
        const cssW = Math.round(RENDER_W / dpr);
        const cssH = Math.round(RENDER_H / dpr);
        canvas.style.width = `${cssW}px`;
        canvas.style.height = `${cssH}px`;
        
        console.log(`Canvas init: dpr=${dpr}, backing=${canvas.width}x${canvas.height}, CSS=${cssW}x${cssH}px, physical=${cssW*dpr}x${cssH*dpr}px`);
        
        renderer = await new IfolRenderWeb(canvas, canvas.width, canvas.height, 30.0);
        renderer.setup_builtins();
        statusTxt.innerText = "Engine Ready. Load a scene.";
    } catch (e) {
        console.error("Init failed:", e);
        statusTxt.innerText = "Failed to load WASM";
        return;
    }

    loadBtn.addEventListener('click', async () => {
        statusTxt.innerText = "Downloading JSON Scene...";
        const res = await fetch('/examples/full_movie_test.json');
        sceneJson = await res.json();
        totalFrames = sceneJson.frames.length;
        
        timeline.max = totalFrames - 1;
        timeline.disabled = false;
        playBtn.disabled = false;
        exportBtn.disabled = false;
        
        // Pre-fetch ALL assets upfront
        await preloadAllSceneAssets(sceneJson);
        
        currentFrame = 0;
        await renderCurrentFrame();
    });

    playBtn.addEventListener('click', () => {
        if (isPlaying) {
            stopPlayback();
        } else {
            if (currentFrame >= totalFrames - 1) currentFrame = 0;
            isPlaying = true;
            playBtn.innerText = "Pause";
            lastFrameTime = performance.now();
            playLoopId = requestAnimationFrame(playLoop);
        }
    });

    timeline.addEventListener('input', async (e) => {
        stopPlayback();
        currentFrame = parseInt(e.target.value);
        await renderCurrentFrame();
    });

    exportBtn.addEventListener('click', async () => {
        stopPlayback();
        statusTxt.innerText = "Sending Export to Backend...";
        exportBtn.disabled = true;
        
        // Read export settings from UI
        const exportDir = document.getElementById('export-dir').value.trim();
        const exportFilename = document.getElementById('export-filename').value.trim() || 'output.mp4';
        const exportFfmpeg = document.getElementById('export-ffmpeg').value.trim();
        
        // Build query params for server
        const params = new URLSearchParams();
        if (exportDir) params.set('dir', exportDir);
        params.set('filename', exportFilename);
        if (exportFfmpeg) params.set('ffmpeg', exportFfmpeg);
        
        try {
            const res = await fetch(`http://localhost:8000/export?${params.toString()}`, {
                method: 'POST',
                body: JSON.stringify(sceneJson)
            });
            const text = await res.text();
            alert(text);
            statusTxt.innerText = "Export dispatched. Check server terminal.";
        } catch (e) {
            console.error("Export failed", e);
            statusTxt.innerText = "Export failed. Check console.";
        }
        exportBtn.disabled = false;
    });
}

bootstrap();
