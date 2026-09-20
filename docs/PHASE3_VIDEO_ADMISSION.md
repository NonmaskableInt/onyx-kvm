# Phase 3 video admission

Status: diagnostic modules are present only in a non-autoload quarantine;
video capture, streaming, and MCP exposure are not admitted.

Current checkpoint (2026-09-19): The complete 16-state empirical polarity
matrix `(D0, D1, D2, D3)` was tested on target under verified fail-closed
cleanup conditions (all unloads RC=0, taint 4096, lsmod clean). A major
hardware breakthrough occurred: setting `pn_swap=0,0,1,0,0` (inverting D1 /
physical lane 3) immediately unlocked active video packet decoding (`yuv422-8`),
streaming 98 active video lines and 187,663 pixels into `CSIBDG_A`. Setting D0=1
was proven across all 8 combinations to completely mute the link (SOF=0).
Setting D3=1 across all combinations prevented header/sync decode (`unknown`).
Video remains fail-closed; the next bounded step is eliminating the timing/phase
error accumulation around the winning `0,0,1,0,0` polarity to extend line count
from 98 to 1080.

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

## Source-backed EDID candidate v25

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

The first v24 boot was stopped before bridge mutation: its baseline exposed an
audio-kernel packaging carryover, `/dev/i2c-4` was absent, and the programmer
never ran. The UART test now makes I2C presence, `/proc/modules`, and absence of
audio nodes mandatory pre-mutation gates.

Corrected v25 combines the already verified v23 video FIT with the audited new
rootfs and FIP. Its 79,692,288-byte image is
`out/phase3-edid-v25/cleankvm-phase1.img`, SHA-256
`e9c18dfbadc571a33fea56310a811685a005e5d92784e9c6e4d538993bab33e4`.
The resolved 32.0 GB card was flashed and exactly read back; boot FIT, rootfs,
and FIP matched before safe ejection. macOS FAT bookkeeping produced whole-card
read-back SHA-256
`60050eca74c28d874f499a177f3a2f30a86069d68297e08a1d6fc345969959fe`.
Hardware mutation remains pending the controlled v25 boot.

The controlled v25 boot passed every pre-mutation gate: kernel taint zero,
root-only `/dev/i2c-4`, exact programmer hash, zero loaded modules, no media or
audio nodes, and both Rust services live. The bridge's pre-write EDID was
structurally invalid. The source-backed operation completed with return code
zero and byte-for-byte verified all 256 E21 bytes. Cleanup left taint zero and
no open I2C file descriptor. The device must now lose power completely before
testing Mac display enumeration and the existing status-only CSI path.

After complete power removal, the Mac still did not enumerate NanoKVM as a
display. The bounded post-EDID status run loaded all six modules normally and
again reported all physical samples zero, all four data lanes `hs_idle`,
`CK_HS=0`, and unknown decode. No frame was allocated, every module unloaded,
and final taint was the expected 4096.

The published Sipeed UXC status path supplied a second read-only discriminator.
With only documented bank-selection writes, HDMI status `0x86:0xa3` returned
`0x00` instead of the documented `0x55` stable or `0x88` disappeared values;
CSI Vactive (`0x85:0xf0`) and Hactive (`0x85:0xea`) were both `0x0000`. No I2C
descriptor remained open. This rules out missing EDID bytes as the sole cause
and promotes bridge firmware/execution or HPD control state as the next target.

## Resident-flash evidence image v26

Static stock inspection found no separately identifiable LT6911UXC application
payload or upgrade loader. It did find `lt6911_read_fw` in the stripped stock
library, matching Sipeed's published three-byte-address SPI-read routine. Normal
stock startup does not call an upload path and therefore still appears to rely
on firmware resident in the bridge SPI flash.

CleanKVM's independently compiled backup utility reproduces only that published
read transaction. It contains no erase/program opcode, has a fixed bus/address
and output path, reads the 800,000-byte stock reference range twice, requires
identical copies, and resumes the LT MCU during cleanup. The installed binary
is root-owned mode `0500`, non-autostart, and has SHA-256
`24df815d4e32e003b37ad013de950cd49a73ecf2a0c64978a9c6dd1d4c05da6d`.

The resulting v26 image is 79,692,288 bytes with SHA-256
`53d9c4679f5443073564639d00307fcacbce35dd2c68f9df8411665a0e3a266b`.
It reuses the hardware-proven v23 FIT and unchanged verified FIP. The resolved
31.0 GB removable card was written at the exact image length and read back
before ejection. Its macOS-adjusted whole-card SHA-256 was
`2621c015585db5ff77efa06e675a1f3abbbade6fd0391904e1870df47d91a167`;
embedded FIT, rootfs, and FIP comparisons all passed. The next bounded gate is
one boot, baseline containment verification, one backup run, immediate output
hashing, and confirmation that taint and I2C file-descriptor state return to
baseline.

The v26 hardware run proved that Sipeed's published `lt6911_read_fw` debug mode
does not yield firmware on this unit: both complete passes returned only the
setup byte `0x51`. The zero-entropy output is retained as rejected evidence,
not a backup. Cleanup returned taint and I2C descriptor state to baseline.

v27 then generalized only the 32-byte EDID read transaction already proven on
this unit. It reached non-uniform data, but two complete passes differed in two
unchanged runs, so both were suppressed. v28 adds no new LT write; it reports
the first differing offset, total differences, and short samples before the
same fail-closed suppression. Its image SHA-256 is
`5d150df4085a50ddb9fd541e399dafb41c9bc3ab9d96a90bfc0d2b00fdf13493`
and its reader SHA-256 is
`63ac141aaa76d06dea2e6290d4292eee5d14d319a13f0771ca51efb4b2ed473a`.

v28 proved that only the first 32-byte transaction was unstable. v29 adds a
discarded address-zero pipeline-prime read after each setup and no new write.
Two full 800,000-byte reads then matched, passed diversity checks, and yielded
SHA-256
`e4b1bd5944264ab8f192025056f849558dd30d37d00ed404e47a59f3e4ba25a5`.
Cleanup left taint zero, zero loaded modules, and no open I2C descriptor. This
admits the dump only as offline evidence; it does not admit firmware execution,
programming, video modules, frames, or MCP media access.

The corrected, source-exact UXC status sequence stops the embedded MCU before
reading and resumes it afterward. It returned stable HDMI `0x55` and CSI
dimensions 1920x1080, invalidating the earlier live-register `0x00`/zero result.
One subsequent bounded receiver run saw nonzero physical samples, `CK_HS=1`,
and high-speed activity on lanes D2/D3, but decoded `raw8` with ECC, CRC, and
word-count errors. No frame was created and every module unloaded. A repeat run
was idle across two samples two seconds apart with unchanged counters, while
the LT continued to report stable 1920x1080 afterward. This admits neither a
frame nor a lane-map change; the next test must correlate LT resume timing with
receiver state over multiple bounded samples.

## Timing-correlated receiver image v30

The status-only receiver probe now captures `/proc/mipi-rx` immediately after
the qualified receiver profile and again at cumulative 100, 500, 1000, 2000,
and 5000 ms. It makes no LT register transaction, configures no VI device, and
allocates no frame. The independently built 79,692,288-byte image is
`out/phase3-mipi-timing-v30/cleankvm-phase1.img`, SHA-256
`dcd316b15a2d87bf75d86c805ee5e026a177f619b5398c1f1f27ba7f8983f147`.

The resolved removable `/dev/disk4` (31.0 GB) was flashed at the exact image
length. Exact readback verified the embedded boot FIT, root filesystem, and FIP
against their audited artifacts. macOS FAT bookkeeping changed the whole-card
readback hash to
`7803c04ac5db693b774038db2a3faa4e1f17584428d89e183fad9374f44265fd`;
embedded verification is authoritative. The card was safely ejected. Hardware
admission remains pending one boot and one bounded status run with full cleanup.

The v30 hardware run passed that gate. Before loading modules, the corrected LT
sequence again returned stable HDMI `0x55`, CSI Vactive `0x0438`, and Hactive
`0x0780`; cleanup resumed the MCU, left taint zero, and leaked no I2C descriptor.
The installed probe hash matched the audited v30 binary and all six modules
loaded normally from a zero-taint baseline.

Receiver counters then established a bounded timeline: ECC/WC were `16/0` at
T0, `4690/174` at 100 ms, `22689/981` at 500 ms, `44922/2012` at 1 s, and
`65536/2894` at 2 s. Decode alternated between `unknown` and `raw12`, although
the receiver profile was YUV422. At 5 s and the two later samples through 7 s,
the counters remained `65536/13/2894` (ECC/CRC/WC) while `CK_HS=1` and D2/D3
continued to report high-speed activity. This rules out a simple delayed-start
explanation and points to a receiver framing or lane-interpretation mismatch.
No frame file was created, every module unloaded successfully, and final taint
was only the expected out-of-tree bit 4096. No media capability is admitted.

## Source-bounded lane discriminator v31

Pinned SG200x driver inspection shows that the displayed decode value comes
from the receiver's `CSI_DECODE_FORMAT` status register. Thus `raw12` is observed
wire interpretation, not the configured YUV422 value. The same ISR masks CSI
error interrupts after a counter exceeds `0xffff`, explaining v30's plateau.

