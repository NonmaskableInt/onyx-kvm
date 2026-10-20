# Architecture

This document describes the system as built at product release candidate v80
(naming per [NAMING.md](NAMING.md)). Design intent that is not yet implemented
is marked *planned*; unfinished work is tracked in
[OPEN_ITEMS.md](OPEN_ITEMS.md).

## Trust boundaries

```text
untrusted LAN / MCP client / browser
                |
        TLS (rustls, ChaCha20-Poly1305 suites) + bearer authentication
                |
     onyx-kvmd  (UID/GID 900, unprivileged)
                |
   bounded Unix-socket RPC in /run/onyx-kvm/  (SO_PEERCRED: UID 900 only)
      /            |                \
 HID broker   video broker      audio capture / playback brokers
 (root)       (root)            (root)
      \            |                /
          Linux device APIs
                |
       SG2002 + attached host
```

`onyx-kvmd` parses all remote input but cannot open the USB gadget, GPIO,
block, or multimedia devices. Each privileged broker accepts typed,
size-bounded requests over its own Unix socket, verifies the caller's UID with
`SO_PEERCRED`, and independently re-checks its fail-closed gate file before
acting. Policy is therefore enforced at both the gateway and the broker.

## Components

| Component | Runs as | Role |
| --- | --- | --- |
| `onyx-kvmd` | UID/GID 900 | HTTPS listener, web console, OAuth metadata, MCP, pairing and setup |
| `onyx-kvm-mdns` | UID/GID 901 (product only) | Answers mDNS for the device's own `<hostname>.local` name |
| `onyx-kvm-hid-broker` | root | Writes keyboard and pointer reports to `/dev/hidg0`, `/dev/hidg1` |
| `onyx-kvm-video-broker` | root | Loads the quarantined video modules per session and drives the raw-frame probe |
| `onyx-kvm-audio-brokers` | root | Separate capture and playback brokers around the ALSA probes |
| `onyx-kvm-raw-frame-probe` | root, seccomp | Captures UYVY/H.264 frames through the vendor VI/VPSS/VENC stack |
| `onyx-kvm-audio-{capture,playback}-probe` | root | One-shot/streaming ALSA helpers (TinyALSA) |
| `onyx-kvm-lt6911uxc-edid-programmer` | root, on demand | Bounded EDID write to the HDMI bridge |
| `onyx-kvm-core` | library | Scopes, operation table, `authorize` / `authorize_grant` |
| `onyx-kvm-mcp` | library | MCP 2025-11-25 server, Ed25519 JWT validation, token minting |
| `onyx-kvm-media-protocol` | library | Fixed-size wire formats and bounded buffers |
| `onyx-kvm-evidence`, `onyx-kvm-memtest` | tools | Stock-device evidence collector, memory qualification |

Gate files under `/run/onyx-kvm/` (`hid-enabled`, `video-enabled`,
`audio-capture-enabled`, `audio-output-enabled`) are the runtime capability
switches. A missing gate makes the corresponding broker refuse requests. The
daemon creates and removes them through `GET`/`POST /features`.

The video broker refuses to run if `/dev/mem` exists, and the raw-frame probe
reads frame memory through a bounded `CVI_VB_CopyBlock` ioctl (patch 0008)
instead of mapping physical memory. The nine vendor modules (`soph_sys`,
`soph_base`, `soph_mipi_rx`, `soph_snsr_i2c`, `soph_vi`, `soph_vpss`,
`mars_vcodec`, `soph_jpeg`, `cvi_vc_driver`) are loaded in that order for a
session and unloaded in reverse; none is loaded at boot.

Audio is a separately enabled capability, not a companion to video. Microphone
capture and speaker output have independent gates, scopes and brokers, and both
default to off. Capture is currently metrics-only, and playback is a fixed
low-volume tone with the DAC mute control as its shutdown boundary.
Continuous audio streaming, a persistent activity indicator, and a build-time
option that omits microphone support are *planned* (see
[PHASE4_CONTINUOUS_MEDIA_ARCHITECTURE.md](PHASE4_CONTINUOUS_MEDIA_ARCHITECTURE.md)).

## HTTPS surface

All routes share one listener on port 8443.

