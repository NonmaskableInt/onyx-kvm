# CleanKVM product completion plan

Updated: 2026-10-06

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

- `cleankvmd`: SHA-256
  `ecf337a8d6aaeb4fd1439ea8b7af531943e527dbde8dfe97869a1fdd1d501aaa`
- `cleankvm-video-broker`: SHA-256
  `84074a5851cc1c8d2942203bb06a36d4f7ec895ed4ae93c1126c800749e14e51`
- verified v50 SD image: `out/phase3-web-video-v50/cleankvm-phase1.img`,
  79,692,288 bytes, SHA-256
  `e57f5b480e0bf4da02350a88ebb8928cf34997ab1c273845c83e3e7732718fe8`
- v50 embedded artifacts: `boot.sd`
  `ec4d60fe34cde9bdf5aa94366589a2353fde7330864b7b5f9618ea125ed73d38`,
  `rootfs.ext4`
  `311cf493b5aebf6242b20e179d6d01be72af12ed0a5a2b03d761957bcab2a9a8`,
  and `fip.bin`
  `6e6d3b5c7a58c1a585bf1ea456c76bd7c91cfd33c76dfc67bd33b31cf86cd301`.

## Phase 2: human web console

Status: first authenticated development shell implemented, built, packaged,
and served successfully from v50. Direct HTTPS and authorization-negative tests
pass. Managed-browser use remains blocked on trusted certificate enrollment;
the certificate warning will not be bypassed.

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
6. Admit snapshot viewing first. Admit continuous video only after a bounded
   media session transport has backpressure, cancellation, and memory-erasure
   tests.

The first shell is embedded in `cleankvmd` and served by the existing TLS
listener at `/`, with separate same-origin `/app.js` and `/app.css` resources.
It keeps the short-lived bearer token only in memory, clears the input after
connection, initializes MCP through `/mcp`, displays the four independent
capability states, and can request an admitted YUV420P snapshot metadata record.
Every response applies no-store, strict same-origin CSP, frame denial,
no-referrer, content-sniffing denial, and camera/geolocation/microphone browser
permission denial. It intentionally does not yet display pixels, provide HID
controls, retain tokens, or perform production enrollment.

Acceptance: an authenticated human can view the attached host and use admitted
HID operations without gaining a broader grant than an equivalent MCP client.

## Phase 3: audio and microphone (Phase 3a)

Status: enumeration and the bounded local microphone proof pass on v51. The
capture returned exactly one second of metrics-only PCM and left no open PCM,
process, output file, gate, or taint. Playback routing and the bounded local
output proof now pass on v52 with clean PCM/process/file/gate/taint cleanup.
The separate brokers plus shared MCP/web controls are implemented and packaged
in v53. On-device Stage D admission passes for independent capture and output,
including default-deny, scope separation, bounded operation, and cleanup.

1. Run the existing root-only one-second microphone metrics probe under fresh
   explicit local authorization. Confirm capture stops, DMA and file
   descriptors are released, and the buffer is zeroized.
2. Establish the physical microphone and DAC/amplifier routing. Keep HDMI audio
   extraction and USB Audio Class disabled until separately proven routed.
3. Implement privileged Rust capture and playback brokers with distinct Unix
   sockets, enable gates, scopes, short leases, one-client limits, fixed PCM
   shapes, conservative playback ceilings, cancellation, and buffer
   zeroization.
4. Add `kvm.audio.capture` and `kvm.audio.play` as bounded control operations.
   MCP returns metrics or a one-time media-session handle, never unbounded PCM.
5. Add matching web controls and persistent privacy indication. Capture and
   playback must be independently disabled by default and independently
   removable at build time.

Acceptance: microphone capture and audio output each pass their own hardware,
privacy, cleanup, authorization, timeout, and cold-boot fail-closed tests.

## Phase 4: continuous media and release engineering

Status: the release product image (v56) has completed native probe packaging,
clean SD-card read-back verification, cold-boot baseline, and full independent
capability acceptance on physical hardware (`192.168.2.105`). All four capabilities—
Video snapshot (1080p YUV420P), USB HID (keyboard and pointer reports), Audio
capture (16 kHz mono S16LE metrics), and Audio output (1 kHz diagnostic tone)—function
out of the box via authenticated MCP and fail-closed upon gate removal. Continuous
pixel/audio streaming sessions, browser trust enrollment, and production
enrollment are the remaining milestones.

1. Add authenticated, short-lived continuous video/audio sessions behind the
   same policy layer, with bounded rings and no credential access in media
   workers.
2. Complete production enrollment, audit records, rate limits, physical/local
   gate UX, and session revocation.
3. Add read-only A/B roots, signed offline updates, recovery and power-failure
   tests, SBOM/provenance output, and two-builder reproducibility.
4. Run the complete release matrix: cold boot, network isolation, MCP, web,
   video, HID, microphone, audio output, cleanup, and negative authorization
   cases.

## Immediate execution order

1. Complete trusted client TLS enrollment for browser access to `https://cleankvm-recovery:8443`
   without bypassing security warnings, and verify the web console capability strip
   and snapshot flows.
2. Package release profile image v56 with verified probe build defaults in rootfs.
3. Advance to continuous media session architecture and production enrollment.
