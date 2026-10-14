pub const INDEX_HTML: &[u8] = br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Onyx KVM</title>
  <link rel="stylesheet" href="/app.css">
</head>
<body class="unauth">
  <div id="viewport" tabindex="0" role="application" aria-label="KVM Display and Input Area">
    <canvas id="kvm-canvas" width="1920" height="1080"></canvas>
  </div>

  <div id="menu-dock" class="dock-right" tabindex="0" aria-label="Menu and Status Dock">
    <div id="dock-drag-handle" class="dock-handle" title="Drag to snap along edge">
      <div class="grip-bar"></div>
    </div>
    <div id="dock-status" class="dock-status" title="Stream status and throughput">
      <span id="heartbeat-dot" class="dot dot-idle"></span>
      <span id="net-rate" class="rate-text">idle</span>
    </div>
    <button id="menu-toggle-btn" class="dock-toggle-btn" title="Toggle control menu" aria-label="Toggle control menu">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <line x1="3" y1="6" x2="21" y2="6"></line>
        <line x1="3" y1="12" x2="21" y2="12"></line>
        <line x1="3" y1="18" x2="21" y2="18"></line>
      </svg>
    </button>
  </div>

  <aside id="menu-drawer" class="drawer-right" aria-label="KVM Controls">
    <div class="drawer-header">
      <h2>Onyx KVM Controls</h2>
      <button id="close-drawer-btn" class="close-btn" title="Close menu" aria-label="Close menu">&times;</button>
    </div>
    <div class="drawer-content">
      <section class="drawer-section">
        <div class="section-title">Stream &amp; Display</div>
        <div class="drawer-row">
          <label>Stream Control:</label>
          <button id="stream-toggle-btn" class="btn-sm">Start Stream</button>
        </div>
        <div class="drawer-row">
          <label for="opt-res">Resolution:</label>
          <select id="opt-res">
            <option value="1080p">1080p Native (1920x1080)</option>
            <option value="720p">720p Scaled (1280x720)</option>
          </select>
        </div>
        <div class="drawer-row">
          <label for="opt-fit">Display Fit:</label>
          <select id="opt-fit">
            <option value="contain">Aspect Ratio (Contain)</option>
            <option value="fill">Fill Screen (Stretch)</option>
          </select>
        </div>
        <div class="drawer-row-check">
          <label><input type="checkbox" id="opt-autostream"> Auto-start stream on connect</label>
        </div>
      </section>

      <section class="drawer-section">
        <div class="section-title">Keyboard &amp; Mouse</div>
        <div class="drawer-row-check">
          <label><input type="checkbox" id="opt-autohid"> Forward keyboard &amp; mouse</label>
        </div>
        <div class="drawer-row-check">
          <label><input type="checkbox" id="opt-pointerlock"> Lock pointer on click</label>
        </div>
        <div class="drawer-btns">
          <button id="send-cad-btn" class="btn-sm">Ctrl + Alt + Del</button>
          <button id="release-keys-btn" class="btn-sm">Release Keys</button>
        </div>
        <p class="section-hint">Click stream viewport to focus. Press Esc to release pointer lock.</p>
      </section>

      <section class="drawer-section">
        <div class="section-title">Audio Hardware</div>
        <div class="drawer-btns">
          <button id="play-tone-btn" class="btn-sm" disabled>Play 1s Test Tone</button>
          <button id="capture-mic-btn" class="btn-sm" disabled>Sample Mic</button>
        </div>
        <div id="audio-status" class="status-subtext">Requires physical gate on device.</div>
      </section>

      <section class="drawer-section">
        <div class="section-title">Hardware Capabilities</div>
        <div class="drawer-btns">
          <button id="fetch-frame-btn" class="btn-sm">Capture Snapshot</button>
        </div>
        <div id="diag-summary" class="diag-table">
          <div class="diag-row"><span>Video:</span><span id="stat-video" class="badge">--</span></div>
          <div class="diag-row"><span>Input (HID):</span><span id="stat-hid" class="badge">--</span></div>
          <div class="diag-row"><span>Speaker:</span><span id="stat-speaker" class="badge">--</span></div>
          <div class="diag-row"><span>Microphone:</span><span id="stat-mic" class="badge">--</span></div>
        </div>
      </section>

      <section class="drawer-section">
        <div class="section-title">Session</div>
        <button id="logout-btn" class="btn-danger btn-sm">Disconnect</button>
      </section>
    </div>
  </aside>

  <div id="auth-modal" class="auth-card" role="dialog" aria-modal="true" aria-labelledby="auth-title">
    <div class="auth-brand">
      <h1 id="auth-title">Onyx KVM</h1>
      <p>Secure Hardware Remote Console</p>
    </div>
    <div class="auth-tabs" role="tablist">
      <button id="tab-pin" class="tab-btn active" role="tab" aria-selected="true">Pairing PIN</button>
      <button id="tab-token" class="tab-btn" role="tab" aria-selected="false">Access Token</button>
    </div>
    <div id="panel-pin" class="tab-panel active" role="tabpanel">
      <p class="auth-hint">Enter the 6-digit pairing PIN shown on the target serial console.</p>
      <div class="auth-input-group">
        <input id="pair-code" type="text" placeholder="123456" maxlength="6" inputmode="numeric" autocomplete="one-time-code" autofocus>
        <button id="pair-btn" class="btn-primary">Pair &amp; Connect</button>
      </div>
    </div>
    <div id="panel-token" class="tab-panel" role="tabpanel">
      <p class="auth-hint">Enter a signed JWT access token for manual session authentication.</p>
      <div class="auth-input-group">
        <input id="token-input" type="password" placeholder="Paste access token..." autocomplete="off">
        <button id="token-btn" class="btn-primary">Connect</button>
      </div>
    </div>
    <div id="auth-message" class="auth-status" role="status">Disconnected</div>
  </div>

  <script src="/app.js" defer></script>
</body>
</html>
"#;

pub const APP_CSS: &[u8] = br#":root {
  color-scheme: dark;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  background: #080b0e;
  color: #e6edf3;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  position: relative;
  background: #000;
}

#viewport {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #000;
  overflow: hidden;
  transition: top 0.25s cubic-bezier(0.16, 1, 0.3, 1),
              bottom 0.25s cubic-bezier(0.16, 1, 0.3, 1),
              left 0.25s cubic-bezier(0.16, 1, 0.3, 1),
              right 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  z-index: 10;
  outline: none;
  user-select: none;
}
#kvm-canvas {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
  background: #000;
  pointer-events: none;
}

body.unauth #viewport {
  filter: blur(8px) brightness(0.25);
  pointer-events: none;
}
body.unauth #menu-dock,
body.unauth #menu-drawer {
  display: none !important;
}
body.auth #auth-modal {
  display: none !important;
}