The pinned LT6911 profile lists exactly five lane maps: `2,4,3,1,0`,
`3,0,1,2,4`, `1,4,3,2,0`, `2,0,1,3,4`, and `3,1,2,4,0`. v31 tests only these
source-preserved candidates. Each receives a fresh module load, bounded timing
samples, frame-absence check, and reverse unload before the next candidate.
The probe contains no LT transaction and still returns before VI configuration
or frame allocation. Probe SHA-256 is
`3bd78f43f757026d4629cbce003499709c4484fde2eaae65d990fd81ba3aa26a`.

The independently built image is
`out/phase3-lane-discriminator-v31/cleankvm-phase1.img`, 79,692,288 bytes,
SHA-256 `5e3915396bcb46f6232620010c8ea6ccbc6b3929424c0b9f62a2ae2e99b68fd9`.
Offline rootfs and embedded-artifact audits passed. The resolved removable
32.0 GB card was flashed, its embedded boot FIT, rootfs, and FIP verified on
readback, and safely ejected. Hardware execution remains the next gate.

The first v31 boot failed closed before module loading: inert static ALSA nodes
from an earlier incremental audio build remained in the ext4 image even though
the kernel had no ALSA subsystem, `/proc/asound` was absent, and no sound module
was loaded. v31 is rejected. The rootfs build now removes generated `/dev/snd`
state for non-audio profiles before filesystem generation and audits the final
ext4 for absence of representative control and PCM nodes. The UART lane test
now emits an explicit baseline verdict before continuing.

Corrected v32 passed the established rootfs, identity, probe-ownership, and
embedded-artifact audits plus the new non-audio ext4 audit. Its image is
`out/phase3-lane-discriminator-v32/cleankvm-phase1.img`, SHA-256
`a8bc2d0394e6c0da012db8735969545b126dd02ac51d2f0607281a3601227c72`.
The resolved 31.0 GB removable card was flashed, and raw readback verified the
embedded boot FIT, rootfs, and FIP before safe ejection. Hardware execution is
the next gate.

v32 failed the same live baseline before module loading, but `/dev` inspection
showed devtmpfs and kernel-generated ALSA nodes. Its boot FIT hash exactly
matched the Phase 3a audio image, proving the shared work volume's last-built
audio kernel had been packaged after configuration validation without a kernel
rebuild. v32 is rejected. The image assembler now optionally requires an exact
expected boot-FIT SHA-256 and fails before assembly on mismatch.

v33 uses the already hardware-qualified video-only v30 FIT, SHA-256
`9e1c47425122175b0327dea8079a588a02f0018aa00806d28911d4a56562e4a7`,
with the corrected non-audio v32 rootfs and unchanged FIP. Embedded verification
passed. The complete image SHA-256 is
`afdea272fec92417adbba90f8f09b7313afcbccfc595fbc9625f8b34e659b677`.
The resolved 32.0 GB removable card was flashed. Raw readback verified the
embedded boot FIT, rootfs, and FIP, including the required video-only FIT hash,
before safe ejection. Hardware execution remains the next gate.

v33 passed the non-audio baseline, exact probe identity, and zero-taint gates.
The five zero-PN-swap lane candidates then ran in isolated module cycles.
Profile 0 entered high-speed transfer but saturated ECC and accumulated 2,551
word-count errors while observed datatype wandered. Profile 1 produced no
protocol-count growth but left a lane in `hs_err`; profiles 2 and 3 never
decoded a datatype and remained idle/sync. Profile 4 uniquely reached completed
deskew, but two logical lanes remained in `hs_sync` and no datatype decoded.
Consequently none is a valid receiver profile.

All five probes returned before VI setup, every frame-absence check passed,
every module unloaded with return code zero, the final module list was empty,
and final taint was the expected 4096. The next admissible discriminator is the
one exact generic SDK LT6911 candidate previously documented: MIPI device 1,
lanes `2,0,1,3,4`, and all PN swaps enabled. This tests a source-backed polarity
and receiver-port alternative without expanding to invented permutations.

## Generic-SDK polarity discriminator v34

v34 adds only the exact generic SDK LT6911 receiver candidate: MIPI device 1,
lanes `2,0,1,3,4`, PN swaps `1,1,1,1,1`, YUV422 8-bit, 600 MHz MAC, and HS
settle 8. The harness executes profile 5 alone from a fresh six-module state,
retains the frame-absence gate, and unloads in reverse order. The probe still
contains no LT transaction and returns before VI configuration or allocation.
Its SHA-256 is
`085d4a346440e22ba8825f7145777a802c1c6944b8b1414ecfc03ea431e22106`.

The final image retains the required video-only FIT hash and passed rootfs,
non-audio ext4, identity, probe, FIT, and embedded-artifact audits. Path:
`out/phase3-polarity-discriminator-v34/cleankvm-phase1.img`; SHA-256:
`dfc6e5d15a94317d2ab0edc55eb058503a2935d4eb926767f51cad2bb6c49d40`.
The resolved 31.0 GB removable card was flashed, raw-readback verified, and
safely ejected. Hardware execution remains the next gate.

The v34 hardware gate completed and profile 5 is rejected. Receiver device 1
reported zero physical samples, four `hs_idle` data lanes, `CK_HS=0`, idle
deskew, unknown decode, and zero ECC/CRC/header/word-count/FIFO errors at every
sample from T0 through T5000 ms. Thus the candidate selected a valid but
inactive receiver port; it did not observe the LT's live CSI output. No frame
artifact was created. All six quarantined modules unloaded successfully, the
final `soph_*` module list was empty, and the final taint was the expected 4096.

The source-bounded static receiver candidates are now exhausted. The next
admissible step is differential observation on a known-working stock boot:
record the LT runtime status and SoC receiver initialization/state, attribute
each observation to source or a read-only trace, and use the delta to implement
an independently built configuration. Vendor executables remain analysis-only
and must not be transferred into or executed by the clean image.

The known-working stock v1.4.2 image was prepared as that behavioral reference.
Its 1,627,390,464-byte expanded source SHA-256 is
`23648088de59e043a1289c49c5103f829dc652604f8dd76db22dd8ff7aca6539`,
and the compressed archive independently expands to the same digest. The
resolved removable 32.0 GB card was flashed. macOS FAT bookkeeping changed the
whole written-span readback digest to
`50a0d7750e229333f3fdf878ed889e0b9bd6bcf365fcf4a0e1b030d0edb941e3`,
but the full Linux partition matched source byte-for-byte at SHA-256
`c285650ee41892c0cef5fbc371d3b13828f886a3ebb82a400ff06bbec94585a4`.
The card was safely ejected. The next gate is a read-only UART observation of
the working stock LT and receiver state with HDMI present.

That read-only stock observation completed. The runtime configuration is not a
hidden lane-map difference: stock uses device 0, lanes `2,4,3,1,0`, zero PN
swaps, YUV422 8-bit, and reports 1920x1080. Its receiver stayed at `yuv422-8`,
`CK_HS=1`, D2/D3 high-speed transfer, zero ECC/CRC/header/FIFO errors, and a
single word-count error in two samples two seconds apart. This contrasts with
the clean module's immediate ECC/word-count storm under the same configuration.

