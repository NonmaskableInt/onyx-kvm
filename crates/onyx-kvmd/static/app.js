'use strict';
let token = '';
let nextId = 1;
let glState = null;
let streaming = false;
let streamAbort = null;
let hidActive = true;
let pointerLockPref = true;
let menuOpen = false;
let dockState = { edge: 'top', pct: 0.5 };
let featureState = { video: true, hid: true, speaker: false, microphone: false };
let audioContext = null;
let audioAbort = null;
let audioStreaming = false;
let audioNextPlayTime = 0;

const byId = (id) => document.getElementById(id);
let lastServerState = null;
let toastTimeout = null;

function showToast(msg) {
  const toast = byId('toast');
  if (!toast) return;
  toast.textContent = msg;
  toast.classList.add('show');
  if (toastTimeout) clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => {
    toast.classList.remove('show');
    toastTimeout = null;
  }, 2200);
}

function getCanvasBlob(canvas) {
  return new Promise((resolve) => {
    if (!canvas || canvas.width === 0 || canvas.height === 0) {
      resolve(null);
      return;
    }
    if (glState && glState.gl) {
      try {
        glState.gl.flush();
      } catch (_) {}
    }
    function fallbackReadPixels() {
      if (glState && glState.gl) {
        try {
          const gl = glState.gl;
          const w = canvas.width;
          const h = canvas.height;
          const pixels = new Uint8Array(w * h * 4);
          gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
          const offscreen = document.createElement('canvas');
          offscreen.width = w;
          offscreen.height = h;
          const ctx2d = offscreen.getContext('2d');
          const imgData = ctx2d.createImageData(w, h);
          const rowBytes = w * 4;
          for (let y = 0; y < h; y++) {
            const srcRow = (h - 1 - y) * rowBytes;
            const dstRow = y * rowBytes;
            imgData.data.set(pixels.subarray(srcRow, srcRow + rowBytes), dstRow);
            for (let x = 0; x < w; x++) {
              imgData.data[dstRow + x * 4 + 3] = 255;
            }
          }
          ctx2d.putImageData(imgData, 0, 0);
          offscreen.toBlob((offBlob) => {
            resolve(offBlob);
          }, 'image/png');
          return;
        } catch (e) {
          console.warn('readPixels fallback failed:', e);
        }
      }
      resolve(null);
    }
    try {
      canvas.toBlob((blob) => {
        if (blob) {
          resolve(blob);
        } else {
          fallbackReadPixels();
        }
      }, 'image/png');
    } catch (_) {
      fallbackReadPixels();
    }
  });
}

async function copyScreenshotToClipboard() {
  const canvas = byId('kvm-canvas');
  if (!canvas) return;

  try {
    const blob = await getCanvasBlob(canvas);
    if (!blob) {
      showToast('Failed to generate screenshot');
      return;
    }
    await navigator.clipboard.write([
      new ClipboardItem({ 'image/png': blob })
    ]);
    const btn = byId('dock-copy-btn');
    if (btn) {
      btn.classList.add('copied');
      setTimeout(() => btn.classList.remove('copied'), 1500);
    }
    showToast('Screenshot copied to clipboard');
  } catch (err) {
    console.warn('Screenshot capture failed:', err);
    showToast('Screenshot error: ' + (err.message || err));
  }
}

function updateSubsystemStatus(state) {
  if (state) lastServerState = state;
  const s = lastServerState;

  const vIcon = byId('subsys-video');
  if (vIcon) {
    const isVidActive = streaming || (s && s.videoEnabled && featureState.video);
    const isGated = !featureState.video;
    vIcon.classList.toggle('active', isVidActive);
    vIcon.classList.toggle('disabled', isGated);
    vIcon.title = isGated ? 'Video: Disabled (Gate Closed)' : (streaming ? 'Video: Streaming' : 'Video: Ready');
  }

  const hIcon = byId('subsys-hid');
  if (hIcon) {
    const isHidActive = hidActive && featureState.hid && (!s || s.mutatingOperationsEnabled);
    const isGated = !featureState.hid;
    hIcon.classList.toggle('active', isHidActive);
    hIcon.classList.toggle('disabled', isGated);
    hIcon.title = isGated ? 'HID: Disabled (Gate Closed)' : (isHidActive ? 'HID: Active (Forwarding)' : 'HID: Inactive');
  }

  const mIcon = byId('subsys-mic');
  if (mIcon) {
    const isMicActive = audioStreaming;
    const isReady = featureState.microphone && (!s || s.audioCaptureEnabled);
    const isGated = !featureState.microphone;
    mIcon.classList.toggle('active', isMicActive);
    mIcon.classList.toggle('disabled', isGated);
    mIcon.title = isGated ? 'Microphone: Disabled (Gate Closed)' : (isMicActive ? 'Microphone: Streaming Audio' : (isReady ? 'Microphone: Ready' : 'Microphone: Gate Blocked'));
  }

  const sIcon = byId('subsys-speaker');
  if (sIcon) {
    const isSpkReady = featureState.speaker && (!s || s.audioOutputEnabled);
    const isGated = !featureState.speaker;
    sIcon.classList.toggle('active', isSpkReady && !isGated);
    sIcon.classList.toggle('disabled', isGated);
    sIcon.title = isGated ? 'Speaker: Disabled (Gate Closed)' : (isSpkReady ? 'Speaker: Ready' : 'Speaker: Gate Blocked');
  }
}

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
  const glOpts = {
    preserveDrawingBuffer: true,
    alpha: false,
    premultipliedAlpha: false,
    antialias: false,
    powerPreference: 'high-performance'
  };
  const gl = canvas.getContext('webgl2', glOpts) || canvas.getContext('webgl', glOpts);
  if (!gl) return null;
  gl.clearColor(0.0, 0.0, 0.0, 1.0);

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

  const fsSourceRgba = 'precision mediump float;' +
    'varying vec2 v_tex;' +
    'uniform sampler2D u_tex;' +
    'void main(){' +
    'vec4 c=texture2D(u_tex,v_tex);' +
    'gl_FragColor=vec4(c.rgb,1.0);' +
    '}';

  const progUyvy = gl.createProgram();
  gl.attachShader(progUyvy, createShader(gl.VERTEX_SHADER, vsSource));
  gl.attachShader(progUyvy, createShader(gl.FRAGMENT_SHADER, fsSourceUyvy));
  gl.linkProgram(progUyvy);

  const progYuv = gl.createProgram();
  gl.attachShader(progYuv, createShader(gl.VERTEX_SHADER, vsSource));
  gl.attachShader(progYuv, createShader(gl.FRAGMENT_SHADER, fsSourceYuv));
  gl.linkProgram(progYuv);

  const progRgba = gl.createProgram();
  gl.attachShader(progRgba, createShader(gl.VERTEX_SHADER, vsSource));
  gl.attachShader(progRgba, createShader(gl.FRAGMENT_SHADER, fsSourceRgba));
  gl.linkProgram(progRgba);

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
  const texRgba = makeTex(gl.LINEAR);

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
    progRgba,
    uTexLocRgba: gl.getUniformLocation(progRgba, 'u_tex'),
    texRgba,
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

