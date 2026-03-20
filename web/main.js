import init, { IfolRenderWeb } from 'ifol-render-wasm';

let renderer = null; 
let sceneJson = null;

let isPlaying = false;
let currentFrame = 0;
let totalFrames = 0;

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

// ── Continuous Video Playback State ──
// Instead of seeking every frame, we let the video play() continuously
// and capture frames as they arrive via requestVideoFrameCallback or timeupdate.
const videoPlaybackState = {};

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

// Capture the current video frame to WASM memory.
// This is a SYNCHRONOUS pixel grab — video must already be at the right time.
// Reusable extraction dimensions (set once, avoid per-frame resize)
let extractW = 0, extractH = 0;

function captureVideoFrameToWasm(assetPath, video, timeSecs, targetW, targetH) {
    const w = targetW || RENDER_W;
    const h = targetH || RENDER_H;
    // Only resize canvas when dimensions change (avoid GPU context thrashing)
    if (extractW !== w || extractH !== h) {
        extractCanvas.width = w;
        extractCanvas.height = h;
        extractW = w;
        extractH = h;
    }
    extractCtx.drawImage(video, 0, 0, w, h);
    
    const imageData = extractCtx.getImageData(0, 0, w, h);
    renderer.cache_video_frame(assetPath, timeSecs, new Uint8Array(imageData.data.buffer), w, h);
}

