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
      <h2 id="session-title">Session & Pairing</h2>
      <label for="pair-code">Pairing PIN (from local console)</label>
      <div class="row"><input id="pair-code" type="text" placeholder="e.g. 123456" autocomplete="off" spellcheck="false"><button id="pair-btn">Pair</button></div>
      <details style="margin-top:.8rem">
        <summary style="cursor:pointer;color:#8b949e">Manual token entry</summary>
        <label for="token" style="margin-top:.4rem">Short-lived access token</label>
        <div class="row"><input id="token" type="password" autocomplete="off" spellcheck="false"><button id="connect">Connect</button></div>
      </details>
      <p id="message" role="status">Disconnected</p>
    </section>
    <section aria-labelledby="state-title">
      <h2 id="state-title">Capability state</h2>
      <dl id="state"><dt>Video</dt><dd id="video">Disabled</dd><dt>Active resolution</dt><dd id="active-res">1080p (Native)</dd><dt>Keyboard and pointer</dt><dd id="hid">Disabled</dd><dt>Microphone</dt><dd id="mic">Disabled</dd><dt>Speaker</dt><dd id="speaker">Disabled</dd></dl>
    </section>
    <section aria-labelledby="video-title">
      <h2 id="video-title">Video snapshot & live frame</h2>
      <div class="row">
        <button id="snapshot" disabled>Capture verified snapshot</button>
        <button id="fetch-frame" disabled>Fetch live frame</button>
        <button id="stream-frame" disabled>Start stream</button>
        <label for="video-res" style="display:flex;align-items:center;font-weight:600;margin:0">Resolution:</label>
        <select id="video-res" aria-label="Video resolution">
          <option value="1080p" selected>1080p Native (1920x1080)</option>
          <option value="720p">720p Scaled (1280x720)</option>
        </select>
      </div>
      <pre id="snapshot-result">No snapshot captured.</pre>
      <p class="notice">Snapshots return verified metadata. Fetch live frame streams raw video binary pixels directly to the interactive display canvas via authenticated HTTPS.</p>
    </section>
    <section aria-labelledby="hid-title">
      <h2 id="hid-title">Interactive control</h2>
      <div id="control-area" tabindex="0" role="application" aria-label="Interactive KVM control input area">
        <canvas id="kvm-canvas" width="1920" height="1080"></canvas>
        <div id="canvas-overlay" style="display:flex;justify-content:space-between;align-items:center;padding:.4rem 0">
          <p id="control-status" style="margin:0">Click display to focus input. When focused, keys and mouse are sent to host.</p>
          <span id="stream-badge" style="background:#212b33;color:#7ee787;padding:.2rem .6rem;border-radius:.3rem;font-size:.85rem;font-weight:600">Idle (1080p)</span>
        </div>
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

pub const APP_CSS: &[u8] = br":root{color-scheme:dark;font-family:system-ui,sans-serif;background:#101418;color:#eef4f7}body{margin:0}main{max-width:52rem;margin:auto;padding:2rem}header{border-bottom:1px solid #33404a;margin-bottom:1.5rem}section{background:#182028;border:1px solid #34434e;border-radius:.6rem;margin:1rem 0;padding:1.2rem}.row{display:flex;gap:.6rem;flex-wrap:wrap;align-items:center}input{min-width:0;flex:1}input,button,select{font:inherit;padding:.65rem;border-radius:.35rem;border:1px solid #60717d}select{background:#182028;color:#eef4f7;cursor:pointer}select:disabled{opacity:.45;cursor:not-allowed}button{background:#d0ecff;color:#07131b;font-weight:650;cursor:pointer}button:disabled{opacity:.45;cursor:not-allowed}dl{display:grid;grid-template-columns:1fr auto;gap:.7rem;margin:0}dt,dd{margin:0}dd{font-weight:650}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#0d1115;padding:1rem}.notice{color:#b9c8d1;font-size:.9rem}label{display:block;margin-bottom:.4rem}#control-area{background:#0d1115;border:2px dashed #34434e;border-radius:.4rem;padding:1rem;text-align:center;cursor:pointer;outline:none;user-select:none;margin-bottom:.8rem}#control-area:focus{border-color:#58a6ff;background:#131c24}#control-status{margin:0;color:#8b949e}#control-area:focus #control-status{color:#58a6ff;font-weight:600}#kvm-canvas{width:100%;height:auto;display:block;border-radius:.3rem;background:#05070a;margin-bottom:.8rem}";

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

