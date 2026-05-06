import init, { IfolRenderWeb } from 'ifol-render-wasm';

let engine = null;
let playing = false;
let timeSec = 0.0;
let duration = 10.0;
let lastTime = 0;

let cam_x = undefined;
let cam_y = undefined;
let cam_zoom = undefined;
let isEditorMode = true;
let isDragging = false;
let isDraggingEntity = false;
let selectedEntityId = undefined;
let lastMouseX = 0;
let lastMouseY = 0;

// Timeline state
let currentScene = null;   // Parsed JSON
let timelineScope = null;  // null = root, string = composition entity ID
let timelineScopePath = []; // breadcrumb path [{id, label}]

const $ = (id) => document.getElementById(id);

// Sync canvas resolution to viewport container (editor mode only)
function syncCanvasToViewport() {
    if (!engine) return;
    const canvas = $('canvasMain');
    const container = $('viewportArea');
    const qLabel = parseFloat($('selQuality') ? $('selQuality').value : "1") || 1;
    
    if (isEditorMode) {
        // Editor: canvas matches container pixel size exactly (no distortion)
        const cw = Math.max(1, Math.floor(container.clientWidth));
        const ch = Math.max(1, Math.floor(container.clientHeight));
        
        let bw = cw; // Physical canvas pixels always match DOM
        let bh = ch;
        
        canvas.style.width = cw + "px";
        canvas.style.height = ch + "px";
        
        if (canvas.width !== bw || canvas.height !== bh) {
            canvas.width = bw;
            canvas.height = bh;
            $('lblCanvasSize').textContent = `${bw}x${bh}`;
        }
    } else {
        // Camera mode: fixed resolution matching active camera
        let camW = 1280;
        let camH = 720;
        const activeCam = $('selViewCamera') ? $('selViewCamera').value : "main_cam";
        if (currentScene && currentScene.entities) {
            const cam = currentScene.entities.find(e => e.id === activeCam);
            if (cam && cam.rect) {
                camW = Math.floor(cam.rect.width);
                camH = Math.floor(cam.rect.height);
            }
        }
        
        let bw = camW;
        let bh = camH;
        
        canvas.style.width = camW + "px";
        canvas.style.height = camH + "px";
        
        if (canvas.width !== bw || canvas.height !== bh) {
            canvas.width = bw;
            canvas.height = bh;
            $('lblCanvasSize').textContent = `${bw}x${bh}`;
        }
    }
}

// Auto-sync canvas when container is resized (drag handles, window resize)
const _vpResizeObserver = new ResizeObserver(() => {
    if (isEditorMode && engine) {
        syncCanvasToViewport();
        if (!playing) requestAnimationFrame(requestRender);
    }
});
_vpResizeObserver.observe($('viewportArea'));

// Get editor camera viewport in world units.
// Root mode AND Scope mode: cam_x/y = CENTER of the editor viewport in local space.
// Thanks to Phase 5 Hierarchy Isolation, composition children always live in local space,
// so the editor camera does not need to offset by the composition's world coordinates.
function getEditorCam() {
    const zoom = (cam_zoom || 1.0);
    const cx = cam_x !== undefined ? cam_x : 640;
    const cy = cam_y !== undefined ? cam_y : 360;

    const w = $('canvasMain').width / zoom;
    const h = $('canvasMain').height / zoom;

    return {
        x: cx - w/2,
        y: cy - h/2,
        w: w, h: h
    };
}

function requestRender() {
    if (!engine) return;
    
    // When scoped into a composition, timeSec IS local time.
    if (timelineScope) {
        engine.set_scope_time(timeSec);
    } else {
        engine.set_scope_time(undefined);
    }
    
    const isDual = $('chkDualViewport').checked;
    const gpuCanvas = $('gpuCanvas');
    
    // Send quality setting to engine
    const qLabel = parseFloat($('selQuality') ? $('selQuality').value : "1") || 1;
    if (engine.set_render_quality) engine.set_render_quality(qLabel);

    // Resize Viewport 1 Canvas (DOM)
    syncCanvasToViewport();
    
    // --- Pass 1: Render Viewport 1 (canvasMain) ---
    const canvas1 = $('canvasMain');
    if (gpuCanvas.width !== canvas1.width || gpuCanvas.height !== canvas1.height) {
        gpuCanvas.width = canvas1.width;
        gpuCanvas.height = canvas1.height;
        engine.resize(canvas1.width, canvas1.height);
    }
    const activeCam = $('selViewCamera') ? $('selViewCamera').value : "main_cam";

    let metricsStr = "";
    if (isEditorMode) {
        const ec = getEditorCam();
        // When scoped into composition, use time=0 for root and pass scope cam
        const renderTime = timelineScope ? 0 : timeSec;
        metricsStr = engine.render_frame_v2(
            renderTime, activeCam, true,
            (ec.x !== undefined) ? ec.x : undefined,
            (ec.y !== undefined) ? ec.y : undefined,
            (ec.w !== undefined) ? ec.w : undefined,
            (ec.h !== undefined) ? ec.h : undefined
        );
    } else {
        metricsStr = engine.render_frame_v2(timelineScope ? 0 : timeSec, activeCam, false, undefined, undefined, undefined, undefined);
    }
    
    let frameComplete = true;
    try {
        const metrics = JSON.parse(metricsStr);
        if (metrics.vram_bytes !== undefined) {
            const d = (metrics.vram_bytes / 1024 / 1024).toFixed(1);
            $('lblVram').textContent = `VRAM: ${d} MB (${metrics.vram_count} Tex)`;
            if (isDual) $('lblVram2').textContent = `VRAM: ${d} MB (${metrics.vram_count} Tex)`;
        }
        if (metrics.frame_complete !== undefined) {
            frameComplete = metrics.frame_complete;
        }
    } catch(e) {}
    
    // Only blit GPU canvas to display canvas if the frame is fully ready.
    // This prevents partial rendering (light entities before heavy ones).
    if (frameComplete) {
        const ctx1 = canvas1.getContext('2d');
        ctx1.clearRect(0, 0, canvas1.width, canvas1.height);
        ctx1.drawImage(gpuCanvas, 0, 0);
    } else {
        // Frame incomplete — poll again to render when video finishes buffering.
        if (!playing) {
            requestAnimationFrame(requestRender);
        }
    }

    // --- Pass 2: Render Viewport 2 (Camera Mode on canvasMain2) ---
    if (isDual) {
        let camW = 1280;
        let camH = 720;
        const activeCam = $('selViewCamera') ? $('selViewCamera').value : "main_cam";
        if (currentScene && currentScene.entities) {
            const cam = currentScene.entities.find(e => e.id === activeCam);
            if (cam && cam.rect) { 
                camW = Math.floor(cam.rect.width); 
                camH = Math.floor(cam.rect.height); 
            }
        }
        const canvas2 = $('canvasMain2');
        if (canvas2.width !== camW || canvas2.height !== camH) {
            canvas2.width = camW; canvas2.height = camH;
            $('lblCanvasSize2').textContent = `${camW}x${camH}`;
        }
        
        if (gpuCanvas.width !== canvas2.width || gpuCanvas.height !== canvas2.height) {
            gpuCanvas.width = canvas2.width;
            gpuCanvas.height = canvas2.height;
            engine.resize(canvas2.width, canvas2.height);
        }
        
        engine.render_frame_v2(timelineScope ? 0 : timeSec, "main_cam", false, undefined, undefined, undefined, undefined);
        
        if (frameComplete) {
            const ctx2 = canvas2.getContext('2d');
            ctx2.clearRect(0, 0, canvas2.width, canvas2.height);
            ctx2.drawImage(gpuCanvas, 0, 0);
        }
    }
    
    renderTimeline();
}