// Extract a video frame by seeking (for scrubbing / single-frame viewing).
// Only used when NOT playing continuously.
async function extractVideoFrame(assetPath, timeSecs, targetW, targetH) {
    const video = await ensureVideoLoaded(assetPath);
    if (!video || video.readyState < 2) return null;
    
    video.currentTime = timeSecs;
    await new Promise((resolve) => {
        video.onseeked = resolve;
        setTimeout(resolve, 200);
    });

    const w = targetW || RENDER_W;
    const h = targetH || RENDER_H;
    extractCanvas.width = w;
    extractCanvas.height = h;
    extractCtx.drawImage(video, 0, 0, w, h);
    
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
// When playing, video frames are captured synchronously from the playing video.
// When scrubbing, falls back to async seek + decode.
// ── Performance diagnostics ──
let perfCaptureMs = 0, perfRenderMs = 0, perfFrameCount = 0;

async function renderCurrentFrame(isPlaybackRender = false) {
    if (!renderer || !sceneJson) return;
    
    const frameData = sceneJson.frames[currentFrame];
    if (!frameData) return;

    if (isPlaybackRender) {
        // During continuous playback:
        // 1. Clear old WASM video frames to prevent memory bloat (if method exists)
        if (typeof renderer.clear_video_frames === 'function') {
            renderer.clear_video_frames();
        }
        cachedVideoFrames.clear();
        
        // 2. Capture current frame synchronously from playing video
        const t0 = performance.now();
        for (const update of frameData.texture_updates) {
            if (update.DecodeVideoFrame) {
                const path = update.DecodeVideoFrame.path;
                const time = update.DecodeVideoFrame.timestamp_secs;
                const w = update.DecodeVideoFrame.width || null;
                const h = update.DecodeVideoFrame.height || null;
                const video = videoPool[path];
                if (video && video.readyState >= 2) {
                    captureVideoFrameToWasm(path, video, time, w, h);
                }
            }
        }
        const captureTime = performance.now() - t0;
        perfCaptureMs += captureTime;
    } else {
        // Scrubbing / single frame: blocking seek + decode
        await preloadFrameVideoFrames(frameData);
    }
    
    // Scene export resolution (pixel coords are authored at this size)
    const sceneW = sceneJson.settings?.width || 1920;
    const sceneH = sceneJson.settings?.height || 1080;
    
    try {
        const t1 = performance.now();
        renderer.render_frame_scaled(JSON.stringify(frameData), sceneW, sceneH);
        const renderTime = performance.now() - t1;
        perfRenderMs += renderTime;
        perfFrameCount++;
        
        // Log performance every 60 frames
        if (isPlaybackRender && perfFrameCount % 60 === 0) {
            const avgCapture = (perfCaptureMs / perfFrameCount).toFixed(1);
            const avgRender = (perfRenderMs / perfFrameCount).toFixed(1);
            console.log(`[perf] avg: capture=${avgCapture}ms, render=${avgRender}ms, total=${(parseFloat(avgCapture) + parseFloat(avgRender)).toFixed(1)}ms (${perfFrameCount} frames)`);
        }
        
        frameCounter.innerText = `${currentFrame} / ${totalFrames - 1}`;
        timeline.value = currentFrame;
    } catch (e) {
        console.error("Render failed:", e);
        stopPlayback();
    }
}

// ── Audio Sync ──
let activeAudioVideo = null;

function startAudioSync() {
    if (!sceneJson) return;
    const firstVideoUpdate = sceneJson.frames[0]?.texture_updates?.find(u => u.DecodeVideoFrame);
    if (!firstVideoUpdate) return;
    
    const videoPath = firstVideoUpdate.DecodeVideoFrame.path;
    const video = videoPool[videoPath];
    if (!video) return;
    
    const fps = sceneJson.settings?.fps || 30;
    const currentTimeSecs = currentFrame / fps;
    
    video.muted = false;
    video.currentTime = currentTimeSecs;
    video.play().catch(e => console.warn("Audio autoplay blocked:", e));
    activeAudioVideo = video;
}

function stopAudioSync() {
    if (activeAudioVideo) {
        activeAudioVideo.pause();
        activeAudioVideo.muted = true;
        activeAudioVideo = null;
    }
    for (const v of Object.values(videoPool)) {
        v.pause();
        v.muted = true;
    }
}

// ── Start continuous video playback for all scene videos ──
function startContinuousVideoPlayback() {
    if (!sceneJson) return;
    const fps = sceneJson.settings?.fps || 30;
    const startTimeSecs = currentFrame / fps;
    
    // Find all video paths and start them playing
    const videoPaths = new Set();
    for (const frame of sceneJson.frames) {
        for (const update of frame.texture_updates) {
            if (update.DecodeVideoFrame) videoPaths.add(update.DecodeVideoFrame.path);
        }
    }
    
    for (const path of videoPaths) {
        const video = videoPool[path];
        if (video) {
            video.currentTime = startTimeSecs;
            video.playbackRate = 1.0;
            video.play().catch(() => {});
        }
    }
}

function stopContinuousVideoPlayback() {
    for (const v of Object.values(videoPool)) {
        v.pause();
    }
}

// ── Playback Controls ──
function stopPlayback() {
    isPlaying = false;
    playBtn.innerText = "Play";
    stopAudioSync();
    stopContinuousVideoPlayback();
}

// ── Wall-clock playback loop using requestAnimationFrame ──
// Mirrors Studio's PlaybackMode::Realtime:
//   - Uses wall-clock time to determine target frame
//   - SKIPS frames if rendering is too slow (drops instead of accumulating delay)
//   - Uses requestAnimationFrame for vsync-aligned timing
let playbackStartTime = 0;
let playbackStartFrame = 0;
let lastDropLog = 0;
let totalDroppedFrames = 0;

function playLoop(timestamp) {
    if (!isPlaying) return;
    
    const fps = sceneJson.settings?.fps || 30;
    const elapsed = (timestamp - playbackStartTime) / 1000.0;
    const targetFrame = playbackStartFrame + Math.floor(elapsed * fps);
    
    if (targetFrame >= totalFrames) {
        currentFrame = totalFrames - 1;
        renderCurrentFrame(true);
        stopPlayback();
        console.log(`[playback] ended. Total dropped: ${totalDroppedFrames} frames`);
        return;
    }
    
    if (targetFrame !== currentFrame && targetFrame >= 0) {
        const skipped = targetFrame - currentFrame - 1;
        if (skipped > 0) {
            totalDroppedFrames += skipped;
            // Batch drop logs (max once per second)
            if (timestamp - lastDropLog > 1000) {
                console.debug(`Dropped ${totalDroppedFrames} total frames so far`);
                lastDropLog = timestamp;
            }
        }
        currentFrame = targetFrame;
        renderCurrentFrame(true);
    }
    
    requestAnimationFrame(playLoop);
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
        const dpr = window.devicePixelRatio || 1;
        
        canvas.width = RENDER_W;
        canvas.height = RENDER_H;
        
        const cssW = Math.round(RENDER_W / dpr);
        const cssH = Math.round(RENDER_H / dpr);
        canvas.style.width = `${cssW}px`;
        canvas.style.height = `${cssH}px`;
        
        console.log(`Canvas init: dpr=${dpr}, backing=${canvas.width}x${canvas.height}, CSS=${cssW}x${cssH}px`);
        
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
            
            // Reset performance counters
            perfCaptureMs = 0;
            perfRenderMs = 0;
            perfFrameCount = 0;
            totalDroppedFrames = 0;
            lastDropLog = 0;
            
            // Start continuous video playback (let browser decode ahead)
            startContinuousVideoPlayback();
            startAudioSync();
            
            // Start rAF playback loop with wall-clock timing
            playbackStartFrame = currentFrame;
            playbackStartTime = performance.now();
            requestAnimationFrame(playLoop);
        }
    });

    timeline.addEventListener('input', async (e) => {
        stopPlayback();
        currentFrame = parseInt(e.target.value);
        await renderCurrentFrame(); // blocking seek for scrubbing
    });

    exportBtn.addEventListener('click', async () => {
        stopPlayback();
        statusTxt.innerText = "Sending Export to Backend...";
        exportBtn.disabled = true;
        
        const exportDir = document.getElementById('export-dir').value.trim();
        const exportFilename = document.getElementById('export-filename').value.trim() || 'output.mp4';
        const exportFfmpeg = document.getElementById('export-ffmpeg').value.trim();
        
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
