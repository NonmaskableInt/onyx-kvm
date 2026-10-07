pub const INDEX_HTML: &[u8] = br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Onyx KVM</title>
  <link rel="stylesheet" href="/app.css">
</head>
<body>
  <main>
    <header><h1>Onyx KVM</h1><p>Local, authenticated KVM control</p></header>
    <section id="session" aria-labelledby="session-title">
      <h2 id="session-title">Session</h2>
      <label for="token">Short-lived access token</label>
      <div class="row"><input id="token" type="password" autocomplete="off" spellcheck="false"><button id="connect">Connect</button></div>
      <p id="message" role="status">Disconnected</p>
    </section>
    <section aria-labelledby="state-title">
      <h2 id="state-title">Capability state</h2>
      <dl id="state"><dt>Video</dt><dd id="video">Disabled</dd><dt>Keyboard and pointer</dt><dd id="hid">Disabled</dd><dt>Microphone</dt><dd id="mic">Disabled</dd><dt>Speaker</dt><dd id="speaker">Disabled</dd></dl>
    </section>
    <section aria-labelledby="video-title">
      <h2 id="video-title">Video snapshot & live frame</h2>
      <div class="row">
        <button id="snapshot" disabled>Capture verified snapshot</button>
        <button id="fetch-frame" disabled>Fetch live frame</button>
        <button id="stream-frame" disabled>Start stream</button>
      </div>
      <pre id="snapshot-result">No snapshot captured.</pre>
      <p class="notice">Snapshots return verified metadata. Fetch live frame streams raw video binary pixels directly to the interactive display canvas via authenticated HTTPS.</p>
    </section>
    <section aria-labelledby="hid-title">
      <h2 id="hid-title">Interactive control</h2>
      <div id="control-area" tabindex="0" role="application" aria-label="Interactive KVM control input area">
        <canvas id="kvm-canvas" width="1920" height="1080"></canvas>
        <p id="control-status">Click display to focus input. When focused, keys and mouse are sent to host.</p>
      </div>
      <div class="row">
        <button id="send-cad" disabled>Send Ctrl+Alt+Del</button>
        <button id="release-keys" disabled>Release input</button>
      </div>
      <p class="notice">Input requires the hid-enabled gate and an authorized mutation token. Input automatically zeroizes on blur.</p>
    </section>
    <section aria-labelledby="audio-title">
      <h2 id="audio-title">Audio</h2>
      <div class="row"><button id="capture-audio" disabled>Measure microphone for one second</button><button id="play-audio" disabled>Play quiet test tone</button></div>
      <pre id="audio-result">No audio operation completed.</pre>
      <p class="notice">Microphone capture returns aggregate levels only. Output is a fixed, low-amplitude one-second tone; each direction has its own physical gate and authorization scope.</p>
    </section>
  </main>
  <script src="/app.js" defer></script>
</body>
</html>
"#;

pub const APP_CSS: &[u8] = br":root{color-scheme:dark;font-family:system-ui,sans-serif;background:#101418;color:#eef4f7}body{margin:0}main{max-width:52rem;margin:auto;padding:2rem}header{border-bottom:1px solid #33404a;margin-bottom:1.5rem}section{background:#182028;border:1px solid #34434e;border-radius:.6rem;margin:1rem 0;padding:1.2rem}.row{display:flex;gap:.6rem}input{min-width:0;flex:1}input,button{font:inherit;padding:.65rem;border-radius:.35rem;border:1px solid #60717d}button{background:#d0ecff;color:#07131b;font-weight:650}button:disabled{opacity:.45}dl{display:grid;grid-template-columns:1fr auto;gap:.7rem;margin:0}dt,dd{margin:0}dd{font-weight:650}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#0d1115;padding:1rem}.notice{color:#b9c8d1;font-size:.9rem}label{display:block;margin-bottom:.4rem}#control-area{background:#0d1115;border:2px dashed #34434e;border-radius:.4rem;padding:1rem;text-align:center;cursor:pointer;outline:none;user-select:none;margin-bottom:.8rem}#control-area:focus{border-color:#58a6ff;background:#131c24}#control-status{margin:0;color:#8b949e}#control-area:focus #control-status{color:#58a6ff;font-weight:600}#kvm-canvas{width:100%;height:auto;display:block;border-radius:.3rem;background:#05070a;margin-bottom:.8rem}";

