# Onyx KVM

Onyx KVM is a security-first, independently built software image for the
Sipeed NanoKVM Lite (SG2002). Network-facing services and device control are
implemented in Rust. Vendor-specific multimedia code is isolated behind a
small, auditable video adapter.

## Status

The image is a hardware-qualified **development** build, not an approved
release. As of product release candidate v80 it boots from an independently
assembled SD image and provides, on a NanoKVM Lite:

- authenticated HTTPS web console and MCP (2025-11-25) with scoped,
  audience-bound Ed25519 tokens;
- USB keyboard and pointer (HID broker), with host wake;
- 1080p60 HDMI capture, hardware H.264 and tile-delta streaming (video broker);
- opt-in, metrics-only microphone capture and a fixed-tone speaker test
  (audio brokers), each behind its own gate and off by default.

Virtual media, continuous audio, A/B updates, verified boot, an SBOM and a
second-builder reproduction are not done; see
[docs/OPEN_ITEMS.md](docs/OPEN_ITEMS.md) for what is open and
[docs/ROADMAP.md](docs/ROADMAP.md) for the phase plan. Start from the
[documentation index](docs/README.md).

## Goals

- Build the bootloader, Linux kernel, root filesystem, and Rust services from
  pinned source revisions.
- Have no shared factory credentials, telemetry, cloud dependency, or
  unattended network update path. Development recovery images receive a
  unique host-generated console credential; only its SHA-512 hash is embedded.
- Expose browser KVM and Model Context Protocol (MCP) interfaces through the
  same authorization and policy layer.
- Keep HID, virtual-media, update, and reboot operations out of the
  network-facing process.
- Produce checksums, an SBOM, provenance metadata, and reproducible images
  (planned; see OPEN_ITEMS B1 to B4).

## Repository layout

- `crates/onyx-kvm-core`: hardware-neutral domain types and authorization
  policy.
- `crates/onyx-kvm-mcp`: bounded MCP 2025-11-25 server (stdio and HTTPS
  transport glue), Ed25519 JWT validation, and default-deny tool handling.
- `crates/onyx-kvm-memtest`: bounded, address-sensitive and March-style Rust
  memory qualification utility for controlled hardware testing.
- `crates/onyx-kvm-hid-broker`: privileged, fail-closed HID broker and USB gadget writer.
- `crates/onyx-kvm-video-broker`: privileged, quarantined video capture broker.
- `crates/onyx-kvm-audio-brokers`: isolated audio capture and playback brokers.
- `crates/onyx-kvm-media-protocol`: shared fixed-size media and snapshot wire protocols.
- `crates/onyx-kvm-evidence`: hardware and security evidence collector.
- `crates/onyx-kvmd`: authenticated HTTPS daemon: web console, MCP endpoint,
  pairing and setup, media streaming.
- `image/`: independently built image integration and pinned inputs.
- `docs/`: architecture, security model, and bring-up records.

The vendor SDK is a pinned, quarantined board-support input. Its commit,
component digests and licenses are recorded in `image/sources.lock`;
`image/verify-sources.sh` refuses to pass while any entry is `PENDING-REVIEW` or
`NO-LICENSE-GRANT` (the vendor FSBL files, the multimedia stack and osdrv are the
latter). The image build runs that gate; development builds waive it with
`ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1`. See [docs/SDK_AUDIT.md](docs/SDK_AUDIT.md)
and [docs/OPEN_ITEMS.md](docs/OPEN_ITEMS.md) B1.

## Licensing status

Images built today are **development builds and are not releasable or
redistributable**. Four vendor components have no license grant:

1. FSBL: the Cvitek/Bitmain-authored files in `fsbl/plat/cv180x`,
   `fsbl/plat/cv181x` and `fsbl/lib/utils` (about 115 files).
2. Middleware: the sources linked into the raw-frame probe (`libsys.a` and four
   `middleware/v2/modules/vpu` objects); `sys/src/peri.c` is marked proprietary.
3. `sophgo-osdrv`: the source of the `soph_jpeg`, `mars_vcodec` and
   `cvi_vc_driver` modules (patch 0028), whose notices contradict their GPL
   declaration.
4. `coda980.bin`: the opaque VPU firmware blob.

The other six vendor kernel modules declare GPL but carry no file headers.
Release needs a written grant from Sophgo/Cvitek or replacement of these parts.
Evidence is in [docs/SDK_AUDIT.md](docs/SDK_AUDIT.md); tracking is
[docs/OPEN_ITEMS.md](docs/OPEN_ITEMS.md) B1.

Release steps and remaining blockers are in
[docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md).

## Developer checks

```sh
cargo test --workspace --locked
cargo fmt --check
cargo clippy --workspace --locked --all-targets -- -D warnings
```

CI runs these checks plus `cargo-deny` and a RISC-V type-check on every pull
request; `main` accepts changes only through a passing pull request.

Build the on-target Rust binaries (static `riscv64gc-unknown-linux-musl`,
cross-linked with `rust-lld`) with the scripts in `image/`, for example:

```sh
image/build-rust-daemon.sh           # onyx-kvmd
image/build-rust-hid-broker.sh
image/build-rust-video-broker.sh
image/build-rust-audio-brokers.sh
image/build-rust-mdns.sh             # product-profile name responder
```

The image build, verification and flashing workflow is in
[image/README.md](image/README.md). The `onyx-kvm` naming standard and the
migration from the earlier CleanKVM name are in
[docs/NAMING.md](docs/NAMING.md). Contributor and hardware rules are in
[AGENTS.md](AGENTS.md).

See [docs/HARDWARE_EVIDENCE.md](docs/HARDWARE_EVIDENCE.md) for the Phase 0
stock-device collector.