#auth-modal {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 90%;
  max-width: 440px;
  background: rgba(22, 27, 34, 0.95);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border: 1px solid #30363d;
  border-radius: 12px;
  padding: 24px;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.8);
  z-index: 100;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.auth-brand h1 { font-size: 1.5rem; font-weight: 700; color: #f0f6fc; }
.auth-brand p { font-size: 0.85rem; color: #8b949e; margin-top: 2px; }
.auth-tabs {
  display: flex;
  background: #0d1117;
  border-radius: 8px;
  padding: 3px;
  gap: 4px;
  border: 1px solid #21262d;
}
.tab-btn {
  flex: 1;
  background: transparent;
  color: #8b949e;
  border: none;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s ease;
}
.tab-btn.active {
  background: #21262d;
  color: #f0f6fc;
  box-shadow: 0 2px 6px rgba(0,0,0,0.3);
}
.tab-panel { display: none; flex-direction: column; gap: 12px; }
.tab-panel.active { display: flex; }
.auth-hint { font-size: 0.82rem; color: #8b949e; line-height: 1.4; }
.auth-input-group { display: flex; gap: 8px; }
.auth-input-group input {
  flex: 1;
  background: #0d1117;
  border: 1px solid #30363d;
  border-radius: 6px;
  color: #f0f6fc;
  padding: 10px 14px;
  font-size: 0.95rem;
  outline: none;
}
.auth-input-group input:focus { border-color: #58a6ff; }
.btn-primary {
  background: #238636;
  color: #ffffff;
  border: 1px solid rgba(240,246,252,0.1);
  border-radius: 6px;
  padding: 10px 16px;
  font-weight: 600;
  font-size: 0.9rem;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.15s;
}
.btn-primary:hover { background: #2ea043; }
.auth-status {
  font-size: 0.82rem;
  color: #8b949e;
  min-height: 1.2rem;
  text-align: center;
}

#menu-dock {
  position: fixed;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 12px;
  background: rgba(22, 27, 34, 0.88);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 9999px;
  box-shadow: 0 6px 22px rgba(0, 0, 0, 0.6);
  z-index: 60;
  cursor: grab;
  touch-action: none;
  user-select: none;
  transition: box-shadow 0.2s, background 0.2s, top 0.25s, bottom 0.25s, left 0.25s, right 0.25s;
}
#menu-dock.dragging {
  cursor: grabbing;
  background: rgba(30, 38, 48, 0.98);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.8);
  transition: none !important;
}
.dock-handle {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  cursor: grab;
}
.grip-bar {
  width: 4px;
  height: 14px;
  border-radius: 2px;
  background: #6e7681;
}
.dock-status {
  display: flex;
  align-items: center;
  gap: 8px;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
  flex-shrink: 0;
}
.dot-live {
  background: #3fb950;
  box-shadow: 0 0 8px #3fb950;
  animation: pulse-dot 1.8s infinite;
}
.dot-stalled {
  background: #d29922;
  box-shadow: 0 0 6px #d29922;
}
.dot-idle {
  background: #f85149;
  box-shadow: 0 0 6px #f85149;
}
@keyframes pulse-dot {
  0% { transform: scale(0.9); opacity: 0.8; }
  50% { transform: scale(1.2); opacity: 1; }
  100% { transform: scale(0.9); opacity: 0.8; }
}
.rate-text {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 11px;
  color: #8b949e;
  white-space: nowrap;
}
.dock-toggle-btn {
  background: transparent;
  border: none;
  color: #c9d1d9;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border-radius: 4px;
  transition: color 0.15s, background 0.15s;
}
.dock-toggle-btn:hover {
  color: #f0f6fc;
  background: rgba(255, 255, 255, 0.1);
}

body.menu-open-right #viewport { right: 360px; }
body.menu-open-left #viewport { left: 360px; }
body.menu-open-top #viewport { top: 280px; }
body.menu-open-bottom #viewport { bottom: 280px; }

body.menu-open-right #menu-dock.dock-right { right: 372px; }
body.menu-open-left #menu-dock.dock-left { left: 372px; }
body.menu-open-top #menu-dock.dock-top { top: 292px; }
body.menu-open-bottom #menu-dock.dock-bottom { bottom: 292px; }

#menu-drawer {
  position: fixed;
  background: rgba(18, 24, 32, 0.96);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  color: #e6edf3;
  z-index: 50;
  box-sizing: border-box;
  overflow: hidden;
  box-shadow: 0 0 35px rgba(0, 0, 0, 0.85);
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  display: flex;
  flex-direction: column;
}
.drawer-right {
  top: 0;
  right: 0;
  width: 360px;
  height: 100vh;
  border-left: 1px solid #30363d;
  transform: translateX(100%);
}
body.menu-open-right .drawer-right { transform: translateX(0); }

.drawer-left {
  top: 0;
  left: 0;
  width: 360px;
  height: 100vh;
  border-right: 1px solid #30363d;
  transform: translateX(-100%);
}
body.menu-open-left .drawer-left { transform: translateX(0); }

.drawer-top {
  top: 0;
  left: 0;
  width: 100vw;
  height: 280px;
  border-bottom: 1px solid #30363d;
  transform: translateY(-100%);
}
body.menu-open-top .drawer-top { transform: translateY(0); }

.drawer-bottom {
  bottom: 0;
  left: 0;
  width: 100vw;
  height: 280px;
  border-top: 1px solid #30363d;
  transform: translateY(100%);
}
body.menu-open-bottom .drawer-bottom { transform: translateY(0); }

.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid #21262d;
  flex-shrink: 0;
}
.drawer-header h2 { font-size: 1.1rem; font-weight: 600; color: #f0f6fc; }
.close-btn {
  background: transparent;
  border: none;
  color: #8b949e;
  font-size: 1.4rem;
  line-height: 1;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
}
.close-btn:hover { color: #f0f6fc; background: #21262d; }

.drawer-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.drawer-top .drawer-content,
.drawer-bottom .drawer-content {
  flex-direction: row;
  flex-wrap: wrap;
  gap: 20px;
}
.drawer-section {
  background: #0d1117;
  border: 1px solid #21262d;
  border-radius: 8px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.drawer-top .drawer-section,
.drawer-bottom .drawer-section {
  min-width: 220px;
  flex: 1;
}
.section-title {
  font-size: 0.8rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: #58a6ff;
  border-bottom: 1px solid #161b22;
  padding-bottom: 6px;
}
.drawer-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 0.85rem;
}
.drawer-row label { color: #c9d1d9; }
.drawer-row-check {
  display: flex;
  align-items: center;
  font-size: 0.85rem;
  color: #c9d1d9;
}
.drawer-row-check label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}
.drawer-btns {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
select, .btn-sm {
  background: #161b22;
  border: 1px solid #30363d;
  border-radius: 6px;
  color: #c9d1d9;
  padding: 6px 10px;
  font-size: 0.85rem;
  cursor: pointer;
  outline: none;
}
select:focus, .btn-sm:focus { border-color: #58a6ff; }
.btn-sm:hover:not(:disabled) {
  background: #21262d;
  color: #f0f6fc;
}
.btn-sm:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.btn-danger {
  background: #21262d;
  color: #f85149;
  border-color: #da3633;
}
.btn-danger:hover {
  background: #b62324 !important;
  color: #fff !important;
}
.section-hint, .status-subtext {
  font-size: 0.75rem;
  color: #8b949e;
  line-height: 1.3;
}
.diag-table {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 0.8rem;
}
.diag-row {
  display: flex;
  justify-content: space-between;
  color: #8b949e;
}
.badge {
  font-weight: 600;
  color: #c9d1d9;
}
"#;

pub const APP_JS: &[u8] = br#"'use strict';
let token = '';
let nextId = 1;
let glState = null;
let streaming = false;
let streamAbort = null;
let hidActive = true;
let pointerLockPref = false;
let menuOpen = false;
let dockState = { edge: 'right', pct: 0.5 };

const byId = (id) => document.getElementById(id);

async function mcp(method, params) {
  const response = await fetch('/mcp', {
    method: 'POST',
    headers: {'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json'},
    body: JSON.stringify({jsonrpc: '2.0', id: nextId++, method, params})
  });
  if (!response.ok) throw new Error(`Request rejected (${response.status})`);
  const message = await response.json();
  if (message.error) throw new Error(message.error.message || 'MCP request failed');
  return message.result;
}

function initWebGL(canvas) {
  const gl = canvas.getContext('webgl');
  if (!gl) return null;

  function createShader(type, source) {
    const s = gl.createShader(type);
    gl.shaderSource(s, source);
    gl.compileShader(s);
    return s;
  }

  const vsSource = 'attribute vec2 a_pos;attribute vec2 a_tex;varying vec2 v_tex;void main(){gl_Position=vec4(a_pos,0.0,1.0);v_tex=a_tex;}';

  const fsSourceUyvy = 'precision mediump float;' +
    'varying vec2 v_tex;' +
    'uniform sampler2D u_tex;' +
    'uniform float u_width;' +
    'void main(){' +
    'vec4 texel=texture2D(u_tex,v_tex);' +
    'float x=min(floor(v_tex.x*u_width),u_width-1.0);' +
    'float y=mod(x,2.0)<0.5?texel.g:texel.a;' +
    'float u=texel.r-(128.0/255.0);' +
    'float v=texel.b-(128.0/255.0);' +
    'float r=y+1.402*v;' +
    'float g=y-0.344136*u-0.714136*v;' +
    'float b=y+1.772*u;' +
    'gl_FragColor=vec4(r,g,b,1.0);' +
    '}';

  const fsSourceYuv = 'precision mediump float;' +
    'varying vec2 v_tex;' +
    'uniform sampler2D u_y;' +
    'uniform sampler2D u_u;' +
    'uniform sampler2D u_v;' +
    'void main(){' +
    'float y=texture2D(u_y,v_tex).r;' +
    'float u=texture2D(u_u,v_tex).r-(128.0/255.0);' +
    'float v=texture2D(u_v,v_tex).r-(128.0/255.0);' +
    'float r=y+1.402*v;' +
    'float g=y-0.344136*u-0.714136*v;' +
    'float b=y+1.772*u;' +
    'gl_FragColor=vec4(r,g,b,1.0);' +
    '}';

  const progUyvy = gl.createProgram();
  gl.attachShader(progUyvy, createShader(gl.VERTEX_SHADER, vsSource));
  gl.attachShader(progUyvy, createShader(gl.FRAGMENT_SHADER, fsSourceUyvy));
  gl.linkProgram(progUyvy);

  const progYuv = gl.createProgram();
  gl.attachShader(progYuv, createShader(gl.VERTEX_SHADER, vsSource));
  gl.attachShader(progYuv, createShader(gl.FRAGMENT_SHADER, fsSourceYuv));
  gl.linkProgram(progYuv);

  const quad = new Float32Array([-1,-1,0,1, 1,-1,1,1, -1,1,0,0, -1,1,0,0, 1,-1,1,1, 1,1,1,0]);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);

  function setupAttribs(prog) {
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    const aPos = gl.getAttribLocation(prog, 'a_pos');
    const aTex = gl.getAttribLocation(prog, 'a_tex');
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 16, 0);
    gl.enableVertexAttribArray(aTex);
    gl.vertexAttribPointer(aTex, 2, gl.FLOAT, false, 16, 8);
  }

  function makeTex(filter) {
    const f = filter || gl.LINEAR;
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, f);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, f);
    return t;
  }

  const texUyvy = makeTex(gl.NEAREST);
  const texY = makeTex(gl.LINEAR);
  const texU = makeTex(gl.LINEAR);
  const texV = makeTex(gl.LINEAR);

  return {
    gl,
    progUyvy,
    uWidthLoc: gl.getUniformLocation(progUyvy, 'u_width'),
    uTexLoc: gl.getUniformLocation(progUyvy, 'u_tex'),
    texUyvy,
    texWidth: 0,
    texHeight: 0,
    progYuv,
    uYLoc: gl.getUniformLocation(progYuv, 'u_y'),
    uULLoc: gl.getUniformLocation(progYuv, 'u_u'),
    uVLoc: gl.getUniformLocation(progYuv, 'u_v'),
    texY, texU, texV,
    setupAttribs,
  };
}