pub const APP_JS: &[u8] = br"'use strict';
let token = '';
let nextId = 1;
let glState = null;
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
  const vsSource = 'attribute vec2 a_pos;attribute vec2 a_tex;varying vec2 v_tex;void main(){gl_Position=vec4(a_pos,0.0,1.0);v_tex=a_tex;}';
  const fsSource = 'precision mediump float;' +
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
  function createShader(gl, type, source) {
    const s = gl.createShader(type);
    gl.shaderSource(s, source);
    gl.compileShader(s);
    return s;
  }
  const program = gl.createProgram();
  gl.attachShader(program, createShader(gl, gl.VERTEX_SHADER, vsSource));
  gl.attachShader(program, createShader(gl, gl.FRAGMENT_SHADER, fsSource));
  gl.linkProgram(program);
  gl.useProgram(program);
  const quad = new Float32Array([-1,-1,0,1, 1,-1,1,1, -1,1,0,0, -1,1,0,0, 1,-1,1,1, 1,1,1,0]);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
  const aPos = gl.getAttribLocation(program, 'a_pos');
  const aTex = gl.getAttribLocation(program, 'a_tex');
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 16, 0);
  gl.enableVertexAttribArray(aTex);
  gl.vertexAttribPointer(aTex, 2, gl.FLOAT, false, 16, 8);
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  const uWidthLoc = gl.getUniformLocation(program, 'u_width');
  const uTexLoc = gl.getUniformLocation(program, 'u_tex');
  gl.uniform1i(uTexLoc, 0);
  return { gl, tex, program, uWidthLoc };
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

function renderUyvy(canvas, u8Array, width, height) {
  const actualHeight = Math.min(height, Math.floor(u8Array.byteLength / (width * 2)));
  if (actualHeight <= 0) return;
  if (!glState) {
    glState = initWebGL(canvas);
  }
  if (glState) {
    const { gl, tex, program, uWidthLoc } = glState;
    if (canvas.width !== width || canvas.height !== actualHeight) {
      canvas.width = width;
      canvas.height = actualHeight;
    }
    gl.viewport(0, 0, width, actualHeight);
    gl.useProgram(program);
    gl.uniform1f(uWidthLoc, width);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    const texWidth = width >> 1;
    const uploadBytes = u8Array.subarray(0, texWidth * actualHeight * 4);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, texWidth, actualHeight, 0, gl.RGBA, gl.UNSIGNED_BYTE, uploadBytes);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  } else {
    renderCanvas2D(canvas, u8Array, width, actualHeight);
  }
}

let streaming = false;
async function toggleStream() {
  streaming = !streaming;
  byId('stream-frame').textContent = streaming ? 'Stop stream' : 'Start stream';
  while (streaming && token) {
    try {
      await fetchFrame();
    } catch (e) {
      streaming = false;
      byId('stream-frame').textContent = 'Start stream';
      byId('message').textContent = e.message;
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!streaming) {
    byId('stream-frame').textContent = 'Start stream';
  }
}

async function connect() {
  token = byId('token').value;
  byId('token').value = '';
  if (!token) throw new Error('A short-lived access token is required');
  await mcp('initialize', {protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'onyx-kvm-web',version:'0.1.0'}});
  await fetch('/mcp', {method:'POST',headers:{'Authorization':`Bearer ${token}`,'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})});
  const status = await mcp('tools/call', {name:'kvm.status',arguments:{}});
  const state = status.structuredContent;
  byId('video').textContent = state.videoEnabled ? 'Enabled' : 'Disabled';
  byId('hid').textContent = state.mutatingOperationsEnabled ? 'Enabled' : 'Disabled';
  byId('mic').textContent = state.audioCaptureEnabled ? 'Enabled' : 'Disabled';
  byId('speaker').textContent = state.audioOutputEnabled ? 'Enabled' : 'Disabled';
  byId('snapshot').disabled = !state.videoEnabled;
  byId('fetch-frame').disabled = !state.videoEnabled;
  byId('stream-frame').disabled = !state.videoEnabled;
  byId('send-cad').disabled = !state.mutatingOperationsEnabled;
  byId('release-keys').disabled = !state.mutatingOperationsEnabled;
  byId('capture-audio').disabled = !state.audioCaptureEnabled;
  byId('play-audio').disabled = !state.audioOutputEnabled;
  byId('message').textContent = 'Authenticated';
}

async function snapshot() {
  const result = await mcp('tools/call', {name:'kvm.video.snapshot',arguments:{format:'yuv420p',deadlineMs:5000}});
  byId('snapshot-result').textContent = JSON.stringify(result.structuredContent, null, 2);
}

async function fetchFrame() {
  byId('snapshot-result').textContent = 'Fetching live frame from /video/frame...';
  const response = await fetch('/video/frame', {
    headers: {'Authorization': `Bearer ${token}`}
  });
  if (!response.ok) throw new Error(`Frame request rejected (${response.status})`);
  const digest = response.headers.get('X-Frame-Digest') || '';
  const width = parseInt(response.headers.get('X-Frame-Width') || '1920', 10);
  const height = parseInt(response.headers.get('X-Frame-Height') || '1080', 10);
  const format = response.headers.get('X-Frame-Format') || 'uyvy';
  const buffer = await response.arrayBuffer();
  renderUyvy(byId('kvm-canvas'), new Uint8Array(buffer), width, height);
  byId('snapshot-result').textContent = JSON.stringify({
    width, height, format, byteLength: buffer.byteLength, sha256: digest
  }, null, 2);
}

async function captureAudio() {
  byId('mic').textContent = 'Active';
  try {
    const result = await mcp('tools/call', {name:'kvm.audio.capture',arguments:{}});
    byId('audio-result').textContent = JSON.stringify(result.structuredContent, null, 2);
  } finally {
    byId('mic').textContent = 'Enabled';
  }
}

async function playAudio() {
  byId('speaker').textContent = 'Active';
  try {
    const result = await mcp('tools/call', {name:'kvm.audio.play',arguments:{}});
    byId('audio-result').textContent = JSON.stringify(result.structuredContent, null, 2);
  } finally {
    byId('speaker').textContent = 'Enabled';
  }
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

function sendKeyReport() {
  const keys = Array.from(pressedKeys).slice(0, 6);
  while (keys.length < 6) keys.push(0);
  mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [modifiers, 0, ...keys]}}).catch(() => {});
}

function sendPointerReport(dx, dy, wheel) {
  const clamp = (v) => Math.max(-127, Math.min(127, Math.round(v))) & 0xFF;
  mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [mouseButtons & 0x07, clamp(dx), clamp(dy), clamp(wheel)]}}).catch(() => {});
}