let videoDecoder = null;
let currentDecoderCodec = '';
let decoderPendingKeyframe = true;

function extractAvcCodecString(payload) {
  for (let i = 0; i < Math.min(payload.length - 4, 128); i++) {
    if (payload[i] === 0 && payload[i + 1] === 0) {
      let nalStart = -1;
      if (payload[i + 2] === 1) {
        nalStart = i + 3;
      } else if (i + 3 < payload.length && payload[i + 2] === 0 && payload[i + 3] === 1) {
        nalStart = i + 4;
      }
      if (nalStart !== -1 && (payload[nalStart] & 0x1f) === 7 && nalStart + 3 < payload.length) {
        const profile = payload[nalStart + 1].toString(16).padStart(2, '0');
        const compat = payload[nalStart + 2].toString(16).padStart(2, '0');
        const level = payload[nalStart + 3].toString(16).padStart(2, '0');
        return `avc1.${profile}${compat}${level}`;
      }
    }
  }
  return 'avc1.4d4028';
}

function renderVideoFrame(canvas, frame) {
  const width = frame.displayWidth || frame.codedWidth;
  const height = frame.displayHeight || frame.codedHeight;
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  if (!glState) {
    glState = initWebGL(canvas);
  }
  if (glState && glState.progRgba) {
    const { gl, progRgba, texRgba, setupAttribs } = glState;
    gl.viewport(0, 0, width, height);
    gl.useProgram(progRgba);
    setupAttribs(progRgba);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texRgba);
    try {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, frame);
      gl.uniform1i(glState.uTexLocRgba, 0);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
    } catch (_) {
      try {
        const cloned = frame.clone();
        createImageBitmap(cloned).then((bmp) => {
          cloned.close();
          if (glState && glState.gl) {
            const { gl, progRgba, texRgba, setupAttribs } = glState;
            gl.viewport(0, 0, width, height);
            gl.useProgram(progRgba);
            setupAttribs(progRgba);
            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, texRgba);
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, bmp);
            bmp.close();
            gl.uniform1i(glState.uTexLocRgba, 0);
            gl.drawArrays(gl.TRIANGLES, 0, 6);
          }
        }).catch(() => {
          cloned.close();
        });
      } catch (err) {
        console.warn('VideoFrame clone/render fallback failed:', err);
      }
    }
  } else {
    const ctx = canvas.getContext('2d');
    if (ctx) {
      ctx.drawImage(frame, 0, 0, width, height);
    }
  }
}

function destroyVideoDecoder() {
  if (videoDecoder) {
    try {
      if (videoDecoder.state !== 'closed') {
        videoDecoder.close();
      }
    } catch (_) {}
    videoDecoder = null;
  }
  currentDecoderCodec = '';
  decoderPendingKeyframe = true;
}

function initVideoDecoder() {
  destroyVideoDecoder();
  if (typeof VideoDecoder === 'undefined') return false;
  try {
    videoDecoder = new VideoDecoder({
      output: (frame) => {
        try {
          renderVideoFrame(byId('kvm-canvas'), frame);
        } finally {
          frame.close();
        }
      },
      error: (e) => {
        console.warn('VideoDecoder error:', e);
        destroyVideoDecoder();
        decoderPendingKeyframe = true;
      }
    });
    return true;
  } catch (e) {
    console.warn('Failed to construct VideoDecoder:', e);
    videoDecoder = null;
    return false;
  }
}

let frameCounter = 0;
let byteCounter = 0;
let lastFrameTs = 0;
let inStandby = false;
let statsTimer = null;