function renderCanvas2D(canvas, u8Array, width, height) {
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  const ctx = canvas.getContext('2d');
  const imgData = ctx.createImageData(width, height);
  const data = imgData.data;
  const numPixels = width * height;
  let src = 0;
  let dst = 0;
  for (let i = 0; i < numPixels; i += 2) {
    const u = u8Array[src] - 128;
    const y0 = u8Array[src + 1];
    const v = u8Array[src + 2] - 128;
    const y1 = u8Array[src + 3];
    src += 4;
    data[dst]     = Math.min(255, Math.max(0, y0 + 1.402 * v));
    data[dst + 1] = Math.min(255, Math.max(0, y0 - 0.344136 * u - 0.714136 * v));
    data[dst + 2] = Math.min(255, Math.max(0, y0 + 1.772 * u));
    data[dst + 3] = 255;
    data[dst + 4] = Math.min(255, Math.max(0, y1 + 1.402 * v));
    data[dst + 5] = Math.min(255, Math.max(0, y1 - 0.344136 * u - 0.714136 * v));
    data[dst + 6] = Math.min(255, Math.max(0, y1 + 1.772 * u));
    data[dst + 7] = 255;
    dst += 8;
  }
  ctx.putImageData(imgData, 0, 0);
}

function renderCanvas2DYuv(canvas, u8Array, width, height, is422) {
  const ctx = canvas.getContext('2d');
  const imgData = ctx.createImageData(width, height);
  const data32 = new Uint32Array(imgData.data.buffer);
  const yLen = width * height;
  const uvStride = width >> 1;
  const uvHeight = is422 ? height : (height >> 1);
  const uvLen = uvStride * uvHeight;
  const uOffset = yLen;
  const vOffset = yLen + uvLen;
  if (u8Array.byteLength < vOffset + uvLen) return;
  let dst = 0;
  for (let y = 0; y < height; ++y) {
    const yRow = y * width;
    const uvRow = (is422 ? y : (y >> 1)) * uvStride;
    for (let x = 0; x < width; ++x) {
      const yVal = u8Array[yRow + x];
      const uvIdx = uvRow + (x >> 1);
      const u = u8Array[uOffset + uvIdx] - 128;
      const v = u8Array[vOffset + uvIdx] - 128;
      const r = Math.min(255, Math.max(0, (yVal + 1.402 * v) | 0));
      const g = Math.min(255, Math.max(0, (yVal - 0.344136 * u - 0.714136 * v) | 0));
      const b = Math.min(255, Math.max(0, (yVal + 1.772 * u) | 0));
      data32[dst++] = (255 << 24) | (b << 16) | (g << 8) | r;
    }
  }
  ctx.putImageData(imgData, 0, 0);
}

