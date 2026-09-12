# Phase 2 acceptance record

Status: hardware-qualified development checkpoint on 2026-09-08.

Canonical candidate: `out/phase2-hid-mcp-dev-v8/cleankvm-phase1.img`

Image size: 79,692,288 bytes.

SHA-256: `483ad168f6d1b963e42ad5f95d25d0a4d932dab9bf818909639b748a9fd247e5`

The candidate was rebuilt from a clean Buildroot target. Its root-filesystem
audit explicitly rejected the stale held-key helper during an earlier
incremental attempt, then passed after the cache was cleared and the image was
rebuilt solely from the release overlay.

## Passed gates

- The Rust workspace passes all tests and strict Clippy with warnings denied.
- The TLS MCP daemon and HID broker are static, stripped RISC-V executables.
- HTTPS validates Ed25519-signed, audience-bound, expiring grants and rejects
  invalid Origin, bearer, scope, and mutation claims.
- The network daemon runs as UID/GID 900 and has no direct gadget-device
  access. The privileged broker has no network listener and accepts only UID
  900 after Linux `SO_PEERCRED` verification.
- The broker socket is `root:cleankvm` with mode `0660`. Keyboard and pointer
  reports have fixed 8-byte and 4-byte formats; malformed and extensible frames
  are rejected. Every client disconnect emits zero release reports.
- The CVITEK DWC2 driver honors the fixed device-tree peripheral role. Cold
  boots automatically enumerate the fixed keyboard and relative-pointer
  descriptors on macOS at 480 Mbps.
- A read-only grant cannot invoke HID. A valid control/mutation grant is still
  denied while the independent volatile local marker is absent.
- With a marker deliberately created through authenticated UART for one boot,
  zero and low-impact nonzero reports traversed HTTPS, MCP, UID 900, the broker,
  and the gadget devices. Invalid lengths, out-of-range bytes, and extra fields
  were rejected.
- Software reboot and abrupt power loss recovered wired DHCP, HTTPS, both HID
  interfaces, and both daemons. The marker never survived a reboot.
- A temporary local-only F13 hold helper verified that host HID state is
  discarded across managed-host USB removal and simultaneous device power
  loss. The helper is prohibited by the canonical root-filesystem audit.
- The canonical v8 card passed final payload readback: `BOOT.SD`, `FIP.BIN`,
  and the complete ext4 root partition matched the packaged artifacts. macOS
  added only its `.fseventsd` bookkeeping to the FAT volume, so that expected
  host-side metadata change was excluded from the payload comparison.
- The final v8 cold boot acquired DHCP at `192.168.2.30`; authenticated HTTPS
  MCP initialization negotiated revision `2025-11-25`. The device reported
  USB role `device` and state `configured`, while macOS enumerated
  `CleanKVM development / Fail-closed HID probe` at 480 Mbps with VID:PID
  `1d6b:0104`. The broker was running, the temporary helper was absent, and
  the volatile mutation marker remained absent.

## Claim boundary

This is a development checkpoint, not a production release. Production still
requires credential rotation, a user-facing physical/local enable mechanism,
source and license closure for boot firmware and toolchains, clean-builder
reproduction, an SBOM, release signing, and verified-boot root-of-trust work.
Video, virtual media, audio, and microphone support remain outside this phase.
