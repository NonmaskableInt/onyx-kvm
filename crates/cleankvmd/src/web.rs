pub const INDEX_HTML: &[u8] = br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>CleanKVM</title>
  <link rel="stylesheet" href="/app.css">
</head>
<body>
  <main>
    <header><h1>CleanKVM</h1><p>Local, authenticated KVM control</p></header>
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
      <h2 id="video-title">Video snapshot</h2>
      <button id="snapshot" disabled>Capture verified snapshot</button>
      <pre id="snapshot-result">No snapshot captured.</pre>
      <p class="notice">This development checkpoint returns authenticated frame metadata. Browser frame pixels and continuous streaming remain gated.</p>
    </section>
    <section aria-labelledby="hid-title">
      <h2 id="hid-title">Interactive control</h2>
      <div id="control-area" tabindex="0" role="application" aria-label="Interactive KVM control input area">
        <p id="control-status">Click to focus input. When focused, keys and mouse are sent to host.</p>
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

pub const APP_CSS: &[u8] = br":root{color-scheme:dark;font-family:system-ui,sans-serif;background:#101418;color:#eef4f7}body{margin:0}main{max-width:52rem;margin:auto;padding:2rem}header{border-bottom:1px solid #33404a;margin-bottom:1.5rem}section{background:#182028;border:1px solid #34434e;border-radius:.6rem;margin:1rem 0;padding:1.2rem}.row{display:flex;gap:.6rem}input{min-width:0;flex:1}input,button{font:inherit;padding:.65rem;border-radius:.35rem;border:1px solid #60717d}button{background:#d0ecff;color:#07131b;font-weight:650}button:disabled{opacity:.45}dl{display:grid;grid-template-columns:1fr auto;gap:.7rem;margin:0}dt,dd{margin:0}dd{font-weight:650}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#0d1115;padding:1rem}.notice{color:#b9c8d1;font-size:.9rem}label{display:block;margin-bottom:.4rem}#control-area{background:#0d1115;border:2px dashed #34434e;border-radius:.4rem;padding:1.5rem;text-align:center;cursor:pointer;outline:none;user-select:none;margin-bottom:.8rem}#control-area:focus{border-color:#58a6ff;background:#131c24}#control-status{margin:0;color:#8b949e}#control-area:focus #control-status{color:#58a6ff;font-weight:600}";

pub const APP_JS: &[u8] = br"'use strict';
let token = '';
let nextId = 1;
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
async function connect() {
  token = byId('token').value;
  byId('token').value = '';
  if (!token) throw new Error('A short-lived access token is required');
  await mcp('initialize', {protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'cleankvm-web',version:'0.1.0'}});
  await fetch('/mcp', {method:'POST',headers:{'Authorization':`Bearer ${token}`,'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})});
  const status = await mcp('tools/call', {name:'kvm.status',arguments:{}});
  const state = status.structuredContent;
  byId('video').textContent = state.videoEnabled ? 'Enabled' : 'Disabled';
  byId('hid').textContent = state.mutatingOperationsEnabled ? 'Enabled' : 'Disabled';
  byId('mic').textContent = state.audioCaptureEnabled ? 'Enabled' : 'Disabled';
  byId('speaker').textContent = state.audioOutputEnabled ? 'Enabled' : 'Disabled';
  byId('snapshot').disabled = !state.videoEnabled;
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
byId('capture-audio').addEventListener('click', () => run(captureAudio));
byId('play-audio').addEventListener('click', () => run(playAudio));
window.addEventListener('pagehide', () => { token = ''; });
";
