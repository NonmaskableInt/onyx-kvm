# Phase 3 video admission

Status: source-closure review; no video module or vendor middleware is admitted
to a CleanKVM image yet.

## Hardware target

- I2C controller: bus 4 / `i2c@04040000`
- live responding 7-bit address: `0x2b`
- populated bridge: LT6911UXC with high confidence
- `0x2c` (LT6911C in the stock detection logic) did not respond

## Minimal kernel-module candidate

Only the following out-of-tree SDK modules are candidates for a raw-frame
snapshot experiment:

1. `soph_sys`
2. `soph_base`
3. `soph_mipi_rx`
4. `soph_snsr_i2c`
5. `soph_vi`
6. `soph_vpss`

The dependency metadata in the retained stock modules says `soph_base` depends
on `soph_sys`; MIPI RX depends on `soph_base`; and VI/VPSS depend on both. The
sensor-I2C module is included because the vendor capture stack uses its private
sensor-control ABI. Every admitted module must be rebuilt from the pinned SDK
source against the exact CleanKVM kernel. Stock `.ko` files are evidence only.

The initial snapshot path explicitly excludes `soph_vcodec`, `soph_jpeg`,
`soph_vc_driver`, `soph_tpu`, `soph_ive`, `soph_dwa`, `soph_vo`,
`soph_mipi_tx`, `soph_rtos_cmdqu`, and every audio module. Generic V4L2/media,
UVC, UAC, ALSA, I2C slave, I2C mux, and GPIO-I2C also remain disabled.

The first independent compile proved that `soph_sys` directly consumes private
extensions in CVITEK's ION allocator (`ion_buffer` physical-address/name fields
and CPU-access helpers). The video-only kernel profile therefore admits built-in
`STAGING`, `ION`, and `ION_CVITEK`; it does not admit DMA-BUF heaps or any
additional in-tree module. This allocator is part of the video trusted surface
and must be included in source review and memory-isolation testing.

## Userspace boundary

The SDK capture API is not an upstream V4L2 interface. Before a frame can be
captured, the required source and binary closure for its VI/VPSS/system/ISP
middleware must be established. The network-facing Rust MCP daemon will never
link this middleware or open its device nodes. A separate local capture broker
will own the minimum nodes, accept the fixed `CKM3` protocol over a Unix socket,
and return only a tightly bounded, freshly allocated snapshot buffer.

The broker must have no network, HID, credential, updater, shell, arbitrary
filesystem, block-write, or general GPIO access. It starts disabled and gains
no MCP route until frame-size, padding, buffer-reuse, malformed-request,
disconnect, restart, and power-loss tests pass.

## Remaining admission blockers

- Rebuild the six candidate modules independently and record their complete
  source/header closure and hashes.
- Establish license provenance for every compiled unit. An in-binary `GPL`
  module tag is not sufficient licensing evidence by itself.
- Determine the smallest userspace VI/VPSS/system/ISP closure and reject opaque
  libraries that cannot be independently accounted for.
- Reconcile the LT6911 reset, interrupt, MIPI lanes, and timing initialization
  with the actual board. Do not copy a full vendor initialization table without
  register-by-register review.
- Capture one raw frame locally before adding encoding, streaming, or MCP
  exposure.

## Independent module build v1

A fresh disposable SDK volume was cloned from verified commit
`d4003f15b35d43ad4842f427050ab2bba0114fa5`, then received the repository-owned
Phase 1, Phase 2 HID, and video-foundation changes. Network access was disabled.
The exact kernel compiled with the pinned RISC-V musl toolchain, followed by an
explicit make invocation naming only the six allowlisted targets. Exactly six
modules were produced and exported to `out/phase3-video-modules-v1`:

- `soph_base.ko`: `cf453e9d5c837f20fdbf76bfb2ff579d5ed345848d07fcbe8e4c9d4aa50458cf`
- `soph_mipi_rx.ko`: `fb987b6dd87238a197219c0b4777026dd2349ae13eed6b3961eb9f55da97c7a9`
- `soph_snsr_i2c.ko`: `1dd4c0412f1b8ceb1fcdd8cabfcc673e5ddfb45e59a0bc6e5d702eef3f2e3c4f`
- `soph_sys.ko`: `9540a47298217f080d4e7ddbf3bea4e3dd1300fd4bc66a7b51a5217bad48fecd`
- `soph_vi.ko`: `d0e1b1624f48423d08abf46d7ad9bcf245b280292d94a0ef3b92699857fde9ce`
- `soph_vpss.ko`: `5d8f749849b9fa5ac135c16b3ca584d941a4d9f53e7ec3247864923927242861`

