/**
 * ecs-test.js — ifol-render ECS Editor v3
 * Fixes: perf (dirty flag), video playback, wireframe selection, pan/zoom, editor/camera modes
 */
import init, { IfolRenderWeb } from 'ifol-render-wasm';

// ═══ STATE ═══
let entities = [], sceneAssets = {}, selectedId = null;
let time = 0, duration = 10, FPS = 60, playing = false, entityCounter = 0;
let sceneDirty = true; // only reload scene when changed
const ASSET_SERVER = 'http://localhost:8000';
const viewports = []; // { id, canvas, engine, cameraId, mode:'editor'|'camera', panX, panY, zoom }
const videoPool = {};
const extractCanvas = document.createElement('canvas');
const extractCtx = extractCanvas.getContext('2d', { willReadFrequently: true });

// ═══ HELPERS ═══
const kf = (t,v,interp) => interp ? {time:t,value:v,interpolation:interp} : {time:t,value:v};
const ft = v => ({keyframes:[kf(0,v)]});
const st = (t,v) => ({keyframes:[{time:t,value:v}]});
const gv = (track,d) => track?.keyframes?.[0]?.value ?? d;
const evalTrack = (track, t, d=0) => {
  if (!track?.keyframes?.length) return d;
  const ks = track.keyframes;
  if (ks.length === 1 || t <= ks[0].time) return ks[0].value;
  if (t >= ks[ks.length-1].time) return ks[ks.length-1].value;
  for (let i=0; i<ks.length-1; i++) {
    if (t >= ks[i].time && t < ks[i+1].time) {
      if (ks[i].interpolation?.type === 'hold') return ks[i].value;
      const frac = (t - ks[i].time) / (ks[i+1].time - ks[i].time);
      let ef = frac;
      const ip = ks[i].interpolation;
      if (ip?.type === 'cubic_bezier') {
        let x=frac; for(let j=0;j<8;j++){const cx=3*ip.x1*x*(1-x)*(1-x)+3*ip.x2*x*x*(1-x)+x*x*x-frac;const dx=3*ip.x1*(1-x)*(1-3*x)+3*ip.x2*x*(2-3*x)+3*x*x;if(Math.abs(dx)<1e-7)break;x-=cx/dx;}
        ef=3*ip.y1*x*(1-x)*(1-x)+3*ip.y2*x*x*(1-x)+x*x*x;
      }
      return ks[i].value + (ks[i+1].value - ks[i].value) * ef;
    }
  }
  return ks[ks.length-1].value;
};
const hex2rgb = h => [parseInt(h.slice(1,3),16)/255,parseInt(h.slice(3,5),16)/255,parseInt(h.slice(5,7),16)/255];
const rgb2hex = (r,g,b) => '#'+[r,g,b].map(v=>Math.round(Math.max(0,Math.min(1,v))*255).toString(16).padStart(2,'0')).join('');
const srgb2lin = v => v<=0.04045 ? v/12.92 : Math.pow((v+0.055)/1.055, 2.4);
const lin2srgb = v => v<=0.0031308 ? v*12.92 : 1.055*Math.pow(v,1/2.4)-0.055;
const $ = id => document.getElementById(id);
function markDirty() { sceneDirty = true; }
// Get entity display size: rect > intrinsic > default
function getEntitySize(e) {
  if (e.rect) { const rw=gv(e.rect.width,0),rh=gv(e.rect.height,0); if(rw>0&&rh>0) return {w:rw,h:rh}; }
  if (e.videoSource) return { w: e.videoSource.intrinsicWidth||360, h: e.videoSource.intrinsicHeight||360 };
  if (e.imageSource) return { w: e.imageSource.intrinsicWidth||360, h: e.imageSource.intrinsicHeight||360 };
  if (e.camera) return { w: gv(e.transform?.width,1280), h: gv(e.transform?.height,720) };
  return { w: 200, h: 200 };
}

// ═══ ENTITY FACTORY ═══
function createEntity(type='solid', name=null) {
  const id = name || `e${++entityCounter}`;
  const base = {
    id, lifespan: { start: 0, end: duration },
    transform: { x:ft(100+entityCounter*40), y:ft(80+entityCounter*30), rotation:ft(0), anchor_x:ft(0), anchor_y:ft(0), scale_x:ft(1), scale_y:ft(1) },
    rect: { width:ft(200), height:ft(150), fitMode:'stretch' },
    opacity:ft(1), blend_mode:st(0,'normal'),
    float_uniforms:{}, materials:[], layer:entityCounter, parent_id:null,
  };
  if (type === 'solid') {
    const [sr,sg,sb] = hex2rgb('#4a7acc'); const r=srgb2lin(sr),g=srgb2lin(sg),b=srgb2lin(sb);
    base.colorSource = { color:{r,g,b,a:1} };
    base.float_uniforms = {color_r:ft(r),color_g:ft(g),color_b:ft(b),color_a:ft(1)};
  } else if (type === 'camera') {
    base.id = name || `cam_${++entityCounter}`;
    base.camera = { postEffects: [] };
    base.transform = { x:ft(0), y:ft(0), width:ft(1280), height:ft(720), rotation:ft(0), anchor_x:ft(0), anchor_y:ft(0), scale_x:ft(1), scale_y:ft(1) };
    base.lifespan = { start: 0, end: 99999 };
  }
  return base;
}

// ═══ SCENE BUILDER ═══
function buildScene(includeSelection=true) {
  const se = [];
  const cams = entities.filter(e=>e.camera);
  if (!cams.length) se.push({type:'camera',id:'cam',lifespan:{start:0,end:99999},transform:{x:ft(0),y:ft(0),width:ft(1280),height:ft(720),rotation:ft(0)},camera:{postEffects:[]},post_effects:[]});
  for (const e of entities) {
    if (e.camera) se.push({type:'camera',id:e.id,lifespan:e.lifespan,transform:e.transform,camera:e.camera,post_effects:e.camera?.postEffects||[]});
    else se.push({...e});
  }
  // Selection is drawn as 2D overlay, not ECS entities
  return { assets: sceneAssets, entities: se };
}

// ═══ RENDER — dirty flag for perf ═══
function resizeViewport(vp) {
  const wrap = vp.canvas.parentElement;
  if (!wrap) return;
  const rect = wrap.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const w = Math.max(1, Math.round(rect.width * dpr));
  const h = Math.max(1, Math.round(rect.height * dpr));
  if (vp.canvas.width !== w || vp.canvas.height !== h) {
    vp.canvas.width = w; vp.canvas.height = h;
    try { vp.engine.resize(w, h); } catch(e){}
  }
}

async function render(force=false) {
  syncSelection(); // Keep core selection state in sync
  if (sceneDirty || force) {
    const json = JSON.stringify(buildScene());
    for (const vp of viewports) { if(vp.engine) try { vp.engine.load_scene_v2(json); } catch(e){console.error(e);} }
    sceneDirty = false;
  }
  for (const vp of viewports) {
    if (!vp.engine) continue;
    try {
      resizeViewport(vp);
      // Camera entity default view region
      const camEnt = entities.find(e => e.id === vp.cameraId);
      const camW = camEnt ? gv(camEnt.transform?.width, 1280) : 1280;
      const camH = camEnt ? gv(camEnt.transform?.height, 720) : 720;
      if (vp.mode === 'editor') {
        // Editor: show world region based on zoom/pan, render at viewport resolution
        const viewW = camW / vp.zoom;
        const viewH = camH / vp.zoom;
        vp.engine.render_frame_v2(time, vp.cameraId, vp.panX, vp.panY, viewW, viewH);
      } else {
        // Camera mode: render at camera's native view, engine auto-scales to canvas
        vp.engine.render_frame_v2(time, vp.cameraId);
      }
    } catch(e){console.error(e);}
  }
  // Selection is now rendered by core via set_selection API
}

// Sync selection state to all engine instances (supports multi-select via comma-separated IDs)
function syncSelection() {
  const ids = Array.isArray(selectedId) ? selectedId.join(',') : (selectedId || '');
  for (const vp of viewports) {
    if (!vp.engine) continue;
    try { vp.engine.set_selection(ids || null); } catch(e){}
  }
}

