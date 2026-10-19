# Phase 4: Continuous media and interactive control architecture

Created: 2026-10-06  
Status: Draft / Approved for implementation

## 1. Architectural principles & security boundaries

Phase 4 transitions Onyx KVM from discrete diagnostic snapshot and probe verification
into an interactive, production-grade remote KVM appliance. The strict security
boundaries established in Phases 1 through 3 remain invariant:

1. **Media worker isolation**: Video and audio brokers run as separate privileged
   daemons communicating solely over root/onyx-kvm Unix domain sockets. They possess
   no network sockets, certificates, or bearer tokens.
2. **Fail-closed capability gates**: Video capture, USB HID, microphone capture,
   and speaker playback each require their respective runtime gate file in
   `/run/onyx-kvm/`. If any gate is absent, the corresponding broker rejects requests.
3. **Zero `/dev/mem`**: Broad physical memory mapping remains permanently disabled.
   Kernel taint must be 0 at boot and transitions only through expected module
   load/unload states.
4. **Separation of control and media transport**:
   - MCP (Model Context Protocol over JSON-RPC) is strictly a control and RPC layer,
     enforcing a 64 KB message boundary (`MAX_MESSAGE_BYTES`). Bulk frame payloads
     must never be serialized inline into MCP messages.
   - Bulk pixel and continuous audio delivery utilize an authenticated HTTPS media
     session initiated through MCP policy authorization.
5. **Bounded memory & zero-residual guarantee**:
   - The SG2002 SOC possesses tightly constrained DDR memory (64 MB system RAM).
     Continuous streaming must utilize a fixed, bounded ring buffer of at most
     2 frames (approx. 6.2 MB for 1080p planar YUV420P).
   - If a client connection experiences latency or backpressure, the ring drops
     stale frames rather than accumulating queues.
   - When a session closes or a gate is removed, all frame and PCM buffers are
     immediately zeroized in RAM and unlinked.

---

## 2. Interactive HID control architecture

### 2.1 Web console input handling
The web console provides a focused interactive container (`<div id="display-container">`
or `<canvas id="kvm-canvas">`):
- **Pointer capture & Mouse Lock**: When the operator clicks the display area, the
  browser requests Pointer Lock API (`requestPointerLock()`) to capture relative
  cursor deltas (`movementX`, `movementY`).
- **Pointer event encoding**:
  - Encoded into standard 4-byte USB HID mouse reports:
    `[buttons, dx, dy, wheel]`
  - `buttons`: bit 0 (Left), bit 1 (Right), bit 2 (Middle).
  - `dx`, `dy`: clamped 8-bit signed integer deltas (`-127..=127`).
  - `wheel`: clamped 8-bit signed scroll wheel delta.
- **Keyboard capture**:
  - Intercepts `keydown` and `keyup` using `event.code`.
  - Maps standard browser codes (e.g., `KeyA`, `Enter`, `ShiftLeft`) to USB HID
    Usage Table page 7 keycodes.
  - Maintains a bitmask of modifiers:
    - Bit 0: Left Ctrl (`0x01`)
    - Bit 1: Left Shift (`0x02`)
    - Bit 2: Left Alt (`0x04`)
    - Bit 3: Left GUI / Meta (`0x08`)
    - Bit 4: Right Ctrl (`0x10`)
    - Bit 5: Right Shift (`0x20`)
    - Bit 6: Right Alt (`0x40`)
    - Bit 7: Right GUI (`0x80`)
  - Formats standard 8-byte USB HID keyboard reports:
    `[modifier, reserved (0), key1, key2, key3, key4, key5, key6]`
- **Emergency release & escape**:
  - Pressing `Escape` or clicking "Release input" exits Pointer Lock.
  - Whenever input focus is lost (`blur`, `pointerlockchange`), an all-zero
    release report is immediately dispatched to avoid stuck keys or pressed mouse buttons.
  - Dedicated UI buttons provide macros (e.g., "Ctrl+Alt+Del").