ELF `.modinfo` inspection reports GPL for all six. Its declared graph is:
`soph_sys` has no module dependency; `soph_base` depends on `soph_sys`;
MIPI RX and sensor I2C depend on `soph_base`; VI and VPSS depend on both.
These tags are useful consistency evidence but do not replace file-level
license provenance. The SDK emitted missing aggregate `Module.symvers` warnings
for VI and VPSS even though their explicit base/sys symbol-version inputs were
present; unresolved-symbol and live load-order validation therefore remain
mandatory.

The subsequent ABI audit resolved every undefined symbol against the exact
kernel or the six-module allowlist. A deterministic source closure covers 217
tracked paths (199 C/header paths), including shared inter-driver headers and
CVITEK ION. Only 16 carry explicit SPDX lines, so file-level provenance remains
an open release gate.

## Non-autoload quarantine image v1

The six modules were installed under
`/usr/lib/cleankvm/video-quarantine`, mode `0400`, with no init script or
automatic loader. The rootfs audit compares every module byte-for-byte, requires
exactly six entries, and rejects init scripts that name the quarantine or a
Sophgo module. Offline ext4 inspection reconfirmed the packaged files.

The resulting 79,692,288-byte image is
`out/phase3-video-quarantine-v1/cleankvm-phase1.img`, SHA-256
`f5ee4b1e3fc678e21e41b2c927dd249701311d3d95741918037449e02ac000ee`.
Its FIT kernel hash is
`f11e12b6cca50c81929ca9793a3829757d260f71b2b325a1d21b2d8eae41cee3`;
the board DTB remains
`a67da16ae839f790bfe871f80a5b66451131cadd0b662f6afc2456dfeb9e3a70`.
The FIP is byte-identical to Phase 2 v8. This artifact does not enable video;
it exists only for controlled UART load-order and unload testing.

## Hardware load/unload result

The quarantine image booted on the NanoKVM Lite and initially exposed no media
or audio nodes and no Sophgo modules. An authenticated UART test loaded exactly
the six allowlisted modules in dependency order. Every `insmod` returned zero,
and the expected `/dev/cvi-{sys,base,mipi-rx,vi,vpss}` nodes appeared. The
sensor-I2C driver attached only to the already enumerated buses 0, 1, 3, and 4.
All six modules then unloaded successfully in reverse dependency order; every
`rmmod` returned zero, the device nodes disappeared, and `/proc/modules` had no
remaining `soph_` entry. The test is reproducible with
`image/phase3-video-quarantine-test.exp`.

This is not yet an admission pass. On the first load the running kernel
reported `soph_sys: bad vermagic: kernel tainted` and the final taint value was
4098 (forced module plus out-of-tree module). The running release string was
`5.10.4-tag-`, matching the visible module release string, so the remaining
configuration-level vermagic mismatch must be identified and removed. Forced
module loading must be disabled before any video module can leave quarantine.

## Fail-closed quarantine image v2

The Phase 3 profile now explicitly disables `CONFIG_MODULE_FORCE_LOAD` and
`CONFIG_MODULE_FORCE_UNLOAD` while retaining ordinary module unload support;
the validator enforces those settings. The kernel was rebuilt first, then
exactly the same six modules were cleaned and rebuilt against that generated
kernel tree with networking disabled. All six report
`5.10.4-tag- preempt mod_unload riscv`.

The rebuilt root filesystem passed the exact-six, byte-identity, mode `0400`,
and non-autoload audits. The assembled image is
`out/phase3-video-quarantine-v2/cleankvm-phase1.img`, SHA-256
`509c0d6ef84c44780ea475d73edfcb8c1a98c3c9cc4bdba33de41613e08b13ac`.
Its FIT kernel hash is
`d0cbaea70975ea74a88e705d9dbad4010c2e10fe27d46ab626f7c2af85aafe2f`;
the DTB and FIP are unchanged. Hardware must now prove that a normal load
succeeds without forced-load taint before this blocker is closed.

