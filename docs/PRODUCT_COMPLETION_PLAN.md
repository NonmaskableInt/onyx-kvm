# Onyx KVM product completion plan

Updated: 2026-10-08

## Product definition

The release target is one independently built NanoKVM Lite image with three
coequal interfaces:

- an authenticated browser interface for human video, keyboard, pointer, and
  explicitly admitted audio controls;
- an authenticated MCP interface using the same scopes, gates, and privileged
  brokers as the browser;
- local recovery through the physical console.

Video, HID, microphone capture, audio output, and virtual media remain separate
admission boundaries. A working video path does not enable audio or HID.

## Phase 1: close the admitted video snapshot contract

Status: complete, including v50 SD read-back, cold-boot fail-closed checks, and
an authenticated 1920x1080 YUV420P MCP snapshot with exact length and digest.

1. Represent planar YUV420 and planar YUV422 as distinct wire formats.
2. Make the broker require the captured byte length to match the requested
   format exactly: 3,110,400 bytes for 1920x1080 YUV420P and 4,147,200 bytes for
   1920x1080 YUV422P.
3. Default MCP snapshot requests to YUV420P because that is the format observed
   through the live v49 broker.
4. Rebuild offline, package the next image, and repeat the fail-closed and
   admitted HTTPS MCP hardware tests. The returned format, length, and digest
   must agree.

Offline artifacts produced after the correction:

- `onyx-kvmd`: static RISC-V daemon
- `onyx-kvm-video-broker`: static RISC-V broker
- verified v50 SD image: `out/phase3-web-video-v50/cleankvm-phase1.img`,
  79,692,288 bytes, SHA-256
  `e57f5b480e0bf4da02350a88ebb8928cf34997ab1c273845c83e3e7732718fe8`
- v50 embedded artifacts: `boot.sd` (`ec4d60fe...`), `rootfs.ext4` (`311cf493...`),
  and `fip.bin` (`6e6d3b5c...`).

## Phase 2: human web console and interactive controls

Status: complete. Embedded web console in `onyx-kvmd` serves same-origin application
with strict CSP, frame-denial, and permission-denial headers. Interactive HID keyboard
and mouse forwarding (Milestone 4.2), `/video/frame` streaming endpoint (Milestone 4.4),
and WebGL UYVY canvas rendering (Milestone 4.5) verified live on hardware.

1. Serve a small static application from the existing TLS daemon; do not add a
   second network service.
2. Complete authorization-code enrollment with short-lived, audience-bound
   grants. Do not place bearer credentials in URLs or persistent browser
   storage.
3. Route status, snapshots, keyboard, and pointer actions through the same
   policy functions and broker transports used by MCP.
4. Add a continuously visible state strip for video, HID, microphone, and
   speaker gates. Microphone activity must remain visible for the entire
   capture lease.
5. Add strict CSP, same-origin enforcement, anti-caching headers, bounded
   request bodies, session expiry, and browser-origin tests.
6. Interactive HID controls: keyboard reports and relative pointer movements with
   pointer lock, safety blur zeroization, and Ctrl+Alt+Del recovery macro.
7. Authenticated `/video/frame` endpoint streaming live raw frames to browser
   with hardware-accelerated WebGL UYVY fragment shader and Canvas 2D fallback.

Acceptance: an authenticated human can view the attached host in real time and
use admitted HID operations without gaining a broader grant than an equivalent
MCP client. Verified on release candidates v57 (`192.168.2.106`), v58 (`192.168.2.107`),
and v59 (`192.168.2.108`).

## Phase 3: audio capture and playback (Phase 3a)

Status: complete. Enumeration, local proofs, independent brokers, and shared MCP/web
controls implemented, built, and packaged. On-device hardware admission passes for
independent capture (16 kHz mono S16LE metrics-only, zero PCM leak) and output
(1 kHz diagnostic tone through AW8010A amplifier), including default-deny, scope
separation, bounded operation, and clean resource release.

1. Run root-only one-second microphone metrics probe under explicit local authorization.
   Confirm capture stops, DMA and file descriptors are released, and buffer is zeroized.
2. Establish physical microphone and DAC/amplifier routing. Keep HDMI audio extraction
   and USB Audio Class disabled until separately proven routed.
3. Implement privileged Rust capture and playback brokers with distinct Unix sockets,
   enable gates, scopes, short leases, one-client limits, fixed PCM shapes, conservative
   playback ceilings, cancellation, and buffer zeroization.
4. Add `kvm.audio.capture` and `kvm.audio.play` as bounded control operations. MCP returns
   metrics or a one-time media-session handle, never unbounded PCM.
5. Add matching web controls and persistent privacy indication. Capture and playback
   remain independently disabled by default and fail closed upon gate teardown.

Acceptance: microphone capture and audio output each pass hardware, privacy, cleanup,
authorization, timeout, and cold-boot fail-closed tests on physical hardware.

## Phase 4: continuous media and release engineering

Status: full-stack rebranding to Onyx KVM (v59) successfully assembled, flashed, and
verified on target hardware (`192.168.2.108`). Baseline invariants, dual-issuer JWT
authentication, broker fallbacks, and all four capability gates verified. Transitioning
to continuous media streaming sessions and release hardening.

1. **Phase 4.1: Continuous Media & Low-Latency Streaming Sessions**:
   - Short-lived, authenticated continuous streaming sessions with explicit renewal leases.
   - Bounded ring buffers with backpressure frame-drop policy and zero-residual memory erasure.
   - Low-overhead binary stream transport (chunked frame stream or binary WebSocket)
     delivering frames with sequence IDs, timestamps, and integrity digests.
2. **Phase 4.2: Production Hardening & Rootfs Integrity**:
   - Read-only rootfs with tmpfs overlays for `/run` and `/tmp`.
   - Production PKI enrollment and local pairing ceremony replacing development tokens.
   - Cryptographically signed offline update packaging and verification tooling.
3. **Phase 4.3: Release Engineering & Packaging**:
   - Complete release acceptance matrix across boot, network, MCP, web, video, HID,
     audio, and teardown.
   - Two-builder reproducible build verification and automated artifact hashing.

## Immediate execution order

1. **Release Candidate v59 Rebranding & Hardware Acceptance** [COMPLETED]:
   - All crates rebranded to `onyx-kvm-*` and `onyx-kvmd`.
   - Built and assembled `out/product-v59/onyx-kvm-phase1.img` (79,692,288 bytes).
   - Verified on physical hardware `192.168.2.108`: cold boot baseline, HID forwarding,
     1080p live video streaming, audio capture metrics, audio tone playback, and fail-closed teardown.
2. **Phase 4.1: Continuous Media & Low-Latency Streaming Session Architecture** [IN PROGRESS]:
   - Design and implement bounded media streaming session management in `onyx-kvmd`.
   - Single-producer/single-consumer frame queue with backpressure drop logic.
   - Low-latency binary streaming endpoint with client heartbeat/keepalive.
3. **Phase 4.2: Production Enrollment & Rootfs Hardening** [NEXT]:
   - Read-only rootfs mount configuration and pairing enrollment.
