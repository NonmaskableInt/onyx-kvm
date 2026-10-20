# Release checklist

A release is an image that may be redistributed. No image built so far is one:
every build to date used `ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1`. Work the list top
to bottom; a release is cut only when every box can be ticked and recorded in
[NEXT_STEPS.md](NEXT_STEPS.md).

## Blocking

- [ ] **License grants** for the four components in
      [OPEN_ITEMS.md](OPEN_ITEMS.md) B1 (vendor FSBL files, linked middleware,
      `sophgo-osdrv`, `coda980.bin`), or replacements for them. Only a vendor can
      supply the grants. Until then `image/verify-sources.sh` fails without the
      waiver, which is the intended gate. Do not remove the gate.
- [ ] `sources.lock` and `builder.lock` contain no `PENDING-REVIEW` or
      `NO-LICENSE-GRANT` entry, and the build ran **without**
      `ONYX_KVM_ALLOW_UNRESOLVED_SOURCES`.

## Decisions to restate in the release notes

- Accepted risks S1 (trust-on-first-use network setup claim) and S2 (the microSD
  card controls root; physical access owns the device). See
  [THREAT_MODEL.md](THREAT_MODEL.md) and [OPEN_ITEMS.md](OPEN_ITEMS.md).
- The product profile ships one extra listener beyond HTTPS 8443: the mDNS name
  responder on UDP 5353 ([NETWORK_POLICY.md](NETWORK_POLICY.md)).
- The device takes a new MAC, hostname and address, and loses its pairings, each
  time the card is reflashed (the MAC is kept in `/boot`).
- Decide whether `S51onyx-kvm-mdns` keeps writing `/boot/onyx-kvm-mdns.log`.
  It exists for serial-less diagnosis and is not needed in a release; removing it
  changes the rootfs and so needs a new hardware pass.

## Build and verify (all recorded in NEXT_STEPS.md)

- [ ] `cargo fmt --check`, `cargo clippy --workspace --locked --all-targets -- -D warnings`
      and `cargo test --workspace --locked` pass.
- [ ] Build on a **fresh** work volume with the sequence in
      [../image/README.md](../image/README.md), `ONYX_KVM_CONFIG_PROFILE=product`
      and `ONYX_KVM_DEV_TOOLS` unset. `audit-rootfs.sh` passes (it rejects dev
      tooling in a product rootfs).
- [ ] The nine video modules rebuilt by `build-phase3-video-modules.sh` match the
      set that ships, and the build is repeated on a second machine or volume
      (B4, secondary reproduction) with identical hashes.
- [ ] Resolve the removable-media identity, flash, read back, and run
      `image/verify-sd-image.py`: `embedded_artifacts=verified`.
- [ ] Hardware pass on the flashed card: boot, video, HID, microphone (metrics
      only), PIN pairing, MCP tool list, `<hostname>.local` resolution, and
      listening-socket enumeration (expect TCP 8443 and UDP 5353 only).
- [ ] UART evidence, where a console is available: initial and final taint,
      loaded modules, nodes ([PHASE3_VIDEO_ADMISSION.md](PHASE3_VIDEO_ADMISSION.md)).
- [ ] Publish `SHA256SUMS` for the image and its embedded artifacts. Keep
      generated images, SDK checkouts, identities and keys out of Git.

## Later (Phase 4, not release blockers for a development image)

Secondary builder reproduction, A/B root failover and signed update packaging
([ROADMAP.md](ROADMAP.md), OPEN_ITEMS B4).
