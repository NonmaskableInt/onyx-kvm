# CleanKVM

CleanKVM is a security-first, independently built software image for the
Sipeed NanoKVM Lite (SG2002). Network-facing services and device control are
implemented in Rust. Vendor-specific multimedia code is isolated behind a
small, auditable video adapter.

The independently assembled Phase 1 recovery checkpoint now boots repeatedly
on target hardware and has passed memory and wired-DHCP qualification. It is
still a **development image**, not an approved release image; see
[docs/PHASE1_ACCEPTANCE.md](docs/PHASE1_ACCEPTANCE.md) for the evidence and
claim boundary.

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
- Produce checksums, an SBOM, provenance metadata, and reproducible images.

## Repository layout

- `crates/cleankvm-core`: hardware-neutral domain types and authorization
  policy.
- `crates/cleankvm-mcp`: bounded MCP 2025-11-25 stdio server and default-deny
  tool handling.
- `crates/cleankvm-memtest`: bounded, address-sensitive and March-style Rust
  memory qualification utility for controlled hardware testing.
- `crates/cleankvmd`: local recovery MCP daemon; it has no network listener.
- `image/`: independently built image integration and pinned inputs.
- `docs/`: architecture, security model, and bring-up records.

The vendor SDK is currently a quarantined discovery input. The source and
build gates intentionally refuse to produce an image until its components,
licenses, and Linux builder are pinned; see
[docs/SDK_AUDIT.md](docs/SDK_AUDIT.md).

## Developer checks

```sh
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Build the local recovery daemon as a self-contained SG2002 userspace binary:

```sh
cargo rustc --release --target riscv64gc-unknown-linux-musl \
  --package cleankvmd --offline -- \
  -C linker=rust-lld -C link-self-contained=yes \
  -C target-feature=+crt-static
```

See [docs/ROADMAP.md](docs/ROADMAP.md) for the staged hardware plan.
The Phase 0 procedure and read-only collector are documented in
[docs/HARDWARE_EVIDENCE.md](docs/HARDWARE_EVIDENCE.md).
