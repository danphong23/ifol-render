import init, { IfolRenderWeb } from 'ifol-render-wasm';
import { createTC19Json } from './tc19_super_stress.js';

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
    
    if (isEditorMode) {
        // Editor: canvas matches container pixel size exactly (no distortion)
        const cw = Math.max(1, Math.floor(container.clientWidth));
        const ch = Math.max(1, Math.floor(container.clientHeight));
        if (canvas.width !== cw || canvas.height !== ch) {
            const zoom = cam_zoom || 1.0;
            
            // Keep center point fixed during resize to prevent drift
            if (cam_x !== undefined && canvas.width > 1 && canvas.height > 1) {
                const oldViewW = canvas.width / zoom;
                const oldViewH = canvas.height / zoom;
                const newViewW = cw / zoom;
                const newViewH = ch / zoom;
                // Old center = cam_x + oldViewW/2; new cam_x = oldCenter - newViewW/2
                cam_x += (oldViewW - newViewW) / 2;
                cam_y += (oldViewH - newViewH) / 2;
            }
            
            canvas.width = cw;
            canvas.height = ch;
            engine.resize(cw, ch);
            $('lblCanvasSize').textContent = `${cw}x${ch}`;
        }
    } else {
        // Camera mode: fixed resolution matching active camera
        let camW = 1280;
        let camH = 720;
        if (currentScene && currentScene.entities) {
            const cam = currentScene.entities.find(e => e.id === "main_cam");
            if (cam && cam.rect) {
                camW = cam.rect.width;
                camH = cam.rect.height;
            }
        }
        
        if (canvas.width !== camW || canvas.height !== camH) {
            canvas.width = camW;
            canvas.height = camH;
            engine.resize(camW, camH);
            $('lblCanvasSize').textContent = `${camW}x${camH}`;
        }
    }
}

// Auto-sync canvas when container is resized (drag handles, window resize)
const _vpResizeObserver = new ResizeObserver(() => {
    if (isEditorMode && engine) {
        syncCanvasToViewport();
        if (!playing) requestRender();
    }
});
_vpResizeObserver.observe($('viewportArea'));

// Get editor camera viewport in world units
function getEditorCam() {
    const canvas = $('canvasMain');
    const zoom = cam_zoom || 1.0;
    // cam_w/cam_h = canvas pixel size / zoom
    // Resizing container → more/less world visible, zoom → magnify
    return {
        x: cam_x,
        y: cam_y,
        w: canvas.width / zoom,
        h: canvas.height / zoom,
    };
}

function requestRender() {
    if(engine) {
        syncCanvasToViewport();
        
        // When scoped into a composition, timeSec IS local time.
        // Pass it as scope_time_override so Core bypasses speed/loop/trim.
        if (timelineScope) {
            engine.set_scope_time(timeSec);
        } else {
            engine.set_scope_time(undefined);
        }
        
        if (isEditorMode) {
            const ec = getEditorCam();
            engine.render_frame_v2(
                timelineScope ? 0 : timeSec, "main_cam", true,
                ec.x, ec.y, ec.w, ec.h
            );
        } else {
            // Camera mode: no overrides
            engine.render_frame_v2(
                timelineScope ? 0 : timeSec, "main_cam", false,
                undefined, undefined, undefined, undefined
            );
        }
        renderTimeline();
    }
}

// ─── INITIALIZATION ───
async function initEngine() {
    $('lblStatus').textContent = "Downloading WASM...";
    await init();
    const canvas = $('canvasMain');
    engine = await new IfolRenderWeb(canvas, canvas.width, canvas.height, 60);
    engine.setup_builtins();
    $('lblStatus').textContent = "Pipeline Ready ✅";
    $('lblStatus').style.color = "#4ade80";
    
    // Auto-load Test Case 1
    $('btnTestCase8').click();
    $('chkEditorMode').onchange = (e) => {
        isEditorMode = e.target.checked;
        const vp = $('viewportArea');
        vp.classList.toggle('editor-mode', isEditorMode);
        vp.classList.toggle('camera-mode', !isEditorMode);
        if (!playing) requestRender();
    };
    
    $('selSelectMode').onchange = (e) => {
        if (engine) engine.set_select_mode(e.target.value);
        if (!playing) requestRender();
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
        
        // 3. Load scene into ECS (with auto-filled intrinsic)
        engine.load_scene_v2(JSON.stringify(scene));
        currentScene = scene;
        
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
$('btnInit').onclick = initEngine;
$('btnUpdateJson').onclick = applyJson;
$('btnPlay').onclick = () => { 
    playing = true; 
    lastTime = performance.now(); 
    if (engine) engine.set_playing(true);
};
$('btnStop').onclick = () => { 
    playing = false; 
    timeSec = 0; 
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
        if(cam_x === undefined) { cam_x = 0; cam_y = 0; cam_zoom = 1.0; }
        // dx/dy are canvas pixels; divide by zoom to get world units
        cam_x -= dx / (cam_zoom || 1);
        cam_y -= dy / (cam_zoom || 1);
        if(!playing) requestRender();
    }
});