function renderUyvy(canvas, u8Array, width, height) {
  const actualHeight = Math.min(height, Math.floor(u8Array.byteLength / (width * 2)));
  if (actualHeight <= 0) return;
  if (!glState) {
    glState = initWebGL(canvas);
  }
  if (glState && glState.progUyvy) {
    const { gl, progUyvy, texUyvy, uWidthLoc, uTexLoc, setupAttribs } = glState;
    if (canvas.width !== width || canvas.height !== actualHeight) {
      canvas.width = width;
      canvas.height = actualHeight;
    }
    gl.viewport(0, 0, width, actualHeight);
    gl.useProgram(progUyvy);
    setupAttribs(progUyvy);
    gl.uniform1f(uWidthLoc, width);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texUyvy);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    const texWidth = width >> 1;
    const uploadBytes = u8Array.subarray(0, texWidth * actualHeight * 4);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, texWidth, actualHeight, 0, gl.RGBA, gl.UNSIGNED_BYTE, uploadBytes);
    glState.texWidth = texWidth;
    glState.texHeight = actualHeight;
    gl.uniform1i(uTexLoc, 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  } else {
    renderCanvas2D(canvas, u8Array, width, actualHeight);
  }
}

function renderYuvPlanar(canvas, u8Array, width, height, is422) {
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  if (!glState) {
    glState = initWebGL(canvas);
  }
  const yLen = width * height;
  const uvStride = width >> 1;
  const uvHeight = is422 ? height : (height >> 1);
  const uvLen = uvStride * uvHeight;
  const uOffset = yLen;
  const vOffset = yLen + uvLen;
  if (u8Array.byteLength < vOffset + uvLen) return;

  if (glState && glState.progYuv) {
    const { gl, progYuv, texY, texU, texV, uYLoc, uULLoc, uVLoc, setupAttribs } = glState;
    gl.viewport(0, 0, width, height);
    gl.useProgram(progYuv);
    setupAttribs(progYuv);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texY);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, width, height, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, u8Array.subarray(0, yLen));
    gl.uniform1i(uYLoc, 0);

    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, texU);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, uvStride, uvHeight, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, u8Array.subarray(uOffset, uOffset + uvLen));
    gl.uniform1i(uULLoc, 1);

    gl.activeTexture(gl.TEXTURE2);
    gl.bindTexture(gl.TEXTURE_2D, texV);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, uvStride, uvHeight, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, u8Array.subarray(vOffset, vOffset + uvLen));
    gl.uniform1i(uVLoc, 2);

    gl.drawArrays(gl.TRIANGLES, 0, 6);
  } else {
    renderCanvas2DYuv(canvas, u8Array, width, height, is422);
  }
}

function renderTileDelta(canvas, u8Array, width, height) {
  if (u8Array.byteLength < 8) return;
  const dirtyCount = (u8Array[0] << 8) | u8Array[1];
  const tileW = (u8Array[2] << 8) | u8Array[3];
  const tileH = (u8Array[4] << 8) | u8Array[5];
  if (tileW !== 64 || tileH !== 40) return;
  if (!glState) {
    glState = initWebGL(canvas);
  }
  if (!glState || !glState.progUyvy) return;
  const { gl, progUyvy, texUyvy, uWidthLoc, uTexLoc, setupAttribs } = glState;
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  gl.viewport(0, 0, width, height);
  gl.useProgram(progUyvy);
  setupAttribs(progUyvy);
  gl.uniform1f(uWidthLoc, width);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, texUyvy);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  const texTileW = tileW >> 1;
  const texWidth = width >> 1;
  if (glState.texWidth !== texWidth || glState.texHeight !== height) {
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, texWidth, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    glState.texWidth = texWidth;
    glState.texHeight = height;
  }
  const tileBytes = tileW * tileH * 2;
  const tileRecordLen = 2 + tileBytes;
  let offset = 8;
  const cols = Math.floor(width / tileW);
  for (let i = 0; i < dirtyCount; ++i) {
    if (offset + tileRecordLen > u8Array.byteLength) break;
    const tileIdx = (u8Array[offset] << 8) | u8Array[offset + 1];
    const col = tileIdx % cols;
    const row = Math.floor(tileIdx / cols);
    const xTexel = col * texTileW;
    const yTexel = row * tileH;
    const tileData = u8Array.subarray(offset + 2, offset + tileRecordLen);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, xTexel, yTexel, texTileW, tileH, gl.RGBA, gl.UNSIGNED_BYTE, tileData);
    offset += tileRecordLen;
  }
  gl.uniform1i(uTexLoc, 0);
  gl.drawArrays(gl.TRIANGLES, 0, 6);
}

function renderFrame(canvas, u8Array, width, height, format) {
  renderUyvy(canvas, u8Array, width, height);
}

let frameCounter = 0;
let byteCounter = 0;
let lastFrameTs = 0;
let statsTimer = null;

function startStatsTimer() {
  if (statsTimer) clearInterval(statsTimer);
  statsTimer = setInterval(() => {
    const now = performance.now();
    const dt = now - lastFrameTs;
    const dot = byId('heartbeat-dot');
    const rateEl = byId('net-rate');
    if (streaming && dt < 2000) {
      if (dot) dot.className = 'dot dot-live';
      const fps = frameCounter;
      let rateStr = '';
      if (byteCounter >= 1048576) {
        rateStr = (byteCounter / 1048576).toFixed(1) + ' MB/s';
      } else {
        rateStr = Math.round(byteCounter / 1024) + ' KB/s';
      }
      if (rateEl) rateEl.textContent = `${fps} fps | ${rateStr}`;
    } else if (streaming && dt < 6000) {
      if (dot) dot.className = 'dot dot-stalled';
      if (rateEl) rateEl.textContent = 'stalled';
    } else {
      if (dot) dot.className = 'dot dot-idle';
      if (rateEl) rateEl.textContent = streaming ? 'connecting...' : 'idle';
    }
    frameCounter = 0;
    byteCounter = 0;
  }, 1000);
}

