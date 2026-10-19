# Image build

This directory holds everything needed to turn pinned inputs into a flashable
Onyx KVM SD image: source verification, kernel/U-Boot/Buildroot configuration
fragments and patches, the root filesystem overlays, offline build and packaging
scripts, image assembly and verification, signed-update tooling, and the UART
(`*.exp`, `*.py`) and C hardware diagnostics. Names follow
[../docs/NAMING.md](../docs/NAMING.md).

Sipeed's NanoKVM-capable SDK is treated as board-support source, not as a
trusted runtime image. All configurations and patches used by the resulting
image belong in this repository (`patches/` is applied in numeric order).
Network access is allowed only in the explicit fetch step; the build consumes
verified local inputs with networking disabled.

`sources.lock` and `builder.lock` record every pinned input. Entries marked
`PENDING-REVIEW` or `NO-LICENSE-GRANT` (the vendor FSBL files, the multimedia
stack, sophgo-osdrv) are not approved for release. A build script must never resolve a placeholder or
silently select a branch head.

## Safety gates

`verify-sources.sh` validates the exact SDK revision, rejects tracked changes,
and refuses every `PENDING-REVIEW` and `NO-LICENSE-GRANT` entry. `build.sh`,
`build-phase1-rootfs.sh` and `package-phase1.sh` invoke it first. Development
builds set `ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1` (OPEN_ITEMS B1); the result is
not releasable. Development images that need `kvm.dev.exec` also set
`ONYX_KVM_DEV_TOOLS=1` for `build-rust-daemon.sh` and the rootfs build.
`apply-osdrv-patch.sh` applies patch 0028 to the pinned sophgo-osdrv checkout, and
`build-phase3-video-modules.sh OUTPUT_DIR` builds the nine video modules and
the VPU firmware from the pinned sources (OPEN_ITEMS B5); point
`ONYX_KVM_VIDEO_MODULE_DIR` at its output for a rootfs build.

The kernel fragment in `configs/phase1-kernel.fragment` documents the recovery
image boundary. The U-Boot fragment in `configs/phase1-uboot.fragment` reduces
the bootloader to serial, SD/FAT, and SHA-256 FIT loading. A source patch fixes
the boot command to one `boot.sd` path and removes unsigned `uEnv.txt` import,
automatic update fallback, network protocol/update commands, display, and
unrelated writable storage protocols. U-Boot's network core and PHY driver are
disabled, but a board-level patch retains the SG2002 internal EPHY power/reset
bootstrap before Linux. The vendor Linux driver explicitly assigns that early
work to `board.c`; moving it into PHY `config_init()` is too late because MDIO
must first read the synthesized PHY ID to bind that driver. This corrected
handoff still requires hardware qualification before replacing the proven image.
Both configurations are normalized and checked against the
pinned vendor Kconfig trees before a build may proceed. See `docs/SDK_AUDIT.md`
for the board-support review.

The canonical discovery checkout lives in the case-sensitive Podman volume
`onyx-kvm-sdk` at `/work/sdk`. A checkout on the default macOS filesystem is
not trustworthy for this SDK because case-colliding tracked paths appear
modified immediately after checkout.

## Linux builder

`builder/Containerfile` pins the official amd64 Ubuntu 22.04 base-image
manifest. amd64 is intentional: the vendor host toolchain contains x86-64
executables. On Apple Silicon, Podman runs it under emulation. Build the local
builder with `build-builder.sh`; its resulting image ID must be recorded before
an image release. Package repository contents are not yet snapshot-pinned, so
the builder remains a development input rather than release provenance.
The current development identity and sorted package-set hash are recorded in
`builder.lock`.

The SDK also expects SOPHGO's separately distributed `host-tools` repository.
Only its RISC-V musl subtree is admitted for development builds, pinned by
commit, Git tree, deterministic tree-archive SHA-256, and compiler SHA-256 in
`builder.lock`. The subtree is a stripped binary toolchain and contains no
bundled license texts, so it is not an approved release input. Release requires
corresponding-source/license closure and an independently sourced compiler
comparison or a source-built replacement.

## Recovery networking

Phase 1 permits wired DHCP on `eth0`. The Buildroot fragment and overlay add no
listening or discovery service. See `docs/NETWORK_POLICY.md` for the acceptance
boundary.

`audit-rootfs.sh` rejects known remote-login, discovery, DHCP-server, Wi-Fi,
update-agent, video, and audio executables and requires wired DHCP client
configuration. Runtime socket enumeration remains a mandatory boot test.