// ═══ HIERARCHY ═══
function refreshHierarchy() {
  const list=$('hierarchy-list'); list.innerHTML='';
  // Build tree: root entities first, then children
  const roots = entities.filter(e=>!e.parent_id).sort((a,b)=>(a.layer||0)-(b.layer||0));
  const children = id => entities.filter(e=>e.parent_id===id).sort((a,b)=>(a.layer||0)-(b.layer||0));
  function addItem(e, depth=0) {
    const div=document.createElement('div'); div.className='ent-item'+(e.id===selectedId?' sel':'');
    if(depth>0) div.classList.add('child');
    div.style.paddingLeft=(6+depth*14)+'px';
    let icon='◆'; if(e.camera) icon='🎥'; else if(e.composition) icon='📁'; else if(e.videoSource) icon='🎬'; else if(e.imageSource) icon='🖼'; else if(e.textSource) icon='📝'; else if(e.colorSource) icon='■';
    let badges=''; if(e.camera) badges+='<span class="comp-badge cam">CAM</span>'; if(e.composition) badges+='<span class="comp-badge" style="background:#7c3aed">COMP</span>'; if(e.videoSource) badges+='<span class="comp-badge video">VID</span>'; if(e.imageSource) badges+='<span class="comp-badge image">IMG</span>'; if(e.colorSource) badges+='<span class="comp-badge color">CLR</span>'; if(e.textSource) badges+='<span class="comp-badge text">TXT</span>';
    div.innerHTML=`<span class="icon">${icon}</span><span class="name">${e.id}${badges}</span><span style="color:var(--dim);font-size:8px;margin-left:auto">${e.layer||0}</span>`;
    div.onclick=()=>{selectedId=e.id;refreshHierarchy();refreshInspector();refreshTimeline();markDirty();render();};
    list.appendChild(div);
    // Add children
    for(const c of children(e.id)) addItem(c, depth+1);
  }
  for(const e of roots) addItem(e);
}

// ═══ INSPECTOR ═══
function refreshInspector() {
  const empty=$('insp-empty'),content=$('insp-content'),ent=entities.find(e=>e.id===selectedId);
  if(!ent){empty.style.display='';content.style.display='none';return;}
  empty.style.display='none';content.style.display='';
  let h='';
  h+=`<div class="insp-section"><div class="shdr">Entity: ${ent.id}</div>${row('Layer','number',ent.layer||0,'_setLayer')}</div>`;
  // Transform
  const tf=ent.transform||{};
  h+=`<div class="insp-section"><div class="shdr">Transform</div>`;
  h+=row('X','number',Math.round(evalTrack(tf.x,time,0)*10)/10,'_setTF_x');
  h+=row('Y','number',Math.round(evalTrack(tf.y,time,0)*10)/10,'_setTF_y');
  h+=row('Rot','number',Math.round(evalTrack(tf.rotation,time,0)*10)/10,'_setTF_rot');
  h+=row('ScaleX','number',gv(tf.scale_x,1),'_setTF_sx',0.01);
  h+=row('ScaleY','number',gv(tf.scale_y,1),'_setTF_sy',0.01);
  h+=`</div>`;
  // Rect (display size + fit mode)
  if(!ent.camera && ent.rect){
    const rc=ent.rect;
    h+=`<div class="insp-section"><div class="shdr">Rect</div>`;
    h+=row('W','number',Math.round(gv(rc.width,200)*10)/10,'_setRectW');
    h+=row('H','number',Math.round(gv(rc.height,150)*10)/10,'_setRectH');
    h+=`<div class="insp-row"><label>Fit</label><select onchange="window._setFit(this.value)">`;
    for(const m of ['stretch','contain','cover']) h+=`<option${(rc.fitMode||'stretch')===m?' selected':''}>${m}</option>`;
    h+=`</select></div></div>`;
  }
  // Visual
  if(!ent.camera){
    h+=`<div class="insp-section"><div class="shdr">Visual</div>`;
    h+=`<div class="insp-row"><label>Opacity</label><input type="range" min="0" max="100" value="${gv(ent.opacity,1)*100}" oninput="this.nextElementSibling.value=this.value;window._setOpacity(this.value)"><input type="number" value="${Math.round(gv(ent.opacity,1)*100)}" style="width:36px" onchange="this.previousElementSibling.value=this.value;window._setOpacity(this.value)"></div>`;
    h+=`<div class="insp-row"><label>Blend</label><select onchange="window._setBlend(this.value)">`;
    for(const m of ['normal','multiply','screen','overlay','add','difference']) h+=`<option${gv(ent.blend_mode,'normal')===m?' selected':''}>${m}</option>`;
    h+=`</select></div></div>`;
  }
  // Composition
  if(ent.composition){
    const comp=ent.composition;
    h+=`<div class="insp-section"><div class="shdr">Composition <button onclick="window._removeComp('composition')">✕</button></div>`;
    h+=row('Speed','number',comp.speed||1,'_setCompSpeed',0.1);
    h+=row('TrimStart','number',comp.trimStart||0,'_setCompTrimStart',0.1);
    h+=row('TrimEnd','number',comp.trimEnd||10,'_setCompTrimEnd',0.1);
    h+=`<div class="insp-row"><label>Loop</label><select onchange="window._setCompLoop(this.value)">`;
    for(const m of ['once','loop','pingPong']) h+=`<option${(comp.loopMode||'once')===m?' selected':''}>${m}</option>`;
    h+=`</select></div>`;
    h+=row('Duration','number',typeof comp.duration==='object'?comp.duration.manual||10:10,'_setCompDuration',0.1);
    h+=`</div>`;
  }
  // Lifespan
  h+=`<div class="insp-section"><div class="shdr">Lifespan</div>${row('Start','number',ent.lifespan?.start??0,'_setLifeStart',0.1)}${row('End','number',ent.lifespan?.end??duration,'_setLifeEnd',0.1)}</div>`;
  // Components
  if(ent.colorSource){const c=ent.colorSource.color;h+=`<div class="insp-section"><div class="shdr">Color Source <button onclick="window._removeComp('colorSource')">✕</button></div><div class="insp-row"><label>Color</label><input type="color" value="${rgb2hex(lin2srgb(c.r),lin2srgb(c.g),lin2srgb(c.b))}" oninput="window._setColor(this.value)"></div></div>`;}
  // Video Source (URL input)
  if(ent.videoSource){h+=`<div class="insp-section"><div class="shdr">Video Source <button onclick="window._removeComp('videoSource')">✕</button></div><div class="insp-row"><label>AssetID</label><input type="text" value="${ent.videoSource.assetId||''}" onchange="window._setVideoAsset(this.value)" placeholder="asset id"></div><div class="insp-row"><label>URL</label><input type="text" value="${sceneAssets[ent.videoSource.assetId]?.url||''}" onchange="window._setVideoUrl(this.value)" placeholder="http://... or local path"></div>${row('Trim','number',ent.videoSource.trimStart||0,'_setVideoTrim',0.1)}${row('W','number',ent.videoSource.intrinsicWidth||0,'_setVidIW')}${row('H','number',ent.videoSource.intrinsicHeight||0,'_setVidIH')}</div>`;}
  if(ent.imageSource){h+=`<div class="insp-section"><div class="shdr">Image Source <button onclick="window._removeComp('imageSource')">✕</button></div><div class="insp-row"><label>AssetID</label><input type="text" value="${ent.imageSource.assetId||''}" onchange="window._setImageAsset(this.value)" placeholder="asset id"></div><div class="insp-row"><label>URL</label><input type="text" value="${sceneAssets[ent.imageSource.assetId]?.url||''}" onchange="window._setImageUrl(this.value)" placeholder="http://... or local path"></div>${row('W','number',ent.imageSource.intrinsicWidth||0,'_setImgIW')}${row('H','number',ent.imageSource.intrinsicHeight||0,'_setImgIH')}</div>`;}
  if(ent.textSource){h+=`<div class="insp-section"><div class="shdr">Text Source <button onclick="window._removeComp('textSource')">✕</button></div><div class="insp-row"><label>Text</label><input type="text" value="${ent.textSource.content||''}" onchange="window._setText(this.value)"></div>${row('Size','number',ent.textSource.fontSize||48,'_setFontSize')}</div>`;}
  if(ent.camera){
    const cam=ent.camera;
    h+=`<div class="insp-section"><div class="shdr">Camera</div>`;
    h+=row('ResW','number',cam.resolutionWidth||1280,'_setCamResW');
    h+=row('ResH','number',cam.resolutionHeight||720,'_setCamResH');
    h+=`<div class="insp-row"><label>BG</label><input type="color" value="${rgb2hex(lin2srgb(cam.bgColor?.[0]||0),lin2srgb(cam.bgColor?.[1]||0),lin2srgb(cam.bgColor?.[2]||0))}" oninput="window._setCamBg(this.value)"></div>`;
    h+=row('FOV','number',cam.fov||0,'_setCamFov',1);
    h+=`</div>`;
  }
  // Parent ID
  if(!ent.camera){
    const comps=entities.filter(x=>x.composition&&x.id!==ent.id);
    h+=`<div class="insp-section"><div class="shdr">Relations</div><div class="insp-row"><label>Parent</label><select onchange="window._setParent(this.value)"><option value="">(none)</option>`;
    for(const c of comps) h+=`<option value="${c.id}"${ent.parent_id===c.id?' selected':''}>${c.id}</option>`;
    h+=`</select></div></div>`;
  }
  if(!ent.camera) h+=`<button class="add-comp-btn" onclick="window._showAddComp(event)">+ Add Component</button>`;
  // Keyframes
  if(!ent.camera){
    h+=`<div class="insp-section"><div class="shdr">Keyframes</div>`;
    h+=`<div class="insp-row"><label>Prop</label><select id="kf-prop">`;
    for(const p of ['x','y','width','height','rotation','scale_x','scale_y','opacity']) h+=`<option value="${p}">${p}</option>`;
    h+=`</select></div><div class="insp-row"><label>Ease</label><select id="kf-easing"><option value="linear">Linear</option><option value="ease">Ease</option><option value="hold">Hold</option></select></div>`;
    h+=`<div style="display:flex;gap:4px;padding:2px 6px"><button class="add-comp-btn" style="margin:0;flex:1" onclick="window._addKeyframe()">+ KF at ${time.toFixed(2)}s</button><button class="add-comp-btn" style="margin:0;flex:1;border-color:var(--red);color:var(--red)" onclick="window._clearKeyframes()">Clear</button></div>`;
    for(const p of ['x','y','width','height','rotation','scale_x','scale_y']){const tk=ent.transform?.[p];if(tk?.keyframes?.length>1) h+=`<div style="padding:1px 6px;font-size:8px;color:var(--dim)"><b>${p}</b>: ${tk.keyframes.map(k=>`${k.time.toFixed(1)}→${k.value.toFixed(0)}`).join(', ')}</div>`;}
    if(ent.opacity?.keyframes?.length>1) h+=`<div style="padding:1px 6px;font-size:8px;color:var(--dim)"><b>opacity</b>: ${ent.opacity.keyframes.map(k=>`${k.time.toFixed(1)}→${k.value.toFixed(2)}`).join(', ')}</div>`;
    h+=`</div>`;
  }
  content.innerHTML=h;
}
function row(l,t,v,fn,step=1){return `<div class="insp-row"><label>${l}</label><input type="${t}" value="${v}" step="${step}" onchange="window.${fn}(this.value)"></div>`;}