async function startStream() {
  if (streaming || !token) return;
  streaming = true;
  updateStreamButton();
  streamAbort = new AbortController();
  const signal = streamAbort.signal;
  try {
    const res = byId('opt-res') ? byId('opt-res').value : '1080p';
    const response = await fetch('/video/stream?format=tile_delta&res=' + res, {
      headers: {'Authorization': `Bearer ${token}`},
      signal: signal,
    });
    if (!response.ok) throw new Error(`Stream rejected (${response.status})`);
    const reader = response.body.getReader();
    let chunks = [];
    let totalLen = 0;
    const HEADER_LEN = 64;

    while (streaming && !signal.aborted) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      totalLen += value.length;

      while (true) {
        if (totalLen < HEADER_LEN) break;

        let headerView;
        if (chunks[0].length >= HEADER_LEN) {
          headerView = new DataView(chunks[0].buffer, chunks[0].byteOffset, HEADER_LEN);
        } else {
          const hdrBuf = new Uint8Array(HEADER_LEN);
          let copied = 0;
          for (const c of chunks) {
            const n = Math.min(c.length, HEADER_LEN - copied);
            hdrBuf.set(c.subarray(0, n), copied);
            copied += n;
            if (copied === HEADER_LEN) break;
          }
          headerView = new DataView(hdrBuf.buffer, 0, HEADER_LEN);
        }

        const payloadLen = headerView.getUint32(24, false);
        const totalFrameLen = HEADER_LEN + payloadLen;

        if (totalLen < totalFrameLen) {
          break;
        }

        const fullBuffer = new Uint8Array(totalFrameLen);
        let offset = 0;
        let consumed = 0;
        let remainder = null;

        for (let i = 0; i < chunks.length; i++) {
          const c = chunks[i];
          const needed = totalFrameLen - offset;
          if (c.length <= needed) {
            fullBuffer.set(c, offset);
            offset += c.length;
            consumed++;
            if (offset === totalFrameLen) break;
          } else {
            fullBuffer.set(c.subarray(0, needed), offset);
            offset += needed;
            consumed++;
            remainder = c.subarray(needed);
            break;
          }
        }

        chunks.splice(0, consumed);
        if (remainder && remainder.length > 0) {
          chunks.unshift(remainder);
        }
        totalLen -= totalFrameLen;

        const view = new DataView(fullBuffer.buffer, fullBuffer.byteOffset, HEADER_LEN);
        const width = view.getUint16(16, false);
        const height = view.getUint16(18, false);
        const formatByte = fullBuffer[20];
        const flags = fullBuffer[21];
        const framePayload = fullBuffer.subarray(HEADER_LEN, totalFrameLen);
        try {
          if (formatByte === 5) {
            if (flags & 1) {
              renderUyvy(byId('kvm-canvas'), framePayload, width, height);
            } else {
              renderTileDelta(byId('kvm-canvas'), framePayload, width, height);
            }
          } else {
            renderFrame(byId('kvm-canvas'), framePayload, width, height, formatByte === 3 ? 'yuv422p' : 'yuv420p');
          }
        } catch (renderErr) {
          console.warn('Frame render error:', renderErr);
        }

        frameCounter++;
        byteCounter += totalFrameLen;
        lastFrameTs = performance.now();
      }
    }
  } catch (e) {
    if (!signal.aborted) {
      console.warn('Stream interrupted:', e);
      const rateEl = byId('net-rate');
      if (rateEl) rateEl.textContent = 'stream error';
      const authMsg = byId('auth-message');
      if (authMsg) authMsg.textContent = 'Stream error: ' + (e.message || e);
    }
  } finally {
    streaming = false;
    streamAbort = null;
    updateStreamButton();
  }
}

async function stopStream() {
  if (!streaming) return;
  streaming = false;
  if (streamAbort) {
    streamAbort.abort();
    streamAbort = null;
  }
  updateStreamButton();
}

async function toggleStream() {
  if (streaming) await stopStream();
  else await startStream();
}

function updateStreamButton() {
  const btn = byId('stream-toggle-btn');
  if (btn) btn.textContent = streaming ? 'Stop Stream' : 'Start Stream';
}

async function restartStream() {
  if (streaming) {
    await stopStream();
    await startStream();
  }
}

function loadDockState() {
  try {
    const raw = localStorage.getItem('onyx_dock');
    if (raw) {
      const parsed = JSON.parse(raw);
      if (['left', 'right', 'top', 'bottom'].includes(parsed.edge) && typeof parsed.pct === 'number') {
        dockState = parsed;
      }
    }
  } catch (_) {}
}

function applyDockPosition() {
  const dock = byId('menu-dock');
  const drawer = byId('menu-drawer');
  if (!dock) return;
  dock.classList.remove('dock-left', 'dock-right', 'dock-top', 'dock-bottom');
  dock.classList.add('dock-' + dockState.edge);
  if (drawer) {
    drawer.className = 'drawer-' + dockState.edge;
  }
  const pct = Math.max(0.08, Math.min(0.92, dockState.pct));
  if (dockState.edge === 'left' || dockState.edge === 'right') {
    dock.style.top = (pct * 100).toFixed(1) + '%';
    dock.style.bottom = 'auto';
    dock.style.left = dockState.edge === 'left' ? '12px' : 'auto';
    dock.style.right = dockState.edge === 'right' ? '12px' : 'auto';
    dock.style.transform = 'translateY(-50%)';
  } else {
    dock.style.left = (pct * 100).toFixed(1) + '%';
    dock.style.right = 'auto';
    dock.style.top = dockState.edge === 'top' ? '12px' : 'auto';
    dock.style.bottom = dockState.edge === 'bottom' ? '12px' : 'auto';
    dock.style.transform = 'translateX(-50%)';
  }
}

function toggleMenu() {
  menuOpen = !menuOpen;
  updateMenuOpenClass();
}

function updateMenuOpenClass() {
  document.body.classList.remove('menu-open-left', 'menu-open-right', 'menu-open-top', 'menu-open-bottom');
  if (menuOpen) {
    document.body.classList.add('menu-open-' + dockState.edge);
  }
}

