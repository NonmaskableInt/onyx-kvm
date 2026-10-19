# Media security boundary

## Process split

`onyx-kvmd` owns TLS, bearer validation, MCP, rate limits, and session policy.
It cannot open multimedia, ALSA, raw block, USB gadget, I2C, or GPIO devices.
It sends typed, bounded requests to local brokers after authorization.

The video adapter is the only process allowed to load the SG2002 VI/VPSS/VENC
and LT6911 sensor interfaces. It receives no credentials and has no network,
HID, audio, updater, or writable block access. A later read-only media broker
is separate from video and may expose only a specifically opened read-only
image to the USB mass-storage function.

Audio capture and playback are separate Phase 3a processes. Capture cannot
play audio; playback cannot open capture devices. Neither is automatically
started by the video profile.

## Bounded protocol rules

- Every message begins with a fixed magic, protocol version, operation, and
  payload length.
- The broker rejects trailing data, unknown operations, non-enumerated values,
  arithmetic overflow, and payloads beyond the operation-specific maximum.
- Snapshot dimensions, pixel/encoded formats, frame size, and deadlines are
  capped before calling vendor code.
- PCM source/sink, sample format, channel count, sample rate, frame count,
  duration, volume, and total bytes are capped before opening ALSA.
- Shared buffers are fixed-size, sealed against resize where supported, and
  cleared before reuse. File descriptors are passed only over local Unix
  sockets and checked for type and access mode.
- Client disconnect, timeout, authorization expiry, or broker restart closes
  all handles and returns hardware to inactive state.

## Default state

The Phase 2 v8 configuration remains the default and contains no video, media,
or audio broker. Phase-specific image profiles must explicitly admit each
driver, executable, device node, group, service, and dependency. Root-filesystem
audits reject accidental media/audio artifacts in profiles that omit them.

