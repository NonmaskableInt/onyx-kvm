# SG2002 SDK audit

## Candidate baseline

The discovery checkout is Sipeed's `LicheeRV-Nano-Build` at commit
`d4003f15b35d43ad4842f427050ab2bba0114fa5`. It remains an unapproved input.
The repository is a vendor aggregation containing Linux 5.10, U-Boot 2021.10,
OpenSBI, Buildroot, firmware, host tools, middleware, and board configuration.

The candidate board is `sg2002_licheervnano_sd`. This is a generic LicheeRV
Nano configuration, not a NanoKVM Lite security profile.

The SDK cannot be checked out faithfully on the default case-insensitive macOS
filesystem because it contains tracked paths that differ only by case. A clean
checkout was therefore created in the case-sensitive Linux volume
`onyx-kvm-sdk`. Its worktree was clean at the pinned commit. The full SDK
SHA-256 hashes a commit-based `git archive --format=tar` stream. Component
SHA-256 values hash their recursive `git ls-tree` manifests, avoiding the
non-deterministic timestamp used when archiving a tree object directly.

## Baseline findings

- The top-level board configuration selects RISC-V, musl, SD boot, SG2002,
  Linux 5.10, U-Boot 2021.10, and the LT6911 sensor option.
- It also enables the wireless coprocessor and several unrelated image sensors.
- The device tree exposes Wi-Fi SDIO, multiple UART/I2C/SPI/PWM buses, display
  and touch hardware, a GPIO SPI device, framebuffer, and audio DAC.
- The kernel permits unsigned/forced loadable modules, disables strict kernel
  W^X, and includes wireless, media, GPIO sysfs, I2C character devices, audio,
  writable thermal controls, and broad vendor multimedia support.
- The vendor RISC-V 5.10 tree does not implement `RANDOMIZE_BASE`, so Phase 1
  cannot claim kernel address-space layout randomization. Strict kernel W^X and
  strong stack protection are available and enabled; eliminating loadable
  modules removes the separate module-W^X dependency.
- The stock partition definition creates writable boot and root partitions.
- Vendor documentation expects a mutable Ubuntu build host and separately
  downloaded host-tool binaries. That process is not acceptable release
  provenance for Onyx KVM.
- The first offline boot-chain compile confirmed that the SDK does not include
  its required `riscv64-unknown-linux-musl` compiler. SOPHGO's official
  `host-tools` repository provides the expected GCC 10.2.0 binaries. U-Boot,
  OpenSBI, and Linux use its musl-prefixed subtree; FSBL separately requires
  its GNU-prefixed subtree. Both are pinned and hashed for development use,
  but their compiler executables are stripped and the subtrees contain no
  bundled license texts. They remain prohibited as release trust anchors until
  corresponding source/license obligations and an independent compiler
  comparison or source-built replacement are complete.
- A clean, network-disabled Phase-1 kernel build now succeeds. Its normalized
  configuration removes modules, virtual consoles/input, host USB and the
  separately controlled USB gadget stack, HID, Wi-Fi/Bluetooth, audio, media,
  framebuffer/backlight, I2C, SPI, debugfs, SysRq, IPv6, netfilter, tunnels,
  NFS, MTD, and SCSI while retaining serial TTY, SD/MMC, IPv4, and STMMAC
  Ethernet. The resulting development `Image` is 10,850,304 bytes with SHA-256
  `9390b2e1393c9ebc3969f7e8fcea53354bb7c73a677a8843a667dc1944a66f70`.
- The first compile trace caught an important vendor-Kconfig distinction:
  disabling host `CONFIG_USB` did not disable USB gadget functions. Phase 1
  now separately asserts that gadget, DWC2, libcomposite, ConfigFS gadget,
  HID, storage, serial, and USB-network support cannot be selected.
- The vendor `build_kernel` wrapper packages a pre-existing SDK ramdisk into
  `boot.itb`; it does not establish an independently built root filesystem.
  That generated bundle is rejected. It also failed its final `boot.sd` move
  because temporary-file extended attributes cannot be copied onto the Podman
  volume. Neither issue affects the compiled kernel, but both block an image.

## Decision

Do not build or flash the vendor defconfig as Onyx KVM. Phase 1 must derive a
minimal board configuration with only boot-critical firmware, serial console,
SD storage, watchdog/thermal observation, and wired Ethernet. Wi-Fi, audio,
video, HID, virtual media, package management, SSH, and web services stay off.