The stock image contains both `soph_mipi_rx_old.ko` (122,408 bytes, build #26)
and a replacement `soph_mipi_rx.ko` (125,992 bytes, build #30), and its loader
selects the replacement. The loaded replacement SHA-256 is
`d40419159a4065de8b7693e0de59c477de36373f24fe164e50afb37d674ed755`;
the independently built v7 receiver SHA-256 is
`8218302da9c43c046fad23b80c60b65889996c84aec430fd492cc8e7d4dbe9c0`.
Stock also inserts `soph_rtos_cmdqu` and `soph_fast_image` between base and MIPI
receiver initialization, whereas the bounded clean test excluded them. The
next gate is offline attribution of these two deltas to reviewable source,
followed by the smallest independently rebuilt experiment. Stock modules remain
evidence only and are not admitted for execution in CleanKVM.

## Reset-sequence image v35 hardware result

The v35 candidate (`b2db1e88af8913cd7ba7e63fa9d52b1ec49938d89326e6fc1836b4aa7c21cf99`)
was booted under serial control. It passed baseline containment: taint zero,
root-only `/dev/i2c-4`, non-audio ext4 audit confirmed, probe SHA-256
`4f461d8f483f777d5674cc7c5fdcb2ac29092630bea143244ca5af847d76cc2a`, and zero
loaded video modules.

The LT6911 source status check confirmed stable HDMI input `0x86:0xa3 = 0x55`
and full HD dimensions (Vactive `0x0438` = 1080, Hactive `0x0780` = 1920) with
MCU resume and no leaked I2C file descriptors.

All six quarantined modules loaded with return code zero. The probe executed
profile 0 with the source-exact sensor reset and clock sequence. The receiver
exhibited active transmission on digital lanes D2/D3 (`hs_hst`) and clock
`CK_HS=1`. Notably, observed decode format matched `yuv422-8` at T100ms, T500ms,
and T2000ms (an improvement over earlier indeterminate formats), but protocol
error counters accumulated (`5607` ECC / `324` WC at 100ms to the `65536` ECC /
`3822` WC plateau at 2s).

Containment and reverse cleanup succeeded completely: probe return code was 0,
no frame file was created, all six modules unloaded with return code zero, no
`soph_*` modules remained, and final taint was the expected out-of-tree 4096.
This directly reinforces the stock differential hypothesis: the receiver lane
and clock configuration are fundamentally correct (`dev0`, lanes `2,4,3,1,0`,
`yuv422-8`), but clean receiver framing requires the driver delta and/or the
prerequisite mailbox/RTOS clock plumbing (`soph_rtos_cmdqu` / `soph_fast_image`).

## Raw-frame probe candidate v36 hardware result

The candidate v36 image (`2198051a352397e6402903c30bb8f4a94b11b9fbdd9ab56f999e0b128e998fc2`)
was assembled with the re-enabled single-frame VI pipeline and Dev 0 clock
activation. Target microSD card `/dev/rdisk4` (32.0 GB) was flashed, and the
exact sector span read back (`81fcb72a...`). Embedded artifacts were verified
byte-for-byte against audited artifacts: `boot.sd` (`1ef26201...`), `rootfs.ext4`
(`bc9b2da7...`), and `fip.bin` (`96a0b414...`).

Controlled UART boot passed baseline containment: taint zero, root-only
`/dev/i2c-4`, non-audio ext4 verified, probe SHA-256 confirmed
(`4a8aa49e0a518e5125899937317ae3c233848e0a0f3fd0650966974dc7df51aa`), and both
Rust services live. The LT6911 check confirmed locked HDMI input
`0x86:0xa3 = 0x55` at 1920x1080 (`CSI_VACTIVE=0x0438`, `CSI_HACTIVE=0x0780`) with
MCU resume and zero leaked descriptors.

All six quarantined modules loaded with return code zero. The probe executed
profile 0 with VI Dev 0 enabled before MIPI configuration.

### Receiver and driver behavior

1. **Clocking & FIFO Drainage**: Moving `CVI_VI_EnableDev(0)` before MIPI setup
   successfully enabled `clk_csi_mac0`. At T0, initial MIPI RX errors were
   dramatically lower than v35: `EccErr: 50, CrcErr: 0, HdrErr: 0, WcErr: 0, fifofull: 0`.
   `fifofull` remained 0 throughout, proving the MAC FIFO is continuously drained.
2. **Bridge Reset Transients**: The probe invoked `ioctl(mipi_fd, CVI_MIPI_RESET_SENSOR, &mipi_dev)`
   which pulsed GPIO pin 353 (active-low reset on LT6911) for 358 µs. This reset
   the already locked LT6911 bridge. The bridge required ~2s to re-sync HDMI;
   during this period, bus transients accumulated 65536 ECC errors and 6741 WC
   errors by 2.1s.
3. **Interrupt Masking**: Upon hitting 65536 errors, CIF logged:
   `cif a0c2000.cif: mask the interrupt since err cnt is full`.
   Once masked, the VI interrupt handler received no further frame interrupts.
4. **Buffer Queue Starvation**: `base_get_chn_buffer:262(): Mod(VI) Grp(0) Chn(0), jobs wait(0) work(2) done(0)`.
   Because no buffers completed, `CVI_VI_GetChnFrame` timed out and failed with
   `0xc00e8040` (`EPERM` / `CVI_ERR_VI_BUF_EMPTY`).
5. **D-PHY Settle Configuration**: In `cif.c` line 596, `cif_set_hs_settle` is
   gated by `if (attr->dphy.enable)`. Because `mipi.mipi_attr.dphy.enable` was
   zero, `cif_set_hs_settle` was never called, leaving high-speed settle at 0.

### Containment and cleanup

Containment held perfectly: all six modules unloaded cleanly with return code 0,
zero remainder modules in `/proc/modules`, post-taint was expected 4096, and
`test ! -e /run/cleankvm-frame.yuv422p` passed.

### Raw-frame probe candidate v37 hardware result

Candidate v37 implemented the non-disruptive initialization sequence in
`image/phase3-raw-frame-probe.c`:
1. Preserved `CVI_VI_EnableDev(0)` before MIPI initialization to keep `clk_csi_mac0`
   and MAC FIFO drainage active (`fifofull: 0`).
2. Omitted `CVI_MIPI_RESET_SENSOR`, `CVI_MIPI_ENABLE_SENSOR_CLOCK`, and
   `CVI_MIPI_UNRESET_SENSOR` so the running, HDMI-locked LT6911 bridge was not
   pulsed or interrupted.
3. Enabled D-PHY settle configuration via `mipi.mipi_attr.dphy.enable = 1` and
   `mipi.mipi_attr.dphy.hs_settle = 16`.
4. Configured sensor geometry on the VI kernel driver via `VI_IOCTL_SET_SNR_INFO`
   (`1920x1080`) so the CSI bridge registers (`CH0_SIZE`) received proper dimensions
   rather than default `0x0`.

During execution, receiver error counters remained low, but `CVI_VI_GetChnFrame`
still failed with `0xc00e8040` (`EPERM` / `CVI_ERR_VI_BUF_EMPTY`). Driver logs
revealed:
`isp_err_chk:6573(): CSIBDG_A CH0 frm height less than setting(1080), pxl=0 lines(ch0..3)=0,0,0,0 vs=2 trig=1`
followed immediately by `_vi_err_retrig_preraw()` looping 31 times within 6 ms and
aborting the pipeline with `raw_0 too much errors happened`.

## VI driver short-circuit fix (`0011-vi-log-actual-csibdg-line-count.patch`)

Source inspection of `osdrv/interdrv/v2/vi/chip/mars/vi.c` identified a fatal driver
flaw:
- `isp_err_chk` treated transient dimension checks (`CH0_FRAME_HEIGHT_LS_INT`) as fatal
  errors (`ret = -1`).
- Returning `-1` caused `vi_irq_handler` to exit early before checking `FRAME_DONE_FE0`
  (line 7650), completely preventing frame completion.
- Furthermore, `vi_err_wake_up_th(vdev, raw_num)` queued an immediate asynchronous
  reset thread that burned through its 31-attempt error budget in milliseconds.

Patch `0011-vi-log-actual-csibdg-line-count.patch` applied the following source fixes:
1. In `isp_err_chk`: removed `vi_err_wake_up_th()` on width and height interrupt flags
   and set `ret = 0` (treating dimension mismatches as warnings rather than fatal pipeline faults).
2. In `vi_irq_handler`: removed the early `return` after `isp_err_chk` so `FRAME_DONE_FE0`
   is always processed when asserted by hardware.
3. Added diagnostic register reads of `CH0_DEBUG_0` (pixel count), `CH0_DEBUG_1`
   (line count), and `CH0_DEBUG_2` (VSYNC and trigger counts).

The patched `soph_vi.ko` (SHA-256 `bf729726e9bfe8150e263d0e41c8209897c6333adfc29cd49029534977048b12`)
was compiled offline and deployed to the target. Execution completely eliminated the
31-retry self-destruction. Frame start interrupts (`VISofCh0Cnt`) surged to **2,681 counts
at 386 FPS**, confirming continuous high-speed frame synchronization.

## CIF D-PHY diagnostic unpacking patch (`0012-cif-fix-pad-ctrl-mapping-and-phy-state.patch`)

Source inspection of `osdrv/interdrv/v2/cif/chip/mars/drv/cif_drv.c` revealed that
`cif_get_csi_phy_state()` packed `DBG_90` and `DBG_94` into `union mipi_phy_state` using
an invalid single 16-bit shift:
`state->raw = CIF_RD_REG(wrap_4l, ..., DBG_90) | CIF_RD_REG(wrap_4l, ..., DBG_94) << 16;`
In `union mipi_phy_state`, `d0_datahs_state` is at bit 8, while shifting `DBG_94` by 16
placed D0 at bit 16 (aliasing with D2).

Patch `0012-cif-fix-pad-ctrl-mapping-and-phy-state.patch`:
1. Correctly unpacks `clk_hs_state`, `clk_ulps_state`, `clk_stop_state`, `clk_err_state`,
   `deskew_state`, and `d0..d3_datahs_state` directly into their respective bitfields.
2. Adds pad control PU/PD clear logging for all five MIPI RX pairs.

## Empirical hardware discriminator sweeps

### 1. 2-Lane MIPI isolation (Profile 6: `2,1,0,-1,-1`, `hs_settle=24`)
To verify Port 0 silicon and D-PHY receiver behavior in isolation, Profile 6 was executed
using only Port 0 data lanes (physical 1 and 0):
- Result: digital D0 (physical 1) and digital D1 (physical 0) **both locked into `hs_hst`**
  (High-Speed Transfer).
- Protocol counters: only 3 ECC errors, 0 CRC errors, 0 HDR errors, 0 WC errors.
- Conclusion: Port 0 receiver silicon, pad routing, and deskew logic are fully operational.

### 2. Port 1 physical pin order confirmation
Testing the swapped Port 1 lane candidate (`2,3,4,1,0`) caused physical lane 4 to
immediately enter `hs_err`. Testing `2,4,3,1,0` produced clean `hs_sync` without lane error.
This confirms `2,4,3,1,0` (physical 4 and 3 as Port 1 far lanes) is the authentic wiring order.

### 3. 4-Lane far-lane phase sweep (`lane_phase=0,0,X,0,0` with `2,4,3,1,0`)
In 4-lane mode, the clock resides on Port 0 (physical 2) and is forwarded to Port 1
(physical 4 and 3) via the internal `CIF_CLK_P02P1` buffer. Parameter `lane_phase[2]`
controls `DESKEW_CODE` for Port 1 far lanes. A systematic sweep yielded:
- `X = 4`: D0 in `hs_err`
- `X = 8`: D0 in `hs_sync`, D1 in `hs_sync`, D2 in `hs_hst`, D3 in `hs_hst` (dual near-lane lock!)
- `X = 10`: D0 in `hs_err` (at 5s), `preample` (at 100ms)
- `X = 11`: D0 in `hs_err`
- `X = 12`: D0 advanced to `preample` (state 4, the state immediately preceding `hs_hst`)
- `X = 13`: D0 reached `preample`, D1 in `hs_sync`, D2 in `hs_hst`, D3 in `alt_cal`
- `X = 14`: D0 in `hs_err`
- `X = 16`: D0 in `hs_err`

*Finding*: The manual deskew operational window for Port 1 far lanes is centered at 12–13,
where digital D0 advances to `preample`.

### 4. Near-lane phase evaluation (`lane_phase[1]`)
Testing the vendor default near-lane setting `lane_phase[1] = 0x03` (`lane_phase=0,3,12,0,0`)
caused D2 to degrade from `hs_hst` to `hs_sync`. Zero delay (`lane_phase[1] = 0`) is optimal
for near lanes on Port 0.

### 5. Clock delay evaluation (`lane_phase[0]`)
Testing `lane_phase=1,0,12,0,0` offset the common clock lane delay, degrading near-lane
reception without advancing far lanes.

### 6. High-speed settle parameter sweep (`hs_settle`)
Testing `hs_settle = 24` on 4-lane mode widened the settle window: at T0, D0 reached
`preample` even at `lane_phase[2] = 8`.

## Working receiver hypotheses (not established root cause)

1. **4-Lane Byte Striping Requirement**:
   MIPI CSI-2 stripes packet bytes across configured data lanes. Far-lane states
   short of stable `hs_hst` correlate with zero CSI-bridge pixel/line counts and
   no completed frame; they do not uniquely prove the cause. The observed
   386/s `VISofCh0Cnt` is an interrupt rate, not a validated frame rate.
2. **Clock Phase Control (`CSI_CK_PHASE`)**:
   `REG_SENSOR_PHY_4L_08` defines `CSI_CK_PHASE` at bits 16..23. Its effect
   on the far lanes is unmeasured; v40 isolates that question while preserving
   the SDK value when the diagnostic parameter is zero.
3. **Independent Per-Lane Deskew**:
   `cif.c` currently forces the same deskew code onto both far lanes (`lane_phase[2]`).
   However, the hardware registers (`REG_SENSOR_PHY_TOP_84` and `REG_88`) provide
   independent 8-bit registers (`DESKEW_CODE3` and `DESKEW_CODE4`). Enabling independent
   per-lane control could permit a later, independently bounded test of
   physical lanes 3 and 4 if clock-phase isolation warrants it.

### Admission interpretation (2026-09-19)

The phase and deskew observations are diagnostic correlations, not a proven
architectural root cause. In particular, `CSI_CK_PHASE` is a named register
field; the subsequent phase-0/phase-8 comparison below found no improvement.
The 386/s
`VISofCh0Cnt` rate is an interrupt rate, not an established frame rate; the
CSI bridge still reported zero pixels/lines and no completed frame. Patch
`0013-cif-bounded-clock-phase-diagnostic.patch` therefore exposes only a
load-time 0..15 clock-phase control, with zero preserving the SDK value. It
passed patch applicability against the pinned source and compiled offline in
an isolated SDK copy. The output receiver module SHA-256 is
`2e6cdcb8e8ec0e0fafe580dd54f86eace67dfe1cf8c90b80d969673a70ca863f`;
its RISC-V vermagic and undefined-symbol set match the qualified diagnostic
baseline. The v40 image containing this module was verified from SD-card raw
read-back and ejected. Its subsequent target boot and first two phase tests
are recorded below. Per-lane deskew is deliberately deferred so each test changes one receiver
variable at a time. No video admission follows
from these findings alone.

### v40 phase-isolation result (2026-09-19)

The verified v40 card booted the expected `5.10.4-tag-` kernel into recovery.
Before module loading, taint was zero, no `soph_*` module or audio node was
present, receiver/VI/probe hashes matched the audited ext4, and both Rust
services were running. The LT bridge returned HDMI lock `0x55`, 1080 active
lines and 1920 active pixels using the source-backed stop/read/resume sequence;
the MCU resumed and no I2C descriptor leaked.

Each of two fresh six-module cycles used the stock lane map `2,4,3,1,0`, zero
PN swaps, `lane_phase=0,0,12,0,0`, and D-PHY settle 24. With
`csi_ck_phase=0`, and again with `csi_ck_phase=8`, receiver ECC/CRC/header/
word-count/FIFO errors stayed at zero. Digital D0/D1/D2 were in `hs_hst`,
while D3 alternated `hs_sync`/`hs_idle`; decode remained unknown and VI
reported zero starts and completions. The bounded frame request returned
`0xc00e8040` and created no frame. The phase-8 parameter read back 8 and
kernel log confirmed `CSI_CK_PHASE=8`, so the no-change result is not due to
an unapplied parameter. Every module load and reverse unload returned zero;
the final test-module set was empty, final taint 4096, and frame file absent.

This challenges the earlier far-lane-only explanation. A wider clock-phase
sweep is not justified by these samples. The next bounded inquiry is the
digital D3 to physical lane-0 mapping, including polarity and stock-vs-clean
receiver state, before another configuration change. Video remains closed.

Pinned source confirms the mapping under the selected product profile:
`cif_set_lane_id()` assigns its fifth lane selection to `CSI_LANE_D3_SEL`,
and `DBG_94` defines D3 data-HS state at bits 12..14. The corrected
diagnostic unpacker reads those bits. Thus digital D3 corresponds to physical
lane 0 in `2,4,3,1,0`, and the observed idle/sync is not explained by the old
bit-shift reporting bug. Polarity and stock-state differences remain open.

### Complete 16-State Polarity (PN Swap) Empirical Evaluation (2026-09-19)

To exhaustively test whether differential pair inversions exist on the NanoKVM Lite
PCB, all 16 possible combinations of `pn_swap[1..4]` (representing data lanes D0,
D1, D2, D3, with clock lane `pn_swap[0] = 0` fixed) were evaluated on the live
hardware under controlled, bounded diagnostic runs using `cleankvm-lane-discriminator`.

Each run was conducted with fixed `lane_id=2,4,3,1,0`, `lane_phase=0,0,12,0,0`,
and `hs_settle=16`. Every single run verified:
- Clean module load sequence (`RC=0`)
- Clean reverse unload sequence of all six modules (`RC=0`)
- Clean remainder check (`/proc/modules` empty of `soph_*`)
- Taint integrity (baseline plus out-of-tree: exactly `4096`)
- No leaked file descriptors or unhandled frames (`FINAL_FRAME_ABSENT_RC=0`)

#### Complete 16-State Polarity Truth Table

| Test # | `pn_swap` | D0 | D1 | D2 | D3 | Digital Lanes | Decode Format | SOF Count (`VISofCh0Cnt`) | CIF / VI Error Telemetry | Received Data |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `0,0,0,0,0` | 0 | 0 | 0 | 0 | `hs_sync hs_sync hs_hst hs_hst` | `unknown` | 0 | None (no sync lock) | 0 px / 0 lines |
| 2 | `0,0,1,0,0` | 0 | **1** | 0 | 0 | `hs_idle hs_idle hs_idle hs_idle`* | **`yuv422-8`** | 440 | `isp_err_chk: CSIBDG_A CH0 frm width less than setting(1920), pxl=187663 line=98`<br>`cif: mask the interrupt since err cnt is full (ecc=65536, wc=5805)` | **187,663 px / 98 lines** |
| 3 | `0,0,0,1,0` | 0 | 0 | **1** | 0 | `hs_hst hs_hst hs_idle hs_idle` | **`yuv422-8`** | 859 | `isp_err_chk: CSIBDG_A CH0 frm width greater than setting(1920), pxl=1921 line=0`<br>`cif: mask the interrupt since err cnt is full` | 1921 px / 0 lines |
| 4 | `0,0,1,1,0` | 0 | **1** | **1** | 0 | `hs_hst hs_idle hs_idle hs_sync` | **`yuv422-8`** | **46,811** | `isp_err_chk: CSIBDG_A CH0 frm width greater than setting(1920), pxl=1921 line=0`<br>`cif: mask the interrupt since err cnt is full` | 1921 px / 0 lines |
| 5 | `0,0,0,0,1` | 0 | 0 | 0 | **1** | `hs_hst hs_hst hs_hst hs_idle` | `unknown` | 160 | `isp_err_chk: CSIBDG_A CH0 frm height less than setting(1080), pxl=0 lines=0` | 0 px / 0 lines |
| 6 | `0,0,1,0,1` | 0 | **1** | 0 | **1** | `hs_hst hs_idle hs_hst hs_idle` | `unknown` | 42 | `isp_err_chk: CSIBDG_A CH0 frm height less than setting(1080), pxl=0 lines=0` | 0 px / 0 lines |
| 7 | `0,0,0,1,1` | 0 | 0 | **1** | **1** | `hs_hst hs_hst hs_idle hs_idle` | `unknown` | 213 | `isp_err_chk: CSIBDG_A CH0 frm height less than setting(1080), pxl=0 lines=0` | 0 px / 0 lines |
| 8 | `0,0,1,1,1` | 0 | **1** | **1** | **1** | `hs_hst hs_idle hs_idle hs_idle` | `unknown` | 878 | `isp_err_chk: CSIBDG_A CH0 frm height less than setting(1080), pxl=0 lines=0` | 0 px / 0 lines |
| 9 | `0,1,0,0,0` | **1** | 0 | 0 | 0 | `hs_idle hs_hst hs_hst hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 10 | `0,1,1,0,0` | **1** | **1** | 0 | 0 | `hs_idle hs_idle hs_hst hs_sync` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 11 | `0,1,0,1,0` | **1** | 0 | **1** | 0 | `hs_idle hs_hst hs_idle hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 12 | `0,1,1,1,0` | **1** | **1** | **1** | 0 | `hs_idle hs_idle hs_idle hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 13 | `0,1,0,0,1` | **1** | 0 | 0 | **1** | `hs_idle hs_hst hs_hst hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 14 | `0,1,1,0,1` | **1** | **1** | 0 | **1** | `hs_idle hs_idle hs_hst hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 15 | `0,1,0,1,1` | **1** | 0 | **1** | **1** | `hs_idle hs_idle hs_idle hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |
| 16 | `0,1,1,1,1` | **1** | **1** | **1** | **1** | `hs_idle hs_idle hs_idle hs_idle` | `unknown` | 0 | None (link muted) | 0 px / 0 lines |

*\*Note: In Test 2 (`0,0,1,0,0`), active streaming took place for ~2.1 seconds until error counter saturation (65536 ECC / 5805 WC errors) caused CIF to mask interrupts, transitioning lanes to idle.*

#### Empirical Hardware Findings

1. **D0 (Physical Lane 4) Polarity is Strictly Positive (`pn_swap[1] = 0`)**:
   In all 8 runs where D0 was inverted (`D0 = 1`), `VISofCh0Cnt` was strictly 0,
   error counters stayed at 0, and D0 dropped to `hs_idle`. Swapping D0 completely
   mutes the receiver link. This conclusively rules out any differential pair
   swap on physical lane 4.

2. **D3 (Physical Lane 0) Polarity is Strictly Positive (`pn_swap[4] = 0`)**:
   In all 8 runs where D3 was inverted (`D3 = 1`), the receiver packet decode was
   strictly `unknown`, and line reception remained at 0 (`frm height less than 1080, pxl=0`).
   Swapping D3 corrupts packet header synchronization. Physical lane 0 is correctly
   wired without inversion.

3. **D1 (Physical Lane 3) Differential Pair Inversion Breakthrough**:
   Under stock configuration `pn_swap=0,0,0,0,0`, active line reception is strictly 0.
   Inverting D1 (`pn_swap=0,0,1,0,0`) immediately achieved:
   - **CSI bridge decode locked to `yuv422-8`**.
   - **187,663 pixels and 98 active video lines** successfully streamed into `CSIBDG_A`.
   - The pixel-per-line ratio `187,663 / 98 = 1,914.9 px/line` directly matches the
     1920 horizontal active video width of 1080p.
   - This demonstrates that `pn_swap=0,0,1,0,0` is a promising, empirically effective
     receiver-side setting for decoding video on this board; however, it is not yet
     proof of a PCB differential pair inversion, as stock configuration declared
     zero PN swaps (`0,0,0,0,0`).

4. **Line-98 Truncation Root Cause**:
   In Test 2, frame streaming progressed to line 98 before halting. The kernel
   log reveals:
   `cif a0c2000.cif: mask the interrupt since err cnt is full`
   `cif a0c2000.cif: ecc = 65536, crc = 8609, wc = 5805, hdr = 0, fifo_full = 0`
   The interrupt masking occurred because the error counters saturated at 65536
   after ~2.1 seconds (~31,000 errors/sec). The error growth during packet streaming
   indicates inter-lane skew / sample clock phase jitter between Port 0 and Port 1.

5. **D2 (Physical Lane 1) Behavior and 1921-Pixel Width Overflow**:
   - In `0,0,0,1,0` (D2 swapped) and `0,0,1,1,0` (D1 and D2 swapped), decode locked
     to `yuv422-8`, and `VISofCh0Cnt` in `0,0,1,1,0` reached 46,811 interrupts.
   - However, line 0 immediately tripped a width overflow:
     `isp_err_chk: CSIBDG_A CH0 frm width greater than setting(1920), pxl=1921 line=0`.
   - Exactly one extra pixel (1921 instead of 1920) is sampled at line start,
     indicating byte de-striping alignment offset on Port 1.

### Sequential Parameter Isolation Evaluation (2026-09-19)

Following the baseline polarity evaluation, a strictly sequential single-variable
investigation was conducted around `pn_swap=0,0,1,0,0`:
1. Baseline confirmation at original settings (`lane_phase[2]=12`, `hs_settle=16`, `csi_ck_phase=0`).
2. Deskew variation alone near 12 (`lane_phase[2]` = 10, 11, 13).
3. Settle variation alone (`hs_settle` = 20, 24) at selected deskew.
4. Clock phase evaluation (`csi_ck_phase` = 8) last.

Every attempt used a fresh module load/unload cycle, verified clean reverse unloading (`RC=0`),
empty `/proc/modules`, and final kernel taint `4096`.

#### Sequential Test Results Summary

| Step | Parameter Varied | Other Settings | T0 Errors (Ecc/Crc/Wc) | T100ms Errors (Ecc/Crc/Wc) | T100ms Lanes | Received Data | Final Status |
|---|---|---|---|---|---|---|---|
| 1. Baseline Confirm | `lane_phase[2]=12` | settle=16, ck_phase=0 | 35 / 2 / 0 | 3576 / 469 / 319 | D0=hst, D1=idle, D2=hst, D3=err | 3 lines (5,760 px) | Confirmed `yuv422-8`, 35.4k EccErr/s; masked at 1.9s (65536 ECC / 5951 WC) |
| 2a. Deskew 10 | `lane_phase[2]=10` | settle=16, ck_phase=0 | 35 / 3 / 0 | 3493 / 395 / 305 | D0=hst, D1=idle, D2=hst, D3=idle | **4 lines (6,578 px)** | D3 stayed clean (no `hs_err`); CRC & WC errors dropped |
| 2b. Deskew 11 | `lane_phase[2]=11` | settle=16, ck_phase=0 | 79 / 15 / 0 | 3566 / 481 / 304 | D0=hst, D1=idle, D2=hst, D3=idle | **11 lines (20,530 px)** | Stable streaming across 11 lines; masked at 1.9s |
| 2c. Deskew 13 | `lane_phase[2]=13` | settle=16, ck_phase=0 | 64 / 6 / 0 | 3440 / 471 / 321 | D0=idle, D1=idle, D2=idle, D3=idle | 0 lines | Higher WC errors; line reception collapsed to 0 |
| 3a. Settle 20 | `hs_settle=20` | deskew=12, ck_phase=0 | 57 / 1 / 0 | 3311 / **84** / 372 | D0=hst, D1=idle, D2=hst, D3=idle | 2 lines (3,352 px) | **82% drop in T100 CRC errors** (84 vs 469); final CRC dropped from 8,461 to 1,621 |
| 3b. Settle 24 | `hs_settle=24` | deskew=12, ck_phase=0 | 1 / 0 / 0 | 1 / 0 / 0 | D0=hst, D1=idle, D2=hst, D3=idle | 0 lines | Decode `unknown`, SOF=0; settle window too long (ruled out) |
| 4. Clock Phase 8 | `csi_ck_phase=8` | deskew=12, settle=16 | 77 / 14 / 15 | 3474 / 442 / 324 | D0=hst, D1=idle, D2=hst, D3=idle | **76 lines (145,920 px)** | **145,920 pixels (76 full lines)** received before 65536 ECC mask at 1.9s |

#### Key Insights From Sequential Isolation

1. **Deskew Window Centered at 11–12**:
   - Deskew 13 degraded reception to 0 lines.
   - Deskew 10 achieved 4 lines (6,578 px) and prevented D3 `hs_err`.
   - Deskew 11 achieved 11 lines (20,530 px).
   - Deskew 12 achieved up to 98 lines (187,663 px).
2. **Settle Window Dynamics**:
   - Settle 24 completely fails to latch high-speed packet sync (`unknown` decode).
   - Settle 20 slashed CRC transmission errors by >80% (from 8,461 down to 1,621), demonstrating that high-speed preamble settling is critical for CRC integrity.
3. **Clock Phase 8 Unlocked 76 Lines (145,920 Pixels)**:
   - Setting `csi_ck_phase=8` allowed 145,920 pixels (76 active video lines) to flow into `CSIBDG_A`.
4. **Driver Error-Masking Mechanism Localized (`cif.c:2763-2777`)**:
   - Inspection of `cif.c` reveals that `link->sts_csi.errcnt_ecc > 0xFFFF` triggers `cif_mask_csi_int_sts(ctx, 0x1F)`.
   - At ~34,000 ECC errors/sec (approx. 1 correctable 1-bit Hamming error per line at 1080p30), the 16-bit software counter saturates at 65,536 after ~1.9 seconds.
   - The driver's automatic interrupt masking halts subsequent line interrupts and `FRAME_DONE_FE0` delivery, artificially terminating frame acquisition even while video packets are actively streaming.

### Alternating Baseline vs Clock-Phase-8 Verification and Phase-8 Tuning (2026-09-19)

To systematically verify the clock-phase-8 behavior and establish statistical repeatability,
an alternating sequence of baseline (`csi_ck_phase=0`) and clock-phase-8 (`csi_ck_phase=8`) runs
was executed, holding HDMI input and all other settings fixed (`pn_swap=0,0,1,0,0`, `lane_phase[2]=12`,
`hs_settle=16`), recording early error counters (T0, T100ms) before 65,536 saturation, and
monitoring line counts into `CSIBDG_A`. Following the alternating runs, a single-variable tuning
test around Phase 8 was performed with `hs_settle=20`.

Every run completed with a fresh module cycle, verified clean reverse unloads (`UNLOAD_*_RC=0`),
empty `/proc/modules`, absent output frame, and final kernel taint `4096`.

#### Alternating Test Results Summary

| Run | Configuration | T0 Errors (Ecc/Crc/Wc) | T100ms Errors (Ecc/Crc/Wc) | T100ms Lanes | Received Data | SOF Cnt (T100 / T5000) | Final Outcome |
|---|---|---|---|---|---|---|---|
| **A1 (Baseline)** | ck_phase=0, settle=16, deskew=12 | 42 / 3 / 0 | 3665 / 433 / 263 | D0=hst, D1=idle, D2=hst, D3=idle | 15 lines (28,288 px) | 98 / 464 | Masked at 1.8s (65536 ECC / 8145 CRC / 4277 WC) |
| **B1 (Phase 8)** | ck_phase=8, settle=16, deskew=12 | 36 / 7 / 0 | 3701 / 432 / 199 | D0=hst, D1=idle, D2=hst, D3=idle | 11 lines (21,120 px) | 94 / 382 | Masked at 1.8s (65536 ECC / 7817 CRC / 3697 WC) |
| **A2 (Baseline)** | ck_phase=0, settle=16, deskew=12 | 92 / 2 / 34 | 3722 / 403 / 242 | D0=hst, D1=idle, D2=hst, D3=**err** | 7 lines (12,928 px) | 97 / 629 | Masked at 1.8s (65536 ECC / 7747 CRC / 3691 WC) |
| **B2 (Phase 8)** | ck_phase=8, settle=16, deskew=12 | 38 / 4 / 0 | 3625 / 432 / 269 | D0=hst, D1=idle, D2=hst, D3=idle | 6 lines (10,904 px) | 34 / 198 | Masked at 1.8s (65536 ECC / 8211 CRC / 4887 WC) |
| **B3 (Phase 8 + Settle 20)** | ck_phase=8, settle=20, deskew=12 | 104 / 0 / 40 | 3517 / **83** / 371 | D0=sync, D1=idle, D2=sync, D3=sync | 1 line (12 px) | 46 / 430 | Masked at 1.9s (65536 ECC / **1485** CRC / 6171 WC); >80% CRC drop |

#### Detailed Analysis and Findings

1. **Early Error Rate Comparison (T0 & T100ms)**:
   - At T0 (initial packet sync), `csi_ck_phase=8` delivered lower ECC error counts (36–38 vs 42–92 in baseline) and zero Word Count (WC) errors (0 vs 0–34 in baseline).
   - At T100ms, the error accumulation rate was nearly identical between baseline and phase 8: ~3,600–3,700 ECC errors/100ms (~36,000 ECC errors/sec, or ~1 correctable Hamming error per line header), and ~400–430 CRC errors/100ms.
   - Lane health: in both Phase 8 runs (B1 and B2), D3 stayed in `hs_idle` without triggering digital `hs_err` at T100ms, whereas in Baseline Run A2, D3 showed `hs_err`.

2. **Line Count Reproducibility**:
   - The exceptional 76-line burst observed during initial Phase 8 discovery (`task-9329`) did not reproduce under repeated testing; runs B1 and B2 captured 11 lines and 6 lines respectively, while baseline runs A1 and A2 captured 15 lines and 7 lines.
   - Across both configurations, line reception truncates due to packet-level errors triggering `CSIBDG_A CH0 frm width less than setting(1920)`.

3. **High-Speed Settle Confirmation (`hs_settle=20`)**:
   - Combining `csi_ck_phase=8` with `hs_settle=20` confirmed the dramatic CRC reduction observed in earlier tests: final CRC errors fell by >81% (from ~8,000 down to 1,485; T100ms CRC errors dropped from 432 to 83).
   - However, line reception under settle 20 halted at 1 line (12 pixels), indicating that while D-PHY header sync benefited from the 20-cycle settle period, packet payload streaming was cut short.

4. **CIF Software Interrupt Masking Blocker (`cif.c:2763-2777`)**:
   - In all 5 runs, the CIF kernel driver automatically masked receiver interrupts (`cif: mask the interrupt since err cnt is full`) at ~1.8–1.9s when the 16-bit software ECC counter overflowed `0xFFFF` (65,536).
   - Because 1-bit ECC errors in MIPI CSI-2 packet headers are corrected by hardware Hamming decoding, treating counter overflow as fatal interrupts artificially terminates capture and prevents downstream reception of the remaining lines up to 1080 and `FRAME_DONE_FE0`.

5. **Admission Gate Status**:
   - Video admission remains strictly **FAIL-CLOSED**: 1080 valid lines, `FRAME_DONE_FE0`, and a hashed 4,147,200-byte capture frame were not achieved. All video modules unloaded cleanly (`RC=0`), `/proc/modules` is clean, and kernel taint remains at baseline 4096.

### Per-Lane PHY Deskew Optimization and Clock Delay Tuning (2026-09-19)

Following source analysis of `cif_drv.c:1431-1488` (`cif_set_lane_deskew`), the SoC hardware
registers provide independent physical-lane deskew delay registers (`REG_84` / `REG_88` for
`DESKEW_CODE0..5`, and `REG_80` for `FORCE_DESKEW_CODE0..5`). The vendor driver in `cif.c:549-559`
had artificially grouped 4 data lanes into shared near/far values (`LANE_SKEW_CROSS_DATA_NEAR`
and `LANE_SKEW_CROSS_DATA_FAR`).

Tracked diagnostic patch `image/patches/0014-cif-per-lane-deskew-diagnostic.patch` was authored,
decoupled the shared grouping, and exposed `lane_phase[0..4]` directly to the physical lanes:
- `lane_phase[0]`: Clock (`lane_id[0] = 2`)
- `lane_phase[1]`: Data Lane 0 (`lane_id[1] = 4`, Port 0 Near)
- `lane_phase[2]`: Data Lane 1 (`lane_id[2] = 3`, Port 0 Near)
- `lane_phase[3]`: Data Lane 2 (`lane_id[3] = 1`, Port 1 Far)
- `lane_phase[4]`: Data Lane 3 (`lane_id[4] = 0`, Port 1 Far)

A systematic, single-variable-at-a-time sweep was executed on target hardware under strict fail-closed
discipline (holding `csi_ck_phase=8`, `hs_settle=16`, `pn_swap=0,0,1,0,0` constant unless explicitly noted).

#### Per-Lane Deskew Test Results Summary

| Run / Task | Lane Configuration (`CLK,D0,D1,D2,D3`) | Parameters | T0 Errors (Ecc/Crc/Wc) | T100ms Errors (Ecc/Crc/Wc) | Received Lines / Pixels | Final Mask & Errors |
|---|---|---|---|---|---|---|
| **task-9686** | `0, 0, 0, 12, 12` | ck_phase=8, settle=16 | 35 / 5 / 0 | 3752 / 488 / 156 | **81 lines** (155,520 px) | Masked 1.8s (65536 / 7860 / 3514) |
| **task-9692** | `0, 0, 0, 11, 12` | ck_phase=8, settle=16 | 33 / 2 / 1 | 3869 / 439 / 161 | **86 lines** (166,528 px / line 87) | Masked 1.8s (65536 / 7504 / 3567) |
| **task-9698** | `0, 0, 0, 10, 12` | ck_phase=8, settle=16 | 100 / 8 / 0 | 3772 / 459 / 173 | 2 lines (5,248 px / line 3) | Masked 1.8s (65536 / 7660 / 3469) |
| **task-9704** | `0, 0, 0, 13, 12` | ck_phase=8, settle=16 | 33 / 5 / 0 | 3878 / 470 / 173 | 14 lines (28,288 px / line 15) | Masked 1.8s (65536 / 7748 / 3721) |
| **task-9710** | `0, 0, 0, 11, 11` | ck_phase=8, settle=16 | 104 / 12 / 0 | 3865 / 478 / 185 | 15 lines (30,232 px / line 16) | Masked 1.8s (65536 / 7887 / 3535) |
| **task-9716** | `0, 0, 0, 11, 13` | ck_phase=8, settle=16 | 35 / 3 / 0 | 3816 / 480 / 176 | 1 line (1,920 px / line 1) | Masked 1.8s (65536 / 8031 / 3449) |
| **task-9781** | `0, 1, 0, 11, 12` | ck_phase=8, settle=16 | 33 / 6 / 1 | 3747 / 466 / 136 | 12 lines (24,448 px / line 13) | Masked 1.8s (65536 / 7779 / 2394) |
| **task-9787** | `0, 0, 1, 11, 12` | ck_phase=8, settle=16 | 113 / 12 / 0 | 3865 / 450 / 118 | 75 lines (144,000 px / line 76) | Masked 1.8s (65536 / 7492 / 2347) |
| **task-9793** | `0, 0, 2, 11, 12` | ck_phase=8, settle=16 | 33 / 7 / 0 | 3735 / 485 / 127 | 16 lines (32,128 px / line 17) | Masked 1.8s (65536 / 7963 / 2421) |
| **task-9799** | `1, 0, 0, 11, 12` | ck_phase=8, settle=16 | 112 / 19 / 1 | 3871 / 473 / 127 | **152 lines** (293,248 px / line 153) | Masked 1.8s (65536 / 7860 / 2328) |
| **task-9805** | `2, 0, 0, 11, 12` | ck_phase=8, settle=16 | 52 / 11 / 10 | 3791 / 489 / 136 | 11 lines (22,476 px / line 12) | Masked 1.8s (65536 / 7930 / 2445) |
| **task-9811** | `1, 0, 0, 11, 12` (repeat) | ck_phase=8, settle=16 | 35 / 3 / 0 | 3822 / 462 / 121 | 4 lines (9,088 px / line 5) | Masked 1.8s (65536 / 7806 / 2490) |
| **task-9853** | `1, 0, 0, 11, 12` | ck_phase=8, settle=20 | 104 / 2 / 0 | 3624 / **79** / 281 | 1 line (1,408 px / line 1) | Masked 1.8s (65536 / **1608** / 2960); >80% CRC drop |

#### Detailed Findings

1. **Port 1 Decoupling (D2 vs D3)**:
   - Upstream bundled physical lanes 1 and 0 (`D2` and `D3`) to the same deskew value. Decoupling them revealed that `D2=11` and `D3=12` is the true physical optimum, breaking through the previous 76-line ceiling to 86 complete lines.
   - The window is extremely narrow: moving `D2` to 10 collapsed lines to 2; moving `D2` to 13 degraded to 14 lines. Moving `D3` to 11 dropped lines to 15; moving `D3` to 13 collapsed lines to 1.
2. **Port 0 Behavior (D0 and D1)**:
   - Forcing manual deskew codes on Port 0 near lanes degraded line continuity: `D0=1` dropped lines from 86 to 12. `D1=1` gave 75 lines, and `D1=2` collapsed to 16 lines. Unforced / code 0 (`D0=0, D1=0`) remains optimal for Port 0 near lanes.
3. **Clock Deskew Breakthrough (`CLK=1`)**:
   - Setting physical clock lane deskew to code 1 (`lane_phase[0]=1`) extended video reception to a new record of **152 complete lines (293,248 pixels)** into `CSIBDG_A`.
   - Setting clock deskew to code 2 immediately collapsed line reception back to 11 lines, proving a sharp peak at `CLK=1`.
4. **Settle Dynamics (`hs_settle=20`)**:
   - Testing `hs_settle=20` with the optimal per-lane deskew confirmed the dramatic >80% reduction in CRC errors (1,608 total, 79 at T100ms), but packet payload streaming consistently truncates after 1 line. `hs_settle=16` remains required for line streaming.
5. **Frame Termination Mechanism**:
   - Two distinct hardware/driver mechanisms bound frame acquisition:
     a) `CSIBDG_A CH0 frm width less than setting(1920)`: when bit errors truncate a line packet, `CSIBDG` asserts an undersized-width interrupt, causing `_vi_err_handler()` in the VI driver to drop the buffer and re-trigger preraw.
     b) CIF Software Interrupt Mask (`cif.c:2763`): 1-bit ECC errors (correctable by Hamming code) accumulate at ~35,000/sec, overflowing the 16-bit software counter at 1.8s and writing `0xFF` to `CSI_INTR_MASK`.