function initDockDrag() {
  const dock = byId('menu-dock');
  if (!dock) return;
  let isDragging = false;
  let startX = 0;
  let startY = 0;

  dock.addEventListener('pointerdown', (e) => {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
    isDragging = false;
    startX = e.clientX;
    startY = e.clientY;
    dock.setPointerCapture(e.pointerId);
  });

  dock.addEventListener('pointermove', (e) => {
    if (!dock.hasPointerCapture(e.pointerId)) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    if (!isDragging && Math.hypot(dx, dy) > 6) {
      isDragging = true;
      dock.classList.add('dragging');
    }
    if (isDragging) {
      dock.style.left = (e.clientX - dock.offsetWidth / 2) + 'px';
      dock.style.top = (e.clientY - dock.offsetHeight / 2) + 'px';
      dock.style.right = 'auto';
      dock.style.bottom = 'auto';
      dock.style.transform = 'none';
    }
  });

  dock.addEventListener('pointerup', (e) => {
    if (!dock.hasPointerCapture(e.pointerId)) return;
    dock.releasePointerCapture(e.pointerId);
    dock.classList.remove('dragging');
    if (isDragging) {
      isDragging = false;
      const w = window.innerWidth;
      const h = window.innerHeight;
      const dLeft = e.clientX;
      const dRight = w - e.clientX;
      const dTop = e.clientY;
      const dBottom = h - e.clientY;
      const minDist = Math.min(dLeft, dRight, dTop, dBottom);
      if (minDist === dLeft) {
        dockState.edge = 'left';
        dockState.pct = e.clientY / h;
      } else if (minDist === dRight) {
        dockState.edge = 'right';
        dockState.pct = e.clientY / h;
      } else if (minDist === dTop) {
        dockState.edge = 'top';
        dockState.pct = e.clientX / w;
      } else {
        dockState.edge = 'bottom';
        dockState.pct = e.clientX / w;
      }
      localStorage.setItem('onyx_dock', JSON.stringify(dockState));
      applyDockPosition();
      updateMenuOpenClass();
    } else {
      toggleMenu();
    }
  });
}

const KEY_MAP = {
  KeyA:4,KeyB:5,KeyC:6,KeyD:7,KeyE:8,KeyF:9,KeyG:10,KeyH:11,KeyI:12,KeyJ:13,KeyK:14,KeyL:15,
  KeyM:16,KeyN:17,KeyO:18,KeyP:19,KeyQ:20,KeyR:21,KeyS:22,KeyT:23,KeyU:24,KeyV:25,KeyW:26,
  KeyX:27,KeyY:28,KeyZ:29,Digit1:30,Digit2:31,Digit3:32,Digit4:33,Digit5:34,Digit6:35,Digit7:36,
  Digit8:37,Digit9:38,Digit0:39,Enter:40,Escape:41,Backspace:42,Tab:43,Space:44,Minus:45,
  Equal:46,BracketLeft:47,BracketRight:48,Backslash:49,Semicolon:51,Quote:52,Backquote:53,
  Comma:54,Period:55,Slash:56,CapsLock:57,F1:58,F2:59,F3:60,F4:61,F5:62,F6:63,F7:64,F8:65,
  F9:66,F10:67,F11:68,F12:69,PrintScreen:70,ScrollLock:71,Pause:72,Insert:73,Home:74,PageUp:75,
  Delete:76,End:77,PageDown:78,ArrowRight:79,ArrowLeft:80,ArrowDown:81,ArrowUp:82
};
const MOD_MAP = {
  ControlLeft:1,ShiftLeft:2,AltLeft:4,MetaLeft:8,ControlRight:16,ShiftRight:32,AltRight:64,MetaRight:128
};
let pressedKeys = new Set();
let modifiers = 0;
let mouseButtons = 0;
let lastSentButtons = 0;
let mouseInFlight = false;
let pendingDx = 0;
let pendingDy = 0;
let pendingWheel = 0;
let keyInFlight = false;
let pendingKeyReport = null;

function flushPointer() {
  if (!token || !hidActive || mouseInFlight) return;
  const buttonsChanged = (mouseButtons !== lastSentButtons);
  if (pendingDx === 0 && pendingDy === 0 && pendingWheel === 0 && !buttonsChanged) return;

  const dx = pendingDx;
  const dy = pendingDy;
  const wheel = pendingWheel;
  pendingDx = 0;
  pendingDy = 0;
  pendingWheel = 0;
  lastSentButtons = mouseButtons;
  mouseInFlight = true;

  const clamp = (v) => Math.max(-127, Math.min(127, Math.round(v))) & 0xFF;
  mcp('tools/call', {
    name: 'kvm.pointer.send',
    arguments: {report: [mouseButtons & 0x07, clamp(dx), clamp(dy), clamp(wheel)]}
  }).catch(() => {}).finally(() => {
    mouseInFlight = false;
    if (pendingDx !== 0 || pendingDy !== 0 || pendingWheel !== 0 || mouseButtons !== lastSentButtons) {
      flushPointer();
    }
  });
}

function queuePointerMove(dx, dy) {
  pendingDx += dx;
  pendingDy += dy;
  flushPointer();
}

function queuePointerWheel(wheel) {
  pendingWheel += wheel;
  flushPointer();
}

function queuePointerButton(button, isDown) {
  const bit = button === 0 ? 1 : button === 2 ? 2 : button === 1 ? 4 : 0;
  if (isDown) mouseButtons |= bit;
  else mouseButtons &= ~bit;
  flushPointer();
}

function sendKeyReport() {
  if (!token || !hidActive) return;
  const keys = Array.from(pressedKeys).slice(0, 6);
  while (keys.length < 6) keys.push(0);
  const report = [modifiers, 0, ...keys];

  if (keyInFlight) {
    pendingKeyReport = report;
    return;
  }
  keyInFlight = true;

  mcp('tools/call', {
    name: 'kvm.keyboard.send',
    arguments: {report}
  }).catch(() => {}).finally(() => {
    keyInFlight = false;
    if (pendingKeyReport) {
      const nextReport = pendingKeyReport;
      pendingKeyReport = null;
      if (token && hidActive) {
        keyInFlight = true;
        mcp('tools/call', {
          name: 'kvm.keyboard.send',
          arguments: {report: nextReport}
        }).catch(() => {}).finally(() => {
          keyInFlight = false;
          if (pendingKeyReport) sendKeyReport();
        });
      }
    }
  });
}

function releaseAll() {
  pressedKeys.clear();
  modifiers = 0;
  mouseButtons = 0;
  lastSentButtons = 0;
  pendingDx = 0;
  pendingDy = 0;
  pendingWheel = 0;
  pendingKeyReport = null;
  if (!token) return;
  mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0,0,0,0,0,0,0,0]}}).catch(() => {});
  mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0,0,0,0]}}).catch(() => {});
}

function isEditingText(e) {
  const target = e.target;
  if (!target) return false;
  const tag = target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target.isContentEditable;
}