| Route | Auth | Purpose |
| --- | --- | --- |
| `GET /`, `/app.js`, `/app.css`, `/favicon.*` | none | Embedded web console (strict CSP) |
| `GET /.well-known/oauth-protected-resource` | none | RFC 9728 metadata |
| `GET /setup/status`, `POST /setup/claim`, `POST /setup/pin` | none / bearer | First-use setup and PIN change |
| `POST /pair` | PIN | Exchanges a PIN for a session token |
| `POST /mcp` | bearer | MCP Streamable HTTP (64 KiB message cap) |
| `GET /video/frame`, `/video/stream` | bearer, `kvm:video` | Snapshot and continuous H.264 / tile-delta stream |
| `GET /audio/stream` | bearer | Audio capture stream |
| `GET`/`POST /features` | bearer | Read and set capability gates |
| `POST /system/reboot` | bearer, `kvm:admin` | Reboot via the privileged trigger watcher |

Bearer tokens are Ed25519 JWTs (`typ at+jwt`), audience-bound, short-lived and
revalidated on every request. The credential is accepted only from the
`Authorization` header, never from a query string.

## MCP tools

Scopes: `kvm:read`, `kvm:video`, `kvm:audio-capture`, `kvm:audio-output`,
`kvm:control`, `kvm:media`, `kvm:admin`. Operations that are not read-only
require the mutation claim in the token (`onyx_kvm_mutation`) in addition to
the scope; `authorize` is default-deny.

| Tool | Scope | Effect |
| --- | --- | --- |
| `kvm.status` | `kvm:read` | read-only |
| `kvm.video.snapshot` | `kvm:video` | read-only, exposes the host screen |
| `kvm.audio.capture` | `kvm:audio-capture` | read-only, captures nearby sound |
| `kvm.audio.play` | `kvm:audio-output` | mutating, emits sound |
| `kvm.keyboard.send`, `kvm.pointer.send` | `kvm:control` | mutating, changes the attached host |
| `kvm.media.attach` | `kvm:media` | mutating; defined in policy, **not implemented** |
| `kvm.device.reboot`, `kvm.system.reboot` | `kvm:admin` | disruptive |
| `kvm.diag.video`, `kvm.diag.dmesg`, `kvm.diag.system` | `kvm:admin` | read-only diagnostics |
| `kvm.dev.exec`, `kvm.system.dev_mode` | `kvm:admin` | mutating; dev mode only |

Tool descriptions and annotations are advisory; the brokers and policy layer
enforce. Rate limits, payload limits, an append-only audit record and optional
physical-presence confirmation are *planned* hardening on top of the current
per-request validation.

Local recovery can also expose MCP over stdio on the physical serial console.
It is never bridged to TCP.

## Enrollment

An unconfigured device accepts the web setup wizard (`/setup/claim`), which
stores a salted, iterated SHA-256 PIN hash (`sha256-5000:salt:hash`) on the boot
partition and returns a console token (no `kvm:admin`). A console ceremony (`onyx-kvm-pair`)
can instead stage a one-time pairing code and session token in
`/run/onyx-kvm`; three wrong attempts wipe the staged code. Dropping a `RESET`
file on `/boot` returns the device to the unconfigured state. The trust
assumptions in this flow are discussed in OPEN_ITEMS S1 and S2.

## Development mode

If `/boot/DEV_MODE` exists at boot (or `kvm.system.dev_mode` enables it at
runtime), `S35onyx-kvm-devmode` starts key-only Dropbear SSH using
`/boot/ssh_authorized_keys`, grants group 900 access to the HDMI bridge's I2C
bus, and starts the root watcher behind `kvm.dev.exec`. It exists for bring-up
and must not ship in a product image (OPEN_ITEMS S3).

## Image layout

- An MBR image with two partitions: a FAT16 boot partition (label `ONYXKVM`)
  holding `FIP.BIN` (the vendor-format first-stage chain with U-Boot) and
  `BOOT.SD` (a FIT with the kernel and device tree), plus operator marker files
  such as `DEV_MODE`, `RESET` and `onyx-kvm-mac.txt`; and a read-only ext4 root
  filesystem. `/run` and `/tmp` are `tmpfs` with `nosuid,nodev`.
- The FIP, the FIT and the rootfs are each verified from card read-back after
  every flash (`image/verify-sd-image.py`).
- Root A/B slots, an encrypted or integrity-protected state area, a recovery
  environment and a signed boot manifest are *planned*. The image must not
  claim secure or verified boot until the SG2002 root of trust and rollback
  behavior are proven on the device.

See [THREAT_MODEL.md](THREAT_MODEL.md) for the controls this layout has to
meet and [NETWORK_POLICY.md](NETWORK_POLICY.md) for what may listen.