$('canvasMain').addEventListener('wheel', e => {
    e.preventDefault();
    if(cam_zoom === undefined) { cam_x = 0; cam_y = 0; cam_zoom = 1.0; }
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
    if (timelineScope === null) {
        // Root: show entities without parentId OR whose parentId does NOT have a composition
        return currentScene.entities.filter(e => {
            if (e.parentId) {
                const parent = currentScene.entities.find(p => p.id === e.parentId);
                if (parent && parent.composition) return false; // hidden, inside composition
            }
            return true;
        });
    } else {
        // Scoped to composition: show children of composition
        return currentScene.entities.filter(e => e.parentId === timelineScope);
    }
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
    const entities = getVisibleEntities();
    
    for (const ent of entities) {
        const label = document.createElement('div');
        label.className = 'timeline-label';
        if (ent.composition) label.className += ' composition';
        if (ent.id === selectedEntityId) label.className += ' selected';
        
        let icon = '▪';
        if (ent.camera) icon = '📷';
        else if (ent.composition) icon = '📁';
        else if (ent.shapeSource && ent.shapeSource.kind === 'ellipse') icon = '⬤';
        
        label.textContent = `${icon} ${ent.id}`;
        label.title = ent.id;
        
        // Click to select
        label.addEventListener('click', () => {
            selectedEntityId = ent.id;
            if(engine) engine.select_entity_v2(ent.id);
            if(!playing) requestRender();
        });
        
        // Double-click to drill into composition
        if (ent.composition) {
            label.addEventListener('dblclick', () => {
                timelineScopePath.push({ id: timelineScope, label: timelineScope || 'Root' });
                setRenderScope(ent.id);
                renderBreadcrumb();
                renderTimeline();
            });
        }
        
        container.appendChild(label);
    }
}

function setRenderScope(scopeId) {
    timelineScope = scopeId;
    timeSec = 0; // Reset to start of local timeline
    playing = false;
    if (engine) {
        engine.set_playing(false);
        engine.set_render_scope(scopeId || undefined);
        if (scopeId) {
            engine.set_scope_time(0); // Start at local time 0
        } else {
            engine.set_scope_time(undefined); // Clear override
        }
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
    
    // Clip tracks below ruler
    ctx.save();
    ctx.beginPath();
    ctx.rect(0, RULER_HEIGHT, W, H - RULER_HEIGHT);
    ctx.clip();
    
    for (let i = 0; i < entities.length; i++) {
        const ent = entities[i];
        const y = trackY0 + i * TRACK_HEIGHT - scrollY;
        
        // Track background (alternating)
        ctx.fillStyle = (i % 2 === 0) ? COLORS.trackBg1 : COLORS.trackBg2;
        if (ent.id === selectedEntityId) ctx.fillStyle = COLORS.trackSelected;
        ctx.fillRect(0, y, W, TRACK_HEIGHT);
        
        // Track separator
        ctx.strokeStyle = '#2a2a2a';
        ctx.beginPath();
        ctx.moveTo(0, y + TRACK_HEIGHT);
        ctx.lineTo(W, y + TRACK_HEIGHT);
        ctx.stroke();
        
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
        
        // Bar border
        ctx.strokeStyle = 'rgba(255,255,255,0.15)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        roundRect(ctx, barX, barY, Math.max(barW, 2), barH, 3);
        ctx.stroke();
        
        // Composition icon indicator  
        if (ent.composition) {
            ctx.fillStyle = '#fff';
            ctx.font = 'bold 9px system-ui';
            ctx.textAlign = 'left';
            ctx.fillText('⟳', barX + 3, barY + barH - 2);
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
                        const track = mat.float_uniforms[key];
                        if (track && track.keyframes) {
                            for (const kf of track.keyframes) {
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
    const entities = getVisibleEntities();
    const trackIdx = Math.floor((cssY - RULER_HEIGHT) / TRACK_HEIGHT);
    if (trackIdx >= 0 && trackIdx < entities.length) {
        const ent = entities[trackIdx];
        selectedEntityId = ent.id;
        if(engine) engine.select_entity_v2(ent.id);
        if(!playing) requestRender();
    }
});

$('canvasTimeline').addEventListener('dblclick', e => {
    const wrap = $('timelineCanvasWrap');
    const rect = wrap.getBoundingClientRect();
    const cssY = e.clientY - rect.top;
    
    if (cssY < RULER_HEIGHT) return;
    
    const entities = getVisibleEntities();
    const trackIdx = Math.floor((cssY - RULER_HEIGHT) / TRACK_HEIGHT);
    if (trackIdx >= 0 && trackIdx < entities.length) {
        const ent = entities[trackIdx];
        if (ent.composition) {
            // Drill into composition
            timelineScopePath.push({ id: timelineScope, label: timelineScope || 'Root' });
            setRenderScope(ent.id);
            renderBreadcrumb();
            renderTimeline();
        }
    }
});

// Timeline scrub drag
let isScrubbing = false;
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
    const clickTime = (cssX / rect.width) * scopeD;
    // When scoped, set local time directly
    timeSec = clickTime;
    playing = false;
    if (engine) engine.set_playing(false);
    requestRender();
});

window.addEventListener('mouseup', () => { isScrubbing = false; });

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

$('btnTestCase1').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.15, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "box_red", shapeSource: { kind: "rectangle", fillColor: [1.0, 0.2, 0.3, 1.0] }, rect: {width:200,height:150}, transform: {x:200,y:200,rotation:0.2,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 1 },
            { id: "circle_blue", shapeSource: { kind: "ellipse", fillColor: [0.2, 0.6, 1.0, 1.0] }, rect: {width:200,height:200}, transform: {x:600,y:200,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 2 },
        ]
    }, null, 2);
    if(engine) applyJson();
};

$('btnTestCase2').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "parent", shapeSource: { kind: "rectangle", fillColor: [0.4, 0.4, 0.8, 1.0] }, rect: {width:300,height:300}, transform: {x:400,y:200,rotation:0.5,scaleX:1.5,scaleY:1.5,anchorX:0.5,anchorY:0.5}, layer: 1 },
            { id: "child", parentId: "parent", shapeSource: { kind: "ellipse", fillColor: [0.8, 0.8, 0.2, 1.0] }, rect: {width:100,height:100}, transform: {x:200,y:100,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 2 }
        ]
    }, null, 2);
    if(engine) applyJson();
};

$('btnTestCase3').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "anim_box", shapeSource: { kind: "rectangle", fillColor: [0.2, 0.8, 0.5, 1.0] }, rect: {width:150,height:150}, transform: {x:100,y:300,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 1,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [ {time:0, value:100}, {time:5, value:1000} ] } },
                    { target: "transformRotation", track: { keyframes: [ {time:0, value:0}, {time:5, value:12.56} ] } }
                ]}
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

$('btnTestCase4').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "loop_comp", composition: { duration: {manual: 2.0}, speed: 2.0, loopMode: "loop", trimStart: 0.0 }, layer: 1 },
            { id: "moving_ball", parentId: "loop_comp", shapeSource: { kind: "ellipse", fillColor: [1.0, 0.4, 0.1, 1.0] }, rect: {width:100,height:100}, transform: {x:200,y:300,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 2,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [ {time:0, value:200}, {time:2, value:800} ] } }
                ]}
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

$('btnTestCase5').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "grandparent", shapeSource: { kind: "rectangle", fillColor: [1.0, 1.0, 1.0, 1.0] }, visual: { opacity: 0.5, blendMode: "normal" }, rect: {width:400,height:400}, transform: {x:640,y:360,rotation:0.2,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 1 },
            { id: "parent", parentId: "grandparent", shapeSource: { kind: "rectangle", fillColor: [1.0, 0.0, 0.0, 1.0] }, visual: { opacity: 0.5, blendMode: "normal" }, rect: {width:200,height:200}, transform: {x:200,y:200,rotation:0.5,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 2 },
            { id: "child", parentId: "parent", shapeSource: { kind: "ellipse", fillColor: [0.0, 1.0, 0.0, 1.0] }, visual: { opacity: 1.0, blendMode: "normal" }, rect: {width:100,height:100}, transform: {x:100,y:100,rotation:-0.7,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 3 }
        ]
    }, null, 2);
    if(engine) applyJson();
};