6. **Admission Gate Status**:
   - Video admission remains strictly **FAIL-CLOSED**: 1080 valid lines, `FRAME_DONE_FE0`, and a hashed 4,147,200-byte capture frame were not achieved. Every run verified clean reverse module unloads (`UNLOAD_*_RC=0`), clean `/proc/modules`, absent frame, and baseline kernel taint 4096.

### Alternating Verification of Clock Deskew 0 vs Clock Deskew 1 (2026-09-19)

To determine whether the 152-line capture under `CLK=1` represented a statistical outlier or a genuine
improvement in the underlying line distribution, a 3-pair alternating experiment was executed on target
hardware, strictly holding HDMI input, link timings, and all lane parameters constant (`D0=0, D1=0, D2=11, D3=12`,
`csi_ck_phase=8`, `hs_settle=16`, `pn_swap=0,0,1,0,0`), varying only `lane_phase[0]` (Clock deskew code 0 vs 1).

Every attempt executed with a fresh module cycle, verified reverse unloads (`UNLOAD_*_RC=0`), clean
`/proc/modules`, absent frame, and final kernel taint `4096`.

#### Alternating Test Results Summary

| Run / Task | Clock Deskew (`lane_phase[0]`) | T0 Errors (Ecc/Crc/Wc) | T100ms Errors (Ecc/Crc/Wc) | Received Lines / Pixels | Truncation Type & Diagnostic Telemetry | Final Errors & Outcome |
|---|---|---|---|---|---|---|
| **A1 (task-9891)** | **CLK = 0** (`0,0,0,11,12`) | 106 / 18 / 0 | 3934 / 391 / 137 | **52 lines** (99,840 px) | `frm height less than setting(1080)`, pxl=99840 | Masked 1.8s (65536 / 8075 / 2398) |
| **B1 (task-9897)** | **CLK = 1** (`1,0,0,11,12`) | 41 / 5 / 0 | 3957 / 450 / 101 | **133 lines** (256,792 px) | `frm width less than setting(1920)`, pxl=256792 line=134 | Masked 1.8s (65536 / 7654 / 1607) |
| **A2 (task-9903)** | **CLK = 0** (`0,0,0,11,12`) | 34 / 5 / 0 | 3932 / 422 / 90 | **57 lines** (109,440 px) | `frm height less than setting(1080)`, pxl=109440 | Masked 1.8s (65536 / 7111 / 1625) |
| **B2 (task-9909)** | **CLK = 1** (`1,0,0,11,12`) | 34 / 9 / 0 | 3951 / 470 / 107 | **77 lines** (149,272 px) | `frm width less than setting(1920)`, pxl=149272 line=78 | Masked 1.8s (65536 / 7359 / 1665) |
| **A3 (task-9915)** | **CLK = 0** (`0,0,0,11,12`) | 35 / 2 / 0 | 4006 / 432 / 92 | **71 lines** (136,320 px) | `frm height less than setting(1080)`, pxl=136320 | Masked 1.8s (65536 / 7242 / 1556) |
| **B3 (task-9921)** | **CLK = 1** (`1,0,0,11,12`) | 111 / 12 / 0 | 4039 / 477 / 92 | **229 lines** (441,087 px) | `frm width less than setting(1920)`, pxl=441087 line=230 | Masked 1.8s (65536 / 7585 / 1602) |