The diagnostic boot then identified the root cause in the vendor kernel:
`load_module()` unconditionally added both `MODULE_INIT_IGNORE_MODVERSIONS` and
`MODULE_INIT_IGNORE_VERMAGIC` on `CONFIG_ARCH_CVITEK`. CleanKVM removes that
architecture-wide bypass with `0007-kernel-enforce-module-version-checks.patch`.
The temporary logging was removed, the kernel and six modules were rebuilt in
order from the same tree, and all containment/image audits passed. The hardware
candidate is `out/phase3-video-operational-v4/cleankvm-phase1.img`, SHA-256
`fa6a07be5a12d4aa889520f7e4dfb3a0e11db311b21f87fe11d6704fe16a78f9`.

Hardware qualification of v4 passed. U-Boot verified both SHA-256 FIT payloads,
Linux obtained DHCP, and pre-load taint was zero. All six modules loaded through
the normal version-checked path with return code zero and created only the five
expected `/dev/cvi-*` nodes. `/proc/modules` marked them `O`, not `FO`. All six
then unloaded in reverse dependency order with return code zero; no video node
or Sophgo module remained. Final taint was 4096, the expected out-of-tree bit
only, proving that the forced-load bit was eliminated. The module identity and
load/unload hardware gate is complete; frame capture and provenance remain
separate admission gates.

## Source-only raw-frame candidate v5

The stock `test_mmf` path was rejected because it combines capture with RTSP,
HTTP, encoding, OLED writes, shell execution, and the prebuilt
`libmaix_mmf.a`. The replacement one-shot probe links only the source-built
CVITEK SYS/VB and VI objects plus musl and `libatomic`; ISP, AE, AWB, VENC,
audio, streaming, and `libmaix` are absent from its link map. Before entering
vendor-derived code it installs `no_new_privs` and a seccomp boundary that
denies IPv4/IPv6 socket creation and all program execution while retaining the
local Unix/netlink operations required by the driver API. It uses the reviewed
MIPI ioctl ABI, does not access LT6911 I2C registers, accepts no network input,
captures at most one 1920x1080 planar-YUV422 frame, creates the output with
mode 0600 and `O_EXCL`, and tears the pipeline down in reverse order.

The 97,088-byte probe has SHA-256
`d31e8b78a7b70e83a95d15d2bb33d4b1787ce25cba0cef4a85a5681d34191323`.
The non-autoload image candidate is
`out/phase3-raw-frame-v5/cleankvm-phase1.img`, SHA-256
`41da24191f4ab6f21e13fcd5f8a43f0ab8ea85e808e76bc00594b4ae14782104`.
Its kernel, DTB, and FIP are unchanged from operational v4. The ext4 audit
confirms the probe is root-owned, mode 0500, byte-identical, and absent from all
init scripts. Final media read-back passed after reinsertion: all embedded boot
artifacts verified byte-for-byte and the ext4 partition matched SHA-256
`1776e4f09ba468a6a258f7ff4813c38cdb39d8489cf6d4075c519a702d3a5a94`.
The whole-media hash differed only in macOS-written FAT bookkeeping outside the
verified payloads. This candidate is cleared for the controlled UART boot test.

The hardware run booted fail-closed and loaded all six quarantined modules.
`CVI_SYS_Init` then stopped because `/dev/mem` is absent; no frame was created.
Reverse cleanup removed every module and media node, with final taint 4096.
Restoring arbitrary physical-memory access is rejected. The next candidate
must instead copy only a live Video Buffer block: resolve it through
`vb_physAddr2Handle`, validate the requested range against its pool block size,
cap plane and total-frame sizes, and copy from `vb_handle2VirtAddr`. Unknown,
released, oversized, and wrapped ranges must fail without copying.
## Bounded-copy raw-frame candidate v7

The source-built base driver now exposes a bounded Video Buffer copy operation
instead of restoring `/dev/mem`. It resolves only a live VB handle, validates
its pool, reference count, offset, and block bounds, limits each copy to 4 MiB,
and invalidates the cache before copying. The probe uses this operation and
contains no `/dev/mem` reference. Its SHA-256 is
`745535f38ad318084ce9549a9349fa5f49600c4e37940c38ed57187a46c09b2d`.
The image is `out/phase3-raw-frame-v7/cleankvm-phase1.img`, SHA-256
`1312b2268a6cfda703956706503d9482cd80ec5ec0e2b3eb994c3fbda9dbc384`.