### 2.2 Broker transport & throttling
- Input events are dispatched via authenticated `kvm.keyboard.send` and `kvm.pointer.send`.
- Pointer motion is throttled to at most 60 reports per second to prevent network saturation.

---

## 3. Bulk pixel delivery & canvas rendering

### 3.1 Media protocol payload extension
In Phase 3, `onyx-kvm-media-protocol` only transferred metadata (`VideoSnapshot`).
Phase 4 adds payload delivery support:
- `Operation::SnapshotFrame`:
  - Request: `SnapshotRequest` (8 bytes).
  - Response: `WireHeader` (payload length = 42 + frame_bytes), followed by
    `VideoSnapshot` (42 bytes), followed by raw planar YUV bytes (3,110,400 bytes).

### 3.2 HTTPS bulk frame endpoint (`GET /video/frame`)
- An authenticated HTTP GET endpoint in `onyx-kvmd`:
  - Header: `Authorization: Bearer <token>`
  - Path: `/video/frame`
  - Validates origin, token signature, expiry, audience, and `kvm:video` scope.
  - Enforces `video-enabled` gate.
  - Connects to `/run/onyx-kvm/video.sock`, captures one live frame, and streams
    raw binary bytes with:
    - `Content-Type: application/octet-stream`
    - `Cache-Control: no-store, no-cache, must-revalidate`
    - `X-Frame-Width: 1920`
    - `X-Frame-Height: 1080`
    - `X-Frame-Format: yuv420p`
    - `X-Frame-Digest: <sha256>`

### 3.3 Browser WebGL / Canvas YUV420P renderer
The browser client receives planar YUV420P (3,110,400 bytes: 1920x1080 Y plane,
followed by 960x540 U plane, followed by 960x540 V plane):
- **WebGL 2.0 conversion**:
  - Three luminance/red textures (`texY`, `texU`, `texV`) uploaded to GPU.
  - Fragment shader computes BT.709 or BT.601 color conversion:
    ```glsl
    precision mediump float;
    varying vec2 v_texCoord;
    uniform sampler2D u_texY;
    uniform sampler2D u_texU;
    uniform sampler2D u_texV;
    void main() {
      float y = texture2D(u_texY, v_texCoord).r;
      float u = texture2D(u_texU, v_texCoord).r - 0.5;
      float v = texture2D(u_texV, v_texCoord).r - 0.5;
      float r = y + 1.402 * v;
      float g = y - 0.344136 * u - 0.714136 * v;
      float b = y + 1.772 * u;
      gl_FragColor = vec4(r, g, b, 1.0);
    }
    ```
  - Blits full 1080p frame at hardware rendering speed with minimal CPU overhead.
- **Fallback 2D Canvas**:
  - For browsers without WebGL, a pure JavaScript 2D ImageData loop performs planar
    YUV420P to RGBA conversion.

---

## 4. Implementation roadmap

1. **Milestone 4.1**: Fix `mutatingOperationsEnabled` in `onyx-kvm-mcp` so HID capability
   state accurately reports in `kvm.status` and the web console. *(Done)*
2. **Milestone 4.2**: Add interactive HID controls to the web console:
   - USB HID usage keycode mapping table.
   - Pointer delta encoding and mouse button bitmasking.
   - Focus/blur safety guards (auto-zeroize on blur).
   - "Release input" and "Ctrl+Alt+Del" UI buttons.
3. **Milestone 4.3**: Implement bulk frame transfer in `onyx-kvm-media-protocol` and
   `onyx-kvm-video-broker`.
4. **Milestone 4.4**: Implement authenticated `/video/frame` endpoint in `onyx-kvmd`.
5. **Milestone 4.5**: Implement WebGL YUV420P canvas renderer in `app.js`.
6. **Milestone 4.6**: Continuous media session streaming with bounded ring buffer
   and backpressure management.