function initInputHandlers() {
  const vp = byId('viewport');
  if (!vp) return;

  window.addEventListener('keydown', (e) => {
    if (!hidActive || !token) return;
    if (isEditingText(e)) return;
    if (document.body.classList.contains('unauth')) return;
    if (menuOpen) return;
    e.preventDefault();
    if (MOD_MAP[e.code]) modifiers |= MOD_MAP[e.code];
    else if (KEY_MAP[e.code]) pressedKeys.add(KEY_MAP[e.code]);
    sendKeyReport();
  }, { passive: false });

  window.addEventListener('keyup', (e) => {
    if (!hidActive || !token) return;
    if (isEditingText(e)) return;
    if (document.body.classList.contains('unauth')) return;
    if (menuOpen) return;
    e.preventDefault();
    if (MOD_MAP[e.code]) modifiers &= ~MOD_MAP[e.code];
    else if (KEY_MAP[e.code]) pressedKeys.delete(KEY_MAP[e.code]);
    sendKeyReport();
  }, { passive: false });

  window.addEventListener('blur', releaseAll);

  vp.addEventListener('mousedown', (e) => {
    if (!hidActive || !token) return;
    vp.focus();
    if (pointerLockPref && document.pointerLockElement !== vp) {
      vp.requestPointerLock().catch(() => {});
    }
    queuePointerButton(e.button, true);
  });

  vp.addEventListener('mouseup', (e) => {
    if (!hidActive || !token) return;
    queuePointerButton(e.button, false);
  });

  vp.addEventListener('contextmenu', (e) => e.preventDefault());

  let lastClientX = null;
  let lastClientY = null;

  vp.addEventListener('mousemove', (e) => {
    if (!hidActive || !token) return;
    if (menuOpen) return;
    let dx = e.movementX;
    let dy = e.movementY;
    if (document.pointerLockElement !== vp) {
      if (dx === undefined || (dx === 0 && dy === 0 && lastClientX !== null)) {
        dx = e.clientX - lastClientX;
        dy = e.clientY - lastClientY;
      }
      lastClientX = e.clientX;
      lastClientY = e.clientY;
    }
    if (dx !== 0 || dy !== 0) {
      queuePointerMove(dx, dy);
    }
  });

  vp.addEventListener('mouseleave', () => {
    lastClientX = null;
    lastClientY = null;
  });

  vp.addEventListener('wheel', (e) => {
    if (!hidActive || !token) return;
    if (menuOpen) return;
    e.preventDefault();
    queuePointerWheel(e.deltaY < 0 ? 1 : e.deltaY > 0 ? -1 : 0);
  }, { passive: false });

  document.addEventListener('pointerlockchange', () => {
    if (document.pointerLockElement !== vp) {
      releaseAll();
    }
  });
}

async function initSession() {
  await mcp('initialize', {protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'onyx-kvm-web',version:'0.1.0'}});
  await fetch('/mcp', {method:'POST',headers:{'Authorization':`Bearer ${token}`,'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})});
  const status = await mcp('tools/call', {name:'kvm.status',arguments:{}});
  const state = status.structuredContent;

  const vEl = byId('stat-video'); if (vEl) vEl.textContent = state.videoEnabled ? 'Active' : 'Disabled';
  const hEl = byId('stat-hid'); if (hEl) hEl.textContent = state.mutatingOperationsEnabled ? 'Active' : 'Disabled';
  const sEl = byId('stat-speaker'); if (sEl) sEl.textContent = state.audioOutputEnabled ? 'Active' : 'Disabled';
  const mEl = byId('stat-mic'); if (mEl) mEl.textContent = state.audioCaptureEnabled ? 'Active' : 'Disabled';

  const toneBtn = byId('play-tone-btn'); if (toneBtn) toneBtn.disabled = !state.audioOutputEnabled;
  const micBtn = byId('capture-mic-btn'); if (micBtn) micBtn.disabled = !state.audioCaptureEnabled;
  const cadBtn = byId('send-cad-btn'); if (cadBtn) cadBtn.disabled = !state.mutatingOperationsEnabled;
  const relBtn = byId('release-keys-btn'); if (relBtn) relBtn.disabled = !state.mutatingOperationsEnabled;
  const snapBtn = byId('fetch-frame-btn'); if (snapBtn) snapBtn.disabled = !state.videoEnabled;

  const audioStatus = byId('audio-status');
  if (audioStatus) {
    if (state.audioOutputEnabled && state.audioCaptureEnabled) {
      audioStatus.textContent = 'Speaker and microphone physical gates active.';
    } else if (state.audioOutputEnabled) {
      audioStatus.textContent = 'Speaker gate active. Microphone disabled.';
    } else if (state.audioCaptureEnabled) {
      audioStatus.textContent = 'Microphone gate active. Speaker disabled.';
    } else {
      audioStatus.textContent = 'Audio gates disabled fail-closed.';
    }
  }

  return state;
}

function setAuthenticated(isAuth) {
  if (isAuth) {
    document.body.classList.remove('unauth');
    document.body.classList.add('auth');
    const autoStream = byId('opt-autostream') ? byId('opt-autostream').checked : true;
    if (autoStream) startStream();
    byId('viewport').focus();
  } else {
    document.body.classList.remove('auth');
    document.body.classList.add('unauth');
    stopStream();
    releaseAll();
    menuOpen = false;
    updateMenuOpenClass();
  }
}

async function pair() {
  const code = byId('pair-code').value.trim();
  if (!code) throw new Error('Pairing PIN is required');
  byId('auth-message').textContent = 'Pairing...';
  const response = await fetch('/pair', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({code})
  });
  if (!response.ok) {
    const err = await response.json().catch(() => ({error: 'Pairing failed'}));
    throw new Error(err.error || `Pairing rejected (${response.status})`);
  }
  const data = await response.json();
  byId('pair-code').value = '';
  token = data.token;
  localStorage.setItem('onyx_token', token);
  byId('auth-message').textContent = 'Authenticated. Initializing...';
  await initSession();
  setAuthenticated(true);
}

async function connectToken() {
  const input = byId('token-input').value.trim();
  if (!input) throw new Error('Access token is required');
  token = input.startsWith('Bearer ') ? input.slice(7) : input;
  byId('token-input').value = '';
  byId('auth-message').textContent = 'Verifying token...';
  localStorage.setItem('onyx_token', token);
  await initSession();
  setAuthenticated(true);
}

function disconnect() {
  token = '';
  localStorage.removeItem('onyx_token');
  setAuthenticated(false);
  byId('auth-message').textContent = 'Disconnected';
}

function initAuthTabs() {
  const tabPin = byId('tab-pin');
  const tabToken = byId('tab-token');
  const panelPin = byId('panel-pin');
  const panelToken = byId('panel-token');
  if (!tabPin || !tabToken) return;

  tabPin.addEventListener('click', () => {
    tabPin.classList.add('active');
    tabPin.setAttribute('aria-selected', 'true');
    tabToken.classList.remove('active');
    tabToken.setAttribute('aria-selected', 'false');
    panelPin.classList.add('active');
    panelToken.classList.remove('active');
  });

  tabToken.addEventListener('click', () => {
    tabToken.classList.add('active');
    tabToken.setAttribute('aria-selected', 'true');
    tabPin.classList.remove('active');
    tabPin.setAttribute('aria-selected', 'false');
    panelToken.classList.add('active');
    panelPin.classList.remove('active');
  });
}

function initOptions() {
  const optRes = byId('opt-res');
  const savedRes = localStorage.getItem('onyx_res') || '1080p';
  if (optRes) {
    optRes.value = savedRes;
    optRes.addEventListener('change', () => {
      localStorage.setItem('onyx_res', optRes.value);
      restartStream();
    });
  }

  const optFit = byId('opt-fit');
  const savedFit = localStorage.getItem('onyx_fit') || 'contain';
  const canvas = byId('kvm-canvas');
  if (optFit) {
    optFit.value = savedFit;
    if (canvas) canvas.style.objectFit = savedFit;
    optFit.addEventListener('change', () => {
      localStorage.setItem('onyx_fit', optFit.value);
      if (canvas) canvas.style.objectFit = optFit.value;
    });
  }

  const optAuto = byId('opt-autostream');
  if (optAuto) {
    optAuto.checked = localStorage.getItem('onyx_autostream') !== 'false';
    optAuto.addEventListener('change', () => {
      localStorage.setItem('onyx_autostream', optAuto.checked ? 'true' : 'false');
    });
  }

  const optHid = byId('opt-autohid');
  if (optHid) {
    hidActive = localStorage.getItem('onyx_autohid') !== 'false';
    optHid.checked = hidActive;
    optHid.addEventListener('change', () => {
      hidActive = optHid.checked;
      localStorage.setItem('onyx_autohid', hidActive ? 'true' : 'false');
      if (!hidActive) releaseAll();
    });
  }

  const optLock = byId('opt-pointerlock');
  if (optLock) {
    pointerLockPref = localStorage.getItem('onyx_pointerlock') === 'true';
    optLock.checked = pointerLockPref;
    optLock.addEventListener('change', () => {
      pointerLockPref = optLock.checked;
      localStorage.setItem('onyx_pointerlock', pointerLockPref ? 'true' : 'false');
    });
  }
}

function initActionButtons() {
  byId('close-drawer-btn').addEventListener('click', toggleMenu);
  byId('menu-toggle-btn').addEventListener('click', (e) => { e.stopPropagation(); toggleMenu(); });
  byId('stream-toggle-btn').addEventListener('click', toggleStream);
  byId('logout-btn').addEventListener('click', disconnect);

  byId('send-cad-btn').addEventListener('click', async () => {
    await mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0x05, 0, 0x4C, 0, 0, 0, 0, 0]}});
    setTimeout(() => {
      mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0, 0, 0, 0, 0, 0, 0, 0]}}).catch(() => {});
    }, 100);
  });
  byId('release-keys-btn').addEventListener('click', releaseAll);

  byId('play-tone-btn').addEventListener('click', async () => {
    const btn = byId('play-tone-btn');
    btn.disabled = true;
    try {
      await mcp('tools/call', {name: 'kvm.audio.play', arguments: {}});
    } catch (e) {
      alert('Audio playback failed: ' + e.message);
    } finally {
      btn.disabled = false;
    }
  });

  byId('capture-mic-btn').addEventListener('click', async () => {
    const btn = byId('capture-mic-btn');
    btn.disabled = true;
    try {
      const res = await mcp('tools/call', {name: 'kvm.audio.capture', arguments: {}});
      alert('Mic sample: ' + JSON.stringify(res.structuredContent));
    } catch (e) {
      alert('Audio capture failed: ' + e.message);
    } finally {
      btn.disabled = false;
    }
  });

  byId('fetch-frame-btn').addEventListener('click', async () => {
    const res = byId('opt-res') ? byId('opt-res').value : '1080p';
    const width = res === '720p' ? 1280 : 1920;
    const height = res === '720p' ? 720 : 1080;
    try {
      const r = await mcp('tools/call', {name:'kvm.video.snapshot',arguments:{width,height,format:'yuv422p',deadlineMs:5000}});
      alert('Verified Snapshot: ' + JSON.stringify(r.structuredContent));
    } catch (e) {
      alert('Snapshot failed: ' + e.message);
    }
  });

  function runAuth(action) {
    action().catch((err) => {
      byId('auth-message').textContent = err.message || String(err);
    });
  }
  byId('pair-btn').addEventListener('click', () => runAuth(pair));
  byId('pair-code').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      runAuth(pair);
    }
  });
  byId('token-btn').addEventListener('click', () => runAuth(connectToken));
  byId('token-input').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      runAuth(connectToken);
    }
  });
}