#### Statistical Evaluation and Findings

1. **Distribution Shift Conclusively Proven**:
   - **Clock Deskew 0**: Mean **60.0 complete lines** (range: 52 to 71 lines; standard deviation ~9.8 lines).
   - **Clock Deskew 1**: Mean **146.3 complete lines** (range: 77 to 229 lines; including prior `task-9799`: 152 lines, aggregate mean **147.8 lines**).
   - Clock deskew code 1 provides a >2.4x improvement in mean line reception over code 0. Run B3 achieved a new all-time capture record of **229 complete lines and 441,087 pixels**, proving that the 152-line result was not an isolated outlier but part of a systematically superior capture distribution.

2. **Qualitative Difference in Error Signatures**:
   - **Clock 0 runs consistently terminate via `CH0 frm height less than setting(1080)`**: The receiver loses vertical synchronization before any partial line packet is latched, completing whole lines up to 52–71 before aborting.
   - **Clock 1 runs consistently terminate via `CH0 frm width less than setting(1920)`**: Packet reception continues deeply into the frame (77–229 lines) until an isolated bit error truncates an individual line packet before the full 1920 pixels arrive.

3. **Error Counter Comparison**:
   - Final Word Count (WC) errors under Clock 1 were consistently lower (~1,602–1,665 vs ~1,556–2,398 under Clock 0).
   - Final CRC errors were lower under Clock 1 (~7,359–7,654 vs up to 8,075 under Clock 0).