The first v7 hardware run passed MIPI configuration and reached the bounded
frame wait. A second run sampled `/proc/mipi-rx` while that wait was active.
The configured values matched the SDK LT6911 candidate (`devno=1`, YUV422
8-bit, lanes `2,0,1,3,4`, all PN swaps set, 600 MHz MAC, HS settle 8), but the
receiver reported zero protocol errors, an unknown decode format, all physical
lane samples zero, every digital data lane `hs_idle`, no high-speed clock, and
idle deskew. VI retained two work buffers and completed none before timeout.
The current blocker is therefore ahead of VB copying: no electrical CSI-2
stream is reaching the SoC.

The SDK sensor object does not program an LT6911 initialization table:
`lt6911_init()` only opens I2C. The RTOS camera path asserts sensor reset,
resets/configures MIPI, enables the optional sensor clock, and deliberately
comments out sensor-reset deassertion; the board device tree supplies one reset
GPIO entry while the LT6911 attributes select MIPI device 1. A blind reset
sequence change is therefore unjustified. The next diagnostic must distinguish
absent HDMI/bridge output from reset routing or bridge firmware state before
any register writes are admitted.

## LT6911 reset and MIPI-pad qualification

The v19 hardware trace proves the stock GPIO451 mapping is port B line 3 with
active-low polarity. U-Boot selects ADC1's GPIOB3 function, assertion changes
the line to an output sampled low, and release is sampled high. The LT6911
continues to acknowledge only bus 4/address `0x2b` and returns ID `0000` in all
three states. Reset routing, polarity, mux, and pad behavior are therefore no
longer candidate causes.

The cv182x LT6911 source also selects function 3 on all ten MIPI-RX pads before
capture. v20 moves that immutable routing into U-Boot and removes the conflicting
CAM_MCLK1 assignment on MIPI_RX0N without exposing `/dev/mem`. v21 then performs
only reset and MIPI receiver configuration, waits 500 ms, and exits before VI
or frame allocation. On hardware, the receiver configuration is accepted, but
all six physical samples remain zero, all four digital data lanes report
`hs_idle`, `CK_HS` remains zero, and decode remains unknown. The corrected pad
mux is therefore not sufficient: the evidence now points upstream to LT6911
power, firmware release, or HDMI/EDID state. No LT6911 initialization table will
be guessed or written without corresponding-source evidence.

Subsequent offline inspection recovered the stock product profile at
`/mnt/data/sensor_cfg.ini.LT`. It selects MIPI device 0, lane order
`2,4,3,1,0`, PN swaps `0,0,0,0,0`, bus 4, and no sensor MCLK. The SDK INI parser
stores the five lane and PN values verbatim, and its normal sensor setup passes
those runtime values into the receiver attributes. This means v21 tested the
generic SDK adapter defaults (device 1, lanes `2,0,1,3,4`, all PN swaps set),
not the NanoKVM Lite wiring. The all-idle result therefore does not yet prove an
upstream bridge power or firmware failure. The next bounded probe must retain
the v21 status-only boundary while using the recovered product profile.

That v22 probe and full image are now built. The build remained network-disabled
and passed source-cache, normalized kernel configuration, rootfs, ownership,
FIT, embedded-artifact, and final-image verification. The 79,692,288-byte image
is `out/phase3-mipi-status-v22/cleankvm-phase1.img`, SHA-256
`7162fa38bf50bcb248a7b14f8fc58f8464d87d26ff287d43b7974b87c3f1b98e`.
It awaits removable-media resolution, flashing, read-back, and one controlled
UART boot.

The first card later presented for read-back proved to be the prior v21 card:
its installed probe and FIP hashes matched v21 rather than v22. No result from
that boot is attributed to v22. The subsequently resolved 32 GB card was
flashed again and verified before ejection; the embedded boot FIT, rootfs, and
FIP each matched the audited v22 artifacts. v22 now awaits its first controlled
UART boot.

The first controlled v22 boot matched the expected diagnostic SHA-256 and
passed baseline containment with taint zero, no Sophgo modules, and no media
nodes. Ethernet acquired `192.168.2.62`; both Rust services were healthy and
the HID nodes remained root-only. With explicit approval, the bounded status
test loaded the six quarantined modules and configured the exact stock profile:
device 0, YUV422 8-bit, lanes `2,4,3,1,0`, and zero PN swaps. Nevertheless, all
six physical samples were zero, every digital data lane was `hs_idle`, `CK_HS`
was zero, and decode was unknown. The reset line again sampled low during
assertion and high after release; bus 4/address `0x2b` still returned protected
ID `0000`. No frame was created. All modules unloaded in reverse order, all
media nodes disappeared, and final taint was the expected out-of-tree-only
4096. The product-profile mismatch is now ruled out, leaving the LT6911/HDMI
side or the physical bridge-to-SoC CSI connection as the remaining boundary.