// ─── INITIALIZATION ───
async function initEngine() {
    $('lblStatus').textContent = "Downloading WASM...";
    await init();
    // Use the hidden gpuCanvas to initialize WebGPU context!
    const canvas = $('gpuCanvas');
    engine = await IfolRenderWeb.create(canvas, 1280, 720, 60);
    window.engine = engine; // Expose globally for DevTools debugging
    engine.set_select_mode($('selSelectMode').value);
    engine.setup_builtins();
    
    // Wake up render loop on async video events
    window.addEventListener('ifol_video_seeked', () => {
        if (!playing) requestRender();
    });
    window.addEventListener('ifol_video_ready', () => {
        if (!playing) requestRender();
    });
    window.addEventListener('ifol_audio_ready', () => {
        if (!playing) requestRender();
    });
    
    // Wire up the new asynchronous Javascript Event Callback interface
    engine.set_event_listener((evt) => {
        if (window.activeTestCase !== 20) return;
        
        let outStr = "";
        try {
            const parsed = JSON.parse(evt.payload);
            outStr = JSON.stringify(parsed, null, 2);
        } catch {
            outStr = evt.payload;
        }
        
        // Accumulate logs in the text area for TC20
        const logArea = $('jsonEditor');
        const eventStr = `[Event Triggered: ${evt.type}]\n${outStr}\n\n`;
        // Only keep last ~50 lines to prevent lag
        let currentLogs = logArea.value.split('\n');
        if (currentLogs.length > 50) currentLogs = currentLogs.slice(-50);
        logArea.value = currentLogs.join('\n') + eventStr;
        logArea.scrollTop = logArea.scrollHeight;
    });
    
    $('lblStatus').textContent = "Pipeline Ready ✅";
    $('lblStatus').style.color = "#4ade80";
    
    // Auto-load Test Case 1
    const selTest = $('selTestCase');
    if (selTest) {
        selTest.value = 'btnTestCase8';
        selTest.dispatchEvent(new Event('change'));
    }
    $('chkEditorMode').onchange = (e) => {
        isEditorMode = e.target.checked;
        const vp = $('viewportArea');
        vp.classList.toggle('editor-mode', isEditorMode);
        vp.classList.toggle('camera-mode', !isEditorMode);
        if (!playing) requestRender();
    };
    
    $('chkDualViewport').onchange = (e) => {
        const dual = e.target.checked;
        $('viewportArea2').style.display = dual ? 'block' : 'none';
        if (!playing) requestRender();
    };
    
    $('selSelectMode').onchange = (e) => {
        if (engine) engine.set_select_mode(e.target.value);
        if (!playing) requestAnimationFrame(requestRender);
    };

    $('selQuality').onchange = (e) => {
        if (!playing) requestAnimationFrame(requestRender);
    };

    $('chkPostEffects').onchange = (e) => {
        if (engine) engine.set_post_effects_enabled(e.target.checked);
        if (!playing) requestAnimationFrame(requestRender);
    };

    // Sync initial state
    if (engine) engine.set_select_mode($('selSelectMode').value);
    
    requestAnimationFrame(loop);
}

// ─── ASSET LOADER ───
// App layer decodes images using browser-native APIs (fast, GPU-accelerated)
// and injects raw RGBA into Core via cache_image(key, data, w, h).
// Returns Map<assetId, {url, width, height}> for intrinsic dimension auto-fill.
async function loadAndCacheAssets(scene) {
    const assetDims = new Map(); // assetId → {url, width, height}
    if (!engine || !scene || !scene.assets) return assetDims;
    
    const loadPromises = [];
    for (const [assetId, assetDef] of Object.entries(scene.assets)) {
        if (assetDef.type === 'image' && assetDef.url) {
            const url = assetDef.url;
            loadPromises.push(
                (async () => {
                    try {
                        const resp = await fetch(url);
                        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
                        const blob = await resp.blob();
                        const bitmap = await createImageBitmap(blob);
                        
                        // Decode via offscreen canvas → RGBA
                        const c = new OffscreenCanvas(bitmap.width, bitmap.height);
                        const ctx = c.getContext('2d');
                        ctx.drawImage(bitmap, 0, 0);
                        const imageData = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
                        
                        // Inject RGBA into Core — use URL as key (matches source_sys texture_key)
                        engine.cache_image(url, imageData.data, bitmap.width, bitmap.height);
                        assetDims.set(assetId, { url, width: bitmap.width, height: bitmap.height });
                        console.log(`[Asset] Loaded image '${assetId}' (${bitmap.width}x${bitmap.height}) from ${url}`);
                        bitmap.close();
                    } catch (e) {
                        console.warn(`[Asset] Failed to load '${assetId}' from ${url}:`, e);
                    }
                })()
            );
        } else if (assetDef.type === 'font' && assetDef.url) {
            const url = assetDef.url;
            loadPromises.push(
                (async () => {
                    try {
                        const resp = await fetch(url);
                        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
                        const buffer = await resp.arrayBuffer();
                        const data = new Uint8Array(buffer);
                        
                        // Inject Font TTF Bytes into Core
                        engine.cache_font(url, data);
                        console.log(`[Asset] Loaded font '${assetId}' (${data.length} bytes) from ${url}`);
                    } catch (e) {
                        console.warn(`[Asset] Failed to load font '${assetId}' from ${url}:`, e);
                    }
                })()
            );
        }
    }
    
    if (loadPromises.length > 0) {
        await Promise.all(loadPromises);
        console.log(`[Asset] All ${loadPromises.length} image(s) loaded.`);
    }
    return assetDims;
}

// ─── JSON INJECTION ───
async function applyJson() {
    if (!engine) {
        alert("Please Init Pipeline first!");
        return;
    }
    try {
        const json = $('jsonEditor').value;
        const scene = JSON.parse(json);
        
        // 1. Pre-load all image assets from URLs → RGBA → cache_image
        const assetDims = await loadAndCacheAssets(scene);
        
        // 2. Auto-fill intrinsicWidth/Height on imageSource entities
        if (scene.entities && assetDims.size > 0) {
            for (const ent of scene.entities) {
                if (ent.imageSource && ent.imageSource.assetId) {
                    const dims = assetDims.get(ent.imageSource.assetId);
                    if (dims) {
                        ent.imageSource.intrinsicWidth = dims.width;
                        ent.imageSource.intrinsicHeight = dims.height;
                    }
                }
            }
        }
        
        // 2.5 Auto-resolve -1 lifespans (App SDK logic for Track cuts)
        // Groups entities by (parent_id, layer) to act as timeline tracks
        if (scene.entities) {
            const trackGroups = {};
            // Gather all entities with lifespans
            for (const ent of scene.entities) {
                if (!ent.lifespan) continue;
                const pid = ent.parent_id || 'root';
                const layer = ent.layer || 0;
                const trackKey = `${pid}_${layer}`;
                if (!trackGroups[trackKey]) trackGroups[trackKey] = [];
                trackGroups[trackKey].push(ent);
            }
            
            // Sort each track and resolve -1
            for (const trackKey in trackGroups) {
                const track = trackGroups[trackKey];
                track.sort((a, b) => a.lifespan.start - b.lifespan.start);
                
                for (let i = 0; i < track.length; i++) {
                    const ent = track[i];
                    if (ent.lifespan.end === -1) {
                        if (i + 1 < track.length) {
                            // Cut exactly when the next entity on the track starts
                            ent.lifespan.end = track[i+1].lifespan.start;
                        } else {
                            // Last entity on track: extend to parent comp duration
                            ent.lifespan.end = scene.project.duration || 9999.0;
                        }
                    }
                }
            }
        }
        
        // 3. Load scene into ECS (with auto-filled intrinsic and resolved lifespans)
        engine.load_scene_v2(JSON.stringify(scene));
        currentScene = scene;
        
        // Update cameras dropdown (filtered by scope)
        updateCameraList();
        
        $('lblEntities').textContent = `${currentScene.entities ? currentScene.entities.length : 0} entities`;
        
        // Reset camera and selection
        cam_x = undefined;
        cam_y = undefined;
        cam_zoom = undefined;
        selectedEntityId = undefined;
        if(engine) engine.select_entity_v2("");
        
        // Reset timeline scope
        timelineScope = null;
        timelineScopePath = [];
        
        // Auto-detect duration from entity lifespans + animations
        duration = detectDuration(currentScene);
        
        requestRender();
    } catch(e) {
        alert("JSON Parse/Load Error:\n" + e);
        console.error(e);
    }
}