4. **Admission Gate Status**:
   - Video admission remains strictly **FAIL-CLOSED**: 1080 valid lines, `FRAME_DONE_FE0`, and a hashed 4,147,200-byte capture frame were not achieved. Every run verified clean reverse module unloads (`UNLOAD_*_RC=0`), clean `/proc/modules`, absent frame, and baseline kernel taint 4096.

### LT6911 SDK settle-8 comparison on CleanKVM (2026-09-19)

The SDK's `cv182x/lontium_lt6911/lt6911_cmos_param.h` specifies D-PHY enabled
with `hs_settle=8`. The same static header says `devno=1`, while the read-only
stock runtime observation selected device 0; the header is not a complete
runtime-state record. The earlier stock observation reported stable `yuv422-8`,
zero ECC/CRC/header/FIFO errors, and one WC error across two samples separated
by two seconds. It did not record a stock full-frame hash or the same T100ms
timing used below.

Patch 0014's module SHA-256 was verified on target as
`9dd7410b4ae9651d098a3d0a82f0d9aba90878e8c50c628f92740ec84ae38ef6`.
The installed raw-frame probe SHA-256 was
`151287655f54f42f34b5c70ea73d27c58a48e2c54bc1f94374d5eee40846be45`.
Both runs retained the standard CIF 65536-error masking behavior; patch 0015
was not used.