`image/verify-sources.sh` deliberately refuses to pass while any source is
marked `PENDING-REVIEW`, the SDK revision differs, or its tracked tree is
modified. `image/build.sh` also refuses non-Linux hosts and remains blocked
until the Phase-1 integration step exists. These failures are release safety
properties, not unfinished-error bypasses.

## Opaque artifact findings

- FSBL contains a 28,688-byte AArch64 Trusted Firmware-style `bl31.bin`
  (`sha256:e58187f79243c4a296dddc1c7c34201676e237b3247c8994783c1c638dbcebd3`).
  Static strings identify EL1/AArch64 and EL3-style runtime services. The build
  selects this file only when `BOOT_CPU=aarch64`; SG2002 RISC-V instead builds
  and packages OpenSBI `fw_dynamic.bin` from the pinned source. The BL31 binary
  is therefore excluded from Onyx KVM Phase 1.
- The RISC-V FIP rule still supplies the opaque 8,192-byte
  `fsbl/test/cv181x/ddr_param.bin`
  (`sha256:5b7d44720997f018621fe4a462111b2d53954a5d1e2a794c83dd1dd6516eebaa`).
  It is byte-for-byte identical to `data/ddr_param.bin` in SOPHGO's public
  `fiptool` repository. That repository documents the file as a packaging input
  for SG2002/Milk-V Duo 256M and carries a BSD-style license, but provides no
  generator, format description, or corresponding parameter source.
- Static measurements are consistent with pseudorandom or deliberately opaque
  data, rather than a conventional parameter table: entropy is 7.974563126
  bits/byte, the byte-frequency chi-square is 291.0625 with 255 degrees of
  freedom, serial correlation is 0.00259634, all 2,048 aligned 32-bit words are
  unique, and compression makes the file larger. It has no recognized magic or
  executable/container header. These measurements do not prove how the bytes
  were generated.
- SG2002 BL2 loads the blob into a 16 KiB byte-array, verifies its FIP CRC, and
  passes it to `ddr_init`. In the pinned source, `ddr_init` only logs the first
  32-bit word. The sole apparent interpreter call, `bldp_init(ddr_param)`, is
  commented out. Actual initialization calls the source-built
  `read_ddr_pkg_info()` and `ddr_sys_bring_up()` routines. A repository-wide
  search found no other SG2002 consumer of the contents.
- The blob is nevertheless structurally required by the unmodified packer:
  `FIP.make()` appends the complete second FIP section only when DDR parameter
  length is nonzero. That same section carries source-built OpenSBI and U-Boot.
  Passing the existing empty test file would therefore produce an incomplete
  boot package.
- Current classification: **non-executable, authenticated but semantically
  unused compatibility filler on the audited SG2002 path**. Hardware testing
  proved its replacement with one deterministic 512-byte zero-filled alignment
  block: the source-built chain booted repeatedly, Linux reached authenticated
  recovery, and four 64-bit patterns passed across a bounded 64 MiB RAM window.
- FreeRTOS is disabled, so `BLCP_2ND_PATH` is unset. The optional fast-image
  coprocessor path falls back to the tracked zero-length `test/empty.bin`.
- Middleware contains many prebuilt static and shared libraries for audio,
  AAC, voice processing, ISP algorithms, CLI support, and sample media
  servers. Several have no top-level license evidence in the aggregation.
- The Phase-1 recovery image therefore excludes middleware entirely. Later
  video or audio phases may admit only the smallest individually identified
  artifacts after source/license correspondence and sandboxing review.
- The SDK contains crypt-tool configuration and a key-generation helper. No
  vendor or example key is permitted to become a Onyx KVM signing identity;
  release keys must be generated and held outside the source tree.

## Required before approval

- Inventory embedded executables, firmware, keys, and host tools; remove those
  not required to boot the NanoKVM Lite.
- Extend the completed zero-DDR boot and bounded 64 MiB pattern test with an
  address-sensitive, long-duration memory qualification. On 2026-09-06, the
  source-owned Rust utility completed 600 address/inverse-address and March
  C-minus passes over a bounded 64 MiB allocation with zero failures in
  473,055 ms. This userspace test covers its allocated virtual-memory region;
  it does not establish coverage of a chosen physical-address window.
  FSBL source files
  carry a mixture of BSD-3-Clause,
  GPL-2.0+, eCos-2.0, and Unlicense markers and require a shipped-file license
  inventory rather than a single aggregate label.
