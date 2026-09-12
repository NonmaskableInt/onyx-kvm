# Image build

This directory will contain the pinned SG2002 board-support source, Buildroot
external tree, kernel configuration, root filesystem overlay, image layout,
and release scripts.

We will initially treat Sipeed's NanoKVM-capable SDK as board-support source,
not as a trusted runtime image. All configurations and patches used by the
resulting image belong in this repository. Network access is allowed only in a
separate fetch step; the build step consumes verified local inputs.

`sources.lock` deliberately contains placeholders until revisions and license
files have been reviewed. A build script must never resolve these placeholders
or silently select a branch head.

The SDK `main` revision recorded in the lock file is only a discovery result.
It remains unapproved until its submodules, patches, NanoKVM/LT6911 board
configuration, licenses, toolchain binaries, and boot result are reviewed.

## Safety gates

`verify-sources.sh` validates the exact SDK revision, rejects tracked changes,
and refuses every `PENDING-REVIEW` entry. `build.sh` invokes that gate before
doing anything and currently stops intentionally: the independent Linux
builder and Phase-1 integration have not yet been approved.

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
`cleankvm-sdk` at `/work/sdk`. A checkout on the default macOS filesystem is
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
The initial `cleankvm-buildroot-dl-v1` cache is quarantined: it was populated
far enough to expose the rejected vendor profile's Wi-Fi/Aircrack selections
and is never an input to the minimal build. The clean profile uses the distinct
`cleankvm-buildroot-dl-v2` cache.

`prepare-build-volume.sh` clones the verified SDK from the read-only
`cleankvm-sdk` volume into a new disposable `cleankvm-work-v4` volume, verifies
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