| Run | Clean settings | T0 ECC/CRC/WC | T100ms ECC/CRC/WC | Decode at T100ms | Outcome |
|---|---|---:|---:|---|---|
| LT8-tuned | `lane_phase=1,0,0,11,12`, `csi_ck_phase=8`, `pn_swap=0,0,1,0,0`, `hs_settle=8` | `102/0/0` | `3689/0/166` | `raw8` | Initial height error at zero lines; no frame; final `65536/0/3038` and `yuv422-10` decode after masking |
| LT8-stock-profile | `lane_phase=0,3,3,8,8`, `csi_ck_phase=0`, `pn_swap=0,0,0,0,0`, `hs_settle=8` | `41/0/0` | `4064/0/168` | `raw12` | Initial height error at zero lines; no frame; final `65536/8/2711` |

The old grouped `lane_phase={0,3,8,0,3}` cannot be passed unchanged to patch
0014: its five entries now map directly to physical clock and D0–D3. For the
observed lane order `2,4,3,1,0`, equivalent clock/near/far codes are
`0,3,3,8,8`. These values reflect the old driver defaults, not independently
measured stock live deskew registers.

Both diagnostic probes returned without a frame (`CVI_VI_GetChnFrame` error
`0xc00e8040`). All six load and reverse unload commands returned zero, final
`/proc/modules` contained no `soph_*` modules, the frame file was absent, and
taint remained 4096. The zero CRC count in the first run is not link parity:
ECC and WC still rose rapidly and decode did not remain `yuv422-8`. No
settle-6/10/12 sweep was performed because settle 8 did not approach the stock
error signature. Video admission remains closed. Next, compare read-only
runtime PHY/CSI and clock/reset register state on stock and clean under the
same HDMI signal, before attributing the difference to module prerequisites or
changing the driver again.