// ═══ INSPECTOR ACTIONS ═══
function sel(){return entities.find(e=>e.id===selectedId);}
window._setTF_x=v=>{const e=sel();if(e){e.transform.x.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setTF_y=v=>{const e=sel();if(e){e.transform.y.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setTF_rot=v=>{const e=sel();if(e){e.transform.rotation.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setTF_sx=v=>{const e=sel();if(e){e.transform.scale_x.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setTF_sy=v=>{const e=sel();if(e){e.transform.scale_y.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setRectW=v=>{const e=sel();if(e?.rect){e.rect.width.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setRectH=v=>{const e=sel();if(e?.rect){e.rect.height.keyframes[0].value=parseFloat(v);markDirty();render();}};
window._setFit=v=>{const e=sel();if(e?.rect){e.rect.fitMode=v;markDirty();render();}};
window._setLayer=v=>{const e=sel();if(e){e.layer=parseInt(v);markDirty();render();refreshHierarchy();}};
window._setBlend=v=>{const e=sel();if(e){e.blend_mode=st(0,v);markDirty();render();}};
window._setOpacity=v=>{const e=sel();if(e){e.opacity.keyframes[0].value=parseFloat(v)/100;markDirty();render();}};
window._setLifeStart=v=>{const e=sel();if(e){e.lifespan.start=parseFloat(v);markDirty();render();}};
window._setLifeEnd=v=>{const e=sel();if(e){e.lifespan.end=parseFloat(v);markDirty();render();}};
window._setColor=hex=>{const e=sel();if(!e?.colorSource)return;const[sr,sg,sb]=hex2rgb(hex);const r=srgb2lin(sr),g=srgb2lin(sg),b=srgb2lin(sb);e.colorSource.color={r,g,b,a:1};e.float_uniforms.color_r=ft(r);e.float_uniforms.color_g=ft(g);e.float_uniforms.color_b=ft(b);markDirty();render();};
window._removeComp=comp=>{const e=sel();if(!e)return;delete e[comp];if(comp==='colorSource'){delete e.float_uniforms.color_r;delete e.float_uniforms.color_g;delete e.float_uniforms.color_b;delete e.float_uniforms.color_a;}markDirty();render();refreshInspector();refreshHierarchy();};
window._setVideoTrim=v=>{const e=sel();if(e?.videoSource){e.videoSource.trimStart=parseFloat(v);markDirty();render();}};
window._setText=v=>{const e=sel();if(e?.textSource){e.textSource.content=v;markDirty();render();}};
window._setFontSize=v=>{const e=sel();if(e?.textSource){e.textSource.fontSize=parseFloat(v);markDirty();render();}};
// Composition handlers
window._setCompSpeed=v=>{const e=sel();if(e?.composition){e.composition.speed=parseFloat(v);markDirty();render();}};
window._setCompTrimStart=v=>{const e=sel();if(e?.composition){e.composition.trimStart=parseFloat(v);markDirty();render();}};
window._setCompTrimEnd=v=>{const e=sel();if(e?.composition){e.composition.trimEnd=parseFloat(v);markDirty();render();}};
window._setCompLoop=v=>{const e=sel();if(e?.composition){e.composition.loopMode=v;markDirty();render();}};
window._setCompDuration=v=>{const e=sel();if(e?.composition){e.composition.duration={manual:parseFloat(v)};markDirty();render();}};
// Camera handlers
window._setCamResW=v=>{const e=sel();if(e?.camera){e.camera.resolutionWidth=parseInt(v);markDirty();render();}};
window._setCamResH=v=>{const e=sel();if(e?.camera){e.camera.resolutionHeight=parseInt(v);markDirty();render();}};
window._setCamBg=hex=>{const e=sel();if(!e?.camera)return;const[sr,sg,sb]=hex2rgb(hex);e.camera.bgColor=[srgb2lin(sr),srgb2lin(sg),srgb2lin(sb),1];markDirty();render();};
window._setCamFov=v=>{const e=sel();if(e?.camera){e.camera.fov=parseFloat(v);markDirty();render();}};
// URL-based asset handlers
window._setVideoAsset=v=>{const e=sel();if(e?.videoSource){e.videoSource.assetId=v;markDirty();render();refreshInspector();}};
window._setVideoUrl=v=>{const e=sel();if(!e?.videoSource)return;const aid=e.videoSource.assetId;sceneAssets[aid]={type:'video',url:v};loadVideoFromUrl(v,aid,e);};
window._setVidIW=v=>{const e=sel();if(e?.videoSource){e.videoSource.intrinsicWidth=parseFloat(v);markDirty();render();}};
window._setVidIH=v=>{const e=sel();if(e?.videoSource){e.videoSource.intrinsicHeight=parseFloat(v);markDirty();render();}};
window._setImageAsset=v=>{const e=sel();if(e?.imageSource){e.imageSource.assetId=v;markDirty();render();refreshInspector();}};
window._setImageUrl=v=>{const e=sel();if(!e?.imageSource)return;const aid=e.imageSource.assetId;sceneAssets[aid]={type:'image',url:v};loadImageFromUrl(v,aid,e);};
window._setImgIW=v=>{const e=sel();if(e?.imageSource){e.imageSource.intrinsicWidth=parseFloat(v);markDirty();render();}};
window._setImgIH=v=>{const e=sel();if(e?.imageSource){e.imageSource.intrinsicHeight=parseFloat(v);markDirty();render();}};
window._setParent=v=>{const e=sel();if(!e)return;e.parent_id=v||null;refreshHierarchy();refreshTimeline();markDirty();render();};

// Load video from URL
async function loadVideoFromUrl(url,aid,ent) {
  $('status').textContent=`Loading video: ${url}...`;
  try {
    const v=document.createElement('video');v.crossOrigin='anonymous';v.preload='auto';v.muted=true;v.playsInline=true;v.src=url;
    await new Promise((r,rj)=>{v.onloadeddata=r;v.onerror=()=>rj(new Error('decode fail'));setTimeout(()=>rj(new Error('timeout')),15000);});
    videoPool[url]=v;
    if(v.videoWidth>0){ent.videoSource.intrinsicWidth=v.videoWidth;ent.videoSource.intrinsicHeight=v.videoHeight;ent.videoSource.duration=v.duration||0;cacheVideoFrame(url,0,v);}
    $('status').textContent=`✓ ${url} (${v.videoWidth}×${v.videoHeight})`;
  } catch(e) { $('status').textContent=`✗ ${e.message}`; }
  markDirty();render();refreshInspector();
}

// Load image from URL
async function loadImageFromUrl(url,aid,ent) {
  $('status').textContent=`Loading image: ${url}...`;
  try {
    const resp=await fetch(url);const blob=await resp.blob();const buf=new Uint8Array(await blob.arrayBuffer());
    const bm=await createImageBitmap(blob);ent.imageSource.intrinsicWidth=bm.width;ent.imageSource.intrinsicHeight=bm.height;
    for(const vp of viewports){if(vp.engine){vp.engine.cache_image(url,buf);vp.engine.cache_image(aid,buf);}}
    $('status').textContent=`✓ ${url} (${bm.width}×${bm.height})`;bm.close();
  } catch(e) { $('status').textContent=`✗ ${e.message}`; }
  markDirty();render();refreshInspector();
}

// ═══ ADD COMPONENT ═══
window._showAddComp=(event)=>{
  document.querySelectorAll('.popup-menu').forEach(el=>el.remove());
  const ent=sel();if(!ent) return;
  const menu=document.createElement('div');menu.className='popup-menu';
  const rect=event.target.getBoundingClientRect();
  menu.style.left=rect.left+'px';menu.style.top=(rect.top-10)+'px';menu.style.transform='translateY(-100%)';
  const items=[];
  if(!ent.colorSource) items.push({label:'🎨 Color Source',fn:()=>{const[sr,sg,sb]=hex2rgb('#4a7acc');const r=srgb2lin(sr),g=srgb2lin(sg),b=srgb2lin(sb);ent.colorSource={color:{r,g,b,a:1}};ent.float_uniforms.color_r=ft(r);ent.float_uniforms.color_g=ft(g);ent.float_uniforms.color_b=ft(b);ent.float_uniforms.color_a=ft(1);markDirty();render();refreshInspector();refreshHierarchy();}});
  if(!ent.videoSource) items.push({label:'🎬 Video Source',fn:()=>{const p=$('pick-video');p.value='';p.onchange=async()=>{const f=p.files[0];if(!f)return;$('status').textContent=`Loading: ${f.name}...`;const aid='vid_'+(++entityCounter);const url=URL.createObjectURL(f);sceneAssets[aid]={type:'video',url};ent.videoSource={assetId:aid,trimStart:0,trimEnd:null,intrinsicWidth:0,intrinsicHeight:0,duration:0,fps:30,_fileName:f.name};delete ent.colorSource;delete ent.float_uniforms.color_r;delete ent.float_uniforms.color_g;delete ent.float_uniforms.color_b;delete ent.float_uniforms.color_a;try{const v=document.createElement('video');v.crossOrigin='anonymous';v.preload='auto';v.muted=true;v.playsInline=true;v.src=url;await new Promise((r,rj)=>{v.onloadeddata=r;v.onerror=()=>rj(new Error('decode fail'));setTimeout(()=>rj(new Error('timeout')),15000);});videoPool[url]=v;if(v.videoWidth>0){ent.videoSource.intrinsicWidth=v.videoWidth;ent.videoSource.intrinsicHeight=v.videoHeight;ent.videoSource.duration=v.duration||0;const a=v.videoWidth/v.videoHeight;ent.transform.width=ft(Math.round(360*a));ent.transform.height=ft(360);cacheVideoFrame(url,0,v);}$('status').textContent=`✓ ${f.name} (${v.videoWidth}×${v.videoHeight})`;}catch(e){$('status').textContent=`✗ ${e.message}`;}markDirty();render();refreshInspector();refreshHierarchy();};p.click();}});
  if(!ent.imageSource) items.push({label:'🖼 Image Source',fn:()=>{const p=$('pick-image');p.value='';p.onchange=async()=>{const f=p.files[0];if(!f)return;$('status').textContent=`Loading: ${f.name}...`;const aid='img_'+(++entityCounter);const url=URL.createObjectURL(f);sceneAssets[aid]={type:'image',url};ent.imageSource={assetId:aid,intrinsicWidth:0,intrinsicHeight:0,_fileName:f.name};delete ent.colorSource;delete ent.float_uniforms.color_r;delete ent.float_uniforms.color_g;delete ent.float_uniforms.color_b;delete ent.float_uniforms.color_a;try{const buf=new Uint8Array(await f.arrayBuffer());const bm=await createImageBitmap(f);ent.imageSource.intrinsicWidth=bm.width;ent.imageSource.intrinsicHeight=bm.height;const a=bm.width/bm.height;ent.transform.width=ft(Math.round(360*a));ent.transform.height=ft(360);for(const vp of viewports){if(vp.engine){vp.engine.cache_image(url,buf);vp.engine.cache_image(aid,buf);}}$('status').textContent=`✓ ${f.name} (${bm.width}×${bm.height})`;bm.close();}catch(e){$('status').textContent=`✗ ${e.message}`;}markDirty();render();refreshInspector();refreshHierarchy();};p.click();}});
  if(!ent.textSource) items.push({label:'📝 Text Source',fn:()=>{ent.textSource={content:'Hello',font:'Inter',fontSize:48,color:{r:1,g:1,b:1,a:1},bold:false,italic:false};markDirty();render();refreshInspector();refreshHierarchy();}});
  if(!ent.composition) items.push({label:'📁 Composition',fn:()=>{ent.composition={speed:1.0,trimStart:0,trimEnd:null,duration:'auto',loopMode:'once',materialCascade:true};markDirty();render();refreshInspector();refreshHierarchy();}});
  if(!items.length) items.push({label:'✓ All added',fn:()=>{}});
  for(const item of items){const d=document.createElement('div');d.className='pm-item';d.textContent=item.label;d.onclick=()=>{menu.remove();item.fn();};menu.appendChild(d);}
  const sep=document.createElement('div');sep.className='pm-sep';menu.appendChild(sep);
  const c=document.createElement('div');c.className='pm-item';c.textContent='✕ Cancel';c.onclick=()=>menu.remove();menu.appendChild(c);
  document.body.appendChild(menu);
  setTimeout(()=>{const h=ev=>{if(!menu.contains(ev.target)&&ev.target!==event.target){menu.remove();window.removeEventListener('mousedown',h);}};window.addEventListener('mousedown',h);},100);
};

// ═══ KEYFRAMES ═══
window._addKeyframe=()=>{const e=sel();if(!e)return;const p=$('kf-prop')?.value||'x';const ea=$('kf-easing')?.value||'linear';let ip=ea==='ease'?{type:'cubic_bezier',x1:.42,y1:0,x2:.58,y2:1}:ea==='hold'?{type:'hold'}:undefined;let tk=p==='opacity'?e.opacity:e.transform[p];if(!tk)return;const cv=evalTrack(tk,time,gv(tk,0));const ex=tk.keyframes.findIndex(k=>Math.abs(k.time-time)<0.01);if(ex>=0){tk.keyframes[ex].value=cv;if(ip)tk.keyframes[ex].interpolation=ip;}else{const o={time,value:cv};if(ip)o.interpolation=ip;tk.keyframes.push(o);tk.keyframes.sort((a,b)=>a.time-b.time);}markDirty();render();refreshInspector();};
window._clearKeyframes=()=>{const e=sel();if(!e)return;const p=$('kf-prop')?.value||'x';let tk=p==='opacity'?e.opacity:e.transform[p];if(!tk)return;tk.keyframes=[kf(0,tk.keyframes[0]?.value??0)];markDirty();render();refreshInspector();};

// ═══ VIDEO FRAME CACHE ═══
function cacheVideoFrame(path,t,v){const w=v.videoWidth||1280,h=v.videoHeight||720;if(extractCanvas.width!==w||extractCanvas.height!==h){extractCanvas.width=w;extractCanvas.height=h;}extractCtx.drawImage(v,0,0,w,h);const rgba=new Uint8Array(extractCtx.getImageData(0,0,w,h).data.buffer);for(const vp of viewports){if(vp.engine)vp.engine.cache_video_frame(path,t,rgba,w,h);}}

async function feedVideoFrames(){
  for(const ent of entities){
    if(!ent.videoSource) continue;
    const url=sceneAssets[ent.videoSource.assetId]?.url;if(!url) continue;
    const video=videoPool[url];if(!video||video.readyState<2) continue;
    let seekTime=time;
    if(ent.lifespan){if(time<ent.lifespan.start||time>=ent.lifespan.end) continue;seekTime=time-ent.lifespan.start;}
    seekTime+=ent.videoSource.trimStart||0;
    if(Math.abs(video.currentTime-seekTime)>0.05){video.currentTime=seekTime;await new Promise(r=>{video.onseeked=r;setTimeout(r,300);});}
    cacheVideoFrame(url,seekTime,video);
  }
}

// ═══ VIEWPORT SYSTEM ═══
async function createViewport(id,canvasId,cameraId='cam'){
  const canvas=$(canvasId);if(!canvas||!navigator.gpu)return null;
  try{const engine=await new IfolRenderWeb(canvas,canvas.width,canvas.height,FPS);engine.setup_builtins();const vp={id,canvas,engine,cameraId,mode:'editor',panX:0,panY:0,zoom:1};viewports.push(vp);return vp;}catch(e){console.error(e);return null;}
}

// ═══ PAN/ZOOM ═══
function setupViewportInteraction(){
  const area=$('viewport-area');
  let dragging=null,startMX,startMY,startEX,startEY;
  let panning=false,panVP=null,panStartMX,panStartMY,panStartX,panStartY;

  function toWorld(mx,my,canvas,vp){
    const r=canvas.getBoundingClientRect();
    const viewW=1280/vp.zoom, viewH=720/vp.zoom;
    return { x:(mx-r.left)/r.width*viewW+vp.panX-viewW/2, y:(my-r.top)/r.height*viewH+vp.panY-viewH/2 };
  }
  function hitTest(wx,wy){
    for(const e of [...entities].filter(e=>!e.camera).sort((a,b)=>(b.layer||0)-(a.layer||0))){
      // Check lifespan visibility
      const ls=e.lifespan||{start:0,end:duration};
      if(time<ls.start||time>=ls.end) continue;
      // Skip children of compositions (they're not directly clickable)
      if(e.parent_id) continue;
      const tf=e.transform||{};
      const ex=evalTrack(tf.x,time,0),ey=evalTrack(tf.y,time,0);
      const sz=getEntitySize(e);
      const sxe=evalTrack(tf.scale_x,time,1), sye=evalTrack(tf.scale_y,time,1);
      const ew=sz.w*sxe, eh=sz.h*sye;
      const rot=evalTrack(tf.rotation,time,0);
      const ax=evalTrack(tf.anchor_x,time,0), ay=evalTrack(tf.anchor_y,time,0);
      // Entity (x,y) IS the pivot/anchor point
      // Rect goes from (ex - ax*ew, ey - ay*eh) to (ex + (1-ax)*ew, ey + (1-ay)*eh)
      const rectLeft=ex-ax*ew, rectTop=ey-ay*eh;
      // Inverse-rotate click point around the pivot (ex, ey)
      const rad=-rot*Math.PI/180;
      const cosR=Math.cos(rad), sinR=Math.sin(rad);
      const dx=wx-ex, dy=wy-ey;
      const localX=dx*cosR-dy*sinR+ex;
      const localY=dx*sinR+dy*cosR+ey;
      if(localX>=rectLeft&&localX<=rectLeft+ew&&localY>=rectTop&&localY<=rectTop+eh) return e;
    }
    return null;
  }

  area.addEventListener('mousedown',e=>{
    const wrap=e.target.closest('.vp-canvas-wrap');if(!wrap)return;
    const canvas=wrap.querySelector('canvas');if(!canvas)return;
    const vp=viewports.find(v=>v.canvas===canvas);if(!vp)return;

    // Middle-click or right-click = pan
    if(e.button===1||e.button===2){
      e.preventDefault();panning=true;panVP=vp;panStartMX=e.clientX;panStartMY=e.clientY;panStartX=vp.panX;panStartY=vp.panY;
      area.style.cursor='grab';return;
    }
    if(e.button!==0)return;
    const w=toWorld(e.clientX,e.clientY,canvas,vp);
    const hit=hitTest(w.x,w.y);
    if(hit){
      selectedId=hit.id;dragging=hit;startMX=e.clientX;startMY=e.clientY;
      startEX=evalTrack(hit.transform?.x,time,0);startEY=evalTrack(hit.transform?.y,time,0);
      area.style.cursor='grabbing';
    } else { selectedId=null; }
    refreshHierarchy();refreshInspector();markDirty();render();e.preventDefault();
  });

  window.addEventListener('mousemove',e=>{
    if(panning&&panVP){
      const r=panVP.canvas.getBoundingClientRect();
      const viewW=1280/panVP.zoom,viewH=720/panVP.zoom;
      panVP.panX=panStartX-(e.clientX-panStartMX)/r.width*viewW;
      panVP.panY=panStartY-(e.clientY-panStartMY)/r.height*viewH;
      render();const zi=$(`${panVP.id}-zoom`);if(zi)zi.textContent=`${Math.round(panVP.zoom*100)}%`;
      return;
    }
    if(!dragging)return;
    const vp=viewports[0];if(!vp)return;
    const r=vp.canvas.getBoundingClientRect();
    const viewW=1280/vp.zoom,viewH=720/vp.zoom;
    const dx=(e.clientX-startMX)/r.width*viewW,dy=(e.clientY-startMY)/r.height*viewH;
    dragging.transform.x.keyframes[0].value=Math.round(startEX+dx);
    dragging.transform.y.keyframes[0].value=Math.round(startEY+dy);
    markDirty();render();refreshInspector();
  });

  window.addEventListener('mouseup',()=>{dragging=null;panning=false;panVP=null;area.style.cursor='default';});

  // Zoom with scroll wheel
  area.addEventListener('wheel',e=>{
    const wrap=e.target.closest('.vp-canvas-wrap');if(!wrap)return;
    const canvas=wrap.querySelector('canvas');if(!canvas)return;
    const vp=viewports.find(v=>v.canvas===canvas);if(!vp)return;
    e.preventDefault();
    const delta=e.deltaY>0?0.9:1.1;
    vp.zoom=Math.max(0.1,Math.min(10,vp.zoom*delta));
    render();const zi=$(`${vp.id}-zoom`);if(zi)zi.textContent=`${Math.round(vp.zoom*100)}%`;
  },{passive:false});

  // Prevent context menu on viewport
  area.addEventListener('contextmenu',e=>e.preventDefault());
}

// ═══ VIEWPORT MODE TOGGLE ═══
function setupViewportMode(vpId){
  const btn=$(`${vpId}-mode`);if(!btn)return;
  btn.onclick=()=>{
    const vp=viewports.find(v=>v.id===vpId);if(!vp)return;
    vp.mode=vp.mode==='editor'?'camera':'editor';
    btn.textContent=vp.mode==='editor'?'Editor':'Camera';
    btn.classList.toggle('cam-mode',vp.mode==='camera');
    if(vp.mode==='camera'){vp.panX=0;vp.panY=0;vp.zoom=1;}
    render();
  };
}

function addNewViewport(){
  const vpArea=$('viewport-area');const vpIdx=viewports.length;const vpId=`vp-${vpIdx}`;
  const container=document.createElement('div');container.className='vp-container';container.id=vpId;
  const cams=entities.filter(e=>e.camera);
  const opts=cams.map(c=>`<option value="${c.id}">${c.id}</option>`).join('')||'<option value="cam">cam</option>';
  container.innerHTML=`<div class="vp-header"><span class="label">VP ${vpIdx+1}</span><button id="${vpId}-mode" class="vp-mode-btn">Editor</button><select id="${vpId}-cam">${opts}</select><span style="flex:1"></span><span id="${vpId}-zoom" style="color:var(--accent);font-size:8px">100%</span><button onclick="window._removeVP('${vpId}')" style="background:none;border:none;color:var(--red);cursor:pointer;font-size:9px">✕</button></div><div class="vp-canvas-wrap"><canvas id="canvas-${vpIdx}" width="1280" height="720"></canvas></div>`;
  vpArea.appendChild(container);
  createViewport(vpId,`canvas-${vpIdx}`,cams[0]?.id||'cam').then(()=>{
    const s=$(`${vpId}-cam`);if(s) s.onchange=()=>{const vp=viewports.find(v=>v.id===vpId);if(vp){vp.cameraId=s.value;markDirty();render();}};
    setupViewportMode(vpId);markDirty();render();
  });
}
window._removeVP=vpId=>{const idx=viewports.findIndex(v=>v.id===vpId);if(idx<=0)return;viewports.splice(idx,1);$(vpId)?.remove();};

function refreshCameraSelectors(){
  const cams=entities.filter(e=>e.camera);
  const opts=cams.map(c=>`<option value="${c.id}">${c.id}</option>`).join('')||'<option value="cam">cam</option>';
  for(const vp of viewports){const s=$(`${vp.id}-cam`);if(!s)continue;const prev=s.value;s.innerHTML=opts;if(cams.find(c=>c.id===prev))s.value=prev;else{s.value=cams[0]?.id||'cam';vp.cameraId=s.value;}}
}

// ═══ TIMELINE ═══
function setupTimeline(){
  const tl=$('timeline'),td=$('time-display'),dur=$('duration-input');
  tl.max=Math.round(duration*FPS);dur.value=duration;
  tl.oninput=async()=>{if(playing)stopPlayback();time=parseInt(tl.value)/FPS;td.textContent=`${time.toFixed(2)}s / ${duration.toFixed(2)}s`;await feedVideoFrames();markDirty();render();refreshInspector();refreshTimeline();};
  dur.onchange=()=>{duration=Math.max(1,parseInt(dur.value)||10);tl.max=Math.round(duration*FPS);td.textContent=`${time.toFixed(2)}s / ${duration.toFixed(2)}s`;refreshTimeline();};
}

// ═══ TIMELINE TRACKS ═══
function refreshTimeline(){
  const wrap=$('tl-tracks-wrap');if(!wrap)return;
  const ruler=$('tl-ruler');if(!ruler)return;
  // Ruler ticks
  let rh='';
  for(let s=0;s<=Math.ceil(duration);s++){
    const pct=(s/duration)*100;
    rh+=`<span style="position:absolute;left:${pct}%;top:0;font-size:7px;color:var(--dim);transform:translateX(-50%)">${s}s</span>`;
    rh+=`<span style="position:absolute;left:${pct}%;top:12px;bottom:0;width:1px;background:var(--border)"></span>`;
  }
  // Ruler playhead marker
  const phPct=(time/duration)*100;
  rh+=`<div style="position:absolute;left:${phPct}%;top:0;bottom:0;width:1px;background:var(--red);z-index:5"></div>`;
  rh+=`<div style="position:absolute;left:${phPct}%;top:-1px;width:0;height:0;border-left:4px solid transparent;border-right:4px solid transparent;border-top:5px solid var(--red);transform:translateX(-4px);z-index:6"></div>`;
  ruler.innerHTML=rh;
  // Ruler drag-to-scrub
  ruler.onmousedown=ev=>{
    const rect=ruler.getBoundingClientRect();
    const scrub=x=>{const pct=Math.max(0,Math.min(1,(x-rect.left)/rect.width));time=pct*duration;$('timeline').value=Math.round(time*FPS);$('time-display').textContent=`${time.toFixed(2)}s / ${duration.toFixed(2)}s`;markDirty();render();refreshTimeline();refreshInspector();};
    scrub(ev.clientX);
    const move=e=>scrub(e.clientX);
    const up=()=>{window.removeEventListener('mousemove',move);window.removeEventListener('mouseup',up);};
    window.addEventListener('mousemove',move);
    window.addEventListener('mouseup',up);
    ev.preventDefault();
  };

  // Group entities by layer → multi-entity tracks
  // Only show root entities (not children of compositions)
  const rootEnts=[...entities].filter(e=>!e.camera && !e.parent_id);
  const trackMap=new Map(); // layer → [entity, ...]
  for(const e of rootEnts){
    const layer=e.layer||0;
    if(!trackMap.has(layer)) trackMap.set(layer,[]);
    trackMap.get(layer).push(e);
  }
  // Sort tracks by layer number
  const trackLayers=[...trackMap.keys()].sort((a,b)=>a-b);

  let th='';
  for(let ti=0;ti<trackLayers.length;ti++){
    const layer=trackLayers[ti];
    const trackEnts=trackMap.get(layer);
    th+=`<div class="tl-track" data-layer="${layer}">`;
    th+=`<div class="tl-track-label">Track ${ti+1} <span style="color:var(--dim);font-size:7px">[${layer}]</span></div>`;
    th+=`<div class="tl-track-bar-area" onmousedown="window._tlBarAreaDown(event)">`;
    // Playhead line in bar area
    th+=`<div class="tl-playhead" style="left:${phPct}%"></div>`;
    // Entity bars with drag handles
    for(const e of trackEnts){
      const ls=e.lifespan||{start:0,end:duration};
      const leftPct=(ls.start/duration)*100;
      const widthPct=((ls.end-ls.start)/duration)*100;
      let cls='color'; if(e.videoSource) cls='video'; else if(e.imageSource) cls='image'; else if(e.textSource) cls='text'; else if(e.composition) cls='comp';
      const isSel=e.id===selectedId;
      th+=`<div class="tl-bar ${cls}${isSel?' sel':''}" style="left:${leftPct}%;width:${widthPct}%" data-eid="${e.id}" onmousedown="window._tlBarDown(event,'${e.id}')" title="${e.id} (${ls.start.toFixed(1)}s → ${ls.end.toFixed(1)}s)">`;
      th+=`<div class="tl-bar-handle left" data-edge="left"></div>`;
      th+=`<span class="tl-bar-label">${e.id}</span>`;
      th+=`<div class="tl-bar-handle right" data-edge="right"></div>`;
      th+=`</div>`;
    }
    th+=`</div></div>`;
  }
  wrap.innerHTML=th;
}

// ═══ TIMELINE BAR INTERACTION ═══
// Click/drag on bar: detect edge vs middle
window._tlBarDown=function(ev, eid){
  ev.stopPropagation();
  const bar=ev.currentTarget;
  const barRect=bar.getBoundingClientRect();
  const area=bar.parentElement;
  const areaRect=area.getBoundingClientRect();
  const ent=entities.find(e=>e.id===eid);
  if(!ent) return;

  // Detect edge: left 6px = trim start, right 6px = trim end, middle = move
  const localX=ev.clientX-barRect.left;
  const edgeZone=6;
  let mode='move';
  if(localX<edgeZone) mode='trim-start';
  else if(localX>barRect.width-edgeZone) mode='trim-end';

  // Select entity
  selectedId=eid;
  refreshHierarchy();refreshInspector();

  const ls=ent.lifespan||{start:0,end:duration};
  const origStart=ls.start, origEnd=ls.end;
  const startX=ev.clientX;

  const pxToTime=px=>(px/areaRect.width)*duration;

  const move=e=>{
    const dx=e.clientX-startX;
    const dt=pxToTime(dx);
    if(mode==='trim-start'){
      ls.start=Math.max(0,Math.min(origEnd-0.1,origStart+dt));
    } else if(mode==='trim-end'){
      ls.end=Math.max(origStart+0.1,origEnd+dt);
    } else { // move
      const len=origEnd-origStart;
      let ns=origStart+dt;
      ns=Math.max(0,ns);
      ls.start=ns;ls.end=ns+len;
    }
    markDirty();render();refreshTimeline();refreshInspector();
  };
  const up=()=>{
    document.body.style.cursor='';
    window.removeEventListener('mousemove',move);
    window.removeEventListener('mouseup',up);
  };
  document.body.style.cursor=mode==='move'?'grabbing':'col-resize';
  window.addEventListener('mousemove',move);
  window.addEventListener('mouseup',up);
};

// Click on empty bar area = deselect (scrub moved to ruler)
window._tlBarAreaDown=ev=>{
  if(ev.target.closest('.tl-bar')) return;
  selectedId=null;
  refreshHierarchy();refreshInspector();refreshTimeline();markDirty();render();
};
window._tlSelect=id=>{selectedId=id;refreshHierarchy();refreshInspector();refreshTimeline();markDirty();render();};

let playbackRAF=null;
function startPlayback(){
  playing=true;$('btn-play').textContent='⏸';
  const sw=performance.now(),st2=time;
  for(const v of Object.values(videoPool)){v.muted=false;v.play().catch(()=>{});}
  function tick(ts){
    if(!playing)return;time=st2+(ts-sw)/1000;
    if(time>=duration){time=0;stopPlayback();return;}
    $('timeline').value=Math.round(time*FPS);
    $('time-display').textContent=`${time.toFixed(2)}s / ${duration.toFixed(2)}s`;
    for(const ent of entities){
      if(!ent.videoSource)continue;const url=sceneAssets[ent.videoSource.assetId]?.url;if(!url)continue;
      const video=videoPool[url];if(video&&video.readyState>=2)cacheVideoFrame(url,time,video);
    }
    markDirty();render();refreshTimeline();syncAudioPlayback();playbackRAF=requestAnimationFrame(tick);
  }
  playbackRAF=requestAnimationFrame(tick);
}
function stopPlayback(){playing=false;$('btn-play').textContent='▶';if(playbackRAF)cancelAnimationFrame(playbackRAF);for(const v of Object.values(videoPool)){v.pause();v.muted=true;}for(const a of Object.values(audioPool)){a.pause();}refreshInspector();}

// ═══ MEDIA IMPORT HELPERS ═══
// Import video: creates Composition parent + VideoSource child
function importVideo(url, mediaDuration=10, name=null) {
  const aid = `asset_${++entityCounter}`;
  const compId = name || `comp_video_${entityCounter}`;
  const childId = `video_${entityCounter}`;
  // Register asset
  sceneAssets[aid] = { type:'video', url };
  // Composition parent
  const comp = createEntity('solid', compId);
  delete comp.colorSource; comp.float_uniforms = {};
  comp.composition = { speed:1.0, trimStart:0, trimEnd:null, duration:'auto', loopMode:'once', materialCascade:true };
  comp.rect = { width:ft(640), height:ft(360), fitMode:'contain' };
  comp.transform.x=ft(0); comp.transform.y=ft(0);
  comp.layer = ++entityCounter;
  comp.lifespan = { start:0, end:mediaDuration };
  entities.push(comp);
  // Child video entity
  const child = createEntity('solid', childId);
  delete child.colorSource; child.float_uniforms = {};
  child.videoSource = { assetId:aid, trimStart:0, trimEnd:null, intrinsicWidth:0, intrinsicHeight:0, duration:mediaDuration, fps:30 };
  child.rect = { width:ft(640), height:ft(360), fitMode:'contain' };
  child.transform.x=ft(0); child.transform.y=ft(0);
  child.parent_id = compId;
  child.lifespan = { start:0, end:mediaDuration };
  child.layer = 0;
  entities.push(child);
  // Load video
  loadVideoFromUrl(url, aid, child);
  return compId;
}

// Import audio: creates Composition parent + AudioSource child
function importAudio(url, mediaDuration=10, name=null) {
  const aid = `asset_${++entityCounter}`;
  const compId = name || `comp_audio_${entityCounter}`;
  const childId = `audio_${entityCounter}`;
  sceneAssets[aid] = { type:'audio', url };
  const comp = createEntity('solid', compId);
  delete comp.colorSource; comp.float_uniforms = {};
  comp.composition = { speed:1.0, trimStart:0, trimEnd:null, duration:'auto', loopMode:'once', materialCascade:false };
  comp.rect = undefined; comp.transform.x=ft(0); comp.transform.y=ft(0);
  comp.layer = ++entityCounter;
  comp.lifespan = { start:0, end:mediaDuration };
  entities.push(comp);
  const child = createEntity('solid', compId);
  child.id = childId;
  delete child.colorSource; child.float_uniforms = {};
  child.audioSource = { assetId:aid, trimStart:0, trimEnd:null, duration:mediaDuration };
  child.rect = undefined;
  child.parent_id = compId;
  child.lifespan = { start:0, end:mediaDuration };
  child.layer = 0;
  entities.push(child);
  // Setup audio element
  setupAudioElement(url, childId);
  return compId;
}

// Audio playback management
const audioPool = {}; // childId → HTMLAudioElement
function setupAudioElement(url, childId) {
  const audio = new Audio(url);
  audio.crossOrigin = 'anonymous';
  audio.preload = 'auto';
  audioPool[childId] = audio;
}
function syncAudioPlayback() {
  for (const e of entities) {
    if (!e.audioSource) continue;
    const audio = audioPool[e.id];
    if (!audio) continue;
    const ls = e.lifespan || {start:0, end:duration};
    const parentEnt = e.parent_id ? entities.find(p=>p.id===e.parent_id) : null;
    const comp = parentEnt?.composition;
    const parentLs = parentEnt?.lifespan || {start:0, end:duration};
    // Calculate effective playback time
    let shouldPlay = false;
    let playbackTime = 0;
    if (time >= parentLs.start && time < parentLs.end) {
      const localTime = (time - parentLs.start) * (comp?.speed||1) + (comp?.trimStart||0);
      if (localTime >= ls.start && localTime < ls.end) {
        shouldPlay = playing;
        playbackTime = (localTime - ls.start) + (e.audioSource.trimStart||0);
      }
    }
    if (shouldPlay) {
      if (audio.paused) audio.play().catch(()=>{});
      if (Math.abs(audio.currentTime - playbackTime) > 0.3) audio.currentTime = playbackTime;
      audio.playbackRate = comp?.speed || 1;
    } else {
      if (!audio.paused) audio.pause();
    }
  }
}

// ═══ DEMO SCENE ═══
function loadDemoScene(){
  entities=[];entityCounter=0;sceneAssets={};duration=6;
  entities.push(createEntity('camera','cam'));
  entityCounter=0;
  const e1=createEntity('solid','blue_box');e1.colorSource={color:{r:srgb2lin(.29),g:srgb2lin(.48),b:srgb2lin(.8),a:1}};e1.rect={width:ft(200),height:ft(150),fitMode:'stretch'};e1.float_uniforms={color_r:ft(srgb2lin(.29)),color_g:ft(srgb2lin(.48)),color_b:ft(srgb2lin(.8)),color_a:ft(1)};e1.transform.x={keyframes:[kf(0,-400),kf(3,0,{type:'cubic_bezier',x1:.42,y1:0,x2:.58,y2:1}),kf(6,400)]};e1.transform.rotation={keyframes:[kf(0,0),kf(6,360)]};e1.layer=1;entities.push(e1);
  const e2=createEntity('solid','pink_box');e2.colorSource={color:{r:srgb2lin(.8),g:srgb2lin(.29),b:srgb2lin(.48),a:1}};e2.rect={width:ft(250),height:ft(180),fitMode:'stretch'};e2.float_uniforms={color_r:ft(srgb2lin(.8)),color_g:ft(srgb2lin(.29)),color_b:ft(srgb2lin(.48)),color_a:ft(1)};e2.transform.x=ft(200);e2.transform.y=ft(-100);e2.opacity={keyframes:[kf(0,0),kf(2,1,{type:'cubic_bezier',x1:.42,y1:0,x2:.58,y2:1})]};e2.transform.scale_x={keyframes:[kf(0,.3),kf(2,1,{type:'cubic_bezier',x1:.2,y1:0,x2:.1,y2:1})]};e2.transform.scale_y={keyframes:[kf(0,.3),kf(2,1,{type:'cubic_bezier',x1:.2,y1:0,x2:.1,y2:1})]};e2.layer=2;entities.push(e2);
  const e3=createEntity('solid','green_box');e3.colorSource={color:{r:srgb2lin(.2),g:srgb2lin(.83),b:srgb2lin(.6),a:1}};e3.rect={width:ft(160),height:ft(120),fitMode:'stretch'};e3.float_uniforms={color_r:ft(srgb2lin(.2)),color_g:ft(srgb2lin(.83)),color_b:ft(srgb2lin(.6)),color_a:ft(1)};e3.transform.x=ft(-250);e3.transform.y={keyframes:[kf(0,200),kf(1.5,-150,{type:'cubic_bezier',x1:.2,y1:0,x2:.1,y2:1}),kf(3,200,{type:'cubic_bezier',x1:.2,y1:0,x2:.1,y2:1}),kf(4.5,-150),kf(6,200)]};e3.layer=3;entities.push(e3);
  const e4=createEntity('solid','orange_dot');e4.colorSource={color:{r:srgb2lin(.98),g:srgb2lin(.57),b:srgb2lin(.14),a:1}};e4.rect={width:ft(80),height:ft(80),fitMode:'stretch'};e4.float_uniforms={color_r:ft(srgb2lin(.98)),color_g:ft(srgb2lin(.57)),color_b:ft(srgb2lin(.14)),color_a:ft(1)};e4.transform.x={keyframes:[kf(0,300),kf(1.5,0),kf(3,-300),kf(4.5,0),kf(6,300)]};e4.transform.y={keyframes:[kf(0,0),kf(1.5,200),kf(3,0),kf(4.5,-200),kf(6,0)]};e4.layer=4;entities.push(e4);
  // Composition group: green + orange inside a group
  const grp = createEntity('solid', 'spin_group');
  grp.colorSource = undefined; grp.float_uniforms = {};
  delete grp.colorSource;
  grp.composition = { speed: 1.0, trimStart: 0, trimEnd: null, duration: 'auto', loopMode: 'loop', materialCascade: true };
  grp.rect = { width: ft(300), height: ft(250), fitMode: 'stretch' };
  grp.transform.x = ft(0); grp.transform.y = ft(0);
  grp.layer = 5; grp.lifespan = { start: 0, end: 10 };
  entities.push(grp);
  // Move green_box and orange_dot inside the group
  e3.parent_id = 'spin_group'; e3.lifespan = { start: 0, end: 4 };
  e4.parent_id = 'spin_group'; e4.lifespan = { start: 1, end: 5 };

  selectedId='blue_box';time=0;$('timeline').value=0;$('duration-input').value=duration;$('timeline').max=Math.round(duration*FPS);$('time-display').textContent=`0.00s / ${duration.toFixed(2)}s`;
  refreshHierarchy();refreshInspector();refreshCameraSelectors();refreshTimeline();markDirty();render();
  $('status').textContent=`Demo loaded — ${entities.length} entities, ${duration}s`;
}

// ═══ TOOLBAR ═══
function setupToolbar(){
  $('btn-new-entity').onclick=()=>{const e=createEntity('solid');entities.push(e);selectedId=e.id;refreshHierarchy();refreshInspector();markDirty();render();};
  $('btn-new-camera').onclick=()=>{const e=createEntity('camera');entities.push(e);selectedId=e.id;refreshHierarchy();refreshInspector();refreshCameraSelectors();markDirty();render();};
  $('btn-dup').onclick=()=>{const src=sel();if(!src||src.camera)return;const dup=JSON.parse(JSON.stringify(src));dup.id=`e${++entityCounter}`;dup.layer=entityCounter;if(dup.transform?.x?.keyframes?.[0])dup.transform.x.keyframes[0].value+=30;if(dup.transform?.y?.keyframes?.[0])dup.transform.y.keyframes[0].value+=30;entities.push(dup);selectedId=dup.id;refreshHierarchy();refreshInspector();markDirty();render();};
  $('btn-del').onclick=()=>{if(!selectedId)return;entities=entities.filter(e=>e.id!==selectedId);selectedId=entities.length?entities[entities.length-1].id:null;refreshHierarchy();refreshInspector();refreshCameraSelectors();markDirty();render();};
  $('btn-hier-add').onclick=$('btn-new-entity').onclick;
  $('btn-add-vp').onclick=addNewViewport;
  $('btn-load-demo').onclick=loadDemoScene;
  // Import media buttons
  const importBar=document.createElement('div');importBar.style.cssText='display:flex;gap:4px;padding:2px 8px;background:var(--panel2);border-bottom:1px solid var(--border)';
  importBar.innerHTML='<button onclick="window._importVideo()">+ Video</button><button onclick="window._importAudio()">+ Audio</button>';
  document.querySelector('.toolbar')?.after(importBar);
  window._importVideo=()=>{const url=prompt('Video URL (HTTP or local):');if(!url)return;const dur=parseFloat(prompt('Duration (s):','10'))||10;const id=importVideo(url,dur);selectedId=id;refreshHierarchy();refreshInspector();refreshTimeline();markDirty();render();};
  window._importAudio=()=>{const url=prompt('Audio URL (HTTP or local):');if(!url)return;const dur=parseFloat(prompt('Duration (s):','10'))||10;const id=importAudio(url,dur);selectedId=id;refreshHierarchy();refreshInspector();refreshTimeline();markDirty();render();};
  $('btn-play').onclick=()=>playing?stopPlayback():startPlayback();
  $('btn-step-back').onclick=()=>{if(playing)stopPlayback();time=Math.max(0,time-1/FPS);$('timeline').value=Math.round(time*FPS);$('time-display').textContent=`${time.toFixed(2)}s / ${duration.toFixed(2)}s`;feedVideoFrames().then(()=>{markDirty();render();refreshInspector();});};
  $('btn-step-fwd').onclick=()=>{if(playing)stopPlayback();time=Math.min(duration,time+1/FPS);$('timeline').value=Math.round(time*FPS);$('time-display').textContent=`${time.toFixed(2)}s / ${duration.toFixed(2)}s`;feedVideoFrames().then(()=>{markDirty();render();refreshInspector();});};
  $('btn-load-json').onclick=()=>$('load-json-file').click();
  $('load-json-file').onchange=async ev=>{const f=ev.target.files[0];if(!f)return;try{const sc=JSON.parse(await f.text());entities=[];entityCounter=0;sceneAssets=sc.assets||{};let me=0;for(const e of(sc.entities||[])){if(e.lifespan)me=Math.max(me,e.lifespan.end||0);if(e.camera||e.type==='camera')entities.push({...e,camera:e.camera||{postEffects:e.post_effects||[]}});else entities.push({...e});entityCounter=Math.max(entityCounter,parseInt((e.id||'').replace(/\D/g,'')||'0'));}if(me>0){duration=me;$('duration-input').value=Math.ceil(duration);$('timeline').max=Math.round(duration*FPS);}selectedId=entities[0]?.id;time=0;refreshHierarchy();refreshInspector();refreshCameraSelectors();markDirty();render();$('status').textContent=`Loaded: ${entities.length} entities`;}catch(e){alert('JSON error: '+e);}};
  $('btn-export-json').onclick=()=>{const sc=buildScene(false);const b=new Blob([JSON.stringify(sc,null,2)],{type:'application/json'});const a=document.createElement('a');a.href=URL.createObjectURL(b);a.download='scene_v2.json';a.click();};
  window.addEventListener('keydown',e=>{if('INPUT SELECT TEXTAREA'.includes(e.target.tagName))return;if(e.code==='Space'){e.preventDefault();playing?stopPlayback():startPlayback();}if(e.code==='ArrowLeft'){e.preventDefault();$('btn-step-back').click();}if(e.code==='ArrowRight'){e.preventDefault();$('btn-step-fwd').click();}if(e.code==='Delete')$('btn-del').click();});
}

// ═══ FPS ═══
let fpsFrames=0,fpsLast=performance.now();
function updateFPS(){fpsFrames++;const n=performance.now();if(n-fpsLast>=1000){$('fps-display').textContent=`${fpsFrames} FPS`;fpsFrames=0;fpsLast=n;}requestAnimationFrame(updateFPS);}

// ═══ BOOT ═══
async function boot(){
  $('status').textContent='Loading WASM...';await init();
  const vp=await createViewport('vp-0','canvas-0','cam');if(!vp)return;
  loadDemoScene();setupToolbar();setupTimeline();setupViewportInteraction();setupViewportMode('vp-0');updateFPS();refreshTimeline();
}
boot();