function detectDuration(scene) {
    let maxEnd = 10.0;
    if (!scene || !scene.entities) return maxEnd;
    for (const ent of scene.entities) {
        if (ent.lifespan && ent.lifespan.end > maxEnd) maxEnd = ent.lifespan.end;
        if (ent.animation && ent.animation.floatTracks) {
            for (const ft of ent.animation.floatTracks) {
                if (ft.track && ft.track.keyframes) {
                    for (const kf of ft.track.keyframes) {
                        if (kf.time > maxEnd) maxEnd = kf.time;
                    }
                }
            }
        }
    }
    return maxEnd; // Uncapped duration representing true maximum lifespan of entities.
}

// ─── RENDER LOOP ───
function loop(ts) {
    if (!engine) return requestAnimationFrame(loop);
    
    if (playing) {
        const dt = (ts - lastTime) / 1000.0;
        timeSec += dt;
        const maxTime = timelineScope ? getScopeDuration() : duration;
        if (timeSec > maxTime) timeSec = 0;
        
        const t0 = performance.now();
        requestRender();
        $('lblRenderMs').textContent = (performance.now() - t0).toFixed(1);
    }
    
    const displayDur = timelineScope ? getScopeDuration() : duration;
    $('lblTime').textContent = `${timeSec.toFixed(2)} / ${displayDur.toFixed(2)}s`;
    lastTime = ts;
    requestAnimationFrame(loop);
}

// ─── EVENTS ───
// ─── EVENTS ───
$('btnInit').onclick = initEngine;
$('btnUpdateJson').onclick = applyJson;

let liveSyncInterval = null;
let lastSceneContent = "";

$('btnLiveSync').onclick = () => {
    if (liveSyncInterval) {
        clearInterval(liveSyncInterval);
        liveSyncInterval = null;
        $('btnLiveSync').textContent = '🔴 Live Sync: OFF';
        $('btnLiveSync').style.background = '#dc2626';
    } else {
        $('btnLiveSync').textContent = '🟢 Live Sync: ON';
        $('btnLiveSync').style.background = '#16a34a';
        
        // Immediate fetch
        fetchScene();
        
        liveSyncInterval = setInterval(fetchScene, 1000);
    }
};

async function fetchScene() {
    try {
        const res = await fetch('/api/scene');
        if (res.ok) {
            const text = await res.text();
            if (text !== lastSceneContent) {
                lastSceneContent = text;
                $('jsonEditor').value = text;
                applyJson();
                console.log("[Live Sync] Scene updated automatically.");
            }
        }
    } catch(e) {
        console.error("[Live Sync] Failed to fetch scene:", e);
    }
}

$('btnSaveDisk').onclick = async () => {
    try {
        const text = $('jsonEditor').value;
        const res = await fetch('/api/scene', {
            method: 'POST',
            body: text
        });
        if (res.ok) {
            alert('Saved to scene.json!');
            lastSceneContent = text; // Prevent live-sync from triggering redundant update
        } else {
            alert('Failed to save to disk.');
        }
    } catch(e) {
        alert('Failed to connect to Vite Server for saving.');
    }
};

$('btnPlay').onclick = () => { 
    if (playing) {
        playing = false;
        $('btnPlay').innerText = '▶ Play';
        if (engine) engine.set_playing(false);
        requestRender(); // Flush paused state to WASM
    } else {
        playing = true; 
        lastTime = performance.now(); 
        $('btnPlay').innerText = '⏸ Pause';
        if (engine) engine.set_playing(true);
    }
};
$('btnStop').onclick = () => { 
    playing = false; 
    timeSec = 0; 
    $('btnPlay').innerText = '▶ Play';
    if (engine) engine.set_playing(false);
    requestRender(); 
};

// Prevent right-click context menu on canvas
$('canvasMain').addEventListener('contextmenu', e => e.preventDefault());

// Helper: convert CSS pixel coordinates to canvas pixel coordinates
function cssToCanvas(cssX, cssY) {
    const canvas = $('canvasMain');
    const rect = canvas.getBoundingClientRect();
    // In editor mode canvas matches container → ratio ≈ 1
    // In camera mode canvas may differ from CSS display size
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    return { x: cssX * scaleX, y: cssY * scaleY };
}

$('canvasMain').addEventListener('mousedown', e => {
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
    
    if (e.button === 0) {
        // LEFT CLICK: Select/Drag entity
        if (engine) {
            const rect = e.target.getBoundingClientRect();
            const cssX = e.clientX - rect.left;
            const cssY = e.clientY - rect.top;
            const canvasCoords = cssToCanvas(cssX, cssY);
            
            const ec = isEditorMode ? getEditorCam() : {};
            const pickedId = engine.pick_entity_v2(
                canvasCoords.x, canvasCoords.y, 
                "main_cam",
                isEditorMode ? ec.x : undefined,
                isEditorMode ? ec.y : undefined,
                isEditorMode ? ec.w : undefined,
                isEditorMode ? ec.h : undefined
            );
            
            selectedEntityId = pickedId;
            engine.select_entity_v2(pickedId || "");
            requestRender();
            
            if (pickedId) {
                isDraggingEntity = true;
                console.log("Picked:", pickedId);
            }
        }
    } else if (e.button === 2) {
        isDragging = true;
    }
});

$('chkDualViewport').addEventListener('change', e => {
    isDualView = e.target.checked;
    resizeCanvas();
    requestRender();
});

$('selSelectMode').addEventListener('change', e => {
    if (engine) engine.set_select_mode(e.target.value);
});

$('selQuality').addEventListener('change', e => {
    requestRender();
});

window.addEventListener('mouseup', () => { 
    isDragging = false; 
    isDraggingEntity = false; 
});

window.addEventListener('mousemove', e => {
    const cssDx = e.clientX - lastMouseX;
    const cssDy = e.clientY - lastMouseY;
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
    
    const canvasDelta = cssToCanvas(cssDx, cssDy);
    const dx = canvasDelta.x;
    const dy = canvasDelta.y;
    
    if (isDraggingEntity && selectedEntityId && engine) {
        const ec = isEditorMode ? getEditorCam() : {};
        engine.drag_entity_v2(
            selectedEntityId,
            dx, dy,
            "main_cam",
            isEditorMode ? ec.w : undefined,
            isEditorMode ? ec.h : undefined
        );
        if(!playing) requestRender();
        return;
    }
    
    if (isDragging) {
        // Standard pan: dragging RIGHT moves the view RIGHT → cam_x decreases (cam_x = center for root, offset for scope)
        if (cam_x === undefined) {
            // Initialize pan center based on mode:
            // Root mode = center of 1280x720 scene; scope mode = no offset (0)
            cam_x = timelineScope ? 0 : 640;
            cam_y = timelineScope ? 0 : 360;
            cam_zoom = cam_zoom || 1.0;
        }
        cam_x -= dx / (cam_zoom || 1);
        cam_y -= dy / (cam_zoom || 1);
        if (!playing) requestRender();
    }
});

$('canvasMain').addEventListener('wheel', e => {
    e.preventDefault();
    if (cam_zoom === undefined) {
        cam_x = timelineScope ? 0 : 640;
        cam_y = timelineScope ? 0 : 360;
        cam_zoom = 1.0;
    }

    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    cam_zoom *= zoomFactor;
    
    if(!playing) requestRender();
});

// ════════════════════════════════════════════════════════════════════
// ─── TIMELINE RENDERING ───
// ════════════════════════════════════════════════════════════════════

const TRACK_HEIGHT = 24;
const RULER_HEIGHT = 22;
const COLORS = {
    bg: '#1a1a1a',
    ruler: '#252525',
    rulerLine: '#444',
    rulerText: '#888',
    trackBg1: '#1e1e1e',
    trackBg2: '#222',
    trackSelected: '#1a3a5c',
    playhead: '#ff4444',
    keyframe: '#fbbf24',
    keyframeStroke: '#92400e',
    composition: '#a855f7',
    camera: '#ff55ff',
    barDefault: '#4a5568',
};