- Resolve the missing top-level license provenance for middleware.
- Identify licenses and corresponding source for every shipped artifact.
- Confirm the Lite-specific Ethernet PHY, LT6911, USB gadget, watchdog, LEDs,
  and audio routing from hardware evidence.
- Build in a pinned, network-disabled Linux environment and publish its recipe
  and digest.
- Build the Phase-1 Buildroot filesystem from verified source inputs, audit its
  complete shipped-file manifest, and replace the vendor ramdisk packaging
  path before creating a bootable image.

## Phase-1 development image result

- The minimal Buildroot ext4 filesystem passed the offline rootfs audit and
  hashes to `ecab95213e2c8a37903150fcfcc51fa485afdf3414b683c5bf8c8e05815fc301`.
- The replacement FIT contains only the source-built Linux kernel and SG2002
  LicheeRV Nano DTB, each protected by a SHA-256 FIT hash. It contains no
  ramdisk node or reference.
- The repository-owned deterministic assembler produced the 80,740,352-byte
  two-partition image `cleankvm-phase1.img`, SHA-256
  `7948006cc8d592cb59aef52d4658bc8db0ec6ca79ba5cc6f96ae1f852844113c`.
  Its verifier confirmed the MBR, bootable FAT16 partition, Linux partition,
  and byte-identical embedded FIT, FIP, and audited rootfs.
- The reduced U-Boot candidate fixes boot to `boot.sd`, removes unsigned
  `uEnv.txt` import and update fallback, and disables DHCP, TFTP, PXE, fastboot,
  display, EFI, and writable FAT/MMC commands. U-Boot shrank from 392,782 to
  255,934 bytes. Hardware testing showed that removing the Ethernet/PHY driver
  initialization broke Linux interface bring-up; retaining only that handoff
  restored the DHCP client without restoring U-Boot network commands.
- The corrected zero-DDR FIP is 275,456 bytes and hashes to
  `d709a7e5f20abc570e671d7763a764be3edbe82733208818caef0e3829d4d18d`.
  Its 79,692,288-byte SD image hashes to
  `c47d065ca511cc52c9aff62aff1480f84f151c86e470417f255ad9d45d0fcbfd`.
  Full SD read-back and the final canonical MBR were verified before boot.
- The memory-qualification image is 79,692,288 bytes and hashes to
  `96ac9bd26a5bf3287c50f6716e95c84379b81603d27c6f2aad22f2bc21f9ab8b`.
  Full SD read-back passed. On hardware, the installed tester matched the
  expected 387,352-byte ELF and SHA-256
  `ea433ce87cbd1fbc0a0f111054d4c8d435ea5139fb661cf5c8e35be6b0586c70`,
  its 8-pass functional run completed in 6,309 ms, and its 600-pass sustained
  run completed in 473,055 ms with no failures. The subsequent normal reboot
  verified both FIT hashes and returned to the locked recovery login.
- The next hardware candidate moves the SG2002 internal EPHY power/reset
  sequence into Linux's `cvitek` PHY driver and disables U-Boot's complete
  network core (`NET`, `NET_RANDOM_ETHADDR`, `DM_ETH`, `ETH_DESIGNWARE`, and
  `PHY_CVITEK`). Binary inspection found the Linux PHY object and its
  `cv182xa_phy_config_init` routine, while the U-Boot ELF contains none of the
  checked Ethernet symbols. The independently assembled 79,692,288-byte image
  hashes to
  `b845f3acc5a5ab99a1de2df418ce0570f1f6f3ce577da71a5744ac5a53c07466`.
  Its FIT and embedded partition artifacts pass offline verification. Physical
  testing confirmed FIT verification and a complete Linux/userspace boot, but
  interface startup failed with `SIOCSIFFLAGS: Invalid argument`; DHCP did not
  run. The candidate therefore does not replace the proven U-Boot handoff.