function startStatsTimer() {
  if (statsTimer) clearInterval(statsTimer);
  statsTimer = setInterval(() => {
    const now = performance.now();
    const dt = now - lastFrameTs;
    const dot = byId('heartbeat-dot');
    const rateEl = byId('net-rate');
    if (streaming && inStandby && dt < 4000) {
      if (dot) dot.className = 'dot dot-standby';
      if (rateEl) rateEl.textContent = 'standby (sleeping)';
    } else if (streaming && dt < 2000) {
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

let streamDesired = false;
let streamReconnectTimer = null;

function showStreamOverlay(title, msg) {
  const overlay = byId('stream-overlay');
  if (!overlay) return;
  const tEl = byId('stream-overlay-title');
  const mEl = byId('stream-overlay-msg');
  if (tEl && title) tEl.textContent = title;
  if (mEl && msg) mEl.textContent = msg;
  overlay.style.display = 'flex';
}

function hideStreamOverlay() {
  const overlay = byId('stream-overlay');
  if (overlay) overlay.style.display = 'none';
}

async function startStream() {
  if (streaming || !token) return;
  if (streamReconnectTimer) {
    clearTimeout(streamReconnectTimer);
    streamReconnectTimer = null;
  }
  streamDesired = true;
  streaming = true;
  updateStreamButton();
  if (frameCounter === 0) {
    showStreamOverlay('Waiting for HDMI video signal...', 'Connecting to video stream...');
  }
  streamAbort = new AbortController();
  const signal = streamAbort.signal;
  try {
    const res = byId('opt-res') ? byId('opt-res').value : '1080p';
    const optFormat = byId('opt-format');
    let reqFormat = optFormat ? optFormat.value : 'h264';
    if (reqFormat === 'h264') {
      if (typeof VideoDecoder === 'undefined' || !initVideoDecoder()) {
        console.warn('WebCodecs VideoDecoder not supported; falling back to tile_delta');
        reqFormat = 'tile_delta';
        if (optFormat) optFormat.value = 'tile_delta';
      }
    } else {
      destroyVideoDecoder();
    }
    const response = await fetch('/video/stream?format=' + reqFormat + '&res=' + res, {
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
        const isStandby = (flags & 4) !== 0;

        if (isStandby) {
          inStandby = true;
          showStreamOverlay('Waiting for HDMI video signal...', 'Target device may be sleeping. Stream connected.');
          decoderPendingKeyframe = true;
          byteCounter += totalFrameLen;
          lastFrameTs = performance.now();
          continue;
        }

        inStandby = false;
        try {
          if (formatByte === 6) {
            if (!videoDecoder || videoDecoder.state === 'closed') {
              initVideoDecoder();
              decoderPendingKeyframe = true;
            }
            if (videoDecoder && videoDecoder.state !== 'closed') {
              const isKey = (flags & 1) !== 0;
              if (decoderPendingKeyframe && !isKey) {
                frameCounter++;
                byteCounter += totalFrameLen;
                lastFrameTs = performance.now();
                continue;
              }
              if (isKey) {
                const codecStr = extractAvcCodecString(framePayload);
                if (currentDecoderCodec !== codecStr || videoDecoder.state === 'unconfigured') {
                  videoDecoder.configure({
                    codec: codecStr,
                    optimizeForLatency: true,
                    hardwareAcceleration: 'prefer-hardware',
                  });
                  currentDecoderCodec = codecStr;
                }
                decoderPendingKeyframe = false;
              }
              if (videoDecoder.state === 'configured') {
                const tsMs = view.getUint32(8, false) * 0x100000000 + view.getUint32(12, false);
                videoDecoder.decode(new EncodedVideoChunk({
                  type: isKey ? 'key' : 'delta',
                  timestamp: tsMs * 1000,
                  data: framePayload,
                }));
              }
            }
          } else if (formatByte === 5) {
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

        hideStreamOverlay();
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
    inStandby = false;
    streamAbort = null;
    destroyVideoDecoder();
    updateStreamButton();
    if (streamDesired && token && !signal.aborted) {
      showStreamOverlay('Waiting for HDMI video signal...', 'No signal detected. Retrying in 3 seconds...');
      streamReconnectTimer = setTimeout(() => {
        if (streamDesired && token) {
          startStream();
        }
      }, 3000);
    }
  }
}

async function stopStream() {
  streamDesired = false;
  inStandby = false;
  if (streamReconnectTimer) {
    clearTimeout(streamReconnectTimer);
    streamReconnectTimer = null;
  }
  hideStreamOverlay();
  if (!streaming) return;
  streaming = false;
  if (streamAbort) {
    streamAbort.abort();
    streamAbort = null;
  }
  destroyVideoDecoder();
  updateStreamButton();
}

async function toggleStream() {
  if (streaming) await stopStream();
  else await startStream();
}

function updateStreamButton() {
  const btn = byId('stream-toggle-btn');
  if (btn) {
    btn.textContent = streaming ? 'Stop Stream' : 'Start Stream';
    btn.classList.toggle('btn-stream-stop', streaming);
    btn.classList.toggle('btn-stream-start', !streaming);
  }
  updateSubsystemStatus();
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
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT' || e.target.closest('button')) return;
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
let accumDx = 0;
let accumDy = 0;
let pointerSpeed = 1.0;
let keyInFlight = false;
let pendingKeyReport = null;
let wakeInFlight = false;
let keepAwakeEnabled = false;
let lastUserActivityTs = performance.now();

async function wakeTarget() {
  if (!token || wakeInFlight) return;
  wakeInFlight = true;
  showToast('Sending wake signal to host...');
  try {
    await mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0, 0, 0x2C, 0, 0, 0, 0, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0, 0, 0, 0, 0, 0, 0, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [1, 0, 0, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0, 0, 0, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0x02, 0, 0, 0, 0, 0, 0, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.keyboard.send', arguments: {report: [0, 0, 0, 0, 0, 0, 0, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0, 2, 2, 0]}});
    await new Promise(r => setTimeout(r, 60));
    await mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0, -2, -2, 0]}});
    showToast('Wake signal sent');
  } catch (err) {
    console.warn('Wake target failed:', err);
    showToast('Wake target error: ' + (err.message || err));
  } finally {
    wakeInFlight = false;
  }
}

function startKeepAwakeTimer() {
  setInterval(async () => {
    if (!keepAwakeEnabled || !token || !hidActive || inStandby || menuOpen) return;
    const idleMs = performance.now() - lastUserActivityTs;
    if (idleMs >= 5000 && !mouseInFlight && !keyInFlight && !wakeInFlight) {
      try {
        await mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0, 1, 0, 0]}});
        await new Promise(r => setTimeout(r, 40));
        await mcp('tools/call', {name: 'kvm.pointer.send', arguments: {report: [0, -1, 0, 0]}});
      } catch (_) {}
    }
  }, 5000);
}

function getVideoGeometry() {
  const canvas = byId('kvm-canvas');
  if (!canvas) return null;
  const rect = canvas.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  const nativeW = canvas.width || 1920;
  const nativeH = canvas.height || 1080;
  const videoAspect = nativeW / nativeH;
  const containerAspect = rect.width / rect.height;

  let dispW, dispH, left, top;
  const fit = canvas.style.objectFit || 'contain';
  if (fit === 'fill') {
    dispW = rect.width;
    dispH = rect.height;
    left = rect.left;
    top = rect.top;
  } else {
    if (containerAspect > videoAspect) {
      dispH = rect.height;
      dispW = rect.height * videoAspect;
      left = rect.left + (rect.width - dispW) / 2;
      top = rect.top;
    } else {
      dispW = rect.width;
      dispH = rect.width / videoAspect;
      left = rect.left;
      top = rect.top + (rect.height - dispH) / 2;
    }
  }

  return {
    rect,
    nativeW,
    nativeH,
    dispW,
    dispH,
    left,
    top,
    scaleX: nativeW / dispW,
    scaleY: nativeH / dispH,
  };
}

function isPointerLocked() {
  const vp = byId('viewport');
  return document.pointerLockElement === vp;
}

function flushPointer() {
  if (!token || !hidActive || mouseInFlight) return;
  lastUserActivityTs = performance.now();
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

function queuePointerMove(rawDx, rawDy, isLocked, clientX, clientY) {
  const geom = getVideoGeometry();

  if (geom && !isLocked && typeof clientX === 'number' && typeof clientY === 'number') {
    const inBounds = clientX >= geom.left &&
                     clientX <= geom.left + geom.dispW &&
                     clientY >= geom.top &&
                     clientY <= geom.top + geom.dispH;
    if (!inBounds) return;
  }

  const scaledDx = rawDx * pointerSpeed;
  const scaledDy = rawDy * pointerSpeed;

  accumDx += scaledDx;
  accumDy += scaledDy;

  const toSendX = Math.round(accumDx);
  const toSendY = Math.round(accumDy);

  accumDx -= toSendX;
  accumDy -= toSendY;

  if (toSendX !== 0 || toSendY !== 0) {
    pendingDx += toSendX;
    pendingDy += toSendY;
    flushPointer();
  }
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
  lastUserActivityTs = performance.now();
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
  accumDx = 0;
  accumDy = 0;
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
    lastUserActivityTs = performance.now();
    if (inStandby) {
      wakeTarget();
      return;
    }
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
    lastUserActivityTs = performance.now();
    if (inStandby) return;
    if (MOD_MAP[e.code]) modifiers &= ~MOD_MAP[e.code];
    else if (KEY_MAP[e.code]) pressedKeys.delete(KEY_MAP[e.code]);
    sendKeyReport();
  }, { passive: false });

  window.addEventListener('blur', releaseAll);

  vp.addEventListener('mousedown', (e) => {
    if (!hidActive || !token) return;
    if (menuOpen) {
      toggleMenu();
      return;
    }
    lastUserActivityTs = performance.now();
    if (inStandby) {
      wakeTarget();
      return;
    }
    vp.focus();
    const locked = isPointerLocked();

    if (pointerLockPref && !locked) {
      vp.requestPointerLock().catch(() => {});
      return;
    }

    if (!locked) {
      const geom = getVideoGeometry();
      if (geom) {
        const inBounds = e.clientX >= geom.left &&
                         e.clientX <= geom.left + geom.dispW &&
                         e.clientY >= geom.top &&
                         e.clientY <= geom.top + geom.dispH;
        if (!inBounds) return;
      }
    }

    queuePointerButton(e.button, true);
  });

  vp.addEventListener('mouseup', (e) => {
    if (!hidActive || !token) return;
    if (menuOpen) return;
    lastUserActivityTs = performance.now();
    if (inStandby) return;
    const locked = isPointerLocked();
    if (!locked && pointerLockPref) return;
    queuePointerButton(e.button, false);
  });

  vp.addEventListener('contextmenu', (e) => e.preventDefault());

  let lastClientX = null;
  let lastClientY = null;

  vp.addEventListener('mousemove', (e) => {
    if (!hidActive || !token) return;
    if (menuOpen) return;
    lastUserActivityTs = performance.now();
    if (inStandby) return;
    const locked = isPointerLocked();

    if (!locked && pointerLockPref) {
      return;
    }

    let dx = e.movementX;
    let dy = e.movementY;
    if (!locked) {
      if (dx === undefined || (dx === 0 && dy === 0 && lastClientX !== null)) {
        dx = e.clientX - lastClientX;
        dy = e.clientY - lastClientY;
      }
      lastClientX = e.clientX;
      lastClientY = e.clientY;
    }
    if (dx !== 0 || dy !== 0) {
      queuePointerMove(dx, dy, locked, e.clientX, e.clientY);
    }
  });

  vp.addEventListener('mouseleave', () => {
    lastClientX = null;
    lastClientY = null;
  });

  vp.addEventListener('wheel', (e) => {
    if (!hidActive || !token) return;
    if (menuOpen) return;
    lastUserActivityTs = performance.now();
    if (inStandby) return;
    const locked = isPointerLocked();
    if (!locked && pointerLockPref) return;
    e.preventDefault();
    queuePointerWheel(e.deltaY < 0 ? 1 : e.deltaY > 0 ? -1 : 0);
  }, { passive: false });

  document.addEventListener('pointerlockchange', () => {
    if (document.pointerLockElement === vp) {
      document.body.classList.add('pointer-locked');
      lastClientX = null;
      lastClientY = null;
    } else {
      document.body.classList.remove('pointer-locked');
      releaseAll();
    }
  });

  document.addEventListener('pointerlockerror', () => {
    document.body.classList.remove('pointer-locked');
    releaseAll();
  });
}

async function fetchFeatures() {
  if (!token) return;
  try {
    const res = await fetch('/features', {
      headers: { 'Authorization': `Bearer ${token}` }
    });
    if (res.ok) {
      featureState = await res.json();
      updateFeatureGateUI();
    }
  } catch (e) {
    console.warn('Failed to fetch features:', e);
  }
}

async function setFeature(name, value) {
  if (!token) return;
  const updated = Object.assign({}, featureState);
  updated[name] = value;
  try {
    const res = await fetch('/features', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(updated)
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Failed to update feature (${res.status})`);
    }
    featureState = await res.json();
    updateFeatureGateUI();
    if (!featureState.video && streaming) {
      await stopStream();
    }
    if (!featureState.microphone && audioStreaming) {
      stopAudioStream();
    }
    if (!featureState.hid) {
      releaseAll();
    }
    await initSession();
  } catch (e) {
    alert('Feature toggle failed: ' + (e.message || e));
    updateFeatureGateUI();
  }
}

function updateFeatureGateUI() {
  const updateToggle = (id, val) => {
    const el = byId(id);
    if (!el) return;
    el.checked = !!val;
  };
  updateToggle('gate-video-btn', featureState.video);
  updateToggle('gate-hid-btn', featureState.hid);
  updateToggle('gate-speaker-btn', featureState.speaker);
  updateToggle('gate-mic-btn', featureState.microphone);

  updateAudioStreamButton();
  updateSubsystemStatus();
}

function updateAudioStreamButton() {
  const btn = byId('stream-audio-btn');
  const vuWrap = byId('vu-meter-wrap');
  if (btn) {
    btn.textContent = audioStreaming ? 'Stop Mic Stream' : 'Start Mic Stream';
    btn.disabled = !featureState.microphone && !audioStreaming;
    btn.classList.toggle('btn-stream-stop', audioStreaming);
    btn.classList.toggle('btn-stream-start', !audioStreaming);
  }
  if (vuWrap) {
    vuWrap.style.display = audioStreaming ? 'block' : 'none';
  }
  updateSubsystemStatus();
}

async function startAudioStream() {
  if (audioStreaming || !token || !featureState.microphone) return;
  audioStreaming = true;
  updateAudioStreamButton();
  audioAbort = new AbortController();
  const signal = audioAbort.signal;

  try {
    const AudioCtx = window.AudioContext || window.webkitAudioContext;
    if (!AudioCtx) throw new Error('Web Audio API not supported in this browser');
    try {
      audioContext = new AudioCtx({ sampleRate: 16000 });
    } catch (_) {
      audioContext = new AudioCtx();
    }
    if (audioContext.state === 'suspended') {
      await audioContext.resume();
    }

    const highpass = audioContext.createBiquadFilter();
    highpass.type = 'highpass';
    highpass.frequency.value = 60;
    highpass.connect(audioContext.destination);

    audioNextPlayTime = audioContext.currentTime + 0.10;

    const response = await fetch('/audio/stream', {
      headers: { 'Authorization': `Bearer ${token}` },
      signal,
    });
    if (!response.ok) throw new Error(`Audio stream rejected (${response.status})`);

    const reader = response.body.getReader();
    let partial = new Uint8Array(0);
    const CHUNK_BYTES = 3200;
    let dcXPrev = 0;
    let dcYPrev = 0;

    while (audioStreaming && !signal.aborted) {
      const { done, value } = await reader.read();
      if (done) break;

      const combined = new Uint8Array(partial.length + value.length);
      combined.set(partial, 0);
      combined.set(value, partial.length);
      partial = combined;

      while (partial.length >= CHUNK_BYTES) {
        const chunk = partial.subarray(0, CHUNK_BYTES);
        partial = partial.subarray(CHUNK_BYTES);

        const sampleCount = chunk.length / 2;
        const view = new DataView(chunk.buffer, chunk.byteOffset, chunk.byteLength);
        const float32 = new Float32Array(sampleCount);
        let sumSq = 0;
        let peak = 0;
        for (let i = 0; i < sampleCount; i++) {
          const s16 = view.getInt16(i * 2, true);
          const raw = s16 / 32768.0;
          const f = raw - dcXPrev + 0.995 * dcYPrev;
          dcXPrev = raw;
          dcYPrev = f;
          float32[i] = f;
          const abs = Math.abs(f);
          if (abs > peak) peak = abs;
          sumSq += abs * abs;
        }
        const rms = Math.sqrt(sumSq / sampleCount);

        const vuBar = byId('vu-meter-bar');
        if (vuBar) {
          const pct = Math.min(100, Math.round(rms * 300));
          vuBar.style.width = pct + '%';
        }

        if (audioContext && audioContext.state !== 'closed') {
          const audioBuffer = audioContext.createBuffer(1, sampleCount, 16000);
          audioBuffer.copyToChannel(float32, 0, 0);
          const source = audioContext.createBufferSource();
          source.buffer = audioBuffer;
          source.connect(highpass);

          const now = audioContext.currentTime;
          if (audioNextPlayTime < now || audioNextPlayTime > now + 0.40) {
            audioNextPlayTime = now + 0.08;
          }
          source.start(audioNextPlayTime);
          audioNextPlayTime += audioBuffer.duration;
        }
      }
    }
  } catch (err) {
    if (!signal.aborted) {
      console.warn('Audio stream error:', err);
    }
  } finally {
    audioStreaming = false;
    audioAbort = null;
    updateAudioStreamButton();
    const vuBar = byId('vu-meter-bar');
    if (vuBar) vuBar.style.width = '0%';
  }
}

function stopAudioStream() {
  if (!audioStreaming) return;
  audioStreaming = false;
  if (audioAbort) {
    audioAbort.abort();
    audioAbort = null;
  }
  if (audioContext && audioContext.state !== 'closed') {
    audioContext.suspend().catch(() => {});
  }
  updateAudioStreamButton();
  const vuBar = byId('vu-meter-bar');
  if (vuBar) vuBar.style.width = '0%';
}

function toggleAudioStream() {
  if (audioStreaming) stopAudioStream();
  else startAudioStream();
}

async function initSession() {
  await mcp('initialize', {protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'onyx-kvm-web',version:'0.1.0'}});
  await fetch('/mcp', {method:'POST',headers:{'Authorization':`Bearer ${token}`,'Content-Type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})});
  const status = await mcp('tools/call', {name:'kvm.status',arguments:{}});
  const state = status.structuredContent;

  await fetchFeatures();

  const vEl = byId('stat-video'); if (vEl) vEl.textContent = state.videoEnabled ? 'Active' : 'Disabled';
  const hEl = byId('stat-hid'); if (hEl) hEl.textContent = state.mutatingOperationsEnabled ? 'Active' : 'Disabled';
  const sEl = byId('stat-speaker'); if (sEl) sEl.textContent = state.audioOutputEnabled ? 'Active' : 'Disabled';
  const mEl = byId('stat-mic'); if (mEl) mEl.textContent = state.audioCaptureEnabled ? 'Active' : 'Disabled';

  const toneBtn = byId('play-tone-btn'); if (toneBtn) toneBtn.disabled = !state.audioOutputEnabled;
  const micBtn = byId('capture-mic-btn'); if (micBtn) micBtn.disabled = !state.audioCaptureEnabled;
  const cadBtn = byId('send-cad-btn'); if (cadBtn) cadBtn.disabled = !state.mutatingOperationsEnabled;
  const relBtn = byId('release-keys-btn'); if (relBtn) relBtn.disabled = !state.mutatingOperationsEnabled;
  updateAudioStreamButton();
  updateSubsystemStatus(state);

  const micStatus = byId('mic-status');
  if (micStatus) {
    micStatus.textContent = state.audioCaptureEnabled ? 'Microphone physical gate active.' : 'Microphone disabled fail-closed.';
  }
  const speakerStatus = byId('speaker-status');
  if (speakerStatus) {
    speakerStatus.textContent = state.audioOutputEnabled ? 'Speaker physical gate active.' : 'Speaker disabled fail-closed.';
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
    stopAudioStream();
    releaseAll();
    menuOpen = false;
    updateMenuOpenClass();
  }
}

let kvmName = 'Onyx KVM';

function setKvmName(name) {
  if (!name) return;
  kvmName = name;
  const dt = byId('drawer-title'); if (dt) dt.textContent = name;
  const dn = byId('dock-name'); if (dn) dn.textContent = name;
  const dock = byId('menu-dock'); if (dock) dock.setAttribute('aria-label', `Menu and Status Dock (${name})`);
  const drawer = byId('menu-drawer'); if (drawer) drawer.setAttribute('aria-label', `${name} Controls`);
  const tb = byId('menu-toggle-btn'); if (tb) { tb.title = `Toggle menu (${name})`; tb.setAttribute('aria-label', `Toggle control menu (${name})`); }
  document.title = `${name} - Onyx KVM`;
}

async function checkSetupStatus() {
  try {
    const res = await fetch('/setup/status');
    if (res.ok) {
      const data = await res.json();
      if (data.name) setKvmName(data.name);
      applySetupMode(!data.configured);
      return data.configured;
    }
  } catch (e) {
    console.warn('Setup status probe failed:', e);
  }
  applySetupMode(false);
  return true;
}

function applySetupMode(isUnconfigured) {
  const panelSetup = byId('panel-setup');
  const panelPin = byId('panel-pin');
  const panelToken = byId('panel-token');
  const authTabs = byId('auth-tabs');
  const subtitle = byId('auth-subtitle');
  if (!panelSetup) return;

  if (isUnconfigured) {
    panelSetup.style.display = 'block';
    panelSetup.classList.add('active');
    if (panelPin) { panelPin.style.display = 'none'; panelPin.classList.remove('active'); }
    if (panelToken) { panelToken.style.display = 'none'; panelToken.classList.remove('active'); }
    if (authTabs) { authTabs.style.display = 'none'; }
    if (subtitle) { subtitle.textContent = 'First-Time Device Setup'; }
    const pinInput = byId('setup-pin');
    if (pinInput) pinInput.focus();
  } else {
    panelSetup.style.display = 'none';
    panelSetup.classList.remove('active');
    if (panelPin) { panelPin.style.display = ''; panelPin.classList.add('active'); }
    if (authTabs) { authTabs.style.display = ''; }
    if (subtitle) { subtitle.textContent = 'Secure Hardware Remote Console'; }
    const pinInput = byId('pair-code');
    if (pinInput) pinInput.focus();
  }
}

async function setupClaim() {
  const pinInput = byId('setup-pin');
  const pin = pinInput ? pinInput.value.trim() : '';
  if (!pin || pin.length < 6 || pin.length > 8) {
    throw new Error('PIN must be between 6 and 8 digits');
  }
  byId('auth-message').textContent = 'Configuring device...';
  const response = await fetch('/setup/claim', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({pin})
  });
  if (!response.ok) {
    const err = await response.json().catch(() => ({error: 'Setup claim failed'}));
    throw new Error(err.error || `Setup rejected (${response.status})`);
  }
  const data = await response.json();
  if (pinInput) pinInput.value = '';
  token = data.token;
  localStorage.setItem('onyx_token', token);
  byId('auth-message').textContent = 'Device secured! Initializing session...';
  applySetupMode(false);
  await initSession();
  setAuthenticated(true);
  showToast('Device PIN established successfully');
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
  stopStream();
  token = '';
  localStorage.removeItem('onyx_token');
  stopAudioStream();
  setAuthenticated(false);
  byId('auth-message').textContent = 'Disconnected';
}

async function rebootDevice() {
  if (!token) return;
  if (!confirm('Are you sure you want to reboot the Onyx KVM device?')) return;
  stopStream();
  showStreamOverlay('Device Rebooting', 'Onyx KVM is rebooting. Reconnecting in 15 seconds...');
  showToast('Device reboot initiated...', false);
  try {
    await fetch('/system/reboot', {
      method: 'POST',
      headers: { 'Authorization': `Bearer ${token}` }
    });
  } catch (e) {
    console.warn('Reboot trigger sent, connection closed:', e);
  }
  let countdown = 15;
  const timer = setInterval(async () => {
    countdown--;
    if (countdown > 0) {
      showStreamOverlay('Device Rebooting', `Onyx KVM is rebooting. Reconnecting in ${countdown}s...`);
    } else {
      clearInterval(timer);
      showStreamOverlay('Reconnecting', 'Attempting to reconnect to Onyx KVM...');
      for (let i = 0; i < 15; i++) {
        try {
          const res = await fetch('/setup/status');
          if (res.ok) {
            window.location.reload();
            return;
          }
        } catch (_) {}
        await new Promise(r => setTimeout(r, 2000));
      }
      window.location.reload();
    }
  }, 1000);
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
  const optFormat = byId('opt-format');
  const savedFormat = localStorage.getItem('onyx_format') || 'h264';
  if (optFormat) {
    optFormat.value = savedFormat;
    optFormat.addEventListener('change', () => {
      localStorage.setItem('onyx_format', optFormat.value);
      restartStream();
    });
  }

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
    const savedLock = localStorage.getItem('onyx_pointerlock');
    pointerLockPref = (savedLock === null) ? true : (savedLock === 'true');
    optLock.checked = pointerLockPref;
    document.body.classList.toggle('no-pointer-lock', !pointerLockPref);
    optLock.addEventListener('change', () => {
      pointerLockPref = optLock.checked;
      localStorage.setItem('onyx_pointerlock', pointerLockPref ? 'true' : 'false');
      document.body.classList.toggle('no-pointer-lock', !pointerLockPref);
    });
  }

  const optSpeed = byId('opt-speed');
  if (optSpeed) {
    const savedSpeed = localStorage.getItem('onyx_speed') || '1.0';
    optSpeed.value = savedSpeed;
    pointerSpeed = parseFloat(savedSpeed) || 1.0;
    optSpeed.addEventListener('change', () => {
      pointerSpeed = parseFloat(optSpeed.value) || 1.0;
      localStorage.setItem('onyx_speed', optSpeed.value);
    });
  }

  const optKeep = byId('opt-keepawake');
  if (optKeep) {
    keepAwakeEnabled = localStorage.getItem('onyx_keepawake') === 'true';
    optKeep.checked = keepAwakeEnabled;
    optKeep.addEventListener('change', () => {
      keepAwakeEnabled = optKeep.checked;
      localStorage.setItem('onyx_keepawake', keepAwakeEnabled ? 'true' : 'false');
      if (keepAwakeEnabled) {
        showToast('Keep host awake enabled');
      }
    });
  }
}

async function resetDefaults() {
  if (!confirm('Reset all KVM settings and admission gates to default values?')) return;
  localStorage.removeItem('onyx_format');
  localStorage.removeItem('onyx_res');
  localStorage.removeItem('onyx_fit');
  localStorage.removeItem('onyx_autostream');
  localStorage.removeItem('onyx_autohid');
  localStorage.removeItem('onyx_pointerlock');
  localStorage.removeItem('onyx_speed');
  localStorage.removeItem('onyx_keepawake');
  localStorage.removeItem('onyx_dock');

  const optFormat = byId('opt-format'); if (optFormat) optFormat.value = 'h264';
  const optRes = byId('opt-res'); if (optRes) optRes.value = '1080p';
  const optFit = byId('opt-fit'); if (optFit) optFit.value = 'contain';
  const canvas = byId('kvm-canvas'); if (canvas) canvas.style.objectFit = 'contain';
  const optAuto = byId('opt-autostream'); if (optAuto) optAuto.checked = true;
  const optHid = byId('opt-autohid'); if (optHid) { optHid.checked = true; hidActive = true; }
  const optLock = byId('opt-pointerlock'); if (optLock) { optLock.checked = true; pointerLockPref = true; document.body.classList.remove('no-pointer-lock'); }
  const optSpeed = byId('opt-speed'); if (optSpeed) { optSpeed.value = '1.0'; pointerSpeed = 1.0; }
  const optKeepReset = byId('opt-keepawake'); if (optKeepReset) { optKeepReset.checked = false; keepAwakeEnabled = false; }

  dockState = { edge: 'top', pct: 0.5 };
  applyDockPosition();
  updateMenuOpenClass();

  try {
    const res = await fetch('/features', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({ video: true, hid: true, speaker: false, microphone: false })
    });
    if (res.ok) {
      featureState = await res.json();
      updateFeatureGateUI();
      await initSession();
    }
  } catch (e) {
    console.warn('Failed to reset features:', e);
  }
}

function initActionButtons() {
  byId('close-drawer-btn').addEventListener('click', toggleMenu);
  byId('menu-toggle-btn').addEventListener('click', (e) => { e.stopPropagation(); toggleMenu(); });
  byId('stream-toggle-btn').addEventListener('click', toggleStream);
  byId('stream-audio-btn').addEventListener('click', toggleAudioStream);
  byId('logout-btn').addEventListener('click', disconnect);
  const rebootBtn = byId('reboot-btn');
  if (rebootBtn) rebootBtn.addEventListener('click', rebootDevice);
  byId('reset-defaults-btn').addEventListener('click', resetDefaults);

  const copyBtn = byId('dock-copy-btn');
  if (copyBtn) copyBtn.addEventListener('click', (e) => { e.stopPropagation(); copyScreenshotToClipboard(); });

  const bindGateToggle = (id, featureKey) => {
    const el = byId(id);
    if (el) {
      el.addEventListener('change', () => setFeature(featureKey, el.checked));
    }
  };
  bindGateToggle('gate-video-btn', 'video');
  bindGateToggle('gate-hid-btn', 'hid');
  bindGateToggle('gate-speaker-btn', 'speaker');
  bindGateToggle('gate-mic-btn', 'microphone');

  const wakeBtn = byId('wake-target-btn');
  if (wakeBtn) wakeBtn.addEventListener('click', wakeTarget);

  const overlayWakeBtn = byId('stream-overlay-wake-btn');
  if (overlayWakeBtn) {
    overlayWakeBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      wakeTarget();
    });
  }

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

  function runAuth(action) {
    action().catch((err) => {
      byId('auth-message').textContent = err.message || String(err);
    });
  }
  const setupBtn = byId('setup-btn');
  if (setupBtn) {
    setupBtn.addEventListener('click', () => runAuth(setupClaim));
  }
  const setupPinInput = byId('setup-pin');
  if (setupPinInput) {
    setupPinInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        runAuth(setupClaim);
      }
    });
  }
  const changePinBtn = byId('change-pin-btn');
  if (changePinBtn) {
    changePinBtn.addEventListener('click', async () => {
      const newPin = prompt('Enter new 4 to 8 digit Device PIN:');
      if (!newPin) return;
      const trimmed = newPin.trim();
      if (trimmed.length < 4 || trimmed.length > 8 || !/^\d+$/.test(trimmed)) {
        alert('PIN must be between 4 and 8 numeric digits');
        return;
      }
      try {
        const res = await fetch('/setup/pin', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': 'Bearer ' + token
          },
          body: JSON.stringify({pin: trimmed})
        });
        if (!res.ok) {
          const err = await res.json().catch(() => ({error: 'Failed to update PIN'}));
          throw new Error(err.error || `HTTP ${res.status}`);
        }
        showToast('Device PIN updated successfully');
      } catch (e) {
        alert('Error updating PIN: ' + (e.message || e));
      }
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
  startKeepAwakeTimer();

  const host = window.location.hostname;
  if (host && host !== 'localhost' && host !== '127.0.0.1' && !/^\d+\.\d+\.\d+\.\d+$/.test(host)) {
    setKvmName(host.replace(/\.local$/, ''));
  }

  await checkSetupStatus();

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
  stopAudioStream();
  releaseAll();
});
