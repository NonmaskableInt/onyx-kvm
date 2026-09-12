# Phase 3 and 3a acceptance record

Status: bring-up in progress. Phase 2 v8 remains the rollback checkpoint.

Discovery candidate: `out/phase3-discovery-v1/cleankvm-phase1.img`

Discovery image size: 79,692,288 bytes.

Discovery image SHA-256:
`4b49288f0c0fbad5b9cd11c85c587fb2adb50b65dbd0b7a7214a581cb5755b4d`

This is not a media-enabled image. It adds only the built-in DesignWare I2C
master and root-only `i2c-dev` surface to the Phase 2 configuration. The
normalized configuration prohibits I2C slave/mux/GPIO adapters, media, sound,
UVC, and UAC. Its kernel FIT hash is
`17a2c0fe64170679cd6eef56049c46cd52fc623da0d58018b9b92ea657a40566`;
the qualified Phase 2 DTB is unchanged.

No Phase 3 image is accepted until every applicable gate below has passed on
the NanoKVM Lite hardware. Video, media, microphone, and audio output remain
disabled in the current image.

## Phase 3: video and read-only media gates

- Inventory and checksum every source, header, firmware object, kernel module,
  and userspace library admitted for LT6911, VI, VPSS, VENC, or media support.
- Confirm the Lite PCB's LT6911 identity and physical I2C, reset, interrupt,
  MIPI-CSI, and HDMI signal paths without running a network-facing vendor
  process.
- Run vendor multimedia code only in a dedicated process with no network,
  credential, HID, block-write, updater, or general GPIO access.
- Accept only fixed, versioned, size-bounded requests over a local Unix socket.
  Reject unknown versions, operations, fields, formats, dimensions, rates,
  durations, and buffer sizes.
- Implement a bounded snapshot before streaming. Erase or overwrite reusable
  frame buffers between callers and never expose padding or stale frames.
- Require an unexpired audience-bound `kvm:video` grant for every snapshot or
  stream-session creation. MCP carries metadata and one-time handles, not bulk
  frame data.
- Add read-only virtual media first. The broker must open backing content
  read-only, reject writable flags, enforce an image-size ceiling, and emit a
  device detach on client/session loss.
- Verify malformed-input, expiry, disconnect, restart, cable-loss, and power-
  loss behavior before enabling either capability by default.

## Phase 3a: opt-in audio gates

- Identify the physical onboard microphone, optional speaker, HDMI-audio, and
  USB Audio Class paths independently. SoC or kernel support is not evidence
  that a path is routed on the Lite PCB.
- Keep ALSA and audio device nodes absent from the default build. Admit only
  the minimum driver and userspace surface in an explicit Phase 3a profile.
- Separate capture and playback brokers, Unix sockets, Unix identities, device
  permissions, scopes, and enable gates. Neither broker receives network
  access or bearer credentials.
- Capture accepts only raw bounded PCM with an enumerated device, channel
  count, sample format, sample rate, and duration. Playback additionally
  enforces a volume ceiling and duration ceiling.
- Microphone capture requires `kvm:audio-capture`, a short-lived session, and a
  separate local opt-in. It must produce a persistent physical or local visible
  indication for the full capture lifetime and an audit start/stop event.
- Playback requires `kvm:audio-output`, the mutation claim, and a separate
  local opt-in. Session expiry or disconnect immediately silences output.
- Zero reusable PCM buffers before reassignment and exclude them from logs,
  crash artifacts, MCP responses, and persistent storage by default.
- Document and test runtime disable, build-time omission, and a board-revision-
  specific physical microphone-disable procedure before claiming privacy.
- Audio must not reuse a video physical-memory interface. Capture and playback
  use independently admitted ALSA PCM nodes, and each broker may access only
  buffers owned by its live PCM handle. Format, channels, sample rate, duration,
  and total bytes are fixed and bounded before start; buffers are zeroed or
  released at stop. Enabling capture must not enable playback, or vice versa.

## Claim boundary

Phase 3 depends on vendor SG2002 multimedia interfaces whose provenance and
memory-safety properties are not yet sufficient for a hardened-release claim.
Containment reduces exposure but does not make an opaque or unsafe component
trusted. The project will report source and hardware limitations explicitly.

## Operational v4 regression — 2026-09-09

The running operational-v4 device passed a post-module-test regression without
touching the SD card reserved for the next build. Ethernet was link-up at
`192.168.2.38/24` with a default route, and the Rust HTTPS/MCP service listened
only on its configured port 8443. Authenticated MCP initialization negotiated
revision `2025-11-25`; `tools/list` exposed only `kvm.status`, and its result
reported one listener with mutation, video, microphone capture, and audio output
all disabled. Missing authorization returned HTTP 401 and an invalid Origin
returned HTTP 403.

Both `cleankvmd` and `cleankvm-hid-broker` were running. The USB gadget was
bound to `4340000.usb`, its two HID nodes remained root-only, and macOS
enumerated vendor `CleanKVM development`, product `Fail-closed HID probe`
(`1d6b:0104`). Following the completed module gate, taint remained at the
expected out-of-tree-only value 4096; no `soph_` module and no media node was
resident. `image/runtime-regression-probe.exp` captures the repeatable,
read-only UART portion of this check.
