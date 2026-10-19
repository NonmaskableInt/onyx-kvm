# Naming

The product, repository artifacts, on-device paths and environment variables
all use **Onyx KVM** / `onyx-kvm`. The earlier working name, CleanKVM
(`cleankvm`), is retired. Nothing in the source tree, image overlays, build
scripts or hardware harnesses may introduce it again; `image/audit-rootfs.sh`
fails an image that contains any file or directory with `cleankvm` in its
name.

## Convention

| Where | Form | Example |
| --- | --- | --- |
| Prose, UI strings | Onyx KVM | "Onyx KVM pairing ceremony" |
| Crates, binaries, services | `onyx-kvm-*` (daemon: `onyx-kvmd`) | `onyx-kvm-video-broker` |
| Rust identifiers | `onyx_kvm_*` | `onyx_kvm_media_protocol` |
| Environment variables | `ONYX_KVM_*` | `ONYX_KVM_HTTPS_BIND` |
| Runtime state | `/run/onyx-kvm/` | `/run/onyx-kvm/video-enabled` |
| Persistent config | `/etc/onyx-kvm/` | `/etc/onyx-kvm/features.json` |
| Boot partition files | `/boot/onyx-kvm-*` | `/boot/onyx-kvm-mac.txt` |
| Helpers and probes | `/usr/libexec/onyx-kvm-*` | `/usr/libexec/onyx-kvm-raw-frame-probe` |
| Init scripts | `S<nn>onyx-kvm-*` | `S47onyx-kvm-video-broker` |
| Unix account / group | `onyx-kvm` (UID/GID 900) | |
| Container and build resources | `onyx-kvm-*` | `localhost/onyx-kvm-builder`, `onyx-kvm-sdk` |
| FAT volume label | `ONYXKVM` | |
| Image file | `onyx-kvm-phase1.img` | |

## What changed in the rename

All of the following previously accepted a `cleankvm` form as a fallback and
now accept only the `onyx-kvm` form:

- Environment variables (`CLEANKVM_*` for the daemon, brokers and every build
  script).
- Runtime and config paths (`/run/cleankvm/*`, `/etc/cleankvm/*`,
  `/run/cleankvm-frame.yuv422p`, `/boot/cleankvm-mac.txt`) and the
  `/run/cleankvm` compatibility symlink the brokers used to create.
- Installed binaries, probes and init scripts (the `cleankvm*` symlinks and
  duplicate copies that the rootfs build used to install next to the
  `onyx-kvm*` files).
- The `cleankvm_mutation` JWT claim and the `https://cleankvm-owner` issuer
  and `cleankvm-recovery` origin aliases.
- Kernel patch log strings and symbols (`cleankvm_rx0n_mux_*`,
  `cleankvm-reset`), expect-script markers, and Podman resource names.

## Compatibility notes

Because the aliases are gone, three things need attention when moving from a
pre-rename image or workstation to a build of this tree:

1. **Re-pair existing clients.** Tokens minted before the rename that carry
   only `cleankvm_mutation`, or an issuer of `https://cleankvm-owner`, are now
   rejected (a missing mutation claim means mutation is *not* enabled, so this
   fails closed). Pair again to mint a token with the `onyx_kvm_mutation`
   claim. Tokens minted by `mint_token` (which has long emitted
   `onyx_kvm_mutation` with the configured issuer) and tokens whose issuer is
   `https://onyx-kvm-owner` are unaffected.
2. **Rebuild every on-target artifact.** The raw-frame probe, audio probes,
   EDID programmer, kernel modules and brokers embed the old names in paths and
   log strings. Binaries under `out/` that were built before the rename keep
   their `cleankvm-*` file names and are intentionally no longer picked up by
   `build-phase1-rootfs.sh`; rebuild them with the `image/build-*.sh` scripts.
   The product profile now refuses to start if a required probe is missing
   instead of silently producing an image without it.
3. **Migrate local container resources once.** Run
   `image/migrate-legacy-names.sh` (dry run by default, `--apply` to act). It
   retags the builder image and copies the reusable `cleankvm-sdk`,
   `cleankvm-host-tools` and `cleankvm-buildroot-dl-v50` volumes. It never
   deletes anything. Create a fresh work volume afterwards with
   `ONYX_KVM_WORK_VOLUME=<name> image/prepare-build-volume.sh`; do not reuse an
   old work volume, which still carries the pre-rename overlay directory and
   configuration headers.

An SD card flashed before the rename keeps its `CLEANKVM` volume label until it
is reflashed. `image/detect-target-disk.py` resolves removable media by device
properties, so the old label does not prevent identification; confirm the
target interactively as usual before every flash.

## Historical records are not rewritten

Dated evidence keeps the names that were true when it was recorded: the entries
in `NEXT_STEPS.md`, `PHASE*_ACCEPTANCE.md`, `PHASE3_VIDEO_ADMISSION.md`,
`PHASE3A_AUDIO_ADMISSION.md`, the collector schema `cleankvm-evidence-v2` named
in `evidence/stock-v1.4.3-review.md`, and the `out/.../cleankvm-phase1.img`
paths and hashes they cite. Rewriting them would misreport what was measured,
built and flashed. Current-state documents (this file, `ARCHITECTURE.md`, the
`*_SECURITY_BOUNDARY.md` files, `image/README.md`) use the new names only.