async function initApp() {
  loadDockState();
  applyDockPosition();
  initDockDrag();
  initAuthTabs();
  initOptions();
  initActionButtons();
  initInputHandlers();
  startStatsTimer();

  const savedToken = localStorage.getItem('onyx_token');
  if (savedToken) {
    token = savedToken;
    byId('auth-message').textContent = 'Restoring session...';
    try {
      await initSession();
      setAuthenticated(true);
      return;
    } catch (e) {
      console.warn('Session restoration failed:', e);
      localStorage.removeItem('onyx_token');
      token = '';
    }
  }
  setAuthenticated(false);
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initApp);
} else {
  initApp();
}

window.addEventListener('pagehide', () => {
  if (streaming && streamAbort) streamAbort.abort();
  streaming = false;
  releaseAll();
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_html_contains_required_elements() {
        let html = std::str::from_utf8(INDEX_HTML).expect("INDEX_HTML must be valid utf-8");
        assert!(html.contains("id=\"viewport\""));
        assert!(html.contains("id=\"kvm-canvas\""));
        assert!(html.contains("id=\"menu-dock\""));
        assert!(html.contains("id=\"menu-drawer\""));
        assert!(html.contains("id=\"auth-modal\""));
        assert!(html.contains("id=\"heartbeat-dot\""));
        assert!(html.contains("id=\"net-rate\""));
        assert!(html.contains("id=\"pair-code\""));
        assert!(html.contains("id=\"token-input\""));
        assert!(html.contains("id=\"close-drawer-btn\""));
        assert!(html.contains("id=\"menu-toggle-btn\""));
    }

    #[test]
    fn app_css_contains_edge_contraction_and_full_screen() {
        let css = std::str::from_utf8(APP_CSS).expect("APP_CSS must be valid utf-8");
        assert!(css.contains("body.menu-open-right #viewport"));
        assert!(css.contains("body.menu-open-left #viewport"));
        assert!(css.contains("body.menu-open-top #viewport"));
        assert!(css.contains("body.menu-open-bottom #viewport"));
        assert!(css.contains("body.unauth #viewport"));
        assert!(css.contains("#kvm-canvas"));
        assert!(css.contains(".dot-live"));
    }

    #[test]
    fn app_js_contains_persistence_and_interactions() {
        let js = std::str::from_utf8(APP_JS).expect("APP_JS must be valid utf-8");
        assert!(js.contains("onyx_dock"));
        assert!(js.contains("onyx_token"));
        assert!(js.contains("onyx_res"));
        assert!(js.contains("onyx_fit"));
        assert!(js.contains("onyx_autostream"));
        assert!(js.contains("onyx_autohid"));
        assert!(js.contains("onyx_pointerlock"));
        assert!(js.contains("setPointerCapture"));
        assert!(js.contains("applyDockPosition"));
        assert!(js.contains("toggleMenu"));
        assert!(js.contains("startStatsTimer"));
    }
}