`fetch-buildroot-sources.sh` is the sole network-enabled filesystem stage. It
populates a dedicated download volume, relies on Buildroot package hashes, and
records and verifies a complete SHA-256 cache manifest. Then
`build-phase1-rootfs.sh` re-verifies that cache and builds with networking
disabled. A cache fetch is development provenance until the resulting manifest
is reviewed and pinned in the repository.

`package-phase1.sh` creates a minimal FIT from the compiled kernel and board
device tree, copies the audited ext4 root filesystem and FIP into `out/phase1`,
and records their SHA-256 hashes. The FIT deliberately contains no ramdisk.
The script does not write an SD card or physical device.

`assemble-phase1-image.sh` creates a deterministic two-partition development SD
image containing only `fip.bin`, `boot.sd`, and the audited ext4 root
filesystem. The repository-owned assembler writes a minimal FAT16/MBR layout;
its verifier checks the MBR, partition types, embedded rootfs hash, and exact
FIT and FIP payloads. It does not use the SDK's opaque `genimage` utility.
The initial download cache (`cleankvm-buildroot-dl-v1`, created before the
rename) is quarantined: it was populated far enough to expose the rejected
vendor profile's Wi-Fi/Aircrack selections and is never an input to the minimal
build. The clean profile used the distinct `cleankvm-buildroot-dl-v2` cache, and
the product builds use `onyx-kvm-buildroot-dl-v50`.

`prepare-build-volume.sh` clones the verified SDK from the read-only
`onyx-kvm-sdk` volume into a new disposable work volume (default `onyx-kvm-work`, configured in `default-env.sh`), verifies
it, replaces the semantically unused opaque DDR input with a deterministic
512-byte zero block, and applies the Phase-1 kernel, Buildroot, board, and
rootfs-overlay changes. It refuses to overwrite an existing work volume. This
separates immutable inputs from generated build state.

`replace-ddr-compat.sh` is fail-closed: it only replaces the exact audited
vendor SHA-256 and verifies the replacement size and SHA-256. The nonempty
alignment block preserves the vendor packer's FIP2 switch; successful serial
boot and memory stress remain required before this substitution is approved.

The Phase-1 patch set also corrects the SDK kernel wrapper so disabling
loadable modules builds `Image` and headers without invoking the invalid
`modules` and `modules_install` targets. The normalized kernel configuration
remains the authority: module targets are retained if a future profile
explicitly enables `CONFIG_MODULES=y`.

The RISC-V Cvitek architecture Kconfig also incorrectly forces `ION_CVITEK`
despite that driver's `STAGING && ION` dependencies. Phase 1 removes that
forced selection so the multimedia buffer allocator and staging subsystem can
remain absent. This does not affect serial, SD, or wired Ethernet bring-up.

`validate-phase1-config.sh` asks Linux Kconfig and Buildroot to normalize the
generated configuration, then asserts both the required wired-DHCP features
and the absence of every prohibited Phase-1 subsystem. This catches transitive
package selections such as `mosh` enabling OpenSSH or OpenCV enabling FFmpeg.
The Buildroot profile is generated from an empty baseline rather than merged
onto the vendor defconfig: that generic profile normalized to 574 enabled
package symbols, including Wi-Fi firmware, Aircrack-ng, BlueZ ALSA, GDB server,
games, Qt, and Python web frameworks.

## Product build workflow

The sequence below is what the review of this tree exercised. Run it from the
repository root on a machine with Podman and the Rust toolchain
(`rust-toolchain.toml`). Every container step runs with `--network none`.
Only step 0 touches the network.

| Step | Command | Output |
| --- | --- | --- |
| 0 | `build-builder.sh`, `fetch-buildroot-sources.sh` | builder image, download volume (fetch stage only) |
| 1 | `build-rust-daemon.sh`, `build-rust-memtest.sh`, `build-rust-hid-broker.sh`, `build-rust-video-broker.sh`, `build-rust-audio-brokers.sh` | static RISC-V binaries in `out/phase2-rust`, `out/phase3-rust`, `out/phase3a-rust` |
| 2 | `build-phase3-raw-frame-probe.sh`, `build-phase3a-capture-probe.sh`, `build-phase3a-playback-probe.sh`, `build-lt6911uxc-edid-programmer.sh` | probes and EDID programmer in versioned `out/` directories |
| 3 | `ONYX_KVM_WORK_VOLUME=<new> prepare-build-volume.sh` | disposable work volume with the Phase-1 profile and all patches |
| 4 | inside the work volume: `apply-phase2-hid.sh`, `apply-phase3-video-foundation.sh`, `apply-phase3a-audio-enumeration.sh`, `apply-product-profile.sh`, each followed by its `validate-*-config.sh`; finish with `validate-product-config.sh` | product kernel/Buildroot profile |
| 5 | `ONYX_KVM_CONFIG_PROFILE=product build-phase1-rootfs.sh` | audited ext4 rootfs (runs `audit-rootfs.sh` and the debugfs ownership audits) |
| 6 | `package-phase1.sh`, `assemble-phase1-image.sh` | `boot.sd`, `rootfs.ext4`, `fip.bin`, `onyx-kvm-phase1.img` and `SHA256SUMS` |
| 7 | `detect-target-disk.py`, flash, read back, `verify-sd-image.py` | embedded FIP, FIT and rootfs verified from the card |