The next source-backed discriminator is reset timing. Normal stock startup only
leaves GPIO451 released high; v22 instead pulsed reset and sampled after 500 ms.
v23 therefore performs no LT6911 I2C transaction and no reset operation. It
configures only the exact stock SoC receiver profile, waits five seconds for
HDMI negotiation, reports status, and still exits before VI/frame allocation.
The network-disabled build and all rootfs/FIT/image audits passed. Its
79,692,288-byte image is `out/phase3-mipi-status-v23/cleankvm-phase1.img`,
SHA-256 `fd83345a8f3a2a1f84c50dcf41e7cd88b895445db55c68c85f673c75071b23c4`.
The resolved card passed immediate read-back for the exact boot FIT, rootfs,
and FIP and was safely ejected for the controlled boot.

The controlled v23 test confirmed the exact installed probe and clean baseline,
then loaded the quarantined modules with explicit approval. With no bridge I2C
transaction, no reset pulse, and a five-second negotiation interval, the result
was unchanged: all physical samples zero, all data lanes `hs_idle`, `CK_HS=0`,
and unknown decode. Cleanup removed every module and media node; no frame was
created and final taint was 4096. This rules out the diagnostic reset pulse and
the shorter v22 wait.

The connected Mac simultaneously failed to enumerate NanoKVM as a display; it
listed only its internal panel and `S34CG50`. This host-side result promotes
missing HPD/EDID presentation above a downstream CSI wiring fault. Sipeed's
official NanoKVM FAQ independently instructs Cube/Lite users to program the
retained 256-byte `E21_NanoKVM.bin` with `nanokvm_update_edid` and power-cycle:
<https://github.com/sipeed/sipeed_wiki/blob/main/docs/hardware/en/kvm/NanoKVM/faq.md>.
The stock utility remains unexecuted and excluded. CleanKVM will not reproduce
its writes until the sequence has auditable source or an independently justified
register-level specification with readback and rollback bounds.

The stock `S15kvmhwd` path exports GPIO451, sets it as an output, and leaves it
high; it does not program EDID, load LT6911 firmware, or pulse reset. The
separate `nanokvm_update_edid` utility contains explicit LT6911C/UXC EDID read
and write paths and GPIO451 toggles, but the normal startup scripts do not invoke
it. This is consistent with a persistently configured bridge and further
reduces the justification for speculative register writes.

## Source-backed EDID candidate v24

Sipeed subsequently published the GPL-3.0 EDID utility source. The reviewed
revision is `bd070b2f5ce27d22618e8a1c8d2bdca543429882`; source and E21 hashes are
recorded in `image/sources.lock`, closing the opaque-table blocker without
admitting the stock executable.

`image/lt6911uxc-edid-programmer.c` is limited to the established Lite endpoint
and embedded validated E21 bytes. It has no arbitrary file/register interface,
requires `--program-e21`, pre-reads, skips a match, verifies all 256 bytes, and
resumes the MCU during cleanup. It is root-only mode `0500`, with no init hook.
Its SHA-256 is
`3f8b4ced84e4445a25c8a058e3d7e194f01af5b757021206e0367c45d0db0195`.

The network-disabled v24 build passed rootfs, FIT/FIP, embedded-artifact, and
image verification. The 79,692,288-byte image is
`out/phase3-edid-v24/cleankvm-phase1.img`, SHA-256
`0b04ac40f25b700fa096a52ff7dac524f9b13006f4513a7a2b6bbc7eab86df35`.
Hardware mutation is pending card flash/read-back, UART baseline, one explicit
local run, physical power removal, host enumeration, and status-only CSI test.

The resolved 31.0 GB external card was then flashed and exactly read back.
Boot FIT, rootfs, and FIP matched the audited v24 artifacts before safe ejection.
The whole-card read-back SHA-256 was
`65ffff153df9583374e7ccbe71095e9f69ef8ae4de4f221b71501ce7fb221342` after
macOS FAT bookkeeping. Hardware mutation remains pending the controlled boot.