function getEntityColor(ent) {
    if (ent.camera) return COLORS.camera;
    if (ent.composition) return COLORS.composition;
    if (ent.shapeSource && ent.shapeSource.fillColor) {
        const c = ent.shapeSource.fillColor;
        return `rgba(${Math.round(c[0]*255)},${Math.round(c[1]*255)},${Math.round(c[2]*255)},0.8)`;
    }
    return COLORS.barDefault;
}

function getVisibleEntities() {
    if (!currentScene || !currentScene.entities) return [];
    let visible = [];
    if (timelineScope === null) {
        // Root: show entities without parentId OR whose parentId does NOT have a composition
        visible = currentScene.entities.filter(e => {
            if (e.parentId) {
                const parent = currentScene.entities.find(p => p.id === e.parentId);
                if (parent && parent.composition) return false; // hidden, inside composition
            }
            return true;
        });
    } else {
        // Scoped to composition: show children of composition
        visible = currentScene.entities.filter(e => e.parentId === timelineScope);
    }
    
    // Sort descending by layer, but ALWAYS pin cameras to the top of the timeline
    visible.sort((a, b) => {
        const isCamA = a.camera ? 1 : 0;
        const isCamB = b.camera ? 1 : 0;
        if (isCamA !== isCamB) return isCamB - isCamA; // Cameras first

        const layerA = a.layer !== undefined ? a.layer : 0;
        const layerB = b.layer !== undefined ? b.layer : 0;
        if (layerA !== layerB) {
            return layerB - layerA; // Descending layer for content
        }
        return 0;
    });
    
    return visible;
}

// Group sorted entities into visual Timeline Tracks based on their layer
function getVisibleTracks() {
    const visible = getVisibleEntities();
    const tracksMap = new Map();
    const orderedTracks = [];
    
    for (const ent of visible) {
        const isCam = ent.camera ? 1 : 0;
        const layer = ent.layer || 0;
        // Unique track key separating cameras from content, even on the same layer
        const trackKey = `track_${isCam}_${layer}`;
        
        if (!tracksMap.has(trackKey)) {
            const track = {
                id: trackKey,
                layer: layer,
                isCam: isCam,
                entities: []
            };
            tracksMap.set(trackKey, track);
            orderedTracks.push(track);
        }
        tracksMap.get(trackKey).entities.push(ent);
    }
    return orderedTracks;
}

function getScopeDuration() {
    if (timelineScope === null) return duration;
    const comp = currentScene.entities.find(e => e.id === timelineScope);
    if (comp && comp.composition) {
        if (comp.composition.duration && comp.composition.duration.manual) {
            return comp.composition.duration.manual;
        }
    }
    return duration;
}

function getEntityLifespan(ent) {
    const scopeD = getScopeDuration();
    if (ent.lifespan) return { start: ent.lifespan.start, end: ent.lifespan.end };
    return { start: 0, end: scopeD };
}

function renderTimelineLabels() {
    const container = $('timelineLabelsScroll');
    container.innerHTML = '';
    const tracks = getVisibleTracks();
    
    for (const track of tracks) {
        const label = document.createElement('div');
        label.className = 'timeline-label';
        
        // If track contains selected entity
        if (track.entities.some(e => e.id === selectedEntityId)) {
            label.className += ' selected';
        }
        
        let icon = '🟦';
        let displayName = `Layer ${track.layer}`;

        if (track.isCam) {
            icon = '📹';
            displayName = `Video Track ${track.layer}`;
        } else if (track.entities.some(e => e.composition)) {
            icon = '📦';
            displayName = `Comp Layer ${track.layer}`;
        }
        
        label.textContent = `${icon} ${displayName}`;
        label.title = displayName;
        
        // Wait, click should select the track or just do nothing? 
        // We'll let users select exact entities by clicking the canvas instead.
        container.appendChild(label);
    }
}

// Update camera dropdown filtered by current scope
function updateCameraList() {
    if (!currentScene || !currentScene.entities) return;
    const selCamera = $('selViewCamera');
    if (!selCamera) return;
    
    let cameras;
    if (timelineScope) {
        // Scoped into a comp: only show cameras that are direct children of this comp
        cameras = currentScene.entities.filter(e => 
            e.camera !== undefined && e.parentId === timelineScope
        );
    } else {
        // Root: show cameras without a composition parent (root-level cameras)
        cameras = currentScene.entities.filter(e => {
            if (e.camera === undefined) return false;
            // Exclude cameras that are children of a composition
            if (e.parentId) {
                const parent = currentScene.entities.find(p => p.id === e.parentId);
                if (parent && parent.composition) return false;
            }
            return true;
        });
    }
    
    const oldVal = selCamera.value;
    selCamera.innerHTML = '';
    
    if (cameras.length === 0) {
        selCamera.innerHTML = '<option value="main_cam">main_cam</option>';
    } else {
        cameras.forEach(c => {
            const opt = document.createElement('option');
            opt.value = c.id;
            opt.textContent = c.id;
            if (c.id === oldVal) opt.selected = true;
            selCamera.appendChild(opt);
        });
    }
}

function setRenderScope(scopeId) {
    timelineScope = scopeId;
    timeSec = 0; // Reset to start of local timeline
    playing = false;
    cam_zoom = 1.0;  // Always reset zoom when switching scope
    
    if (engine) {
        engine.set_playing(false);
        engine.set_render_scope(scopeId || undefined);
        engine.set_scope_time(scopeId ? 0 : undefined);

        if (scopeId && engine.get_scope_camera_params) {
            const scStr = engine.get_scope_camera_params();
            if (scStr) {
                const sc = JSON.parse(scStr);
                // Center the editor camera perfectly on the inner camera's view rect
                cam_x = sc.x + sc.w / 2;
                cam_y = sc.y + sc.h / 2;
            } else {
                cam_x = 640;
                cam_y = 360;
            }
        } else {
            cam_x = 640;
            cam_y = 360;
        }

        updateCameraList();
        requestRender();
    }
}



function renderBreadcrumb() {
    const bc = $('timelineBreadcrumb');
    bc.innerHTML = '';
    
    // Root
    const rootSpan = document.createElement('span');
    rootSpan.textContent = 'Root';
    if (timelineScope === null) {
        rootSpan.className = 'current';
    } else {
        rootSpan.addEventListener('click', () => {
            setRenderScope(null);
            timelineScopePath = [];
            renderBreadcrumb();
            renderTimeline();
        });
    }
    bc.appendChild(rootSpan);
    
    // Intermediate levels (if any)
    for (let i = 0; i < timelineScopePath.length; i++) {
        const sep = document.createElement('span');
        sep.textContent = ' › ';
        sep.style.color = '#666';
        sep.style.cursor = 'default';
        bc.appendChild(sep);
        
        const item = timelineScopePath[i];
        if (item.id !== null) {
            const span = document.createElement('span');
            span.textContent = item.id || 'Root';
            span.addEventListener('click', () => {
                setRenderScope(item.id);
                timelineScopePath = timelineScopePath.slice(0, i);
                renderBreadcrumb();
                renderTimeline();
            });
            bc.appendChild(span);
        }
    }
    
    // Current scope
    if (timelineScope !== null) {
        const sep = document.createElement('span');
        sep.textContent = ' › ';
        sep.style.color = '#666';
        sep.style.cursor = 'default';
        bc.appendChild(sep);
        
        const current = document.createElement('span');
        current.className = 'current';
        current.textContent = timelineScope;
        bc.appendChild(current);
    }
}