let streaming = false;
let streamAbort = null;
async function toggleStream() {
  if (streaming) {
    streaming = false;
    if (streamAbort) {
      streamAbort.abort();
      streamAbort = null;
    }
    byId('stream-frame').textContent = 'Start stream';
    return;
  }
  if (!token) return;
  streaming = true;
  byId('stream-frame').textContent = 'Stop stream';
  streamAbort = new AbortController();
  const signal = streamAbort.signal;
  try {
    const res = byId('video-res') ? byId('video-res').value : '1080p';
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
        const seq = view.getUint32(4, false);
        const width = view.getUint16(16, false);
        const height = view.getUint16(18, false);
        const formatByte = fullBuffer[20];
        const flags = fullBuffer[21];
        const framePayload = fullBuffer.subarray(HEADER_LEN, totalFrameLen);
        if (formatByte === 5) {
          if (flags & 1) {
            renderUyvy(byId('kvm-canvas'), framePayload, width, height);
          } else {
            renderTileDelta(byId('kvm-canvas'), framePayload, width, height);
          }
        } else {
          renderFrame(byId('kvm-canvas'), framePayload, width, height, formatByte === 3 ? 'yuv422p' : 'yuv420p');
        }

        const formatName = formatByte === 5 ? (flags & 1 ? 'tile-keyframe' : 'tile-delta') : (formatByte === 3 ? 'yuv422p' : 'yuv420p');
        if (byId('stream-badge')) byId('stream-badge').textContent = `${width}x${height} (${formatName})`;
        byId('snapshot-result').textContent = JSON.stringify({
          stream: true,
          seq,
          width,
          height,
          format: formatName,
          byteLength: payloadLen,
        }, null, 2);
      }
    }
  } catch (e) {
    if (!signal.aborted) {
      byId('message').textContent = e.message;
    }
  } finally {
    streaming = false;
    streamAbort = null;
    byId('stream-frame').textContent = 'Start stream';
    updateResDisplay();
  }
}

async function pair() {
  const code = byId('pair-code').value.trim();
  if (!code) throw new Error('Pairing code is required');
  byId('message').textContent = 'Pairing...';
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
  await initSession();
}

async function connect() {
  token = byId('token').value;
  byId('token').value = '';
  if (!token) throw new Error('A short-lived access token is required');
  await initSession();
}

async function initSession() {
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
  if (byId('video-res')) byId('video-res').disabled = !state.videoEnabled;
  updateResDisplay();
  byId('send-cad').disabled = !state.mutatingOperationsEnabled;
  byId('release-keys').disabled = !state.mutatingOperationsEnabled;
  byId('capture-audio').disabled = !state.audioCaptureEnabled;
  byId('play-audio').disabled = !state.audioOutputEnabled;
  byId('message').textContent = 'Authenticated';
}

async function snapshot() {
  const res = byId('video-res') ? byId('video-res').value : '1080p';
  const width = res === '720p' ? 1280 : 1920;
  const height = res === '720p' ? 720 : 1080;
  const result = await mcp('tools/call', {name:'kvm.video.snapshot',arguments:{width,height,format:'yuv422p',deadlineMs:5000}});
  byId('snapshot-result').textContent = JSON.stringify(result.structuredContent, null, 2);
}

async function fetchFrame() {
  const res = byId('video-res') ? byId('video-res').value : '1080p';
  byId('snapshot-result').textContent = `Fetching live frame (${res}) from /video/frame...`;
  const response = await fetch(`/video/frame?format=yuv422p&res=${res}`, {
    headers: {'Authorization': `Bearer ${token}`}
  });
  if (!response.ok) throw new Error(`Frame request rejected (${response.status})`);
  const digest = response.headers.get('X-Frame-Digest') || '';
  const width = parseInt(response.headers.get('X-Frame-Width') || (res === '720p' ? '1280' : '1920'), 10);
  const height = parseInt(response.headers.get('X-Frame-Height') || (res === '720p' ? '720' : '1080'), 10);
  const format = response.headers.get('X-Frame-Format') || 'yuv422p';
  const buffer = await response.arrayBuffer();
  renderFrame(byId('kvm-canvas'), new Uint8Array(buffer), width, height, format);
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

function updateResDisplay() {
  const resEl = byId('video-res');
  const val = resEl ? resEl.value : '1080p';
  const label = val === '720p' ? '720p (Hardware Scaler)' : '1080p (Native)';
  if (byId('active-res')) byId('active-res').textContent = label;
  if (!streaming && byId('stream-badge')) byId('stream-badge').textContent = `Idle (${val})`;
}
if (byId('video-res')) {
  byId('video-res').addEventListener('change', () => {
    updateResDisplay();
    if (streaming) {
      toggleStream().then(() => toggleStream());
    }
  });
}
function run(action) { action().catch((error) => { byId('message').textContent = error.message; }); }
byId('pair-btn').addEventListener('click', () => run(pair));
byId('pair-code').addEventListener('keydown', (e) => { if (e.key === 'Enter') run(pair); });
byId('connect').addEventListener('click', () => run(connect));
byId('snapshot').addEventListener('click', () => run(snapshot));
byId('fetch-frame').addEventListener('click', () => run(fetchFrame));
byId('stream-frame').addEventListener('click', () => run(toggleStream));
byId('capture-audio').addEventListener('click', () => run(captureAudio));
byId('play-audio').addEventListener('click', () => run(playAudio));
window.addEventListener('pagehide', () => { if (streaming && streamAbort) streamAbort.abort(); streaming = false; token = ''; });
";
