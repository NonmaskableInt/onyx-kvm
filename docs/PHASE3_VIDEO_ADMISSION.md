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