function releaseAll() {
  pressedKeys.clear();
  modifiers = 0;
  mouseButtons = 0;
  mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0,0,0,0,0,0,0,0]}}).catch(() => {});
  mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0,0,0,0]}}).catch(() => {});
}

const ctrl = byId('control-area');
ctrl.addEventListener('keydown', (e) => {
  e.preventDefault();
  if (MOD_MAP[e.code]) modifiers |= MOD_MAP[e.code];
  else if (KEY_MAP[e.code]) pressedKeys.add(KEY_MAP[e.code]);
  sendKeyReport();
});
ctrl.addEventListener('keyup', (e) => {
  e.preventDefault();
  if (MOD_MAP[e.code]) modifiers &= ~MOD_MAP[e.code];
  else if (KEY_MAP[e.code]) pressedKeys.delete(KEY_MAP[e.code]);
  sendKeyReport();
});
ctrl.addEventListener('blur', releaseAll);
ctrl.addEventListener('mousedown', (e) => {
  const bit = e.button === 0 ? 1 : e.button === 2 ? 2 : e.button === 1 ? 4 : 0;
  mouseButtons |= bit;
  sendPointerReport(0, 0, 0);
});
ctrl.addEventListener('mouseup', (e) => {
  const bit = e.button === 0 ? 1 : e.button === 2 ? 2 : e.button === 1 ? 4 : 0;
  mouseButtons &= ~bit;
  sendPointerReport(0, 0, 0);
});
ctrl.addEventListener('contextmenu', (e) => e.preventDefault());
ctrl.addEventListener('mousemove', (e) => {
  if (document.activeElement === ctrl && (e.movementX !== 0 || e.movementY !== 0)) {
    sendPointerReport(e.movementX, e.movementY, 0);
  }
});
ctrl.addEventListener('wheel', (e) => {
  e.preventDefault();
  sendPointerReport(0, 0, e.deltaY < 0 ? 1 : e.deltaY > 0 ? -1 : 0);
});
byId('send-cad').addEventListener('click', async () => {
  await mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0x05, 0, 0x4C, 0, 0, 0, 0, 0]}});
  setTimeout(() => {
    mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0, 0, 0, 0, 0, 0, 0, 0]}}).catch(() => {});
  }, 100);
});
byId('release-keys').addEventListener('click', releaseAll);

function run(action) { action().catch((error) => { byId('message').textContent = error.message; }); }
byId('connect').addEventListener('click', () => run(connect));
byId('snapshot').addEventListener('click', () => run(snapshot));
byId('fetch-frame').addEventListener('click', () => run(fetchFrame));
byId('stream-frame').addEventListener('click', () => run(toggleStream));
byId('capture-audio').addEventListener('click', () => run(captureAudio));
byId('play-audio').addEventListener('click', () => run(playAudio));
window.addEventListener('pagehide', () => { streaming = false; token = ''; });
";
