# Roadmap

Status at product release candidate v80 (2026-10-18). Unfinished work and
recommendations are in [OPEN_ITEMS.md](OPEN_ITEMS.md).

| Phase | Status |
| --- | --- |
| 0 Evidence and baseline | Done; results in `evidence/` |
| 1 Independent recovery image | Hardware-qualified development checkpoint |
| 2 Rust control plane | Done for HTTPS, MCP, pairing, HID; production enrollment hardening open (S1, S2, S4) |
| 3 Video and media | Video admitted: 1080p60 capture, H.264, tile-delta streaming, host-sleep standby and wake. Virtual media not started |
| 3a Opt-in audio | Metrics-only capture and fixed-tone playback admitted behind separate gates; continuous audio is Phase 4 |
| 4 Integrity and release | Not started: A/B roots, verified boot, SBOM, reproduction, on-device signed update (B1 to B4) |

## Phase 0: evidence and baseline

- Record board revision, boot log, partition table, device tree, loaded kernel
  modules, device nodes, listening ports, and USB descriptors from an isolated
  stock device.
- Archive HDMI, Ethernet, USB HID, watchdog, thermal, ALSA device, microphone,
  and speaker behavior as acceptance tests. Determine whether HDMI audio and
  USB Audio Class gadget paths are physically routed. Do not connect the stock
  image to a trusted network.
- Identify every firmware and multimedia artifact required for SG2002 boot and
  LT6911 HDMI capture.

## Phase 1: independently built recovery image

Hardware-qualified development checkpoint. See `PHASE1_ACCEPTANCE.md`.

- Pin the Sipeed SG2002 SDK as a temporary board-support baseline.
- Build bootloader, Linux, BusyBox/Buildroot userspace, and device tree.
- Boot with serial console and wired DHCP client networking, but no listening,
  discovery, remote-login, or automatic-update services.
- Remove package managers, compilers, factory credentials, telemetry, Wi-Fi,
  boot-demo recording, and vendor update agents. Audio drivers remain absent
  until the dedicated audio phase is enabled.

## Phase 2: Rust control plane

- Hardware-qualified development checkpoint. See `PHASE2_ACCEPTANCE.md`.
- MCP protocol revision 2025-11-25 is implemented over local recovery stdio and
  authenticated HTTPS. Requests are bounded to 64 KiB. HTTPS runs as UID/GID
  900 and revalidates every request without retaining bearer credentials.
- `kvm.status`, `kvm.keyboard.send`, and `kvm.pointer.send` are implemented.
  HID tools require an audience-bound, unexpired `kvm:control` grant with an
  explicit mutation claim and a separate volatile, device-local enable gate.
- The shared authorization kernel now enforces exact resource audience,
  expiration, enumerated scopes, and a separate mutation-enable gate. The MCP
  HTTP boundary helpers emit RFC 9728 protected-resource metadata, require
  header-only bounded Bearer credentials, construct scoped challenges, and
  reject mismatched browser origins. Remote sessions now consume only owned,
  bounded validated claims, and a pure-Rust Ed25519 JWT validator rejects
  malformed headers, invalid signatures, wrong issuers, and unknown scopes.
  Development TLS and OAuth issuer identity generation is repository-owned and
  fail-fast: it creates a unique Ed25519 TLS leaf, a separate Ed25519 issuer
  keypair, and a raw 32-byte verification key without overwriting existing
  identity. Rustls' maintained ring provider now cross-builds as a static
  RISC-V musl binary using vendor GCC for ring's C objects and Rust LLD for the
  final link. The HTTPS listener is opt-in through complete environment
  configuration, runs as UID/GID 900, and revalidates every request without
  retaining bearer material. Host tests passed strict certificate validation,
  protected-resource metadata, challenge, Origin rejection, initialization,
  tool listing, and `kvm.status`. Target hardware then passed strict TLS
  certificate verification, metadata, authentication denials, initialization,
  notification handling, and `kvm.status` at `192.168.2.18:8443`. On-device
  inspection confirmed the daemon's real/effective/saved UID and GID are 900.
- Complete production enrollment, authorization UX, audit records, and session
  expiry around the qualified development identity and token implementation.
- Replace the UART-only development enable procedure with an independently
  audited physical/local boot-bounded production mechanism.
- Add browser video/control only after it shares the same policy enforcement.

## Phase 3: video and media

- Bring-up started; acceptance gates are defined in `PHASE3_ACCEPTANCE.md` and
  process/protocol isolation is defined in `MEDIA_SECURITY_BOUNDARY.md`.
- The LT6911UXC target is identified at bus 4/address `0x2b`. A video-foundation
  profile now permits out-of-tree module loading while retaining no in-tree
  modules and keeping generic media, sound, UVC, and UAC disabled. The six
  candidate drivers and remaining provenance gates are documented in
  `PHASE3_VIDEO_ADMISSION.md`. The foundation profile was subsequently admitted
  and the MIPI RX pinmux/pad configuration (patch 0027) unblocked frame
  capture; see `NEXT_STEPS.md`.
- Done: bring up LT6911 input and the SG2002 VI/VPSS/VENC pipeline in a
  sandboxed adapter.
- Done: bounded snapshots and streaming.
- Open: read-only virtual media first (`kvm.media.attach` is defined in policy
  but not implemented); writable media remains optional.

## Phase 3a: opt-in audio

- Phase 3a is an independently gated companion track. Audio remains absent
  from the default and video-only profiles.
- Bring up the onboard microphone and optional speaker using the smallest ALSA
  kernel/userspace surface that passes hardware tests.
- Determine whether HDMI audio extraction and USB Audio Class forwarding are
  supported by the actual Lite PCB, rather than assuming SoC capability.
- Implement isolated PCM capture/playback brokers, bounded codecs, visible
  activity indication, volume ceilings, session expiry, and buffer erasure.
- Add `kvm.audio.capture` and `kvm.audio.play` only after authorization,
  privacy, and session-termination tests pass.
- Document software, build-time, and physical microphone-disable procedures.

## Phase 4: image integrity and release

- Add read-only A/B roots, signed offline updates, recovery, and power-failure
  testing.
- Determine whether SG2002 supports a usable verified-boot root of trust and
  document limitations precisely.
- Reproduce builds on two clean builders and complete the release checklist.
- Obtain a written license grant from Sophgo/Cvitek, or replace, the three
  components marked `NO-LICENSE-GRANT` (FSBL vendor files, linked middleware,
  `sophgo-osdrv`) and review the remaining vendor kernel modules (OPEN_ITEMS B1).
  No image is releasable before this.