function renderTimeline() {
    if (!currentScene) return;
    
    const canvas = $('canvasTimeline');
    const wrap = $('timelineCanvasWrap');
    const rect = wrap.getBoundingClientRect();
    canvas.width = rect.width * devicePixelRatio;
    canvas.height = rect.height * devicePixelRatio;
    canvas.style.width = rect.width + 'px';
    canvas.style.height = rect.height + 'px';
    
    const ctx = canvas.getContext('2d');
    ctx.scale(devicePixelRatio, devicePixelRatio);
    const W = rect.width;
    const H = rect.height;
    
    // Get scroll offset from label scroll container for sync
    const labelsScroll = $('timelineLabelsScroll');
    const scrollY = labelsScroll ? labelsScroll.scrollTop : 0;
    
    // Background
    ctx.fillStyle = COLORS.bg;
    ctx.fillRect(0, 0, W, H);
    
    const entities = getVisibleEntities();
    const scopeD = getScopeDuration();
    const timeToX = (t) => (t / scopeD) * W;
    
    // ─── Ruler ───
    ctx.fillStyle = COLORS.ruler;
    ctx.fillRect(0, 0, W, RULER_HEIGHT);
    
    // Time ticks
    const tickInterval = getTickInterval(scopeD, W);
    ctx.strokeStyle = COLORS.rulerLine;
    ctx.fillStyle = COLORS.rulerText;
    ctx.font = '10px system-ui, sans-serif';
    ctx.textAlign = 'center';
    
    for (let t = 0; t <= scopeD; t += tickInterval) {
        const x = timeToX(t);
        // Major tick
        ctx.beginPath();
        ctx.moveTo(x, RULER_HEIGHT - 8);
        ctx.lineTo(x, RULER_HEIGHT);
        ctx.stroke();
        ctx.fillText(formatTime(t), x, RULER_HEIGHT - 10);
    }
    
    // Sub-ticks
    const subTick = tickInterval / 4;
    ctx.strokeStyle = '#333';
    for (let t = 0; t <= scopeD; t += subTick) {
        const x = timeToX(t);
        ctx.beginPath();
        ctx.moveTo(x, RULER_HEIGHT - 4);
        ctx.lineTo(x, RULER_HEIGHT);
        ctx.stroke();
    }
    
    // ─── Entity Tracks ───
    const trackY0 = RULER_HEIGHT;
    
    renderTimelineLabels();
    
    const tracks = getVisibleTracks();
    
    // Clip tracks below ruler
    ctx.save();
    ctx.beginPath();
    ctx.rect(0, RULER_HEIGHT, W, H - RULER_HEIGHT);
    ctx.clip();
    
    for (let i = 0; i < tracks.length; i++) {
        const track = tracks[i];
        const y = trackY0 + i * TRACK_HEIGHT - scrollY;
        
        // Track background (alternating)
        ctx.fillStyle = (i % 2 === 0) ? COLORS.trackBg1 : COLORS.trackBg2;
        if (track.entities.some(e => e.id === selectedEntityId)) ctx.fillStyle = COLORS.trackSelected;
        ctx.fillRect(0, y, W, TRACK_HEIGHT);
        
        // Track separator
        ctx.strokeStyle = '#2a2a2a';
        ctx.beginPath();
        ctx.moveTo(0, y + TRACK_HEIGHT);
        ctx.lineTo(W, y + TRACK_HEIGHT);
        ctx.stroke();
        
        // Draw each entity in this track
        for (const ent of track.entities) {
            // Lifespan bar
            const ls = getEntityLifespan(ent);
            const barX = timeToX(ls.start);
            const barW = timeToX(ls.end) - barX;
            const barY = y + 4;
            const barH = TRACK_HEIGHT - 8;
            
            const color = getEntityColor(ent);
            ctx.fillStyle = color;
            ctx.beginPath();
            roundRect(ctx, barX, barY, Math.max(barW, 2), barH, 3);
            ctx.fill();
            
            // Bar border & Label
            if (ent.id === selectedEntityId) {
                ctx.strokeStyle = '#00ffaa';
                ctx.lineWidth = 2;
            } else {
                ctx.strokeStyle = 'rgba(255,255,255,0.15)';
                ctx.lineWidth = 1;
            }
            ctx.beginPath();
            roundRect(ctx, barX, barY, Math.max(barW, 2), barH, 3);
            ctx.stroke();
            
            // Print entity ID on the bar
            ctx.fillStyle = '#fff';
            ctx.font = '11px system-ui';
            ctx.textAlign = 'left';
            ctx.fillText(ent.id, barX + 5, barY + 12);
            
            // Composition icon indicator  
            if (ent.composition) {
                ctx.fillStyle = '#fff';
                ctx.font = 'bold 9px system-ui';
                ctx.textAlign = 'right';
                ctx.fillText('⟳', barX + barW - 3, barY + barH - 2);
            }
            
            // Keyframe diamonds (Animation Component)
            if (ent.animation && ent.animation.floatTracks) {
                for (const ft of ent.animation.floatTracks) {
                    if (ft.track && ft.track.keyframes) {
                        for (const kf of ft.track.keyframes) {
                            const kx = timeToX(kf.time);
                            if (kx >= barX && kx <= barX + barW) {
                                drawKeyframeDiamond(ctx, kx, y + TRACK_HEIGHT / 2, 4);
                            }
                        }
                    }
                }
            }
            
            // Keyframe diamonds (Material Uniforms)
            if (ent.materials) {
                for (const mat of ent.materials) {
                    if (mat.float_uniforms) {
                        for (const key in mat.float_uniforms) {
                            const uniTrack = mat.float_uniforms[key];
                            if (uniTrack && uniTrack.keyframes) {
                                for (const kf of uniTrack.keyframes) {
                                    const kx = timeToX(kf.time);
                                    if (kx >= barX && kx <= barX + barW) {
                                        drawKeyframeDiamond(ctx, kx, y + TRACK_HEIGHT / 2, 4);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    ctx.restore(); // End track clipping
    
    // ─── Playhead ───
    // When scoped, timeSec IS local time — no conversion needed
    const scopeTime = timeSec;
    const phX = timeToX(Math.max(0, Math.min(scopeTime, scopeD)));
    
    ctx.strokeStyle = COLORS.playhead;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(phX, 0);
    ctx.lineTo(phX, H);
    ctx.stroke();
    
    // Playhead top triangle
    ctx.fillStyle = COLORS.playhead;
    ctx.beginPath();
    ctx.moveTo(phX - 5, 0);
    ctx.lineTo(phX + 5, 0);
    ctx.lineTo(phX, 8);
    ctx.closePath();
    ctx.fill();
    
    ctx.lineWidth = 1;
}

function getCompositionLocalTime(globalTime) {
    // Find the composition entity and compute its local time
    if (!timelineScope || !currentScene) return globalTime;
    const comp = currentScene.entities.find(e => e.id === timelineScope);
    if (!comp || !comp.composition) return globalTime;
    
    const c = comp.composition;
    const speed = c.speed || 1;
    const trimStart = c.trimStart || 0;
    const compDur = (c.duration && c.duration.manual) || 10;
    
    // Get lifespan of the composition entity
    const ls = comp.lifespan || { start: 0, end: 100 };
    const localTime = globalTime - ls.start;
    if (localTime < 0) return 0;
    
    let contentTime = localTime * speed + trimStart;
    
    // Apply loop
    if (c.loopMode === 'loop' && compDur > 0) {
        contentTime = contentTime % compDur;
    } else if (c.loopMode === 'pingPong' && compDur > 0) {
        const cycle = Math.floor(contentTime / compDur);
        contentTime = contentTime % compDur;
        if (cycle % 2 === 1) contentTime = compDur - contentTime;
    }
    
    return contentTime;
}

function drawKeyframeDiamond(ctx, x, y, size) {
    ctx.fillStyle = COLORS.keyframe;
    ctx.strokeStyle = COLORS.keyframeStroke;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(x, y - size);
    ctx.lineTo(x + size, y);
    ctx.lineTo(x, y + size);
    ctx.lineTo(x - size, y);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
}

function roundRect(ctx, x, y, w, h, r) {
    r = Math.min(r, w / 2, h / 2);
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.arcTo(x + w, y, x + w, y + r, r);
    ctx.lineTo(x + w, y + h - r);
    ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
    ctx.lineTo(x + r, y + h);
    ctx.arcTo(x, y + h, x, y + h - r, r);
    ctx.lineTo(x, y + r);
    ctx.arcTo(x, y, x + r, y, r);
}

function getTickInterval(dur, width) {
    const targetCount = Math.max(4, Math.floor(width / 80));
    const raw = dur / targetCount;
    // Snap to nice intervals: 0.1, 0.25, 0.5, 1, 2, 5, 10, 30, 60
    const niceIntervals = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60];
    for (const ni of niceIntervals) {
        if (ni >= raw) return ni;
    }
    return 60;
}

function formatTime(t) {
    if (t < 10) return t.toFixed(1) + 's';
    return Math.round(t) + 's';
}

// ─── Timeline Click ───
$('canvasTimeline').addEventListener('mousedown', e => {
    const wrap = $('timelineCanvasWrap');
    const rect = wrap.getBoundingClientRect();
    const cssX = e.clientX - rect.left;
    const cssY = e.clientY - rect.top;
    
    const scopeD = getScopeDuration();
    const clickTime = (cssX / rect.width) * scopeD;
    
    if (cssY < RULER_HEIGHT) {
        // Click on ruler → scrub
        // When scoped, set local time directly (no conversion)
        timeSec = clickTime;
        playing = false;
        if (engine) engine.set_playing(false);
        requestRender();
        return;
    }
    
    // Click on track → select entity
    const tracks = getVisibleTracks();
    const trackIdx = Math.floor((cssY - RULER_HEIGHT) / TRACK_HEIGHT);
    if (trackIdx >= 0 && trackIdx < tracks.length) {
        const track = tracks[trackIdx];
        let clickedEnt = null;
        for (const ent of track.entities) {
            const ls = getEntityLifespan(ent);
            if (clickTime >= ls.start && clickTime <= ls.end) {
                clickedEnt = ent; // Last matching wins (top z-index visually)
            }
        }
        if (clickedEnt) {
            selectedEntityId = clickedEnt.id;
            if (engine) engine.select_entity_v2(clickedEnt.id);
            if (!playing) requestRender();
        }
    }
});

$('canvasTimeline').addEventListener('dblclick', e => {
    const wrap = $('timelineCanvasWrap');
    const rect = wrap.getBoundingClientRect();
    const cssX = e.clientX - rect.left;
    const cssY = e.clientY - rect.top;
    
    if (cssY < RULER_HEIGHT) return;
    
    // Double click to drill
    const tracks = getVisibleTracks();
    const trackIdx = Math.floor((cssY - RULER_HEIGHT) / TRACK_HEIGHT);
    if (trackIdx >= 0 && trackIdx < tracks.length) {
        const track = tracks[trackIdx];
        const scopeD = getScopeDuration();
        const clickTime = (cssX / rect.width) * scopeD;
        
        for (const ent of track.entities) {
            const ls = getEntityLifespan(ent);
            if (clickTime >= ls.start && clickTime <= ls.end) {
                if (ent.composition) {
                    // Drill into composition
                    timelineScopePath.push({ id: timelineScope, label: timelineScope || 'Root' });
                    setRenderScope(ent.id);
                    renderBreadcrumb();
                    renderTimeline();
                }
                break;
            }
        }
    }
});

// Timeline scrub drag — RAF-debounced to prevent multiple WebGPU
// surface present() calls within a single browser frame.
let isScrubbing = false;
let scrubRAF = null;

$('canvasTimeline').addEventListener('mousedown', e => {
    const wrap = $('timelineCanvasWrap');
    const rect = wrap.getBoundingClientRect();
    const cssY = e.clientY - rect.top;
    if (cssY < RULER_HEIGHT) {
        isScrubbing = true;
    }
});

window.addEventListener('mousemove', e => {
    if (!isScrubbing) return;
    const wrap = $('timelineCanvasWrap');
    const rect = wrap.getBoundingClientRect();
    const cssX = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    const scopeD = getScopeDuration();
    // Update time immediately (timeline UI stays responsive)
    timeSec = (cssX / rect.width) * scopeD;
    playing = false;
    if (engine) engine.set_playing(false);
    // Render only once per animation frame
    if (!scrubRAF) {
        scrubRAF = requestAnimationFrame(() => {
            requestRender();
            scrubRAF = null;
        });
    }
});

window.addEventListener('mouseup', () => {
    if (isScrubbing) {
        isScrubbing = false;
        // Final render at exact drop position
        if (scrubRAF) {
            cancelAnimationFrame(scrubRAF);
            scrubRAF = null;
        }
        requestRender();
    }
});

// Listen for Wasm Native Video async decode completion
window.addEventListener('ifol_video_seeked', () => {
    if (!playing) {
        requestRender();
    }
});

function globalTimeFromLocal(localTime) {
    if (!timelineScope || !currentScene) return localTime;
    const comp = currentScene.entities.find(e => e.id === timelineScope);
    if (!comp || !comp.composition) return localTime;
    const c = comp.composition;
    const speed = c.speed || 1;
    const trimStart = c.trimStart || 0;
    const ls = comp.lifespan || { start: 0, end: 100 };
    // Inverse: contentTime = localTime * speed + trimStart → localTime = (contentTime - trimStart) / speed
    return ls.start + (localTime - trimStart) / speed;
}


// ════════════════════════════════════════════════════════════════════
// ─── TEST CASES ───
// ════════════════════════════════════════════════════════════════════

const BASE_CAM = { 
    id: "main_cam", 
    camera: { postEffects: [] }, 
    rect: { width: 1280, height: 720 },
    transform: { x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0 }, 
    lifespan: {start:0,end:100} 
};


$('selTestCase').onchange = async (e) => {
    const testId = e.target.value;
    if (!testId) return;

    // ── Inline TC20 (ShaderScope: Clipped vs Padded) ──
    if (testId === 'btnTestCase20_shader') {
        const tc20 = {
            project: { width: 1280, height: 720, fps: 30, duration: 5, name: "TC20 ShaderScope" },
            entities: [
                { id: "main_cam", camera: { resolutionWidth: 1280, resolutionHeight: 720, bgColor: [0.06,0.06,0.1,1] }, rect: { width: 1280, height: 720 }, transform: { x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0, end:5 }, layer:0 },
                { id: "bg", shapeSource: { kind:"rectangle", fillColor:[0.06,0.06,0.1,1] }, rect: { width:1280, height:720 }, transform: { x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0, end:5 }, layer:0 },
                { id: "padded_circle", shapeSource: { kind:"ellipse", fillColor:[0.2,0.6,1.0,1.0] }, rect: { width:200, height:200 }, transform: { x:400,y:300,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 },
                  materials: [{ shader_id:"glow", scope:"padded", float_uniforms:{ u0_radius:{keyframes:[{time:0,value:40}]}, u1_intensity:{keyframes:[{time:0,value:3}]} }, vec4_uniforms:{ u0_color:{keyframes:[{time:0,value:[0.3,0.7,1,1]}]} } }],
                  lifespan: { start:0, end:5 }, layer:1 },
                { id: "clipped_rect", shapeSource: { kind:"rectangle", fillColor:[1.0,0.4,0.2,1.0] }, rect: { width:200, height:120 }, transform: { x:850,y:300,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 },
                  materials: [{ shader_id:"blur", scope:"clipped", float_uniforms:{ u2_radius:{keyframes:[{time:0,value:20}]} } }],
                  lifespan: { start:0, end:5 }, layer:1 },
                { id: "label_padded", textSource: { content:"PADDED (glow overflow ✓)", fontSize:22, color:[0.5,0.9,1,1] }, rect: { width:300, height:40, fitMode:"contain" }, transform: { x:400,y:430,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0, end:5 }, layer:2 },
                { id: "label_clipped", textSource: { content:"CLIPPED (blur hard edge ✓)", fontSize:22, color:[1,0.6,0.3,1] }, rect: { width:300, height:40, fitMode:"contain" }, transform: { x:850,y:430,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0, end:5 }, layer:2 }
            ]
        };
        $('jsonEditor').value = JSON.stringify(tc20, null, 2);
        if (engine) applyJson();
        return;
    }

    // ── Inline TC21 (Multi-Camera Layer Isolation) ──
    if (testId === 'btnTestCase21') {
        const tc21 = {
            project: { width: 1280, height: 720, fps: 30, duration: 5, name: "TC21 Multi-Camera Layers" },
            entities: [
                { id: "main_cam", camera: { resolutionWidth:1280, resolutionHeight:720, bgColor:[0.05,0.05,0.12,1], renderMode:"layers", targetLayers:[0,1,2], renderOrder:0 }, rect: { width:1280, height:720 }, transform: { x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0, end:5 }, layer:0 },
                { id: "overlay_cam", camera: { resolutionWidth:1280, resolutionHeight:720, bgColor:[0,0,0,0], renderMode:"layers", targetLayers:[100], renderOrder:1 }, rect: { width:1280, height:720 }, transform: { x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0, end:5 }, layer:0 },
                { id: "bg", shapeSource: { kind:"rectangle", fillColor:[0.06,0.06,0.12,1] }, rect: { width:1280, height:720 }, transform: { x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 }, lifespan: { start:0,end:5 }, layer:0 },
                { id: "glow_circle", shapeSource: { kind:"ellipse", fillColor:[0.3,0.6,1,1] }, rect: { width:260,height:260 }, transform: { x:640,y:340,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5 },
                  materials:[{ shader_id:"glow", scope:"padded", float_uniforms:{ u0_radius:{keyframes:[{time:0,value:50}]}, u1_intensity:{keyframes:[{time:0,value:3}]} }, vec4_uniforms:{ u0_color:{keyframes:[{time:0,value:[0.2,0.5,1,1]}]} } }],
                  lifespan:{start:0,end:5}, layer:1 },
                { id: "content_text", textSource:{ content:"Content Camera (Layer 0-2) + Glow", fontSize:24, color:[1,1,1,1] }, rect:{width:500,height:50,fitMode:"contain"}, transform:{x:640,y:590,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:2 },
                { id: "overlay_frame", shapeSource:{ kind:"rectangle", fillColor:[0,0,0,0], strokeColor:[0,1,0.4,1], strokeWidth:5 }, rect:{width:280,height:280}, transform:{x:640,y:340,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:100 },
                { id: "overlay_text", textSource:{ content:"Overlay Camera (Layer 100) — No glow bleed ✓", fontSize:18, color:[0,1,0.4,1] }, rect:{width:700,height:40,fitMode:"contain"}, transform:{x:640,y:60,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:100 }
            ]
        };
        $('jsonEditor').value = JSON.stringify(tc21, null, 2);
        if (engine) applyJson();
        return;
    }

    // ── Inline TC22 (Master Camera Compositor) ──
    if (testId === 'btnTestCase22') {
        const tc22 = {
            project: { width: 1280, height: 720, fps: 30, duration: 5, name: "TC22 Master Camera" },
            entities: [
                { id: "viewport_master", camera: { resolutionWidth:1280, resolutionHeight:720, renderMode:"cameras", targetCameras:["cam_content","cam_editor"], renderOrder:0 }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:0 },
                { id: "cam_content", camera: { resolutionWidth:1280, resolutionHeight:720, bgColor:[0.05,0.05,0.15,1], renderMode:"layers", targetLayers:[0,1,2], renderOrder:0 }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:0 },
                { id: "cam_editor", camera: { resolutionWidth:1280, resolutionHeight:720, bgColor:[0,0,0,0], renderMode:"layers", targetLayers:[10], renderOrder:1 }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:0 },
                { id: "bg", shapeSource:{ kind:"rectangle", fillColor:[0.08,0.08,0.18,1] }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:0 },
                { id: "glow_ball", shapeSource:{ kind:"ellipse", fillColor:[0.2,0.5,1,1] }, rect:{width:250,height:250}, transform:{x:640,y:350,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                  materials:[{ shader_id:"glow", scope:"padded", float_uniforms:{ u0_radius:{keyframes:[{time:0,value:50}]}, u1_intensity:{keyframes:[{time:0,value:3}]} }, vec4_uniforms:{ u0_color:{keyframes:[{time:0,value:[0.3,0.6,1,1]}]} } }],
                  lifespan:{start:0,end:5}, layer:1 },
                { id: "content_lbl", textSource:{ content:"Content Camera (Layer 0-2) • Glow active", fontSize:24, color:[1,1,1,1] }, rect:{width:550,height:50,fitMode:"contain"}, transform:{x:640,y:580,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:2 },
                { id: "editor_sel", shapeSource:{ kind:"rectangle", fillColor:[0,0,0,0], strokeColor:[0,1,0.4,1], strokeWidth:5 }, rect:{width:266,height:266}, transform:{x:640,y:350,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:10 },
                { id: "editor_lbl", textSource:{ content:"Master Cam → [cam_content, cam_editor] • Editor layer=10", fontSize:18, color:[0,1,0.4,1] }, rect:{width:800,height:40,fitMode:"contain"}, transform:{x:640,y:55,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:10 }
            ]
        };
        $('jsonEditor').value = JSON.stringify(tc22, null, 2);
        if (engine) applyJson();
        return;
    }
    
    // ── Inline TC23 (Camera Track Cuts) ──
    if (testId === 'btnTestCase23') {
        const tc23 = {
            project: { width: 1280, height: 720, fps: 30, duration: 10, name: "TC23 Camera Cuts" },
            entities: [
                { id: "bg", shapeSource:{ kind:"rectangle", fillColor:[0.1,0.1,0.1,1] }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer:0 },
                { id: "content_left", shapeSource:{ kind:"rectangle", fillColor:[1,0.2,0.2,1] }, rect:{width:300,height:300}, transform:{x:300,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer:1 },
                { id: "content_right", shapeSource:{ kind:"rectangle", fillColor:[0.2,0.5,1,1] }, rect:{width:300,height:300}, transform:{x:980,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer:1 },
                
                // Track 1 (Layer 10) - Cameras with lifespan.end = -1
                { id: "cam_1", camera: { renderMode:"layers", targetLayers:[0,1], renderOrder:0 }, rect:{width:1280,height:720}, transform:{x:300,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:-1}, layer:10 },
                { id: "cam_2", camera: { renderMode:"layers", targetLayers:[0,1], renderOrder:0 }, rect:{width:1280,height:720}, transform:{x:980,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:3.5,end:-1}, layer:10 },
                { id: "cam_3", camera: { renderMode:"layers", targetLayers:[0,1], renderOrder:0 }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1.5,scaleY:1.5,anchorX:0.5,anchorY:0.5}, lifespan:{start:7.0,end:-1}, layer:10 },
                
                // Master Cam that renders whatever camera is active on Layer 10
                { id: "master_cam", camera: { renderMode:"cameras", targetCameras:["cam_1", "cam_2", "cam_3"], renderOrder:99 }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer:99 }
            ]
        };
        $('jsonEditor').value = JSON.stringify(tc23, null, 2);
        if (engine) applyJson();
        return;
    }

    // ── Inline TC24 (Blend Modes — 2-Pass Visual Validation) ──
    if (testId === 'btnTestCase24') {
        const blendModes = ["normal","multiply","screen","overlay","add","subtract","darken","lighten","soft_light","hard_light","difference"];
        const cols = 4;
        const cellW = 300, cellH = 170;
        const startX = 160, startY = 100;
        const entities = [
            { id: "main_cam", camera: { resolutionWidth:1280, resolutionHeight:720, bgColor:[0.08,0.08,0.12,1] }, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:0 },
            { id: "bg", shapeSource:{kind:"rectangle",fillColor:[0.08,0.08,0.12,1]}, rect:{width:1280,height:720}, transform:{x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:0 },
            { id: "title", textSource:{content:"TC24: Blend Modes (2-Pass GPU Composite)", fontSize:28, color:[1,1,1,1]}, rect:{width:800,height:50,fitMode:"contain"}, transform:{x:640,y:40,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:5}, layer:999 }
        ];
        blendModes.forEach((mode, i) => {
            const col = i % cols;
            const row = Math.floor(i / cols);
            const cx = startX + col * cellW;
            const cy = startY + row * cellH;
            const layerBase = (i + 1) * 10;
            // Background shape (destination)
            entities.push({
                id: `bg_${mode}`, shapeSource:{kind:"rectangle",fillColor:[0.2,0.5,1.0,1]},
                rect:{width:120,height:100}, transform:{x:cx,y:cy,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan:{start:0,end:5}, layer:layerBase
            });
            // Foreground shape (source — with blend mode applied)
            entities.push({
                id: `fg_${mode}`, shapeSource:{kind:"ellipse",fillColor:[1.0,0.3,0.2,1]},
                rect:{width:120,height:100}, transform:{x:cx+40,y:cy+20,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                visual:{opacity:0.9, blendMode:mode},
                lifespan:{start:0,end:5}, layer:layerBase + 1
            });
            // Label
            entities.push({
                id: `lbl_${mode}`, textSource:{content:mode.replace("_"," "), fontSize:14, color:[1,1,1,0.9]},
                rect:{width:200,height:25,fitMode:"contain"}, transform:{x:cx+20,y:cy+68,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan:{start:0,end:5}, layer:layerBase + 2
            });
        });
        const tc24 = {
            project: { width: 1280, height: 720, fps: 30, duration: 5, name: "TC24 Blend Modes" },
            entities
        };
        $('jsonEditor').value = JSON.stringify(tc24, null, 2);
        if (engine) applyJson();
        return;
    }

    // ── Generic: load from /tests/ folder ──
    try {
        const res = await fetch(`/tests/${testId}.json`);
        const jsonText = await res.text();
        $('jsonEditor').value = jsonText;
        if (engine) applyJson();
    } catch(err) {
        console.error("Failed to load test case", err);
    }
};

$('selViewCamera').onchange = () => {
    if (!playing) requestRender();
};

// ════════════════════════════════════════════════════════════════════
// ─── EXPORT MODAL LOGIC ───
// ════════════════════════════════════════════════════════════════════

const exportModal = $('exportModal');

$('btnExport').onclick = () => {
    exportModal.style.display = 'flex';
};

$('btnCancelExport').onclick = () => {
    exportModal.style.display = 'none';
};

$('btnConfirmExport').onclick = async () => {
    const dir = $('exportDir').value || 'C:\\Users\\abc\\Desktop';
    const filename = $('exportFilename').value || 'output.mp4';
    const codec = $('exportCodec').value || 'h264';
    const preset = $('exportPreset').value || 'medium';
    const crf = parseInt($('exportCRF').value, 10) || 23;
    const fpsOverride = parseInt($('exportFPS').value, 10) || 30;
    const widthOverride = parseInt($('exportWidth').value, 10) || 1920;
    const heightOverride = parseInt($('exportHeight').value, 10) || 1080;
    const ffmpeg_path = $('exportFFmpeg').value || '';
    
    // Combine path
    const fullPath = dir.replace(/\\/g, '/') + '/' + filename;

    // Parse current JSON from the Editor
    let sceneJson;
    try {
        sceneJson = JSON.parse($('jsonEditor').value);
    } catch(e) {
        alert("Invalid Scene JSON. Cannot export.");
        return;
    }
    
    // 1. UI Loading state
    $('btnConfirmExport').disabled = true;
    $('btnCancelExport').disabled = true;
    $('exportModalActions').style.display = 'none';
    $('exportProgressContainer').style.display = 'block';
    $('exportProgressBar').style.width = '0%';
    $('exportProgressText').textContent = 'Starting Backend Parser...';
    $('exportProgressEta').textContent = 'ETA: --';
    
    $('lblStatus').textContent = "Exporting via Backend...";
    $('lblStatus').style.color = "#f59e0b";
    
    let pollInterval;
    const cleanup = () => {
        clearInterval(pollInterval);
        exportModal.style.display = 'none';
        $('btnConfirmExport').disabled = false;
        $('btnCancelExport').disabled = false;
        $('exportModalActions').style.display = 'flex';
        $('exportProgressContainer').style.display = 'none';
    };

    try {
        // Send request to Vite Dev Server internal proxy
        const res = await fetch('/api/export', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                scene: sceneJson,
                filename: fullPath,
                codec: codec,
                preset: preset,
                crf: crf,
                fps: fpsOverride,
                width: widthOverride,
                height: heightOverride,
                ffmpeg: ffmpeg_path || undefined
            })
        });
        
        if(res.ok) {
            // Start Polling Loop Every 500ms
            pollInterval = setInterval(async () => {
                try {
                    const statusRes = await fetch('/api/export/progress');
                    if (statusRes.ok) {
                        const state = await statusRes.json();
                        
                        // Update Progress UI
                        if (state.status === 'exporting') {
                            $('exportProgressBar').style.width = `${state.percent}%`;
                            $('exportProgressText').textContent = `Rendering: ${state.frame} / ${state.total} frames (${state.percent.toFixed(1)}%) | ${state.fps.toFixed(1)} fps`;
                            $('exportProgressEta').textContent = `ETA: ${state.eta}s`;
                        } else if (state.status === 'completed') {
                            $('exportProgressBar').style.width = `100%`;
                            $('exportProgressText').textContent = `Done! Video encoded successfully.`;
                            $('exportProgressEta').textContent = `ETA: 0s`;
                            
                            clearInterval(pollInterval);
                            setTimeout(() => {
                                cleanup();
                                alert(`✅ Export successfully completed in ${state.elapsed}s!\nFile saved to:\n${fullPath}`);
                                $('lblStatus').textContent = "V4 ECS Ready";
                                $('lblStatus').style.color = "#10b981";
                            }, 500);
                        } else if (state.status === 'error') {
                            clearInterval(pollInterval);
                            cleanup();
                            alert(`❌ Export Failed!\nReason: ${state.error}`);
                            $('lblStatus').textContent = "V4 ECS Ready";
                            $('lblStatus').style.color = "#10b981";
                        }
                    }
                } catch(e) {
                    console.error("Progress poll fetch failed:", e);
                }
            }, 500);
            
            $('lblStatus').textContent = "Backend Render Active";
        } else {
            alert("Export proxy failed: " + res.statusText);
            cleanup();
        }
    } catch(err) {
        console.error(err);
        alert("Failed to connect to export proxy. Is the Vite server running via 'npm run dev'?");
        cleanup();
    }
};

// ════════════════════════════════════════════════════════════════════
// ─── SYNC SCROLL + RESIZE HANDLES ───
// ════════════════════════════════════════════════════════════════════

// Sync scroll: when scrolling labels, redraw canvas tracks
$('timelineLabelsScroll').addEventListener('scroll', () => {
    renderTimeline();
});

// Also handle mousewheel on canvas to scroll tracks
$('canvasTimeline').addEventListener('wheel', e => {
    e.preventDefault();
    const labelsScroll = $('timelineLabelsScroll');
    labelsScroll.scrollTop += e.deltaY;
    renderTimeline();
});

// ─── Horizontal Resize (between left-panel and viewport) ───
{
    const handle = $('resizeH');
    const leftPanel = $('leftPanel');
    let startX = 0, startW = 0;

    handle.addEventListener('mousedown', e => {
        e.preventDefault();
        startX = e.clientX;
        startW = leftPanel.getBoundingClientRect().width;
        handle.classList.add('active');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';

        const onMove = ev => {
            const newW = Math.max(200, Math.min(startW + (ev.clientX - startX), window.innerWidth - 300));
            leftPanel.style.width = newW + 'px';
        };
        const onUp = () => {
            handle.classList.remove('active');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
            window.removeEventListener('mousemove', onMove);
            window.removeEventListener('mouseup', onUp);
            if (!playing) renderTimeline();
        };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
    });
}

// ─── Vertical Resize (between main area and timeline) ───
{
    const handle = $('resizeV');
    const timeline = $('timelinePanel');
    let startY = 0, startH = 0;

    handle.addEventListener('mousedown', e => {
        e.preventDefault();
        startY = e.clientY;
        startH = timeline.getBoundingClientRect().height;
        handle.classList.add('active');
        document.body.style.cursor = 'row-resize';
        document.body.style.userSelect = 'none';

        const onMove = ev => {
            // Dragging up = making timeline taller, dragging down = smaller
            const newH = Math.max(80, Math.min(startH - (ev.clientY - startY), window.innerHeight - 200));
            timeline.style.height = newH + 'px';
        };
        const onUp = () => {
            handle.classList.remove('active');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
            window.removeEventListener('mousemove', onMove);
            window.removeEventListener('mouseup', onUp);
            if (!playing) renderTimeline();
        };
        window.addEventListener('mousemove', onMove);
        window.addEventListener('mouseup', onUp);
    });
}