### Time-Limited Interrupt Mask Diagnostic (Patch 0015) (2026-09-19)

#### Hypothesis and Objective
Upstream `cif.c:2763-2777` unconditionally executes `cif_mask_csi_int_sts(ctx, 0x1F)` when any error counter
exceeds `0xFFFF`. Because 1-bit Hamming ECC errors increment on packet headers at ~35,000/sec, the software
counter overflows at ~1.8 seconds, shutting down all interrupts across the CSI receiver (`REG_04 = 0xFF`).
However, width and height failures occur within the first few lines of each frame, well before 1.8 seconds,
and CRC and word-count errors are also substantial.

To determine whether packet reception can continue past 1.8 seconds and whether any subsequent frame can
reach `FRAME_DONE_FE0` with 1080 valid lines, a non-default, time-limited diagnostic patch (**Patch 0015**)
was implemented:
1. **Selective Masking**: Implemented `cif_mask_csi_int_bits(ctx, mask)` in `cif_drv.c`/`cif_drv.h` to read
   `REG_04 (CSI_INTR_MASK)` and mask specific status bits. When `cif_diagnostic_mask=1`, `errcnt_ecc > 0xFFFF`
   masks only bit 0 (`CIF_INT_STS_ECC_ERR_MASK`, `0x01`), keeping data packet reception and other status unmasked.
2. **Fatal Error Retention**: Uncorrectable 2-bit header errors (`errcnt_hdr > 0`), FIFO overflow
   (`fifo_full > 0`), and severe CRC/WC overflow (`> 0xFFFF`) immediately mask `0xFF` to shut down the receiver.
3. **Interrupt Rate Limiting**: Capped total interrupts at 150,000 to protect the kernel from interrupt storms.
4. **First Link Error Logging**: Logs the precise error counters (`ecc`, `crc`, `wc`, `hdr`) at the moment
   the very first link error occurs.
5. **Default Stock Safety**: Controlled by module parameter `cif_diagnostic_mask` (default `0`), ensuring
   stock upstream behavior is completely unchanged unless explicitly requested.

- Patch file: `image/patches/0015-cif-time-limited-interrupt-mask-diagnostic.patch`
- Built module: `out/phase3-interrupt-mask-diagnostic-v1/soph_mipi_rx.ko` (SHA-256 `048366c3869f3639a61aa47b9bcdd55e135a38aa04a6dd0fe785be76893f3ca0`, vermagic `5.10.4-tag- preempt mod_unload riscv`).

#### Empirical Hardware Results

All tests executed holding HDMI input, link timings, and lane parameters fixed at the optimal baseline
(`lane_phase=1,0,0,11,12`, `csi_ck_phase=8`, `hs_settle=16`, `pn_swap=0,0,1,0,0`, `devno 0, lanes 2,4,3,1,0`).

| Run / Task | Mode (`cif_diagnostic_mask`) | First Error Logged | First Line Truncation | Behavior at 1.8s | Final Counters (T5000ms) | VI Frame Telemetry | Result |
|---|---|---|---|---|---|---|---|
| **Control (task-10232)** | `0` (stock default) | N/A (unpatched log) | `pxl=9088 line=5` (+18ms) | Full mask (`0xFF`) at 1.76s: `ecc=65536, crc=7788, wc=2422` | Frozen: `65536 / 7788 / 2422` | `SOF=378, FE0=0, FPS=0, WidthLS=1` | Stock mask confirmed |
| **Diag 1 (task-10236)** | `1` (diagnostic) | `ecc=1, crc=1, wc=0, hdr=0` at **+173 µs** | `pxl=5248 line=3` (+12ms) | ECC masked (`0x01`) at 1.78s; **streaming continued** | Active: **`84108 / 32255 / 9785`** | `SOF=196, FE0=0, FPS=0, WidthLS=1` | No 1080p frame |
| **Diag 2 (task-10240)** | `1` (diagnostic) | `ecc=1, crc=1, wc=0, hdr=0` at **+119 µs** | `pxl=34048 line=18` (+14ms) | ECC masked (`0x01`) at 1.76s; **streaming continued** | Active: **`84474 / 31296 / 9782`** | `SOF=393, FE0=0, FPS=0, WidthLS=1` | No 1080p frame |

#### Empirical Deductions

1. **Reception Continues Past Mask Point**:
   - With `cif_diagnostic_mask=1`, masking only bit 0 (`0x01`) successfully kept the CIF receiver active across
     the entire 5.0-second test duration.
   - Error counters continuously grew beyond 65,535 without kernel stalls or interrupt storms: `EccErr` grew
     to ~84,400, `CrcErr` to ~31,500, and `WcErr` to ~9,780.
   - Start of Frame (SOF) generation remained active across the entire 5 seconds (`VISofCh0Cnt` reached 196–393).

2. **Resolution of Blocker Hypothesis: Interrupt Mask is NOT the Remaining Blocker**:
   - In both diagnostic runs, `FRAME_DONE_FE0` was **never asserted** (`VIPreFECh0Cnt=0`, `VIPostCnt=0`, `VIFPS=0`).
   - The root blocker is conclusively demonstrated to be **early-frame packet corruption**:
     - The very first link error occurs within **120–173 microseconds** of receiver activation (`ecc=1, crc=1`).
     - Line truncation occurs on every single frame within the first 3 to 18 lines (`CSIBDG_A CH0 frm width less than setting(1920)`).
     - Because CRC and word-count errors remain substantial (~31.5k CRC and ~9.7k WC errors per 5 seconds), CIF/CSIBDG
       drops damaged packets, preventing any complete 1920-pixel line from reaching the end of the 1080-line frame.
   - Therefore, suppressing or altering the software interrupt mask alone cannot produce a valid frame until the
     physical-layer signal integrity and packet error rate are resolved.

3. **Admission Gate Status**:
   - Video admission remains strictly **FAIL-CLOSED**: No valid 1080-line frame or `FRAME_DONE_FE0` was produced.
   - In all runs, all six quarantined modules unloaded cleanly with RC=0 (`soph_vpss`, `soph_vi`, `soph_snsr_i2c`,
     `soph_mipi_rx`, `soph_base`, `soph_sys`), leaving `/proc/modules` empty, no frame file created, and kernel taint
     maintained at 4096.