Known gaps in this sequence, all tracked in [../docs/OPEN_ITEMS.md](../docs/OPEN_ITEMS.md)
B2 and B3: patch 0028 does not apply to a pristine SDK, the vendor middleware
libraries and the video kernel modules are not built by any script here, the
TinyALSA source archive (`/private/tmp/tinyalsa-v2.0.0.tar.gz`, SHA-256 in
`sources.lock`) must be staged by hand, and the default work-volume name differs
between scripts, so set `ONYX_KVM_WORK_VOLUME` explicitly.

Set `ONYX_KVM_CONFIG_PROFILE` to `phase1`, `phase2-hid`, `phase3-discovery`,
`phase3-video-foundation`, `phase3a-audio-enumeration` or `product`. The
`product` profile refuses to start without the raw-frame probe, both audio
probes and the EDID programmer. `ONYX_KVM_INCREMENTAL_ROOTFS=1` reuses a
previous Buildroot target; do not use it across the naming change, because it
retains the old overlay directory (the script now also deletes any leftover
`cleankvm*` paths).

## Local container resources

| Resource | Default name |
| --- | --- |
| Builder image | `localhost/onyx-kvm-builder:ubuntu-22.04-amd64` |
| Pinned SDK source (read-only input) | volume `onyx-kvm-sdk` |
| Vendor host tools (read-only input) | volume `onyx-kvm-host-tools` |
| Buildroot download cache | volume `onyx-kvm-buildroot-dl-v50` |
| Disposable work volume | `ONYX_KVM_WORK_VOLUME` |

Hosts that still have the pre-rename resources run `migrate-legacy-names.sh`
once (dry run by default; `--apply` retags the builder and copies the three
reusable volumes, deleting nothing).

## Environment variables

All build and runtime variables use the `ONYX_KVM_` prefix. The main ones are
`ONYX_KVM_CONFIG_PROFILE`, `ONYX_KVM_WORK_VOLUME`, `ONYX_KVM_BUILDER_TAG`,
`ONYX_KVM_INCREMENTAL_ROOTFS`, `ONYX_KVM_IDENTITY_DIR`,
`ONYX_KVM_RECOVERY_PASSWORD_HASH_FILE`, `ONYX_KVM_VIDEO_MODULE_DIR`,
`ONYX_KVM_RAW_FRAME_PROBE`, `ONYX_KVM_AUDIO_CAPTURE_PROBE`,
`ONYX_KVM_AUDIO_PLAYBACK_PROBE`, `ONYX_KVM_EDID_PROGRAMMER`,
`ONYX_KVM_RUST_ARTIFACT_DIR` and `ONYX_KVM_ARTIFACT_DIR`. The daemon reads
`ONYX_KVM_HTTPS_*`, `ONYX_KVM_MCP_RESOURCE`, `ONYX_KVM_TOKEN_ISSUER`,
`ONYX_KVM_TLS_*` and `ONYX_KVM_ISSUER_*` (set by `S50onyx-kvmd`), and each broker
reads optional `ONYX_KVM_<HID|VIDEO|AUDIO>_*` path overrides whose defaults are the production paths.

## Hardware harnesses

`phase*.exp`, `product-*.exp` and `serial-monitor.py` drive a board over UART
with bounded, non-autoloading, cleanup-verifying sequences. They take the console credential from a file path argument, send it only in
response to a `Password:` prompt, and embed none in the repository. Scripts named for a
release (`phase3-v50-*`, `phase3a-v53-*`) reproduce that release's acceptance
test and are kept as evidence. Before running any of them, read the newest
entry in `docs/NEXT_STEPS.md`, and record initial and final taint, loaded
modules, nodes and output-file disposition as `AGENTS.md` requires.