- Source-level review explains that failure: U-Boot `board.c` powers and resets
  the integrated EPHY, synthesizes its PHY ID, and exposes it to MDIO before
  Linux. Linux `cvitek` PHY `config_init()` retains calibration and filtering,
  while its corresponding bootstrap block is commented with `do this in
  board.c`. Since `config_init()` runs only after PHY discovery, it cannot own
  the bootstrap. A corrected candidate therefore disables U-Boot networking
  and `PHY_CVITEK` while retaining only this ASIC board-level bootstrap under
  `CONFIG_TARGET_CVITEK_CV181X_ASIC`. Its 79,692,288-byte image hashes to
  `c14bd98ce18861c7e8280d78eeb83b785fd3dd842658db56af29586989c6d937`;
  offline FIT, partition, and embedded-artifact verification passed. Physical
  UART testing then verified both FIT payloads, registered the Ethernet
  interface, and started `udhcpc`. A subsequent boot with packet capture armed
  observed the device's Discover/Request exchange, the Mac sharing server's
  Offer/ACK, and assignment of `192.168.2.10`; two ICMP probes returned with
  zero loss. Carrier was 100 Mbps full-duplex, RX/TX counters advanced with no
  hardware errors. This qualifies the corrected boot-stage PHY handoff and
  wired DHCP path.
- This remains a development image, not an approved release: the FIP still
  depends on development-only toolchains, and source/license inventory,
  clean-builder reproduction, broader physical testing, and release-key
  verified boot remain open.

## Shipped-source license review (2026-10-19)

Reviewed in the case-sensitive volume `onyx-kvm-sdk` (pinned commit `d4003f15`) and the pinned sophgo-osdrv checkout. Methods: license-file inventory, SPDX and header scan of every `.c`/`.h`/`.S` file, and the actual link lines (`fsbl/plat/cv181x/platform.mk`, `image/build-phase3-raw-frame-probe.sh`).

**FSBL** (builds BL2 inside `FIP.BIN`; corresponding source is the pinned tree):

| Component | Files | License evidence |
| --- | --- | --- |
| Arm Trusted Firmware derived `include/`, `lib/cpu`, `lib/stdlib`, `lib/tf_printf`, part of `plat/` | about 100 | BSD-3-Clause (SPDX or header) |
| `lib/lz4` | 10 | BSD-2-Clause (`LICENSE`) |
| `lib/BigDigits` | 27 | MPL-2.0 (`LICENSE.txt`, `bigdigits.c` header); linked by `plat/cv181x`; headerless siblings fall under the same directory license |
| `lib/libtommath` | 143 | Unlicense (SPDX) |
| `lib/libtomcrypt` (SHA only is linked) | 425 | public domain or WTFPL (`LICENSE`) |
| `lib/lzma` | 3 | public domain |
| `lib/crc/crc16.c` | 1 | eCos-2.0 (SPDX) |
| Cvitek/Bitmain-authored `plat/cv180x`, `plat/cv181x`, `lib/utils` | about 115 | **no license header or file** |

The earlier aggregate label omitted MPL-2.0 (BigDigits) and the unlicensed vendor-authored platform files. The MPL-2.0 and eCos-2.0 files must keep their notices if the FIP is distributed.

**Middleware** (`middleware/v2`): the product's raw-frame probe links `libsys.a` and four `modules/vpu/src` objects. They are built from source in the pinned tree (so corresponding source exists), but the linked sources carry no license header; `modules/sys/src/peri.c` is marked "Proprietary and confidential" and `hashmap.c` is Apache-2.0. No license file exists for the tree. Outcome: **no license grant**.

**sophgo-osdrv** (`1b3a4964`, source of the `cvi_vc_driver` hardware H.264 module, patch 0028): no top-level license and no header in the patched sources. Outcome: **no license grant**.

**Not yet reviewed**: the other vendor kernel modules the product loads (`soph_sys`, `soph_base`, `soph_mipi_rx`, `soph_snsr_i2c`, `soph_vi`, `soph_vpss`, `soph_jpeg`, `mars_vcodec`), which come from the SDK `osdrv/interdrv/v2` tree. They have no lock entries.

**Builder**: the builder is fixed by the local image digest and the package manifest in `image/builder.lock`; there is no apt snapshot because the builder is not rebuilt from apt during a build.

**Conclusion**: nothing here is a technical blocker for development builds, which waive the markers explicitly (`ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1`). A release needs a written license grant from Sophgo/Cvitek for the vendor-authored FSBL, middleware and osdrv files, or replacement of them.