$('btnTestCase6').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            { id: "main_cam", camera: { postEffects: [] }, rect: { width: 1280, height: 720 }, transform: { x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0 }, lifespan: {start:0,end:100},
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [ {time:0, value:0}, {time:4, value:300} ] } },
                    { target: "rectWidth", track: { keyframes: [ {time:0, value:1280}, {time:4, value:640} ] } },
                    { target: "rectHeight", track: { keyframes: [ {time:0, value:720}, {time:4, value:360} ] } }
                ]}
            },
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.2, 0.2, 0.2, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "target_box", shapeSource: { kind: "rectangle", fillColor: [0.9, 0.4, 0.1, 1.0] }, rect: {width:200,height:200}, transform: {x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 1 }
        ]
    }, null, 2);
    if(engine) applyJson();
};

$('btnTestCase7').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { id: "morph_box", shapeSource: { kind: "rectangle", fillColor: [1.0, 1.0, 1.0, 1.0] }, visual: { opacity: 1.0, blendMode: "normal" }, rect: {width:100,height:100}, transform: {x:200,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, layer: 1,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [ {time:0, value:200}, {time:3, value:1000} ] } },
                    { target: "transformRotation", track: { keyframes: [ {time:0, value:0}, {time:3, value:6.28} ] } },
                    { target: "transformScaleX", track: { keyframes: [ {time:0, value:1}, {time:1.5, value:3}, {time:3, value:1} ] } },
                    { target: "transformScaleY", track: { keyframes: [ {time:0, value:1}, {time:1.5, value:0.5}, {time:3, value:2} ] } },
                    { target: "colorR", track: { keyframes: [ {time:0, value:1}, {time:3, value:0} ] } },
                    { target: "colorG", track: { keyframes: [ {time:0, value:0}, {time:3, value:1} ] } },
                    { target: "colorB", track: { keyframes: [ {time:0, value:0}, {time:3, value:1} ] } },
                ]}
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC8: Lifespan Staggered ───
$('btnTestCase8').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.08, 0.08, 0.12, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            { 
                id: "early_box", 
                shapeSource: { kind: "rectangle", fillColor: [1.0, 0.3, 0.3, 1.0] }, 
                rect: {width:180,height:120}, 
                transform: {x:200,y:200,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, 
                lifespan: {start:0, end:3},
                layer: 1
            },
            { 
                id: "mid_circle", 
                shapeSource: { kind: "ellipse", fillColor: [0.3, 0.6, 1.0, 1.0] }, 
                rect: {width:150,height:150}, 
                transform: {x:500,y:200,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, 
                lifespan: {start:2, end:6},
                layer: 2
            },
            { 
                id: "late_box", 
                shapeSource: { kind: "rectangle", fillColor: [0.3, 1.0, 0.4, 1.0] }, 
                rect: {width:180,height:120}, 
                transform: {x:800,y:200,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, 
                lifespan: {start:5, end:10},
                layer: 3
            },
            { 
                id: "full_bar", 
                shapeSource: { kind: "rectangle", fillColor: [1.0, 0.8, 0.2, 1.0] }, 
                rect: {width:200,height:60}, 
                transform: {x:100,y:500,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, 
                lifespan: {start:0, end:10},
                layer: 4,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:100}, {time:10, value:1100}] } }
                ]}
            },
            {
                id: "blink_dot",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 0.0, 1.0, 1.0] },
                rect: {width:60,height:60},
                transform: {x:640,y:400,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:1, end:2},
                layer: 5
            },
            {
                id: "blink_dot_2",
                shapeSource: { kind: "ellipse", fillColor: [0.0, 1.0, 1.0, 1.0] },
                rect: {width:60,height:60},
                transform: {x:640,y:400,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:3, end:4},
                layer: 5
            },
            {
                id: "blink_dot_3",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 1.0, 0.0, 1.0] },
                rect: {width:60,height:60},
                transform: {x:640,y:400,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:7, end:9},
                layer: 5
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC9: Composition Timeline (Rich) ───
$('btnTestCase9').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.06, 0.06, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // ── Static reference (always visible at root) ──
            { id: "ref_marker", shapeSource: { kind: "rectangle", fillColor: [0.5, 0.5, 0.2, 1.0] }, rect: {width:60,height:60}, transform: {x:640,y:50,rotation:0.78,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan: {start:0,end:100}, layer: 1 },
            
            // ══════ COMP A: "intro" — loop 4s, trimStart=0.5 ══════
            {
                id: "comp_intro",
                composition: { duration: {manual: 4.0}, speed: 1.0, loopMode: "loop", trimStart: 0.5 },
                transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0},
                lifespan: {start:0, end:30},
                layer: 2
            },
            // comp_intro children (content_time 0..4, trimStart shifts by 0.5)
            {
                id: "intro_flash",
                parentId: "comp_intro",
                shapeSource: { kind: "rectangle", fillColor: [1.0, 1.0, 1.0, 0.8] },
                rect: {width:1280,height:720},
                transform: {x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:0.3},
                layer: 3
            },
            {
                id: "intro_title",
                parentId: "comp_intro",
                shapeSource: { kind: "rectangle", fillColor: [0.9, 0.2, 0.4, 1.0] },
                rect: {width:300,height:60},
                transform: {x:640,y:200,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0.2, end:3.0},
                layer: 4,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0.2, value:200}, {time:1.0, value:640}, {time:2.5, value:640}, {time:3.0, value:1100}] } },
                    { target: "opacity", track: { keyframes: [{time:0.2, value:0}, {time:0.6, value:1}, {time:2.5, value:1}, {time:3.0, value:0}] } }
                ]}
            },
            {
                id: "intro_circle",
                parentId: "comp_intro",
                shapeSource: { kind: "ellipse", fillColor: [0.3, 0.8, 1.0, 1.0] },
                rect: {width:80,height:80},
                transform: {x:640,y:400,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:1.0, end:4.0},
                layer: 5,
                animation: { floatTracks: [
                    { target: "transformY", track: { keyframes: [{time:1, value:500}, {time:2, value:300}, {time:3, value:400}, {time:4, value:200}] } },
                    { target: "transformScaleX", track: { keyframes: [{time:1, value:0.5}, {time:2.5, value:2.0}, {time:4, value:1}] } },
                    { target: "transformScaleY", track: { keyframes: [{time:1, value:0.5}, {time:2.5, value:2.0}, {time:4, value:1}] } },
                    { target: "transformRotation", track: { keyframes: [{time:1, value:0}, {time:4, value:6.28}] } }
                ]}
            },
            
            // ══════ COMP B: "main" — pingPong 5s, speed=1.5, lifespan 2-50 ══════
            {
                id: "comp_main",
                composition: { duration: {manual: 5.0}, speed: 1.5, loopMode: "pingPong", trimStart: 0.0 },
                transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0},
                lifespan: {start:2, end:50},
                layer: 10
            },
            // comp_main children (content_time 0..5, speed 1.5x, pingPong)
            {
                id: "main_bg_bar",
                parentId: "comp_main",
                shapeSource: { kind: "rectangle", fillColor: [0.15, 0.15, 0.25, 1.0] },
                rect: {width:1000,height:8},
                transform: {x:640,y:550,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:5},
                layer: 11
            },
            {
                id: "main_slider",
                parentId: "comp_main",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 0.6, 0.1, 1.0] },
                rect: {width:40,height:40},
                transform: {x:140,y:550,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:5},
                layer: 12,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:140}, {time:5, value:1140}] } }
                ]}
            },
            {
                id: "main_box_a",
                parentId: "comp_main",
                shapeSource: { kind: "rectangle", fillColor: [0.2, 0.9, 0.3, 1.0] },
                rect: {width:120,height:90},
                transform: {x:300,y:450,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0.5, end:3.0},
                layer: 13,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0.5, value:200}, {time:3, value:700}] } },
                    { target: "transformRotation", track: { keyframes: [{time:0.5, value:0}, {time:3, value:1.57}] } },
                    { target: "colorR", track: { keyframes: [{time:0.5, value:0.2}, {time:3, value:1}] } },
                    { target: "colorG", track: { keyframes: [{time:0.5, value:0.9}, {time:3, value:0.3}] } }
                ]}
            },
            {
                id: "main_box_b",
                parentId: "comp_main",
                shapeSource: { kind: "rectangle", fillColor: [0.8, 0.2, 0.9, 1.0] },
                rect: {width:100,height:100},
                transform: {x:800,y:350,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:2.0, end:5.0},
                layer: 14,
                animation: { floatTracks: [
                    { target: "transformY", track: { keyframes: [{time:2, value:300}, {time:3.5, value:500}, {time:5, value:350}] } },
                    { target: "opacity", track: { keyframes: [{time:2, value:0}, {time:2.5, value:1}, {time:4.5, value:1}, {time:5, value:0}] } }
                ]}
            },
            {
                id: "main_late_dot",
                parentId: "comp_main",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 1.0, 0.3, 1.0] },
                rect: {width:50,height:50},
                transform: {x:500,y:380,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:3.5, end:4.8},
                layer: 15,
                animation: { floatTracks: [
                    { target: "transformScaleX", track: { keyframes: [{time:3.5, value:0.1}, {time:4.0, value:2}, {time:4.8, value:0.1}] } },
                    { target: "transformScaleY", track: { keyframes: [{time:3.5, value:0.1}, {time:4.0, value:2}, {time:4.8, value:0.1}] } }
                ]}
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC10: Nested Compositions (3 levels, complex lifespans & trim) ───
$('btnTestCase10').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {},
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.04, 0.04, 0.08, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // ── Root-level entities with different lifespans ──
            { id: "ref_dot", shapeSource: { kind: "ellipse", fillColor: [1.0, 0.2, 0.2, 1.0] }, rect: {width:24,height:24}, transform: {x:40,y:40,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan: {start:0,end:80}, layer: 1 },
            { id: "ref_bar", shapeSource: { kind: "rectangle", fillColor: [0.4, 0.4, 0.4, 0.2] }, rect: {width:1280,height:2}, transform: {x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan: {start:0,end:80}, layer: 0 },
            
            // ══════════════════════════════════════════════════════════════
            // COMP A: once, 6s, trimStart=1.5 (plays content from 1.5→6.0), ls 0-40
            //   → Root t=0: content=1.5, Root t=4.5: content=6.0 (end, freezes)
            //   → A_early (ls 0-2): only visible briefly (content 1.5-2.0) = first 0.5s at root
            //   → A_mid (ls 1.5-4.5): visible from root t=0 to t=3
            //   → A_late (ls 3.5-6): visible from root t=2 to t=4.5
            // ══════════════════════════════════════════════════════════════
            {
                id: "comp_A",
                composition: { duration: {manual: 6.0}, speed: 1.0, loopMode: "once", trimStart: 1.5 },
                transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0},
                lifespan: {start:0, end:40},
                layer: 2
            },
            {
                id: "A_early",
                parentId: "comp_A",
                shapeSource: { kind: "rectangle", fillColor: [1.0, 0.9, 0.3, 1.0] },
                rect: {width:200,height:40},
                transform: {x:640,y:100,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:2.0},
                layer: 3,
                animation: { floatTracks: [
                    { target: "opacity", track: { keyframes: [{time:0, value:0}, {time:0.5, value:1}, {time:1.5, value:1}, {time:2, value:0}] } }
                ]}
            },
            {
                id: "A_mid",
                parentId: "comp_A",
                shapeSource: { kind: "rectangle", fillColor: [0.3, 0.8, 1.0, 1.0] },
                rect: {width:300,height:60},
                transform: {x:640,y:200,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:1.5, end:4.5},
                layer: 4,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:1.5, value:200}, {time:3, value:640}, {time:4.5, value:1080}] } },
                    { target: "transformRotation", track: { keyframes: [{time:1.5, value:-0.3}, {time:4.5, value:0.3}] } }
                ]}
            },
            {
                id: "A_late",
                parentId: "comp_A",
                shapeSource: { kind: "ellipse", fillColor: [0.9, 0.3, 0.7, 1.0] },
                rect: {width:80,height:80},
                transform: {x:900,y:350,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:3.5, end:6.0},
                layer: 5,
                animation: { floatTracks: [
                    { target: "transformScaleX", track: { keyframes: [{time:3.5, value:0.2}, {time:5, value:1.5}, {time:6, value:0.5}] } },
                    { target: "transformScaleY", track: { keyframes: [{time:3.5, value:0.2}, {time:5, value:1.5}, {time:6, value:0.5}] } }
                ]}
            },
            
            // ══════════════════════════════════════════════════════════════
            // COMP B: loop, 7s, speed=1.5, ls 5-55 — contains nested COMP C
            //   → B_flash (ls 0-0.3): quick flash at start of each loop
            //   → B_box_early (ls 0.5-3): early part of each loop
            //   → B_box_late (ls 4-6.5): late part of each loop
            //   → comp_C: nested inside B (ls 1-4.5 in B's content time)
            // ══════════════════════════════════════════════════════════════
            {
                id: "comp_B",
                composition: { duration: {manual: 7.0}, speed: 1.5, loopMode: "loop", trimStart: 0.0 },
                transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0},
                lifespan: {start:5, end:55},
                layer: 10
            },
            {
                id: "B_bg",
                parentId: "comp_B",
                shapeSource: { kind: "rectangle", fillColor: [0.08, 0.12, 0.2, 0.6] },
                rect: {width:900,height:400},
                transform: {x:640,y:400,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:7},
                layer: 11
            },
            {
                id: "B_slider",
                parentId: "comp_B",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 0.6, 0.1, 1.0] },
                rect: {width:30,height:30},
                transform: {x:190,y:560,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:7},
                layer: 15,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:190}, {time:7, value:1090}] } }
                ]}
            },
            {
                id: "B_flash",
                parentId: "comp_B",
                shapeSource: { kind: "rectangle", fillColor: [1.0, 1.0, 1.0, 0.7] },
                rect: {width:900,height:400},
                transform: {x:640,y:400,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:0.3},
                layer: 16
            },
            {
                id: "B_box_early",
                parentId: "comp_B",
                shapeSource: { kind: "rectangle", fillColor: [0.2, 0.9, 0.4, 1.0] },
                rect: {width:120,height:90},
                transform: {x:400,y:450,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0.5, end:3.0},
                layer: 12,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0.5, value:250}, {time:3, value:700}] } },
                    { target: "transformRotation", track: { keyframes: [{time:0.5, value:0}, {time:3, value:1.57}] } }
                ]}
            },
            {
                id: "B_box_late",
                parentId: "comp_B",
                shapeSource: { kind: "rectangle", fillColor: [0.8, 0.2, 0.9, 1.0] },
                rect: {width:100,height:100},
                transform: {x:800,y:350,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:4.0, end:6.5},
                layer: 13,
                animation: { floatTracks: [
                    { target: "transformY", track: { keyframes: [{time:4, value:300}, {time:5.5, value:500}, {time:6.5, value:350}] } },
                    { target: "opacity", track: { keyframes: [{time:4, value:0}, {time:4.5, value:1}, {time:6, value:1}, {time:6.5, value:0}] } }
                ]}
            },
            
            // ══════════════════════════════════════════════════════════════
            // COMP C: nested in comp_B — once, 4s, trimStart=0.8, ls 1-4.5 (in B's time)
            //   → Plays content from 0.8→4.0 (3.2s of content)
            //   → C_circle (ls 0-4): partially visible (starts from content 0.8)
            //   → C_square (ls 1.0-3.5): appears at content 1.0, fades out at 3.5
            //   → C_dot_brief (ls 2.0-2.8): only 0.8s of visibility
            // ══════════════════════════════════════════════════════════════
            {
                id: "comp_C",
                parentId: "comp_B",
                composition: { duration: {manual: 4.0}, speed: 1.0, loopMode: "once", trimStart: 0.8 },
                transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0},
                lifespan: {start:1, end:4.5},
                layer: 20
            },
            {
                id: "C_circle",
                parentId: "comp_C",
                shapeSource: { kind: "ellipse", fillColor: [0.3, 1.0, 0.8, 1.0] },
                rect: {width:70,height:70},
                transform: {x:640,y:450,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:0, end:4},
                layer: 21,
                animation: { floatTracks: [
                    { target: "transformX", track: { keyframes: [{time:0, value:350}, {time:2, value:640}, {time:4, value:930}] } },
                    { target: "transformY", track: { keyframes: [{time:0, value:350}, {time:2, value:500}, {time:4, value:350}] } }
                ]}
            },
            {
                id: "C_square",
                parentId: "comp_C",
                shapeSource: { kind: "rectangle", fillColor: [1.0, 0.5, 0.2, 1.0] },
                rect: {width:50,height:50},
                transform: {x:640,y:380,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:1.0, end:3.5},
                layer: 22,
                animation: { floatTracks: [
                    { target: "transformRotation", track: { keyframes: [{time:1, value:0}, {time:3.5, value:6.28}] } },
                    { target: "opacity", track: { keyframes: [{time:1, value:0}, {time:1.5, value:1}, {time:3, value:1}, {time:3.5, value:0}] } }
                ]}
            },
            {
                id: "C_dot_brief",
                parentId: "comp_C",
                shapeSource: { kind: "ellipse", fillColor: [1.0, 1.0, 0.3, 1.0] },
                rect: {width:20,height:20},
                transform: {x:500,y:420,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                lifespan: {start:2.0, end:2.8},
                layer: 23
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC11: Easing via CubicBezier (Core primitive + Frontend presets) ───
// Frontend defines presets as cubic-bezier control points.
// Core only knows: hold, linear, cubic_bezier, bezier.
const EASING_PRESETS = {
    hold:       { type: "hold" },
    linear:     { type: "linear" },
    ease_in:    { type: "cubic_bezier", x1: 0.42, y1: 0.0,  x2: 1.0,  y2: 1.0 },
    ease_out:   { type: "cubic_bezier", x1: 0.0,  y1: 0.0,  x2: 0.58, y2: 1.0 },
    ease_in_out:{ type: "cubic_bezier", x1: 0.42, y1: 0.0,  x2: 0.58, y2: 1.0 },
    ease_in_quad:  { type: "cubic_bezier", x1: 0.55, y1: 0.085, x2: 0.68, y2: 0.53 },
    ease_out_quad: { type: "cubic_bezier", x1: 0.25, y1: 0.46,  x2: 0.45, y2: 0.94 },
    ease_in_out_cubic: { type: "cubic_bezier", x1: 0.65, y1: 0.0, x2: 0.35, y2: 1.0 },
    ease_in_back:  { type: "cubic_bezier", x1: 0.6, y1: -0.28, x2: 0.735, y2: 0.045 },
    ease_out_back: { type: "cubic_bezier", x1: 0.175, y1: 0.885, x2: 0.32, y2: 1.275 },
};

$('btnTestCase11').onclick = () => {
    const presetNames = Object.keys(EASING_PRESETS);
    const colors = [
        [0.5, 0.5, 0.5, 1.0], [1.0, 1.0, 1.0, 1.0],
        [1.0, 0.3, 0.3, 1.0], [0.3, 1.0, 0.3, 1.0],
        [0.3, 0.5, 1.0, 1.0], [0.9, 0.6, 0.1, 1.0],
        [0.2, 0.9, 0.7, 1.0], [0.7, 0.3, 1.0, 1.0],
        [1.0, 0.8, 0.2, 1.0], [1.0, 0.3, 0.9, 1.0],
    ];
    
    const entities = [
        BASE_CAM,
        { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.06, 0.06, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
        { id: "line_start", shapeSource: { kind: "rectangle", fillColor: [0.3, 0.3, 0.3, 0.5] }, rect: {width:2,height:650}, transform: {x:100,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan: {start:0,end:10}, layer: 0 },
        { id: "line_end", shapeSource: { kind: "rectangle", fillColor: [0.3, 0.3, 0.3, 0.5] }, rect: {width:2,height:650}, transform: {x:1100,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan: {start:0,end:10}, layer: 0 },
    ];
    
    presetNames.forEach((name, i) => {
        const y = 60 + i * 65;
        entities.push({
            id: `ball_${name}`,
            shapeSource: { kind: "ellipse", fillColor: colors[i % colors.length] },
            rect: {width:30, height:30},
            transform: {x:100, y, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
            lifespan: {start:0, end:10},
            layer: i + 1,
            animation: { floatTracks: [
                { target: "transformX", track: { keyframes: [
                    {time: 0, value: 100, interpolation: EASING_PRESETS[name]},
                    {time: 4, value: 1100, interpolation: {type: "hold"}}
                ]}}
            ]}
        });
    });
    
    $('jsonEditor').value = JSON.stringify({ assets: {}, entities }, null, 2);
    if(engine) applyJson();
};

// ─── TC12: Image Loading & FitMode Comparison ───
$('btnTestCase12').onclick = () => {
    // 1. Local image loaded from the backend (Vite dev server serving the web directory)
    const localImgUrl = "./examples/cmt_0.png";
    
    // 2. External image loaded from the internet
    const webImgUrl = "https://picsum.photos/400";
    
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "photo_local": { type: "image", url: localImgUrl },
            "photo_web": { type: "image", url: webImgUrl }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.08, 0.08, 0.12, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // ── STRETCH (Using local backend image) ──
            // Blue outline = Rect boundary (300x300)
            { id: "frame_stretch", shapeSource: { kind: "rectangle", fillColor: [0.2,0.3,0.8,0.2] }, rect: {width:300,height:300}, transform: {x:200,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer: 1 },
            {
                id: "img_stretch",
                imageSource: { assetId: "photo_local" },
                rect: {width:300, height:300, fitMode: "stretch"},
                transform: {x:200, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 2
            },
            
            // ── CONTAIN (Using local backend image) ──
            { id: "frame_contain", shapeSource: { kind: "rectangle", fillColor: [0.2,0.8,0.3,0.2] }, rect: {width:300,height:300}, transform: {x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer: 1 },
            {
                id: "img_contain",
                imageSource: { assetId: "photo_local" },
                rect: {width:300, height:300, fitMode: "contain"},
                transform: {x:640, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 2
            },
            
            // ── COVER (Using internet image) ──
            { id: "frame_cover", shapeSource: { kind: "rectangle", fillColor: [0.8,0.3,0.2,0.2] }, rect: {width:300,height:300}, transform: {x:1080,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer: 1 },
            {
                id: "img_cover",
                imageSource: { assetId: "photo_web" },
                rect: {width:300, height:300, fitMode: "cover"},
                transform: {x:1080, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 2
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC13: Image Alignment (alignX / alignY) ───
$('btnTestCase13').onclick = () => {
    const localImgUrl = "./examples/cmt_0.png";
    const webImgUrl = "https://picsum.photos/400";
    
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "photo_local": { type: "image", url: localImgUrl },
            "photo_web": { type: "image", url: webImgUrl }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.15, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // CONTAIN - Top Left
            { id: "frame_c1", shapeSource: { kind: "rectangle", fillColor: [0.2,0.8,0.3,0.2] }, rect: {width:300,height:300}, transform: {x:300,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer: 1 },
            {
                id: "img_contain_tl",
                imageSource: { assetId: "photo_local" },
                rect: {width:300, height:300, fitMode: "contain", alignX: 0.0, alignY: 0.0},
                transform: {x:300, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 2
            },
            
            // CONTAIN - Bottom Right
            { id: "frame_c2", shapeSource: { kind: "rectangle", fillColor: [0.2,0.8,0.3,0.2] }, rect: {width:300,height:300}, transform: {x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer: 1 },
            {
                id: "img_contain_br",
                imageSource: { assetId: "photo_local" },
                rect: {width:300, height:300, fitMode: "contain", alignX: 1.0, alignY: 1.0},
                transform: {x:640, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 2
            },
            
            // COVER - Left Aligned (Shift UV)
            { id: "frame_cv1", shapeSource: { kind: "rectangle", fillColor: [0.8,0.3,0.2,0.2] }, rect: {width:200,height:500}, transform: {x:1000,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5}, lifespan:{start:0,end:10}, layer: 1 },
            {
                id: "img_cover_l",
                // Use a square image, but Cover into a tall rectangle → crops sides. 
                // alignX=0.0 means crop only the right side, keep the left edge.
                imageSource: { assetId: "photo_web" },
                rect: {width:200, height:500, fitMode: "cover", alignX: 0.0, alignY: 0.5},
                transform: {x:1000, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 2
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC14: Blur and Glow Effects ───
$('btnTestCase14').onclick = () => {
    // Requires Blur and Glow shaders
    const webImgUrl = "https://picsum.photos/400";
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "blur_shader": { type: "shader", url: "./examples/blur.wgsl" }, // uses built-in or loaded blur
            "glow_shader": { type: "shader", url: "./examples/glow.wgsl" },
            "photo": { type: "image", url: webImgUrl },
            "inter": { type: "font", url: "https://fonts.gstatic.com/s/inter/v13/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuGKYMZhrib2Bg-4.ttf" }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.1, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // 1. Text with Glow (Padded - defaults to bleeding outside original rect)
            {
                id: "text_glow_padded",
                textSource: { content: "PADDED GLOW", fontSize: 60, color: [1, 1, 1, 1], font: "inter" },
                rect: { width: 500, height: 200, fitMode: "contain" },
                transform: {x:320, y:200, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [
                    {
                        shader_id: "glow",
                        scope: "padded",
                        float_uniforms: {
                            u0_r: { keyframes: [{time: 0, value: 0.0}] },
                            u1_g: { keyframes: [{time: 0, value: 0.8}] },
                            u2_b: { keyframes: [{time: 0, value: 1.0}] },
                            u3_a: { keyframes: [{time: 0, value: 1.0}] },
                            u4_size: { keyframes: [{time: 0, value: 15.0}] },
                            u5_intensity: { keyframes: [{time: 0, value: 2.0}] },
                            u6_pad1: { keyframes: [{time: 0, value: 0.0}] },
                            u7_pad2: { keyframes: [{time: 0, value: 0.0}] }
                        }
                    }
                ],
                lifespan: {start:0, end:10},
                layer: 1
            },
            
            // 2. Text with Glow (Masked - Inner glow, does not bleed)
            {
                id: "text_glow_masked",
                textSource: { content: "MASKED GLOW", fontSize: 60, color: [1, 1, 1, 1], font: "inter" },
                rect: { width: 500, height: 200, fitMode: "contain" },
                transform: {x:320, y:520, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [
                    {
                        shader_id: "glow",
                        scope: "masked", // Output is multiplied by the original alpha mask!
                        float_uniforms: {
                            u0_r: { keyframes: [{time: 0, value: 1.0}] },
                            u1_g: { keyframes: [{time: 0, value: 0.8}] },
                            u2_b: { keyframes: [{time: 0, value: 0.0}] },
                            u3_a: { keyframes: [{time: 0, value: 1.0}] },
                            u4_size: { keyframes: [{time: 0, value: 30.0}] },
                            u5_intensity: { keyframes: [{time: 0, value: 4.0}] },
                            u6_pad1: { keyframes: [{time: 0, value: 0.0}] },
                            u7_pad2: { keyframes: [{time: 0, value: 0.0}] }
                        }
                    }
                ],
                lifespan: {start:0, end:10},
                layer: 1
            },

            // 3. Image with Blur
            {
                id: "img_blur",
                imageSource: { assetId: "photo" },
                rect: {width: 300, height: 300, fitMode: "stretch"},
                transform: {x:960, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [
                    {
                        shader_id: "blur",
                        scope: "padded", // Let the blur bleed outside
                        float_uniforms: {
                            u0_dx: { keyframes: [{time: 0, value: 1.0}] },
                            u1_dy: { keyframes: [{time: 0, value: 1.0}] },
                            u2_radius: { keyframes: [{time: 0, value: 0}, {time: 2, value: 20}, {time: 4, value: 0}] },
                            u3_texel: { keyframes: [{time: 0, value: 0.003}] }
                        }
                    }
                ],
                lifespan: {start:0, end:10},
                layer: 1
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC15: Drop Shadow Matrix ───
$('btnTestCase15').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "shadow_shader": { type: "shader", url: "./examples/drop_shadow.wgsl" },
            "inter": { type: "font", url: "https://fonts.gstatic.com/s/inter/v13/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuGKYMZhrib2Bg-4.ttf" }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.9, 0.9, 0.9, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            {
                id: "rect_shadow",
                shapeSource: { kind: "rectangle", fillColor: [0.9, 0.2, 0.2, 1] },
                rect: { width: 200, height: 200 },
                transform: {x:640, y:360, rotation: 0.5, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [
                    {
                        shader_id: "drop_shadow",
                        float_uniforms: {
                            u0_r: { keyframes: [{time: 0, value: 0.0}] },
                            u1_g: { keyframes: [{time: 0, value: 0.0}] },
                            u2_b: { keyframes: [{time: 0, value: 0.0}] },
                            u3_a: { keyframes: [{time: 0, value: 0.8}] },
                            u4_offset_x: { keyframes: [{time: 0, value: 20.0}] },
                            u5_offset_y: { keyframes: [{time: 0, value: 30.0}] },
                            u6_blur: { keyframes: [{time: 0, value: 15.0}] },
                            u7_pad: { keyframes: [{time: 0, value: 0.0}] }
                        }
                    }
                ],
                animation: {
                    floatTracks: [
                        {
                            target: "transformRotation",
                            track: {
                                keyframes: [
                                    {time: 0, value: 0},
                                    {time: 5, value: 6.28}
                                ]
                            }
                        }
                    ]
                },
                lifespan: {start:0, end:10},
                layer: 1
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC16: Multi Effect Chain ───
$('btnTestCase16').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "shadow_shader": { type: "shader", url: "./examples/drop_shadow.wgsl" },
            "glow_shader": { type: "shader", url: "./examples/glow.wgsl" },
            "inter": { type: "font", url: "https://fonts.gstatic.com/s/inter/v13/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuGKYMZhrib2Bg-4.ttf" }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.15, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            {
                id: "text_chain",
                textSource: { content: "CHAINED EFFECTS", fontSize: 80, color: [1, 1, 1, 1], font: "inter" },
                rect: { width: 800, height: 200, fitMode: "contain" },
                transform: {x:640, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [
                    {
                        shader_id: "glow",
                        float_uniforms: {
                            u0_r: { keyframes: [{time: 0, value: 1.0}] },
                            u1_g: { keyframes: [{time: 0, value: 0.2}] },
                            u2_b: { keyframes: [{time: 0, value: 0.8}] },
                            u3_a: { keyframes: [{time: 0, value: 1.0}] },
                            u4_size: { keyframes: [{time: 0, value: 10.0}] },
                            u5_intensity: { keyframes: [{time: 0, value: 1.5}] },
                            u6_pad: { keyframes: [{time: 0, value: 0.0}] },
                            u7_pad: { keyframes: [{time: 0, value: 0.0}] }
                        }
                    },
                    {
                        shader_id: "drop_shadow",
                        float_uniforms: {
                            u0_r: { keyframes: [{time: 0, value: 0.0}] },
                            u1_g: { keyframes: [{time: 0, value: 0.0}] },
                            u2_b: { keyframes: [{time: 0, value: 0.0}] },
                            u3_a: { keyframes: [{time: 0, value: 0.9}] },
                            u4_offset_x: { keyframes: [{time: 0, value: 10.0}] },
                            u5_offset_y: { keyframes: [{time: 0, value: 10.0}] },
                            u6_blur: { keyframes: [{time: 0, value: 8.0}] },
                            u7_pad: { keyframes: [{time: 0, value: 0.0}] }
                        }
                    }
                ],
                lifespan: {start:0, end:10},
                layer: 1
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC17: Explicit Audio/Video Keyframe Sync ───
$('btnTestCase17').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "test_video": { type: "video", url: "http://localhost:5173/examples/38.mp4" },
            "test_audio": { type: "audio", url: "http://localhost:5173/examples/audio_00001.mp3" }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.08, 0.0, 0.08, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // Background ambient audio (volume 0.5)
            {
                id: "bgm",
                audioSource: { assetId: "test_audio", duration: 15.0 },
                visual: { opacity: 1.0, volume: 0.5, blendMode: "normal" },
                lifespan: { start: 0, end: 15 },
                animation: { floatTracks: [
                    { target: "playbackTime", track: { keyframes: [
                        { time: 0, value: 0 },
                        { time: 15, value: 15 }
                    ]}}
                ]},
                layer: 1
            },
            
            // Nested Video inside a composition to test volume cascade and speed manipulation
            {
                id: "comp_vid",
                composition: { duration: {manual: 8.0}, speed: 1.0, loopMode: "once", trimStart: 0.0 },
                transform: {x:640,y:360,rotation:0,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                visual: { opacity: 1.0, volume: 1.0, blendMode: "normal" },
                lifespan: { start: 2, end: 10 },
                layer: 2
            },
            {
                id: "video_track",
                parentId: "comp_vid",
                videoSource: { assetId: "test_video", duration: 8.0 },
                rect: { width: 800, height: 450, fitMode: "contain" },
                transform: {x:0,y:0,rotation:0.1,scaleX:1,scaleY:1,anchorX:0.5,anchorY:0.5},
                visual: { opacity: 1.0, volume: 1.0, blendMode: "normal" },
                lifespan: { start: 0, end: 8 },
                animation: { floatTracks: [
                    // Explicit linear keyframing for video
                    { target: "playbackTime", track: { keyframes: [
                        { time: 0, value: 0 },
                        { time: 8, value: 8 }
                    ]}},
                    // Animate volume to test Audio Sync dynamically
                    { target: "volume", track: { keyframes: [
                        { time: 0, value: 0.0 },
                        { time: 2, value: 1.0 },
                        { time: 6, value: 1.0 },
                        { time: 8, value: 0.0 }
                    ]}}
                ]},
                layer: 3
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC13: Text Rendering via ab_glyph TTF ArrayBuffer ───
$('btnTestCase13b').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "font_anton": { type: "font", url: "http://localhost:5173/examples/Anton-Regular.ttf" }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.15, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            {
                id: "txt_hello",
                textSource: {
                    content: "HELLO WASM TEXT RENDERER",
                    font: "font_anton",
                    fontSize: 72,
                    color: [0.38, 0.65, 0.98, 1.0],
                    continuousRasterization: false
                },
                rect: { width: 800, height: 150, fitMode: "contain" },
                transform: {x:640, y:360, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 10,
                animation: { floatTracks: [
                    { target: "transformRotation", track: { keyframes: [{time:0, value:0}, {time:10, value:0.5}] } },
                    { target: "transformScaleX", track: { keyframes: [{time:0, value:0.8}, {time:5, value:1.2}, {time:10, value:0.8}] } },
                    { target: "transformScaleY", track: { keyframes: [{time:0, value:0.8}, {time:5, value:1.2}, {time:10, value:0.8}] } }
                ]}
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC18: Comprehensive Blur Test ───
$('btnTestCase18').onclick = () => {
    $('jsonEditor').value = JSON.stringify({
        assets: {
            "blur_shader": { type: "shader", url: "./examples/blur.wgsl" },
            "photo": { type: "image", url: "https://picsum.photos/400" },
            "inter": { type: "font", url: "https://fonts.gstatic.com/s/inter/v13/UcCO3FwrK3iLTeHuS_fvQtMwCp50KnMw2boKoduKmMEVuGKYMZhrib2Bg-4.ttf" }
        },
        entities: [
            BASE_CAM,
            { id: "bg", shapeSource: { kind: "rectangle", fillColor: [0.1, 0.1, 0.15, 1.0] }, rect: {width:1280,height:720}, transform: {x:0,y:0,rotation:0,scaleX:1,scaleY:1,anchorX:0,anchorY:0}, layer: 0 },
            
            // 1. Text Blur: Padded (bleeds soft light outward)
            {
                id: "padded_text",
                textSource: { content: "PADDED TEXT", fontSize: 50, color: [1, 1, 1, 1], font: "inter" },
                rect: { width: 400, height: 100, fitMode: "contain" },
                transform: {x:320, y:200, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [{
                    shader_id: "blur", scope: "padded",
                    float_uniforms: {
                        u0_dx: { keyframes: [{time:0, value:1.0}] },
                        u1_dy: { keyframes: [{time:0, value:1.0}] },
                        u2_radius: { keyframes: [{time:0, value:25.0}] },
                        u3_texel: { keyframes: [{time:0, value:0.003}] }
                    }
                }],
                lifespan: {start:0, end:10},
                layer: 1
            },
            
            // 2. Shape Blur: Masked (keeps exact original solid silhouette)
            {
                id: "masked_shape",
                shapeSource: { kind: "solid_ellipse", fillColor: [0.3, 0.8, 0.4, 1.0] },
                rect: { width: 150, height: 150 },
                transform: {x:960, y:200, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [{
                    shader_id: "blur", scope: "masked", // Output is multiplied by original alpha !
                    float_uniforms: {
                        u0_dx: { keyframes: [{time:0, value:1.0}] },
                        u1_dy: { keyframes: [{time:0, value:1.0}] },
                        u2_radius: { keyframes: [{time:0, value:15.0}] },
                        u3_texel: { keyframes: [{time:0, value:0.01}] } // Coarse blur
                    }
                }],
                lifespan: {start:0, end:10},
                layer: 1
            },
            
            // 3. Image Blur: Clipped (sharp boundary at the rect box)
            {
                id: "clipped_img",
                imageSource: { assetId: "photo" },
                rect: { width: 250, height: 250, fitMode: "stretch" },
                transform: {x:320, y:500, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [{
                    shader_id: "blur", scope: "clipped", // Clamped at 250x250, no bleed outside
                    float_uniforms: {
                        u0_dx: { keyframes: [{time:0, value:1.0}] },
                        u1_dy: { keyframes: [{time:0, value:1.0}] },
                        u2_radius: { keyframes: [{time:0, value:20.0}] },
                        u3_texel: { keyframes: [{time:0, value:0.003}] }
                    }
                }],
                lifespan: {start:0, end:10},
                layer: 1
            },
            
            // 4. Base Image underneath Adjustment Layer
            {
                id: "base_img",
                imageSource: { assetId: "photo" },
                rect: { width: 400, height: 300, fitMode: "stretch" },
                transform: {x:960, y:500, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                lifespan: {start:0, end:10},
                layer: 1
            },
            
            // 5. Adjustment Layer Blur: Layer (blurs everything physically under this rect)
            {
                id: "layer_blur",
                shapeSource: { kind: "rectangle", fillColor: [1, 1, 1, 0.0] }, // Invisible bounds
                rect: { width: 350, height: 150 }, // Half height to show bottom half sharp
                transform: {x:960, y:500, rotation:0, scaleX:1, scaleY:1, anchorX:0.5, anchorY:0.5},
                materials: [{
                    shader_id: "blur", scope: "layer", // Screenspace effect applying to anything under the 400x150 box
                    float_uniforms: {
                        u0_dx: { keyframes: [{time:0, value:1.0}] },
                        u1_dy: { keyframes: [{time:0, value:1.0}] },
                        u2_radius: { keyframes: [{time:0, value:0.0}, {time:2, value:30.0}, {time:4, value:0.0}] },
                        u3_texel: { keyframes: [{time:0, value:0.003}] }
                    }
                }],
                lifespan: {start:0, end:10},
                layer: 5 // Must be higher than the target
            }
        ]
    }, null, 2);
    if(engine) applyJson();
};

// ─── TC19: Backend Export Heavy TC (Super Stress Test) ───
// Tests everything from TC1 to TC18 inside a ~6 minute composition matching exact video length
// Extracted to tc19_super_stress.js to keep main.js readable
$('btnTestCase19').onclick = () => {
    // Exact duration extracted from ffprobe (353.639683 seconds)
    const v_dur = 353.639683;
    
    // Generate the big JSON representing the stress test
    const jsonObj = createTC19Json(v_dur);
    
    $('jsonEditor').value = JSON.stringify(jsonObj, null, 2);
    if(engine) applyJson();
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
    const fpsOverride = parseInt($('exportFPS').value, 10) || 60;
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
