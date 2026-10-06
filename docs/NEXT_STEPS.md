# Next steps

Updated: 2026-10-05

## Product completion plan

The ordered path from the admitted video breakthrough to a complete product is
tracked in `docs/PRODUCT_COMPLETION_PLAN.md`. The final image includes an
authenticated human web console and MCP, with separately admitted video, HID,
microphone capture, and audio output.

The first follow-up closed a discovered protocol mismatch: the v49 MCP capture
returned 3,110,400 bytes, which is 1920x1080 planar YUV420, while labeling the
result `yuv422p`. The protocol and broker now represent `yuv420p` separately,
default MCP snapshots to it, and reject a captured buffer whose exact byte
length does not match the requested format. Offline tests must pass before the
next image is packaged. Workspace tests and strict lints now pass. Offline
static RISC-V artifacts were rebuilt (current web-enabled `cleankvmd`
`ecf337a8d6aa...`; video
broker `84074a5851cc...`). The independently rebuilt v50 image is now packaged
at `out/phase3-web-video-v50/cleankvm-phase1.img` (79,692,288 bytes, SHA-256
`e57f5b480e0bf4da02350a88ebb8928cf34997ab1c273845c83e3e7732718fe8`).
Its FIP, FIT, and rootfs passed embedded-artifact verification, SD-card
read-back, cold-boot fail-closed verification, and an admitted YUV420P MCP
snapshot test.

The first human web shell is now embedded in the same TLS daemon. It uses the
existing MCP authorization and policy path, keeps a pasted development token
only in memory, displays independent video/HID/microphone/speaker state, and
requests bounded YUV420P snapshot metadata. Static resources are same-origin
and all responses carry no-store, strict CSP, frame-denial, no-referrer,
no-sniff, and browser permission-denial headers. Pixel delivery, HID controls,
and production enrollment remain intentionally gated.

## Immediate bounded hardware step

The v50 flash and cold-boot baseline are complete. The resolver selected exactly
one external removable target, `/dev/disk4` (USB, 30,992,760,832 bytes, prior
`CLEANKVM` label). Exactly 79,692,288 bytes were written. The same span read back
with SHA-256
`23d3406dc13793790f587023f78896bf4a9db4909948e5bc25dc16bb7cb9e7a2`;
macOS FAT bookkeeping changed the whole-span hash, while embedded FIP, FIT, and
rootfs artifacts all matched the audited v50 inputs. The card was safely
ejected.

Cold boot passed the fail-closed baseline over authenticated UART: kernel taint
`0`, no preloaded `soph_*` modules, `/dev/mem` absent, video gate absent,
microphone gate absent, speaker gate absent, and `/dev/snd/controlC0` absent.
Both Rust services were running. Installed `cleankvmd` and video-broker hashes
matched `ecf337a8d6aa...` and `84074a5851cc...` respectively. DHCP assigned
`eth0=192.168.2.98/24`.

The TLS web shell returned HTTP 200 at `https://cleankvm-recovery:8443/` using
the provisioned certificate, with no-store, strict CSP, permission denial,
no-referrer, no-sniff, and frame-denial headers. `/app.js` returned 200; an MCP
`tools/list` request without a bearer grant returned 401. No media gate was
enabled during these checks.

The first admitted v50 MCP snapshot is also complete. Before admission,
authenticated `tools/list` exposed only `kvm.status`. Opening only
`/run/cleankvm/video-enabled` added `kvm.video.snapshot`; a bounded request for
1920x1080 `yuv420p` returned exactly 3,110,400 bytes with SHA-256
`64e3d95b05308ee66ee47305c1ba0fd16727ec21297e961c1006142ebbb33927`.
The broker reported `isError: false` and returned the corrected `yuv420p` format.

Afterward the video gate was removed. The captured frame was absent, `/dev/mem`
remained absent, no `soph_*` module remained loaded, post-taint was the expected
4096, and microphone and speaker gates remained disabled. The next bounded step
was browser-side authenticated snapshot-metadata verification. The application
and JavaScript resources passed direct HTTPS checks, but the managed browser
correctly refused the development certificate (`ERR_CERT_AUTHORITY_INVALID`),
and policy forbids bypassing that warning. Certificate trust/enrollment is now
a release prerequisite rather than a reason to weaken TLS.

The separate Phase 3a microphone image is now prepared at
`out/phase3a-web-capture-v51/cleankvm-phase1.img` (79,692,288 bytes, SHA-256
`f7725d253b1739eca6c077bb632b54207cf3261badb5c17640c3eb0559e123a7`).
Its FIT (`1b504a2c...`), rootfs (`9295c98e...`), and FIP (`6e6d3b5c...`)
passed offline verification. A profile-transition hardening change removes all
optional probes, video modules, and video startup artifacts from reusable
overlay and target trees before selectively restoring only requested content;
the v51 audio rootfs therefore contains no inherited video broker or quarantine
modules. The image was flashed to the sole external physical removable target,
`/dev/disk4` (USB, 30,992,760,832 bytes, prior `CLEANKVM` layout). Exactly the
79,692,288-byte image span was read back. Its whole-span SHA-256 changed to
`8b934bf8d6b11af1c1541a96db80622b7ad3ce2837526a78046f7dff9837bbed`
after macOS FAT bookkeeping, while the embedded FIP, FIT, and rootfs all matched
the audited v51 inputs. The card was safely ejected. The next bounded step is a
cold-boot fail-closed baseline and the root-only, one-second microphone metrics
probe. Audio output remains disabled and is not part of this test.

The v51 cold boot and Stage B probe are now complete. The device acquired
`192.168.2.99`; taint was `0`, `/dev/mem` was absent, loadable-module support was
kernel-disabled, every media/audio gate was absent, sound nodes were root-only,
and no PCM was open. The audited probe captured exactly 16,000 mono S16LE frames
(32,000 bytes at 16 kHz) in about 1.01 seconds and returned metrics only:
peak 32,767, mean absolute 10,990, RMS 13,098, clipped 0, zeros 0. Post-capture,
no PCM descriptor, probe process, or audio output file remained; taint stayed 0
and both audio gates stayed absent. Stage B passes. The next audio step is the
separately bounded Stage C output-routing investigation; no playback is yet
admitted.

Stage C source and schematic attribution is complete. The board deletes its
only inherited amplifier-mute GPIO, while the populated AW8010A PA is
hardware-enabled; safe software shutdown is therefore the SG2002 DAC's internal
mute plus DAC/I2S-off state. A root-only, non-autostarting probe now applies the
lowest nonzero DAC gain and a 1.56%-full-scale, one-second 1 kHz tone, then
drains, re-mutes, closes, and zeroizes. The verified v52 test image is
`out/phase3a-playback-v52/cleankvm-phase1.img` (79,692,288 bytes, SHA-256
`46e3116e5e011f1173ed7b40bb985d791f55caef0fd8d293d0f25b10d2c2e545`).
Flash/read-back and the single bounded hardware playback proof are next.

The v52 flash/read-back is complete on the confirmed 31 GB CleanKVM card.
Embedded FIP, FIT, and rootfs matched; FAT bookkeeping changed the exact-span
read-back SHA-256 to
`aaafba1076b7e2d63d75aef01085d48de85fe96cb73e7b574649f328863e36d2`.
The card was safely ejected. Next: cold boot v52, verify the playback-probe hash
and fail-closed baseline, execute its self-test and one bounded tone, then prove
internal mute, PCM release, absent output files/processes, and unchanged taint.

## First hardware video frame captured: MIPI RX pinmux and pad pull-down resolution (2026-09-23)

Offline attribution of the persistent D3 instability and packet header
corruption uncovered the root-cause mechanisms:

1. **Physical Lane 0 (`PAD_MIPIRX0N`, `0x0300118c`) Pinmux Conflict**:
   - Stock `libkvm_mmf.so` in `lt6911_probe` invoked `system("devmem 0x0300118C 32 0x3")`
     via `/dev/mem` right before capture to override `/etc/init.d/S02config`'s
     `CAM_MCLK1` function 5 assignment back to function 3 (`MIPI_RX`).
   - CleanKVM enforces zero `/dev/mem` and forbids stock binaries/scripts, leaving
     `0x0300118c` at function 5, which actively drove an external camera clock onto
     the negative leg of differential pair D3! Furthermore, `PAD_MIPIRX4N/P`
     (`0x0300116c`/`0x03001170`) remained at function 0 rather than function 3.
   - This signal integrity violation flipped bit 0 of the Data Identifier
     (corrupting `0x1E` `YUV422-8` into `0x1F` `YUV422-10`), saturated ECC errors
     at 65,536, and caused height underflows.

2. **Pad Pull-Down Clearing**:
   - SG2002 power-on reset defaults MIPI pad controls (`0x03001c30..0x03001c5c`)
     to `0x48` (`PAD_CTRL_PD` bit 3 set).
   - CleanKVM patch `0012` previously cleared pull-down bits to `0x40`, which
     persisted until reboot.
   - Enforcing `(v & ~PAD_CTRL_PU) | PAD_CTRL_PD` (`0x48`) permanently restores
     the hardware pull-down state.

### Tracked Patch 0027

Tracked patch `image/patches/0027-cif-enforce-pad-pulls-and-mipi-rx-pinmux.patch`
sets all ten MIPI RX pads (`0x0300116c` through `0x03001190`) to function `0x3`
in `soph_mipi_rx.ko` with clean restoration on unload, and enforces `0x48`
pull-downs across all twelve pad registers.

### Breakthrough Hardware Results

Executed via `image/phase3-enforced-pad-pulls-test.exp` on target:
- `PROBE_RC=0`
- Frame captured: `1920x1080` `YUV422P` (4,147,200 bytes) at `/run/cleankvm-frame.yuv422p`
- SHA-256: `7c0b2f1de2356e98a842ffe499491a3019a94e81e87ab85b9e60261864809761`
- `Digital: D0=hs_hst, D1=hs_hst, D2=hs_hst, D3=hs_hst` (all 4 data lanes locked!)
- `Deskew: done`
- `EccErr`: 1 (reduced from 65,536 saturation!)
- `CrcErr`: 0, `HdrErr`: 0, `WcErr`: 1, `fifofull`: 0
- `decode: yuv422-8`
- `VIFPS: 60`, `VIDevFPS: 47`
- `VISofCh0Cnt: 97`, `VIPreFECh0Cnt: 125` (pre-FE admission gate passed!)
- `VICsiCh0HeightLSCnt: 0` (height underflow eliminated!)
- Reverse unload of all 6 modules returned 0, leaving `/proc/modules` empty,
  `/tmp/soph_mipi_rx.ko` deleted, `/dev/mem` absent, and post-taint at 4096.

### Cold-Boot Verification on Flashed v48 Image (`0d4dd05f...`)

The v48 SD image (`out/phase3-enforced-pad-pulls-v48/cleankvm-phase1.img`,
SHA-256 `0d4dd05f7762740c0f052a1df846e6a2bb5cf628af2591cdda642803b7812208`,
`rootfs.ext4` SHA-256 `27ebe768be65b98a13cbf93bf4a1883b9fd967e94dedacc54d4c3f3d6b34751f`)
was flashed to `/dev/disk4`, verified on readback (`embedded_artifacts=verified`),
and cold-booted on the NanoKVM Lite (`image/phase3-v48-quarantine-frame-test.exp`):
- Cold-boot `PRE_TAINT=0`, `DEVMEM_ABSENT_RC=0`, `PRELOADED=` (empty).
- Built-in `/usr/lib/cleankvm/video-quarantine/soph_mipi_rx.ko` verified at
  SHA-256 `e93d36dcc9bcc242d882547b8183339278fb5945c387ce5b8bb03ed33a551835`
  (zero UART upload required).
- All 6 pad pairs initialized at `0x48` (`P(0x48->0x48) N(0x48->0x48)`) and all
  10 MIPI RX pinmux registers set to `0x3`.
- Receiver counters: **`EccErr=1, CrcErr=0, HdrErr=0, WcErr=0, fifofull=0`**,
  `decode=yuv422-8`, all 4 data lanes in `hs_hst`, `Deskew=done`.
- VI counters: `VISofCh0Cnt=107`, `VIPreFECh0Cnt=126`, `VIFPS=60`,
  `VICsiCh0HeightLSCnt=0`, `VICsiCh0WidthLSCnt=0`.
- `PROBE_RC=0`: captured `1920x1080` `YUV422P` frame (`4,147,200` bytes),
  SHA-256 `501d9d4ed0562d1a425c004c65e1c49ccc67e5a2fbabd382799138fdac5d3c65`.
- Frame removed (`FRAME_CLEANUP_RC=0`), all 6 modules reverse-unloaded (`RC=0`),
  `FINAL_LOADED=` empty, and `POST_TAINT=4096`.

### Rust Control Plane & MCP `kvm.video.snapshot` Integration

1. **`cleankvm-media-protocol`**:
   - Added `SnapshotFormat::Yuv422p` (`3`), `WireHeader::encode()`, `SnapshotRequest::encode()`/`decode()` (`8` bytes), and `VideoSnapshot` (`42` bytes: `width`, `height`, `format`, `byte_length`, and 32-byte `sha256` digest).
2. **`cleankvm-mcp`**:
   - Added `VideoTransport` and `Server::with_video()`.
   - Exposed `kvm.video.snapshot` in `tools/list` and `tools/call` when `VideoTransport` is attached and the authenticated grant has `Scope::Video` (`kvm:video`).
   - Updated `kvm.status` to report `videoEnabled: true` when `VideoTransport` is attached and authorized, while keeping `audioCaptureEnabled: false` and `audioOutputEnabled: false` fail-closed.
3. **`cleankvmd`**:
   - Added `BrokerVideo` over `/run/cleankvm/video.sock` gated on `/run/cleankvm/video-enabled`.
4. **Remaining Roadmap to End-to-End Video Admission**:
   - **Step 1 — Implement `cleankvm-video-broker` (`crates/cleankvm-video-broker`) [COMPLETED]**:
     - Implemented `crates/cleankvm-video-broker` binding `/run/cleankvm/video.sock` (`0660`, `root:cleankvm`, `GID 900`) with `SO_PEERCRED` (`UID 900` `cleankvm`) and `/run/cleankvm/video-enabled` gating.
     - Decodes the 12-byte `WireHeader` + 8-byte `SnapshotRequest` (`20` bytes), enforces `!/dev/mem`, loads the 6 quarantine modules (`soph_sys` through `soph_vpss`), runs `/usr/libexec/cleankvm-raw-frame-probe 0 1080 8 0,0,0,0,0 2,4,3,1,0`, validates exact 1080p planar YUV frame lengths (`3,110,400` or `4,147,200` bytes), computes the SHA-256 digest in-memory, deletes `/run/cleankvm-frame.yuv422p`, reverse-unloads all 6 modules via a `Drop` guard (`soph_vpss` down to `soph_sys`), and returns the 42-byte `VideoSnapshot` wire response.
   - **Step 2 — Cross-Compile & Package into the CleanKVM v49 Rootfs [COMPLETED]**:
     - Built offline static stripped `riscv64gc-unknown-linux-musl` `cleankvm-video-broker` (`out/phase3-rust/cleankvm-video-broker`, SHA-256 `bcee855c38024d79ae927b5ada05d59939b26e11ca2ac7457a176ee1041f108d`) and `cleankvmd` (`out/phase2-rust/cleankvmd`, SHA-256 `98fe72b6bfd80114708a97f9418f513564a5b2fa93100738933c61c436d8af20`).
     - Added `/etc/init.d/S47cleankvm-video-broker` (`image/phase3-video-overlay/etc/init.d/S47cleankvm-video-broker`) and updated `image/audit-rootfs.sh`, `image/apply-phase3-video-foundation.sh`, and `image/build-phase1-rootfs.sh`.
     - Assembled and verified `out/phase3-video-broker-v49/cleankvm-phase1.img` (`79,692,288` bytes, SHA-256 `05394aca602c8fa95b72fce290db181c94eb3f1fb3bda2cf0b3e8c7ab48f3251`, `rootfs.ext4` SHA-256 `a16e0ab0455eef8dbd5570135770ae074aa40b272db2dbe40061b428d4e36fd9`, `embedded_artifacts=verified`).
   - **Step 3 — On-Target End-to-End MCP `kvm.video.snapshot` Verification [COMPLETED]**:
     - Flashed the v49 SD image to `/dev/disk4`, verified embedded FIP/FIT/rootfs hashes on read-back (`embedded_artifacts=verified`), and booted on the NanoKVM Lite (`eth0` IP `192.168.2.97`, `https://cleankvm-recovery:8443/mcp`).
     - **Gate 1 (Fail-Closed Default)**: Confirmed cold-boot `PRE_TAINT=0`, `DEVMEM_ABSENT_RC=0`, `PRELOADED=` empty, and `VIDEO_DISABLED_BY_DEFAULT_RC=0` (`/run/cleankvm/video-enabled` absent). Over authenticated HTTPS (`EdDSA` `at+jwt` with `scope="kvm:read kvm:video"`), `tools/list` returned only `kvm.status`, `kvm.status` reported `videoEnabled: false` (`audioCaptureEnabled: false`, `audioOutputEnabled: false`), and `tools/call kvm.video.snapshot` failed closed with `-32602` (`"Unknown or disabled tool"`).
     - **Gate 2 (Admitted Snapshot Capture)**: With `/run/cleankvm/video-enabled` created, authenticated `tools/list` exposed `kvm.video.snapshot`, `kvm.status` reported `videoEnabled: true` while keeping `audioCaptureEnabled: false` and `audioOutputEnabled: false`, and `tools/call kvm.video.snapshot` (both with `{"deadlineMs":5000}` and with omitted `arguments`) captured live 1920x1080 planar YUV frames (`byteLength: 3110400`, SHA-256 `23ed026e237b68bb087a8cfa258435a0265ad24f6007d39c8b74fb063690d848` and `8f0bdd7ef6ea7326d39d4570ba0dea1ec0ec304a44b33cd3d2e243541003f204`), deleted `/run/cleankvm-frame.yuv422p` (`FRAME_ABSENT_RC=0`), verified `/dev/mem` absent (`DEVMEM_ABSENT_RC=0`), and reverse-unloaded all 6 quarantine modules (`FINAL_LOADED=` empty, `POST_TAINT=4096`).

## Settle table sweep on SG2002 platform kernel (2026-09-23)

A bounded 9-point sweep across HS settle values (8, 10, 12, 14, 16, 18, 20, 22,
24) was executed on the live SG2002 platform kernel (v43). All tests used lanes
`2,4,3,1,0` and zero PN swaps. Pre-test baseline confirmed `/dev/mem` absent and
preloaded modules empty. Each point completed a fresh load/probe/unload cycle:

| Settle | T100ms ECC/CRC/WC | Final T5000ms ECC/CRC/WC | Final Decode | SOF Count | Pre-FE Count |
| ---: | --- | --- | --- | ---: | ---: |
| 8 (stock) | `5061/0/152` | `65536/4/1991` | `yuv422-10` | 95,875 | 0 |
| 10 | `5009/0/170` | `65536/1/1950` | `raw10` | 98,823 | 0 |
| 12 | `5088/0/158` | `65536/2/1929` | `yuv422-10` | 101,209 | 0 |
| 14 | `5083/11/148` | `65536/119/2047` | `yuv422-8` | 98,805 | 0 |
| 16 (clean default) | `4745/709/103` | `65536/8952/1445` | `yuv422-8` | 63,840 | 0 |
| 18 | `5386/165/127` | `65536/1785/1696` | `yuv422-8` | 80,106 | 0 |
| 20 | `4877/1/164` | `65536/4/1963` | `raw12` | 91,878 | 0 |
| 22 | `4615/1/116` | `65536/3/2085` | `yuv422-10` | 88,856 | 0 |
| 24 | `1/0/0` | `1/0/0` | `unknown` | 0 | 0 |

Findings:
1. Stock uses **settle 8** (`REG_10 = 0x00000208`), not 16.
2. At stock settle 8 (and 10, 12), CRC errors are minimal (1 to 4), but ECC saturates
   and decoded packet format shifts from expected `yuv422-8` to `yuv422-10`/`raw10`.
3. Settle 14, 16, 18 preserve `yuv422-8` decoding, but suffer elevated CRC errors
   (peaking at 8,952 at settle 16).
4. Settle 24 completely halts reception (SOF drops to 0).
5. In all active points, ECC saturates at 65,536, Pre-FE remains at 0, and no frame
   is admitted.

All 54 loads and reverse unloads returned zero (`UNLOAD_*_RC=0`). Final loaded
modules remained empty, no lingering video nodes or files remained, and final
taint was 4096. Video remains fail-closed.

## SG2002 platform kernel support excluded as CSI packet discriminator (2026-09-23)

The rebuilt CleanKVM v43 image was flashed to hardware and booted with all six
SG2002 platform selections enabled (`ARCH_CVITEK`, `SOC_THEAD`, `ARCH_CV181X`,
`ARCH_CV181X_ASIC`, `CVITEK_PINCTRL_CV1835`, and `COMMON_CLK_CVITEK`).
Pre-test verification confirmed `/dev/mem` absent, pre-taint 0, and no preloaded
modules. All six platform drivers registered cleanly in sysfs (`cvitek,pinctrl-cv181x`,
`ion-cvitek`, `cvi`, `cvi-reboot`, `gpio-clk`, and reset controller `nr_reset=64`).

The LT6911 bridge status was queried before the probe and confirmed active HDMI
lock at 1920x1080 (`HDMI_SIGNAL_A3=0x55`, `CSI_VACTIVE=1080`, `CSI_HACTIVE=1920`)
with zero taint.

The bounded raw-frame probe executed with baseline profile 0 (lanes `2,4,3,1,0`,
settle 16). Clocks and resets bound with platform IRQs (`irq-30` as `cif-irq0`,
`irq-31` as `cif-irq1`). During the probe:
- High-speed clock was active (`CK_HS=1`).
- Receiver reported `yuv422-8` with ECC 65,536, CRC 9,917, and WC 2,351.
- VI reported 1920x1080 dimensions with SOF reaching 56,804, but pre-FE count
  remained 0 and one height-underflow event occurred (`VICsiCh0HeightLSCnt: 1`).
- `CVI_VI_GetChnFrame` returned `0xc00e8040` (no frame acquired).
- Reverse module unloading of all six modules succeeded with return code 0 for
  each module, leaving zero remainder in `/proc/modules`, no residual video device
  nodes, and post-taint at 4096.

This closes missing SG2002 platform kernel support as the cause of upstream CSI
packet corruption. The driver infrastructure is now verified one-to-one at the
platform initialization boundary, yet packet errors persist upstream of the VI
pre-FE admission gate. Continue investigating boot-time PHY/clock timing and
LT6911 transmitter/receiver electrical state. Video remains fail-closed.

## Pre-Linux and stock capture-path audit closes another software boundary (2026-09-23)

The stock FIT was extracted and its Linux device tree compared with the current
CleanKVM v42 tree. CIF register ranges, reset IDs, clock IDs and names,
interrupts, VI resources, and mailbox resources are identical. The differences
are the intentional sensor-reset polarity/GPIO choice, framebuffer and ION
reserved-memory sizing, and USB mode. The raw frame probe does not issue sensor
reset, and reserved-memory and USB differences cannot change upstream CSI packet
decoding.

Stock and CleanKVM BL2 executable instructions are identical; the 26 differing
bytes are build timestamp/revision text. Monitor executable instructions are
also identical; its four differing data bytes encode the known reserved-memory
address/size change. The remaining second-stage-loader-only writes resolve to
USB reset and disabled display-output support. No loader instruction unique to
stock targets the CIF/CSI register range, and the shared CSI/VIP clock setup has
the same instruction sequence. This closes a missing pre-Linux CSI clock or
reset write as the current explanation.

Offline disassembly of stock `libkvm_mmf.so` (SHA-256
`b4c14aa7ca6311036662ea3f134ae9e958c62ae2a64584168bfea848e30026e7`)
shows that `SAMPLE_PLAT_VI_INIT` follows the pinned middleware source: start the
sensor callbacks, set and enable the VI device, reset/configure MIPI and release
the sensor clock/reset, probe the sensor, create/start the VI pipe, create ISP,
and configure/enable the VI channel. `SAMPLE_COMM_VI_StartMIPI` contains no
additional receiver ioctl and does not unreset MIPI. Its reset and clock subset
was already exercised by the source-exact v35 diagnostic without producing a
frame. The raw probe already performs the same VI-device, MIPI-attribute, pipe,
and channel operations required before frame acquisition. The stock LT6911
sensor probe only opens I2C and has no hidden register programming table.

That built-in comparison found a material configuration mismatch. Stock enables
`ARCH_CVITEK`, `SOC_THEAD`, `ARCH_CV181X`, `ARCH_CV181X_ASIC`,
`CVITEK_PINCTRL_CV1835`, and `COMMON_CLK_CVITEK`; the v42 kernel contains none
of them. Generic clock, reset, GPIO, regmap, and CVITEK PHY support remain, so
the loadable modules can probe, but CleanKVM is not yet one-to-one with stock at
the platform initialization boundary. The tracked Phase 1 fragment now pins
all six SG2002 platform selections, and both Phase 1 and Phase 3 validators
assert them. The Phase 3 normalized configuration passes against the cached
SDK.

Build and verify a fresh video image from a new disposable volume, confirm the
six symbols in the generated kernel configuration before packaging, and run the
same bounded raw-frame test. Do not add the opaque stock library or repeat the
closed standalone reset/clock sequence. Video remains fail-closed.

## Stock beta RX0N mux is already present on CleanKVM (2026-09-23)

Offline extraction of stock `/etc/init.d/S02config` found a post-module-load
beta-board write that the earlier loader comparison did not cover: it writes
function 5 (`CAM_MCLK1`) to MIPI_RX0N at `0x0300118c`. The stock alpha branch
instead writes function 3 there and function 5 to RX4N. The NanoKVM Lite path
is beta, so this was a source-backed, receiver-adjacent software difference
worth checking after the bridge became active.

Tracked patch `0026-cif-stock-rx0n-mclk1-diagnostic.patch` adds a default-off,
single-word diagnostic. It maps only `0x0300118c`, saves the full word, changes
only function bits 0..2 to 5, verifies the readback, and restores the complete
saved word on unload. The offline-built module SHA-256 was
`f80e755c670278039cc30fec67cb1ea8a055aa5038b3dfe1f7527dca53e05095`.

On hardware, the value saved before any diagnostic write was already
`0x00000005`; the active readback remained `0x00000005`. Thus current CleanKVM
already has the exact stock beta RX0N mux state, and the diagnostic write was a
no-op. The pull words remained `0x48`. The run produced no frame, saturated
ECC at 65,536, accumulated CRC/WC `13/6423`, and left pre-FE at zero. All six
modules reverse-unloaded with return code zero, no Sophgo module or temporary
payload remained, and taint stayed 4096.

This closes the late stock RX0N mux write as the discriminator. Continue the
offline pre-Linux audit with source-attributed BL2/monitor/loader writes to the
VIP, CSI clock, and reset domains and compare the relevant live device-tree
resources. Video remains fail-closed.

## RTOS boot-time handoff excluded (2026-09-23)

The source-built mailbox module and a containment-patched fast-image module
completed a handshake-only hardware cycle. Both payload hashes matched on the
target. The run began with `/dev/mem` absent, taint zero, and no Sophgo module
loaded. `soph_rtos_cmdqu` loaded, registered the mailbox on PLIC IRQ101, and
showed zero interrupts. The bounded 200 ms fast-image request then reported
`RTOS_CMDQU_SEND_WAIT timeout`, `SYS_CMD_INFO_LINUX_INIT_DONE fail`, and
`communicate with rtos fail`.

This is the same failed handoff already observed in the working stock boot.
No valid transfer structure was returned, so fast-image did not mark a handoff
ready, allocate RTOS-described ION regions, or transfer ISP/JPEG/codec IRQ
ownership. All four modules reverse-unloaded with return code zero, no payload
remained in `/tmp`, and the final loaded-module set was empty. Final taint was
the expected out-of-tree bit, 4096.

The tested fast-image patch is default-off, refuses its exported stop routine
without a successful handoff, initializes every IRQ slot invalid, enables only
discovered IRQs, checks platform-driver registration, and frees all three ION
classes on exit. Its SHA-256 was
`2e7d377d2633a2a7a721b5cdf3f5ab44b8d0f3873319646d31eefc65df241864`;
the mailbox module remained
`929ab1671f8edf064412ad92a14f9f9f1dd573012c0861e7eae91b95b02e86ae`.

Do not add these RTOS modules to the full video load: the working stock system
also receives no handoff, and the handshake-only run produced no initialization
state that can account for its clean CSI packets. Continue offline comparison
of stock versus CleanKVM boot configuration and pre-Linux pin/clock/reset state,
using source-backed reads and image differences. Video remains fail-closed.

## Physical comparison withdrawn; receiver binary delta narrowed (2026-09-23)

The physical waveform comparison is withdrawn as a next step. The same device,
bridge, HDMI source, cable, and Mac display path work under the stock image and
fail under CleanKVM. Continue software attribution rather than treating the
repeatable image-dependent result as a physical fault.

A RISC-V function-level comparison of the loaded stock build-30 receiver and
independently built CleanKVM v7 found 72 shared functions. All core hardware
functions have identical machine-code bodies, including CSI streaming, lane
selection, lane deskew, settle, clock direction/edge, interrupt status/mask/
clear, PHY state, and register-base access. The previously established CIF and
VI ISR bodies are also identical.

Only three shared functions differ materially: probe, exit, and `_cif_ioctl`.
Stock additionally contains a 72-byte `button_isr` plus proc diagnostic
open/show/write helpers. Its probe installs a GPIO361 threaded interrupt and
diagnostic proc entry. CleanKVM instead discovers the device-tree sensor-reset
GPIO. CleanKVM's only extra receiver function is the 418-byte bounded sensor
reset helper. Within `_cif_ioctl`, the call graph is otherwise identical; the
sole functional call difference is stock's direct `gpio_direction_output`
versus CleanKVM's polarity-aware reset helper. The raw-frame probe explicitly
does not invoke sensor reset or sensor clock, so that ioctl delta is inactive
in every failed comparison.

This closes missing receiver ISR and missing core receiver operation as the
explanation. The remaining credible software boundary is boot-time state and
initialization order outside `soph_mipi_rx`, especially the independently
reviewed effects of `soph_rtos_cmdqu` and `soph_fast_image`. Before any load,
enumerate their exact DT resources, shared-memory ranges, IRQ ownership changes,
ION allocations, init/exit symmetry, and target log behavior; then construct a
source-built bounded experiment or reject it. Do not execute stock modules.
Video remains fail-closed.

The existing verified offline SDK was cloned to disposable Podman volume
`cleankvm-work-rtos-audit-v1`. With networking disabled, the pinned sources
built against the prepared CleanKVM kernel and verified RISC-V musl toolchain:

- `soph_rtos_cmdqu.ko`: SHA-256
  `929ab1671f8edf064412ad92a14f9f9f1dd573012c0861e7eae91b95b02e86ae`,
  depends on `soph_base`.
- `soph_fast_image.ko`: SHA-256
  `da8c40f68c8096e25bc94712b26eb6b4ef3a8f23dd1ed29c142d5a0d5bcb0fd9`,
  depends on `soph_sys,soph_rtos_cmdqu`.

Both report GPL and target vermagic
`5.10.4-tag- preempt mod_unload riscv`. They have not been loaded.
`soph_rtos_cmdqu` maps mailbox `0x01900000..0x01900fff`, initializes its
hardware spinlock/mailbox context, and requests PLIC IRQ101. `soph_fast_image`
maps the DT FreeRTOS reserved region, performs a 200 ms init handshake, validates
the returned transfer configuration, and may allocate ISP/image/encoder ION
buffers at RTOS-specified physical addresses.

The unmodified fast-image exit is not admissible for a contained diagnostic:
it unconditionally sends STOP_ISR and calls `enable_irq` for ISP/JPEG/H.264/
H.265 variables even if the handshake and IRQ discovery never succeeded, and
its normal remove path does not free every possible ION allocation. Before a
hardware load, add a default-off diagnostic enable, track successful handoff
and valid IRQ discovery, bound every wait, and free all allocations on every
exit path. Build and inspect that patch offline, then run one boot-scoped load
ahead of the six video modules with reverse cleanup or mandatory reboot.

## Post-snapshot reboot restores clean baseline (2026-09-23)

After the active interrupt runner lost its serial session, CleanKVM was
rebooted. The authenticated post-boot verification found no probe process and
all six `rmmod` checks reported that the named module was absent. The explicit
Sophgo module remainder was empty, the diagnostic payload/log/PID and frame
paths were removed or absent, frame absence passed, and kernel taint was zero.
This closes the cleanup exception without carrying diagnostic state into the
next experiment.

The matched IRQ evidence places the divergence before VI pre-FE/scaler and
outside per-frame RTOS mailbox transport. The binary audit above reopens only
boot-time RTOS/shared-memory initialization as a bounded software hypothesis.
Video remains fail-closed.

## Active CleanKVM interrupt comparison localizes the break (2026-09-23)

The matched active snapshot used stock polarity, lanes `2,4,3,1,0`, D1 deskew
12, settle 16, clock phase 0, and preserved `0x48` pad words. Both hashes
matched, `/dev/mem` was absent, the run began at taint zero with no Sophgo
module loaded, and all six diagnostic modules loaded successfully.

| Sample | CIF IRQ26 | VI IRQ24 | Scaler IRQ25 | ECC/CRC/WC | SOF | Pre-FE |
| --- | ---: | ---: | ---: | --- | ---: | ---: |
| Before probe | 0 | 0 | 0 | n/a | n/a | n/a |
| T+1 s | 45600 | 13940 | 0 | `35328/4279/6365` | 28747 | 0 |
| T+3 s | 85004 | 59896 | 0 | `65536/7833/11721` | 74927 | 0 |

VI reported the configured 1920x1080 dimensions but FPS zero, pre-FE zero,
pre-BE/post zero, one height-underflow event, and no scaler interrupt. Stock,
by contrast, produced no new CIF error IRQ, advanced VI by 365 and scaler by
201 over two seconds, and advanced pre-FE by 201 with stable receiver errors.

This localizes the CleanKVM break before pre-FE/scaler admission: corrupted CSI
packets generate an interrupt storm and false SOFs, but no valid frame reaches
the downstream pipeline. RTOS mailbox/frame transport and missing Linux ISR
logic are excluded as primary causes. `soph_fast_image` cannot repair malformed
CSI packets upstream.

The background probe failed to exit after the measurements. Attempts to
interrupt it and regain the authenticated console ended with serial EOF before
reverse unload could be verified. The runner is now bounded to terminate the
probe explicitly after its T+3 sample. The current target must be rebooted
before any further experiment; verify zero loaded modules and absence of all
temporary files after boot. Video remains fail-closed.

## Stock runtime IRQ ownership captured (2026-09-23)

The read-only stock snapshot booted kernel `5.10.4-tag- #46`, with the expected
27 modules including `soph_rtos_cmdqu`, `soph_fast_image`, receiver build #30,
VI, VPSS, codec, and scaler modules. Taint was 4098. No command changed a file,
module, register, or device state.

Across the fixed two-second interval:

| Hardware IRQ | Linux owner | T0 | T2000 | Delta |
| ---: | --- | ---: | ---: | ---: |
| 26 | `cif-irq0` | 1604 | 1604 | 0 |
| 24 | `a000000.vi` | 11587 | 11952 | 365 |
| 25 | `CVI_VIP_SCL` | 7208 | 7409 | 201 |
| 101 | `mailbox` | 0 | 0 | 0 |
| 22/21/23 | H.265/H.264/SBM | 0/0/0 | 0/0/0 | 0/0/0 |

During the same interval, receiver ECC/CRC/WC remained `1/0/1` with stable
`yuv422-8`. VI SOF advanced 5331 to 5505 (+174), and pre-FE count advanced
6326 to 6527 (+201), matching the scaler IRQ delta. All reported width/height
values were 1920x1080, and every VI CSI dimension/overflow counter remained
zero. Pre-BE, post, drop, and dump counters remained zero in this snapshot.

This demonstrates that active stock video events are handled through Linux VI
and scaler IRQs. The RTOS mailbox does not carry a per-frame interrupt stream,
and the codec IRQs were inactive. `soph_fast_image`/`soph_rtos_cmdqu` may still
perform boot-time ownership and shared-memory setup, but ongoing mailbox frame
transport is closed as the explanation. The zero CSI IRQ delta is expected
because no new receiver error occurred; CleanKVM's CSI IRQ storm corresponds
to its accumulating protocol errors.

The next matched step is to boot CleanKVM and capture this same inventory while
the fixed pull-preserved receiver/VI probe is active. Compare VI IRQ, SOF,
pre-FE, scaler, and CSI error deltas directly. Video remains fail-closed.

## Stock and CleanKVM video ISRs are byte-identical (2026-09-23)

Offline ELF symbol and section parsing compared the extracted stock modules
with independently built CleanKVM v7. The complete 262-byte `cif_isr` machine
code is byte-identical, SHA-256
`81c99f22bfea462d3ec529e711333de586f8f56d0ce0bd4a2f6e9f7be374674d`.
Its relocation targets also match: five calls to `cif_check_csi_int_sts`, then
the same counter-limit branch, `cif_mask_csi_int_sts`, error reports, and
`cif_clear_csi_int_sts`.

The VI interrupt paths are also byte-identical. The 22-byte `vi_core_isr` has
SHA-256 `3844890fdc88d3c8213ea6075b0429c6c645d938cdd93045174e6beb6fbad1c2`;
the complete 2,444-byte `vi_irq_handler` has SHA-256
`fd38193a6818948ce81017bed1d6edbbb1a65b2fc67c61c6a3450f96c9f9df49`.
CleanKVM's observed CSI error counters and VI SOF counts independently show
that both handlers execute on target. Missing CSI or VI ISR implementation is
therefore closed as the no-frame explanation.

Pinned source confirms `soph_rtos_cmdqu` owns a separate mailbox IRQ and
dispatches RTOS command callbacks. `soph_fast_image` sends the Linux/RTOS init
handshake, maps RTOS shared memory, allocates expected ISP/image/encoder ION
buffers, and coordinates transfer of ISP/JPEG/codec interrupt ownership. It
does not register or replace the CIF ISR or VI frame-done ISR. These modules
could affect downstream ISP ownership and buffer handoff, but cannot by
themselves explain upstream CSI ECC/WC corruption. Stock has also logged
`SYS_CMD_INFO_LINUX_INIT_DONE fail`, so successful RTOS handoff must be
measured rather than assumed.

`image/phase3-runtime-interrupt-snapshot.exp` now captures an unchanged,
read-only two-second inventory of loaded modules, every IRQ count, MIPI state,
VI state, taint, and relevant device-tree nodes. Run it first on stock with the
receiver active, then under the matched CleanKVM diagnostic condition. Compare
CIF, VI/ISP, and mailbox IRQ ownership and deltas before considering a bounded
source-built RTOS experiment. Video remains fail-closed.

The unloaded CleanKVM baseline completed with no module, MIPI, or VI proc entry.
PLIC hardware IRQ 26 (CSI0) retained count 1,562,120 and IRQ 24 (VI/ISP)
retained count 3,128,024 from the preceding diagnostic cycles, but neither
advanced during the two-second sample. Their Linux action names were empty
after module unload. Mailbox IRQ 101 had no `/proc/interrupts` entry. This is
consistent with clean handler removal and independently confirms that both
Linux video IRQ paths ran during earlier probes; it is not evidence of live
RTOS interrupt activity in the unloaded baseline.

## Settle refinement closes the bounded software sweep (2026-09-23)

The fixed-condition refinement sampled settle 17, 18, 19, 21, 22, and 23
around the earlier settle-20 result. Pads remained `0x48`; polarity, lane map,
D1 deskew 12, and clock phase 0 remained fixed.

| Settle | T100 ECC/CRC/WC | Final ECC/CRC/WC | Final decode | SOF |
| ---: | --- | --- | --- | ---: |
| 17 | `3053/533/225` | `65536/10997/4453` | `yuv422-8` | 184974 |
| 18 | `3004/326/254` | `65536/6826/4990` | `yuv422-8` | 205648 |
| 19 | `2864/263/285` | `65536/6182/5696` | `yuv422-8` | 219582 |
| 20 | `2779/85/299` | `65536/2067/6462` | `yuv422-8` | 236821 |
| 21 | `2675/3/299` | `65536/41/7350` | raw8 | 256691 |
| 22 | `2658/3/281` | `65536/58/7284` | raw10 | 255738 |
| 23 | `876/3/53` | `65536/53/3744` | unknown | 126629 |
| 24 | `1/0/0` | `1/0/0` | unknown | 0 |

The transition is monotonic through settle 22 for CRC and SOF, but it trades
CRC for increasing WC and then loses the required YUV format at 21. ECC
saturates at every active point, and no frame completed. There is no timing
point in this bounded interval that improves all error classes or meets the
admission gate.

All 36 unloads returned zero, each remainder was empty, both hashes matched,
`/dev/mem` was absent, all pad checks showed `0x48`, temporary files were
removed, and final taint remained 4096. The next experiment is the prepared
physical stock-versus-CleanKVM differential measurement at FPC2, starting with
clock and D3/RX0, under the fixed conditions in
`docs/PHASE3_MIPI_SIGNAL_MEASUREMENT.md`. Video remains fail-closed.

## Pull-preserved timing sweep narrows HS settle (2026-09-23)

The established seven-point, one-variable timing sweep was repeated at stock
polarity with all MIPI pad words preserved at `0x48`. Fixed settings were lanes
`2,4,3,1,0`, D1 deskew 12, settle 16, and clock phase 0 except for the named
variable:

| Point | D1 deskew | Settle | Clock phase | T100 ECC/CRC/WC | Final ECC/CRC/WC | Final decode | SOF |
| --- | ---: | ---: | ---: | --- | --- | --- | ---: |
| baseline | 12 | 16 | 0 | `2987/479/226` | `65536/9859/5183` | `yuv422-8` | 198316 |
| D1 10 | 10 | 16 | 0 | `3018/443/224` | `65536/9818/5030` | `yuv422-8` | 199650 |
| D1 11 | 11 | 16 | 0 | `3066/452/267` | `65536/9683/5127` | `yuv422-8` | 199521 |
| D1 13 | 13 | 16 | 0 | `3009/430/243` | `65536/9605/5088` | `yuv422-8` | 200004 |
| settle 20 | 12 | 20 | 0 | `2779/85/299` | `65536/2067/6462` | `yuv422-8` | 236821 |
| settle 24 | 12 | 24 | 0 | `1/0/0` | `1/0/0` | unknown | 0 |
| clock 8 | 12 | 16 | 8 | `2950/405/248` | `65536/9960/5026` | `yuv422-8` | 200339 |

No point completed a frame. D1 deskew 10 through 13 and clock phase 8 were
equivalent within run variation. Settle 20 cut final CRC errors by roughly 79%
and raised SOF activity, although ECC still saturated and WC increased. Settle
24 disabled useful reception entirely. The refinement above completed the
planned interval and found no admissible point. Video remains fail-closed.

Both transferred hashes matched, `/dev/mem` remained absent, and each point
verified all ten selected P/N pad words at `0x48`. All 42 unloads returned zero,
every remainder was empty, temporary files were removed, and final taint stayed
4096. An initial local result-filter quoting error occurred before the first
probe; its six loaded modules were immediately reverse-unloaded with all return
codes zero and an empty remainder before the corrected sweep began.

## Complete polarity matrix with stock pull-downs active (2026-09-22)

The complete 16-state D0/D1/D2/D3 PN matrix was repeated with the MIPI pad
pull-down bit preserved at stock value `0x48`. All other matrix conditions
matched the 2026-09-19 run: lanes `2,4,3,1,0`, deskew `0,0,12,0,0`, settle
16, and clock phase 0. Both transferred hashes matched, `/dev/mem` remained
absent, and the run started with no Sophgo module loaded.

| Test | PN swap | Final decode | Final ECC/CRC/WC | Post-probe SOF | Final D0/D1/D2/D3 |
| ---: | --- | --- | --- | ---: | --- |
| 1 | `0,0,0,0,0` | `yuv422-8` | `65536/9056/3871` | 191000 | idle/idle/idle/idle |
| 2 | `0,0,1,0,0` | `yuv422-8` | `65536/5301/1724` | 385 | hst/idle/hst/hst |
| 3 | `0,0,0,1,0` | `yuv422-8` | `65536/1596/4239` | 1433 | idle/idle/idle/idle |
| 4 | `0,0,1,1,0` | `yuv422-8` | `65536/4762/9419` | 205460 | hst/idle/idle/idle |
| 5 | `0,0,0,0,1` | unknown | `65536/0/4` | 171 | hst/hst/hst/idle |
| 6 | `0,0,1,0,1` | unknown | `65536/0/0` | 182 | hst/idle/hst/idle |
| 7 | `0,0,0,1,1` | unknown | `65536/0/4` | 221 | idle/idle/idle/idle |
| 8 | `0,0,1,1,1` | unknown | `65536/0/0` | 874 | hst/idle/idle/idle |
| 9 | `0,1,0,0,0` | unknown | `1/0/0` | 0 | idle/hst/hst/idle |
| 10 | `0,1,1,0,0` | unknown | `0/0/0` | 0 | idle/idle/hst/idle |
| 11 | `0,1,0,1,0` | unknown | `0/0/0` | 0 | idle/hst/idle/idle |
| 12 | `0,1,1,1,0` | unknown | `0/0/0` | 0 | idle/idle/idle/err |
| 13 | `0,1,0,0,1` | unknown | `0/0/0` | 0 | idle/idle/idle/idle |
| 14 | `0,1,1,0,1` | unknown | `0/0/0` | 0 | idle/idle/hst/idle |
| 15 | `0,1,0,1,1` | unknown | `0/0/0` | 0 | idle/hst/idle/idle |
| 16 | `0,1,1,1,1` | unknown | `0/0/0` | 0 | idle/idle/idle/idle |

Every probe returned 1 and no frame was created. All 96 module unloads returned
zero, every per-state remainder was empty, the temporary module and logs were
removed, and final taint remained 4096.

With pull-downs active, all four states having D0=0 and D3=0 decoded
`yuv422-8`; D1 inversion is no longer uniquely required. D3 inversion still
prevents format decoding, and D0 inversion still eliminates SOF in all eight
permutations. Stock polarity reached high SOF activity but still saturated
ECC/CRC/WC and completed no frame.

The earlier single stock-profile pull test only showed that preserving `0x48`
was insufficient at stock deskew `0,8,8,3,3` and settle 8. It did not close the
pad pull state across the timing space. The next bounded software step is to
repeat the existing single-variable deskew/clock/settle evaluation with
pull-downs preserved, beginning at unswapped polarity. Physical comparison
remains useful if that search cannot eliminate errors. Video remains
fail-closed.

## Physical MIPI comparison prepared (2026-09-22)

Offline disassembly of the retained stock and CleanKVM second-stage loaders
shows identical read-mask-write sequences that select function 3 on all ten
MIPI RX pins. This closes pin mux as a remaining discriminator without another
live write or helper.

The official LicheeRV Nano schematic and the stock lane profile map logical D3,
the repeatedly unstable lane, to physical MIPI RX0 at camera connector FPC2
pins 18/19. Clock is RX2 at pins 12/13; D0/D1/D2 are RX4/RX3/RX1 at pins
6/7, 9/10, and 15/16. The bounded stock-versus-CleanKVM procedure, equipment
boundary, capture order, and interpretation gate are recorded in
`docs/PHASE3_MIPI_SIGNAL_MEASUREMENT.md`. Physical capture now requires a
suitable differential probe or CSI analyzer and safe access to FPC2. Video
remains fail-closed.

## Preserving stock MIPI pad pulls does not recover the link (2026-09-22)

The prepared CleanKVM diagnostic passed both on-target hash checks from a
zero-taint, no-module baseline, and `/dev/mem` remained absent. With
`cif_diagnostic_preserve_pad_pulls=1`, all five selected clock/data P/N pairs
remained `0x48` through setup, exactly matching stock. Every static CSI,
wrapper, and PHY control word again matched the stock baseline.

The link did not improve materially. ECC/CRC/WC was `2948/1/246` at 100 ms
and `65536/12/5588` at five seconds, decode was `yuv422-10`, D3 moved through
`hs_err`/idle states, VI recorded one height-underflow event, and no frame
completed. The comparable pull-cleared run was `2756/1/235` at 100 ms and
`65536/15/6249` at five seconds, also with no frame. The small counter
variation supplies no recovery signal.

All six modules reverse-unloaded with return code zero, the frame and temporary
module were absent, and final taint was only the expected out-of-tree bit 4096.
This shows only that pad preservation is insufficient at the stock-equivalent
timing profile. The complete matrix above demonstrates that it materially
changes receiver behavior, so no production default is selected yet.

One remaining repeatable discriminator is live electrical behavior: stock
reaches `DBG_94=0x5555` with stable `yuv422-8`, while this clean run sampled
`DBG_94=0x0555` immediately after setup and repeatedly showed D3 instability.
Physical measurement of the clock and four data pairs, especially D3, remains
prepared if the pull-preserved timing sweep does not resolve the errors. Do not
infer a new register write from status alone. Video remains fail-closed.

## Stock preserves MIPI pad pulls; bounded clean test next (2026-09-22)

The extended stock reader was rebuilt offline and matched SHA-256
`5cd69f546316589f702b43cad32f3e999d43e2bd7a301c5c5139b3ab977a13d3`
on the stock kernel `#46` before execution. Two snapshots two seconds apart
read all twelve MIPI P/N pad words at `0x03001c30` through `0x03001c5c` as
`0x00000048`. The temporary reader was removed and verified absent. Stock
taint remained 4098. The receiver was active as `yuv422-8`, with zero
ECC/CRC/header errors and one WC error in the final live sample.

This exposes a concrete stock/CleanKVM difference. Pinned source defines bit 2
as pad pull-up and bit 3 as pad pull-down. The clean receiver's ordinary setup
clears both bits on the five selected clock/data lane pairs, producing `0x40`;
stock leaves bit 3 set on all six pairs at `0x48`. Do not assume the SDK's
commented pull clearing is correct for this board.

The completed bounded experiment used a default-off CleanKVM diagnostic that
preserves
the initial `0x48` pad words during otherwise unchanged stock-equivalent setup,
verifies the post-setup words, runs the existing short frame probe, then
reverse-unloads and removes the temporary module. Video remains fail-closed.

Tracked patch `0024-cif-preserve-stock-pad-pulls-diagnostic.patch` implements
that single default-off switch without adding a register write. The module was
built with networking disabled from a disposable copy of the verified SDK.
`out/phase3-pad-pulls-v45/soph_mipi_rx.ko` has SHA-256
`39c08a6df7ce3ee229450c8c6cf529bbfe6486df9b07819fa7d4250faac99c90`;
its deterministic gzip transfer has SHA-256
`4b4b68be72354a6a6cf3b659928be5040576a9e4fe08961cae9fd9dc0e9d4694`.
The module reports GPL, depends only on `soph_base`, and has target vermagic
`5.10.4-tag- preempt mod_unload riscv`. `image/phase3-pad-pulls-test.exp`
performs the hash checks, confirms `/dev/mem` is absent, records preserved pad
values plus receiver state, and enforces reverse cleanup. Its result is recorded
above. The existing verified
CLEANKVM card was identified as `/dev/disk5` alongside an unrelated 23.8 GB
device and safely ejected without rewriting it.

## Full stock/CleanKVM CSI comparison closes receiver configuration (2026-09-22)

The clean-side diagnostic module and compressed UART payload matched their
recorded SHA-256 values on target. `/dev/mem` remained absent. All six modules
loaded normally, the replacement receiver used device 0, lanes `2,4,3,1,0`,
zero PN swaps, settle 8, clock phase 0, and deskew `0,8,8,3,3`. The diagnostic
then read the same 46 source-identified words immediately after ordinary MIPI
setup.

Every configuration word matched stock exactly, including all CSI mode,
interrupt, lane selection, polarity, settle, deskew force/code, and PHY timing
words. Two observed values differed but are status, not configuration:

- CSI `REG_40` was `0x00010001` in the live stock sample and zero in the clean
  setup-time sample. Pinned fields identify this register as ECC/CRC/WC/FIFO
  and decoded-format status; stock's value means no ECC error and decode format
  1 at that instant.
- PHY4L `STATUS_A8` was `0x60` on stock and `0x20` on clean. Pinned fields
  identify it solely as the live D3 low-power status output. Stock dynamic
  samples already demonstrated that PHY status changes with link timing.

The clean link still diverged dynamically: at T0 ECC was 25 with unknown
decode; at 100 ms ECC/CRC/WC was `2756/1/235` with `yuv422-10`; at five seconds
it was `65536/15/6249`, D3 was `hs_err`, and no frame completed. VI saw one
height-underflow event, zero completed frames, and the probe returned 1. Every
module reverse-unloaded with return code zero, no `soph_*` module or frame
remained, the temporary module was deleted, and taint returned to the initial
4096 left by the earlier rejected-module attempt.

This closes a stock/CleanKVM difference in the complete source-identified CSI
receiver configuration set sampled here. Do not write stock status values into
CleanKVM. The next bounded discriminator is the read-only MIPI pad electrical
state (`0x03001c30` through `0x03001c5f`) before and after normal setup, followed
by physical signal measurement if those controls also match. Video remains
fail-closed.

The completed clean run already supplies its setup transition: for clock/data
lane IDs `2,4,3,1,0`, every P and N pad word changed from `0x48` to `0x40` when
the existing driver cleared only the documented pull-up/pull-down bits. The
stock comparison above shows that stock instead preserves `0x48`.

## Clean full-register diagnostic prepared (2026-09-22)

Tracked patch `0023-cif-full-read-only-register-snapshot.patch` adds a
default-off module parameter that reads the same bounded 46-word CSI, wrapper,
and four-lane PHY set immediately after the driver's ordinary MIPI setup. It
uses only the driver's existing mappings and `_reg_read`; it adds no mapping or
write. The kernel and diagnostic receiver were compiled with networking
disabled from a disposable copy of the verified SDK. The resulting module is
`out/phase3-full-readback-v44/soph_mipi_rx.ko`, SHA-256
`f77f2df5d91accaef31e2ec9e6304838f12527a9b34eff609fd73963d4339a2b`,
with target vermagic `5.10.4-tag- preempt mod_unload riscv`. Its compressed
UART transfer has SHA-256
`acfa93563c820e0f2ac0db5023c092c97c5d7e56aa385bc2b8538e11d40ca70e`.

`image/phase3-full-readback-test.exp` verifies both hashes, requires
`/dev/mem` to remain absent, loads the existing six-module allowlist with the
replacement receiver and stock-equivalent settings, runs one bounded local
probe, records the snapshot, then reverse-unloads and deletes temporary files.
The first requested CleanKVM preflight still reached stock hostname `kvm-44be`;
the script exited at login before uploading anything. Boot CleanKVM recovery
and confirm kernel build `#1` before running this diagnostic.

## Full read-only stock CSI snapshot captured (2026-09-22)

The stock card booted kernel `5.10.4-tag- #46` at its newly assigned address
`192.168.2.5`. The offline-built reader matched its expected SHA-256
`15ead6efb22ed20f1a86da4e4e6be30a7c9cb629d76f20e1f16ed3659484c870`
on target before execution. Two snapshots two seconds apart were identical
across all 46 source-identified CSI control, wrapper, and four-lane PHY words.
The reader was then removed from `/tmp` and its absence verified. It made no
register write.

The six previously compared control words reproduced exactly: CSI control
`REG_04=0x000c0000`; wrapper top `REG_80/84/88 =
0x001b003f/0x08000303/0x00002008`; PHY4L `REG_08=0x00000002` and
`REG_10=0x00000208`. Additional stable stock controls include
`CSI_CTRL_00=0x00000003`, `CSI_CTRL_40=0x00010001`,
`CSI_CTRL_70=0x00000f00`, `PHY4L_04=0x00000134`,
`PHY4L_0C=0x00ff000f`, `PHY4L_20=0x0fff0009`,
`PHY4L_34=0x06000caa`, and `PHY4L_38=0x000000ff`; the remaining captured
control words were zero.

On the first boot, dynamic PHY state was active during both reads:
`DBG_90=0x00000021` and `DBG_94=0x00005555`. A second stock reboot
reproduced every static control and status word exactly, while its first
dynamic sample was idle (`DBG_90=0x00000001`, `DBG_94=0`) and its second was
active (`0x21`/`0x5555`). This is expected sampling of a live link, not a
configuration difference. `STATUS_98/9C=0`, `STATUS_A4=0x20202001`, and
`STATUS_A8=0x00000060` were invariant. `/proc/mipi-rx` reported device 0,
lanes `2,4,3,1,0`, zero PN swaps, `yuv422-8`, zero ECC/CRC/header/WC/FIFO
errors, high-speed clock active, and D2/D3 in high-speed transition. The LT
interrupt count was zero. Baseline stock taint was 4098 with 27 modules and
did not change.

This expands the stock baseline but does not yet identify a causal control
difference. Next boot CleanKVM and collect the same 46 words immediately after
its normal CSI setup using an independently built, default-off read-only module
diagnostic. Separate static control differences from dynamic PHY status before
any driver mutation. Video remains fail-closed.

## Stock userspace LT6911 initialization path closed (2026-09-22)

The stock v1.4.2 root filesystem was extracted and inspected offline. No stock
program or script was executed. `S15kvmhwd` releases GPIO451 high, recreates
the GPIO-backed I2C bus, and uses `i2cdetect` only to distinguish address
`0x2c` from `0x2b`. `S95nanokvm` copies `sensor_cfg.ini.LT` into place and
starts `kvm_system` and `NanoKVM-Server`; it does not invoke the separate EDID
utility.

RISC-V disassembly found exactly two direct calls to the I2C write wrapper in
the stripped `kvm_system` binary. Both belong to the OLED paths: its two I2C
objects are constructed for buses 1 and 5 and write addresses `0x3d` and
`0x3c`. The bridge operations reside in stock `libkvm.so`. Its exported
symbols and strings match the pinned, published `kvm_vision.cpp`. During the
normal LT6911UXC loop that source stops the MCU, selects the documented HDMI
status or CSI-dimension bank, reads status and dimensions, and resumes the
MCU. Firmware reads and EDID programming are separate functions and have no
normal-startup call site. There is no CSI-transmitter setup table or bridge
firmware upload in the ordinary stock runtime.

This closes a hidden stock userspace bridge-initialization sequence as the
stock/clean discriminator. CleanKVM has already reproduced the same published
status path and observed zero HDMI status and zero CSI dimensions. The next
bounded experiment is a read-only stock-versus-clean snapshot of the SoC CSI
receiver configuration and status registers while the same HDMI source is
connected. Compare only source-identified registers, make no stock or clean
register writes, and keep video fail-closed.

The stock-side reader is now prepared as
`image/diagnostics/stock-csi-read.c`. It opens `/dev/mem` read-only and maps
only the source-identified CSI control, wrapper-top, and four-lane PHY words
with `PROT_READ`; it contains no write path and must never be packaged in
CleanKVM. The offline static RISC-V binary SHA-256 is
`15ead6efb22ed20f1a86da4e4e6be30a7c9cb629d76f20e1f16ed3659484c870`.
`image/phase3-stock-csi-snapshot.exp` captures the stock kernel, module,
`/proc/mipi-rx`, interrupt, VI, and receiver-log context without changing
target state.

An initial 2026-09-22 UART preflight found CleanKVM recovery still installed;
the subsequent stock boot and completed capture are recorded above.

## Stock live GPIOE comparison closes GPIO361 mux hypothesis (2026-09-22)

A separate stock reboot on 2026-09-22 reproduced kernel `#46`, receiver
build `#30`, zero ECC/CRC/header/WC errors, GPIO353 input high IRQ, GPIO361
output low, and GPIO451 output high. After SHA-256 verification, the same
read-only helper returned the identical six register values twice, two
seconds apart: muxes `0/0/0`, GPIOE output `0x5`, direction `0x205`, and
input `0x167`. The temporary helper was removed. This confirms the stock
GPIOE0/E2 output-high state persists across a reboot. The offline loader
trace below identifies it as stock U-Boot display setup.

The stock v1.4.2 card booted with kernel `5.10.4-tag- #46` and receiver build
`#30`. During stable `yuv422-8` reception, `/proc/mipi-rx` reported
ECC/CRC/header/WC `0/0/0/0`. GPIO debugfs showed GPIO353 (`LT-INT`) as an
input high with IRQ, GPIO361 (`snsr-rst-gpio`) as output low, and the interrupt
counter at zero. Stock GPIO451 remained output high.

Because stock pinctrl debugfs registers no pin controller, a temporary
source-auditable reader in `image/diagnostics/stock-gpioe-read.c` used the
stock system's existing `/dev/mem` **read-only**, mapping one 4 KiB page at a
time with `PROT_READ` and reading only six fixed 32-bit registers. No register
write or CleanKVM `/dev/mem` exposure was added. A direct `pread` attempt
returned `EFAULT`; the bounded read-only mappings succeeded. The final
offline-built RISC-V reader SHA-256 was
`63119661409a572d160c7014716cac5e720c1ce7f7b49e80db4a76d5631570ab`,
verified on the stock device before use and removed from `/tmp` afterward.

| Register | Stock, HDMI stable | Clean v42 diagnostic |
| --- | --- | --- |
| `PWR_ON` mux, `0x0300109c` | `0` | `0` |
| `PWR_GPIO0` mux, `0x030010a4` | `0` | `0` |
| `PWR_GPIO2` mux, `0x030010ac` | `0` | `0` |
| GPIOE output, `0x05021000` | `0x00000005` | `0x00000000` |
| GPIOE direction, `0x05021004` | `0x00000205` | `0x00000000` at module probe; `0x00000200` after the reset diagnostic |
| GPIOE input, `0x05021050` | `0x00000167`/`0x00000567` | `0x00000162` at module probe; previously `0x00000562`/`0x00000162` |

Both stock and clean leave the GPIO361/`PWR_ON` pad at mux function 0 with
GPIOE9 output direction, low output latch, and low sampled input. Therefore
the GPIO361 mux is **not** the stock/clean video discriminator. The toggling
input-word bit 10 is independent of GPIOE9. Stock additionally drives GPIOE0
and GPIOE2 high as outputs, with mux function 0 on both; clean v42 showed
those GPIOE bits low as inputs. The offline loader trace below identifies the
source as the stock U-Boot display driver. This state difference is unrelated
to CSI receive and is closed as a video discriminator. Resume the bounded
CSI initialization comparison. Video remains fail-closed.

### Clean read-only GPIOE boot snapshot (2026-09-22)

Tracked patch `0022-cif-log-read-only-gpioe-boot-state.patch` adds one
probe-time diagnostic that maps and reads the three mux words and GPIOE
output, direction, and sampled-input words. It contains no GPIO or register
write. The module was built offline with networking disabled. Its SHA-256 is
`40b960a3abb7440aee63b4204334b6ce4ae3e233d35b6a6c7eb367befb1d845e`;
the transferred gzip SHA-256 is
`1592a39d419bbf9ef10a8ec8658200bd7a2ddc82616ede6021f040613ea2e29d`.

The v42 baseline had zero modules, taint zero, and no `/dev/mem`. After hash
verification, loading only `soph_sys`, `soph_base`, and the diagnostic
receiver produced muxes `0/0/0`, GPIOE output `0`, direction `0`, and sampled
input `0x162`. All three modules unloaded, `/proc/modules` returned to zero,
the temporary module was removed, and taint was the expected `4096`. This
confirms the stock/clean difference exists before the clean reset ioctl:
stock boot establishes GPIOE0/E2 output-high and GPIOE9 output-low, whereas
CleanKVM leaves all three as inputs with low output latches. The shared mux
values rule out a mux difference. The owner identified below removes the need
for an output experiment.

### GPIOE0/E2 owner identified and hypothesis closed (2026-09-22)

The stock and v42 FIPs were parsed offline with the pinned CVITEK FIP tool.
Their LZMA `LOADER_2ND` payloads were decompressed as data and disassembled;
no stock binary or script was executed. Stock loader SHA-256 remains
`5dde20754744ec177447340e6e849f4c669d9908d392f3da0526ef4b5af4ec65`;
v42 is `74633e501bbb4bf91aa574ec288a97a56e38e1676cdc877841f28d35563f097d`.

Both embedded U-Boot device trees contain the same `cvitek,vo` node. It maps
`reset-gpio = <&gpio4 0 GPIO_ACTIVE_LOW>` and
`pwm-gpio = <&gpio4 2 GPIO_ACTIVE_LOW>`, which are GPIOE0 and GPIOE2. Pinned
source `drivers/video/cvitek/cvi_disp.c` requests both with
`GPIOD_IS_OUT | GPIOD_IS_OUT_ACTIVE`; the MIPI/LVDS display paths subsequently
set their logical values. The matching warning and display-GPIO strings exist
in the stock loader and are absent from v42. CleanKVM's verified U-Boot
fragment explicitly disables `CONFIG_DM_VIDEO` and `CONFIG_VIDEO_CVITEK`.

This explains the live observation: stock U-Boot probes the display path and
leaves E0/E2 as output-high, while reduced CleanKVM U-Boot does not bind that
driver and Linux initially sees both as inputs. The offline-extracted stock
rootfs startup scripts contain no E0/E2 operation. These pins are display
reset and PWM controls in the shared boot device tree, not LT6911 or CSI
receive controls. Do not reproduce this unused display initialization in
CleanKVM. Close GPIOE0/E2 as the no-frame cause and return to a bounded
stock/clean CSI initialization comparison.

## GPIOE9 mux discriminator completed on v42 (2026-09-21)

Tracked patch `0021-cif-read-pwr-on-gpioe-reset-state.patch` changes only
default-off CIF reset telemetry: it reads the source-identified `PWR_ON` mux
register `0x0300109c`, GPIOE registers at `0x05021000`, and the return from
the existing `gpio_direction_output` operation. It adds no register write.
The offline-built temporary receiver module SHA-256 is
`f0560a842d666c1ed67373c18714b979a5f726096f594ccc18e13e7e5f3ce17d`;
target vermagic is `5.10.4-tag- preempt mod_unload riscv`. Its compressed
transfer and the audited v35 reset probe both matched their host hashes on
the running v42 device.

One bounded reset-sequence run showed:

| Step | `PWR_ON` mux | GPIOE output | GPIOE direction | GPIOE sampled input | GPIO361 read | direction return |
| --- | --- | --- | --- | --- | --- | --- |
| Before assert | `0` | `0x00000000` | `0x00000200` | `0x00000562` | — | — |
| After assert | `0` | `0x00000200` | `0x00000200` | `0x00000162` | 0 | 0 |
| Before release | `0` | `0x00000200` | `0x00000200` | `0x00000562` | — | — |
| After release | `0` | `0x00000000` | `0x00000200` | `0x00000562` | 0 | 0 |

GPIOE9's output latch (bit 9) changed low-high-low and direction remained
output, but its sampled input bit 9 stayed low. The `PWR_ON` mux remained
function 0 throughout, while pinned U-Boot definitions assign GPIOE9 mode 3.
The changes in the sampled input word are bit 10, not bit 9. The normal GPIO
calls returned success; they did not make a visible GPIOE9 pad pulse. This
explains the v42 GPIO361 readback. The stock comparison above shows the same
mux value, so it is not the video discriminator; the board electrical
connection remains unmeasured.

The diagnostic receiver's ECC/CRC/WC reached `65536/12/2474` by two seconds;
no video admission followed. All six modules unloaded with return code 0,
the two temporary binaries and frame path were removed, and taint remained
4096 from the earlier module load. The subsequent stock read-only comparison
is recorded above. Do not set mux mode 3: stock also leaves it at 0.
Video remains fail-closed.

## Stock GPIO361 pinmux and software-path audit (2026-09-21)

Offline inspection of the stock v1.4.2 root filesystem and matching CIF source
separates three paths. CIF's GPIO353 `LT-INT` edge handler samples the pin and
updates `/proc/lt_int`; it does not write GPIO, pinmux, or LT6911 registers.
The same stock CIF change hardcodes GPIO361 as `snsr-rst-gpio`; only the
`CVI_MIPI_RESET_SENSOR` and `CVI_MIPI_UNRESET_SENSOR` ioctl paths call
`gpio_direction_output` for it. The stock `S15kvmhwd` script instead exports
GPIO451 (`PCIe_HDMI_RST_Pin`), sets it as output, and leaves it high. It does
not operate GPIO353 or GPIO361.

The SG200x board DTS maps port E to `0x05021000`. GPIO361 is port E line 9,
whose pad is named `PWR_ON`; the pinned U-Boot pin-function table assigns
`PWR_ON` function 0 and `PWR_GPIO_9` function 3 at pinmux register
`0x0300109c`. The Nano board initialization source has no explicit write to
that register. This makes a missing GPIO mux a concrete hypothesis, not a
proven stock/clean difference: the shipped stock FIP loader is packed. The
later live comparison above resolved the mux value as 0 on both images.
Existing reset telemetry reads GPIOB
registers and ignores the return from `gpio_direction_output`, so it cannot
resolve the hypothesis.

The stock `kvm_system` and `NanoKVM-Server` binaries were extracted as data
from the stock image and inspected for GPIO/MIPI paths without execution.
The server exposes GPIO503/504/505/507 paths; simple string inspection found
no GPIO353/361 or direct MIPI device path in either binary. This does not rule
out indirect calls through linked code. The stock CIF source and observed
module behavior provide the stronger evidence for the interrupt/reset paths.

The running CleanKVM kernel has `/dev/gpiochip0` through `4` but no sysfs GPIO
or debugfs mountpoint; `/dev/mem` remains absent. A read-only GPIO line-info
query can identify ownership, but direct mux and GPIOE register values require
a bounded kernel diagnostic or a later stock runtime capture. Next implement
a default-off diagnostic that checks `gpio_direction_output` return status and
reads `0x0300109c` plus GPIOE registers at `0x05021000` before and after an
existing reset ioctl. Do not write pinmux registers or infer a physical pulse
from `gpio_get_value` alone. Video remains fail-closed.

## v41 boot result and corrected reset polarity v42 (2026-09-21)

### v42 boot and bounded reset-sequence result

v42 booted with FIT kernel and DTB hash checks passing. The live CIF property
was `7 9 0`; initial taint was 0, no `soph_*` module was loaded, and no frame
existed. All six quarantined modules loaded successfully, and CIF logged
`rst_pin = 361, pol = 0`. The previously audited v35 reset-sequence probe
(SHA-256 `4f461d8f483f777d5674cc7c5fdcb2ac29092630bea143244ca5af847d76cc2a`)
was transferred to `/tmp` and verified by device-side SHA-256. It ran once on
profile 0 with the source-exact sensor reset/MIPI/clock sequence. The driver's
reset telemetry logged `on=1` then `on=0`, but `gpio_get_value(GPIO361)` was
0 after **both** operations. Thus a physical high-low pulse was not proven.
The older diagnostic reads GPIOB at `0x03021000`, whereas the board DTS maps
port E to `0x05021000`; its raw bank trace cannot establish GPIO361 pad state.

Receiver ECC/CRC/WC were `68/1/8` at T0, `4483/7/496` at 100 ms, and
`65536/73/6532` at two seconds. D3 entered `hs_err`; VI produced no frame.
All six modules unloaded with return code 0, the temporary probe and frame
were removed, and final taint was 4096. Video remains fail-closed. Before
another pulse experiment, inspect the GPIO361 controller mapping, pinmux,
`gpio_direction_output` return value, and a bank-correct output/readback path
using bounded, source-backed diagnostics. Do not treat this test as proof that
reset routing or an actual pulse has been reproduced.

### v41 no-pulse result and v42 build

v41 booted normally with kernel `5.10.4-tag- #1`, initial taint 0, and no
`soph_*` modules or captured frame. The live CIF property was phandle 7,
offset 9, flag 1. Its receiver requested GPIO361 with polarity 1. A bounded
stock-profile probe without a reset pulse used lane phases `0,8,8,3,3`,
clock phase 0, settle 8, PN swaps `0,0,0,0,0`, and lanes `2,4,3,1,0`.
ECC/CRC/WC rose from `30/0/15` at T0 to `4348/6/624` at 100 ms and
`65536/168/8875` at five seconds. VI saw zero lines, no frame was created,
and the probe returned 1. All six modules unloaded; final taint was 4096.
This closes the hypothesis that GPIO361 routing alone fixes the link.

Corresponding stock source hardcodes GPIO361 but leaves `snsr_rst_pol` at its
zero initialization; stock logs polarity 0 and drives GPIO361 low at rest.
v41's active-low flag would invert a reset pulse. Patch `0020` now uses
`GPIO_ACTIVE_HIGH`, retaining GPIO361 with stock-equivalent polarity 0.
The corrected DTB has `/cif/snsr-reset` `7 9 0` and SHA-256
`e4ee9d88f4f276cf9edabdee0e5942ff05b8c5c7eecd4a040ab49f630afda928`.
The offline-built v42 FIT SHA-256 is
`c54ee63e57f2128d419c970602c10df8ef1230537a7f65381ceada243bb14b4b`.
The assembled, embedded-artifact-verified v42 image is
`out/phase3-reset-polarity-v42/cleankvm-phase1.img`, 79,692,288 bytes,
SHA-256 `523da9612c301d556dbb06133e0650e91e449c7ae5896e7ca957fec2f78d193a`.
It reuses the audited v40 rootfs and FIP. On 2026-09-21 the resolver uniquely
identified `/dev/disk4` as the removable 31.0 GB card with the expected 16.8 MB
FAT and 62.9 MB Linux partitions. Exactly 79,692,288 image bytes were written.
The exact-span read-back SHA-256 was
`ad68fed9cc514532126c1594069ac613dd30cb713a68a4c4f1c573539a115584`;
macOS FAT bookkeeping changed the whole-span digest, while the embedded FIT,
rootfs, and FIP matched. The card was ejected. Next boot v42 and confirm
GPIO361 polarity 0 before one bounded source-exact reset-pulse experiment.
Video remains fail-closed.

## Stock clock comparison and reset-routing image v41 (2026-09-20)

The running stock v1.4.2 image was inspected read-only over the established SSH
login. Kernel `5.10.4-tag- #46` had taint 4098 and receiver build `#30`. Three
receiver samples remained `yuv422-8` with ECC/CRC/header/FIFO `0/0/0/0` and
one word-count error. Stock debugfs reported MIPI PLL 900 MHz, display PLL
1,187,999,988 Hz, FPLL 1.5 GHz, VIP SYS2 593,999,994 Hz, and CSI MAC0
300 MHz. These match the CleanKVM clock diagnostic rates exactly, closing the
clock-rate mismatch hypothesis.

The stock boot log reported `SYS_CMD_INFO_LINUX_INIT_DONE fail` and
`communicate with rtos fail` before CIF probe. A successful RTOS handshake is
therefore not required for the observed stock low-error state. Stock's LT-INT
IRQ count was zero. Offline disassembly and local SDK history identify exact
corresponding source commit `d4af3c62f` (`Modify cif.c for NanoKVM`): it assigns
GPIO353 to a passive LT interrupt/counter and replaces the device-tree reset
lookup with GPIO361. The retained stock module hash remains
`d40419159a4065de8b7693e0de59c477de36373f24fe164e50afb37d674ed755`.
Its interrupt handler only logs, samples GPIO353, and increments the `/proc/lt_int`
counter; it does not program CIF or LT registers.

Patch `0020-board-map-cif-reset-to-stock-gpio.patch` expresses the stock reset
routing in auditable board source by changing `snsr-reset` from port E line 1
to port E line 9. It does not add the stock world-writable proc node or interrupt
handler. A fresh verified SDK volume applied the complete patch chain and built
the kernel/device tree offline. The repository-owned packager verified the FIT
and embedded artifacts:

- Image: `out/phase3-reset-routing-v41/cleankvm-phase1.img`
- Image SHA-256: `99e1e64cc56958303baaa538f68d993d2d4f942536257a68d06d20445e55d5fb`
- Size: 79,692,288 bytes
- FIT SHA-256: `29fabec01f957246c17bdcd6f930b8a5fcb1034ab5e9e25c9c5ee7221e1f44ed`
- DTB SHA-256: `db6c8f1b382c75d62d367c3dba850773ba45a5f93b18e4accd67f2af5f7f95ed`
- DTB `/cif/snsr-reset`: phandle 7, offset 9, active-low
- Reused audited v40 rootfs: `9edec1e81b1b3b3e45654a645f9aebfcb98ce636e37f0a1cd1261e43d58c7751`
- Reused audited FIP: `96a0b414c6ace8c9911ae4fb140893290706230162099be2daf4ef370fdc05f1`

The resolver found two external devices. `/dev/disk4` was a separate 23.8 GB
ExFAT USB device and was not touched. `/dev/disk5` was the expected 31.0 GB
CleanKVM card, identified by its 16.8 MB FAT and 62.9 MB Linux layout. After
unmount, exactly 79,692,288 bytes of v41 were written. The exact-span raw
read-back SHA-256 was
`ba416a988edac7a51925c0d91784485f2d40ca1caa0e174f6f641de48c188cf2`;
macOS FAT bookkeeping changed the whole-span hash, while the embedded FIT,
rootfs, and FIP each matched their audited artifacts. The card was force-unmounted
only after verification and safely ejected.

The planned v41 no-pulse test was completed on 2026-09-21 as recorded above.
Its active-low polarity requires correction before a reset-pulse test.

## Current stock runtime handoff (2026-09-20)

The verified stock v1.4.2 card is now installed and the stock image is booting.
Use the previously established stock SSH login and password for the next
read-only comparison; the credential itself remains outside Git and is not
reproduced in this record. HDMI and UART should remain attached. The immediate
experiment is the stock-side clock/reset snapshot described below after HDMI
lock. Do not copy a stock executable or module into CleanKVM.

## Clock/reset readback and reset-GPIO lifetime test (2026-09-20)

Patches `0017` through `0019` add a default-off clock/reset snapshot, log the
existing sensor-reset GPIO lookup/request result, and replace the unmanaged
`gpio_request` with `devm_gpio_request`. The final offline-built receiver module
is `out/phase3-clock-reset-readback-v4/soph_mipi_rx.ko`, SHA-256
`b174b976734cffca6c2129ccdd2147e4c40d9796ed32702049262985ac767bba`,
with target vermagic `5.10.4-tag- preempt mod_unload riscv`.

After a fresh boot cleared reservations leaked by the earlier diagnostic
versions, two consecutive bounded runs both requested sensor-reset GPIO 353
successfully. The second success without reboot proves the device-managed
request is released on module unload. Both runs reported identical state:

- VIP clock selector/control `0x1c`: `0x01300221`
- SYS2 clock: `593999994` Hz; parent: `1187999988` Hz
- MIPI PLL: `900000000` Hz; display PLL: `1187999988` Hz; FPLL: `1500000000` Hz
- selected CSI MAC enum: `4`
- PHY and PHY-APB reset status: `-524` (`-ENOTSUPP`)

The reset handles are present. The status result reflects the pinned CVITEK
reset controller's lack of a `.status` callback, so it is not evidence of a
missing reset. All six receiver control words still matched stock. Run A had
ECC/CRC/WC `2504/0/303` at 100 ms and `65536/15/8994` finally; run B had
`2473/0/384` and `65536/23/9021`. Both produced zero lines and no frame. All
six loads and reverse unloads returned zero in both runs, no `soph_*` module
remained, the volatile module was removed, and final taint was 4096.

Live device-tree inspection resolved `cif/snsr-reset` to phandle 7, port E line
1, GPIO353. The verified pinned Nano SDK source declares the same value, so
this is not a stale FIT. The stock startup script's GPIO451 (port B line 3) is
a separate board-level control path; earlier records must not describe it as
the CIF `snsr-reset` property. Existing hardware evidence shows that pulsing
GPIO353 restarts the LT6911 link, while stock startup holds GPIO451 high. The
relationship between these two controls remains unresolved, and neither pin
should be changed without corresponding source evidence and a bounded
hypothesis.

The next discriminating comparison is the same read-only clock/reset snapshot
on a stock boot after HDMI lock, followed by source attribution of any
difference. Do not load RTOS/fast-image modules into CleanKVM or alter either
GPIO on the current evidence. Video remains fail-closed.

## Six-word clean readback completed (2026-09-20)

The default-off `0016` CIF diagnostic reads six control words through existing
driver mappings after normal CSI interrupt unmask; it adds no register write,
mapping, or broad memory interface. The rebuilt development builder ID is
`63dfec0366d6b30d5bc36ea478a1025026c3ef89ed60e7122334635ac1e8bc68`.
The fresh case-sensitive SDK volume passed snapshot verification. The pinned
host-tools commit, musl tree ID, and GCC binary hash matched `builder.lock`;
the historical subtree tar digest did not match a newly generated archive and
needs provenance follow-up. The disposable work volume passed Phase 1, HID,
and video-foundation validation. Kernel and receiver compilation ran offline.

The final module `out/phase3-control-readback-v2/soph_mipi_rx.ko` has SHA-256
`0ef652f0cbcaf2d0c78d3d71c12dbbf2523fc76d79fe8f8d8602e4194bc3cd59`,
GPL metadata, `soph_base` dependency, and target vermagic
`5.10.4-tag- preempt mod_unload riscv`. Its volatile UART copy matched that
hash. The initial v1 snapshot was taken before normal CSI unmask and therefore
showed low-byte `ff`; moving the read immediately after unmask corrected this
measurement timing. With stock-equivalent deskew `0,8,8,3,3`, settle 8, clock
phase 0, PN swaps all zero, device 0, and lanes `2,4,3,1,0`, the clean
readback matched all six stock control words exactly:

| Word | Stock and CleanKVM |
| --- | --- |
| PHY4L `REG_08` | `0x00000002` |
| PHY4L `REG_10` | `0x00000208` |
| PHY-TOP `REG_80` | `0x001b003f` |
| PHY-TOP `REG_84` | `0x08000303` |
| PHY-TOP `REG_88` | `0x00002008` |
| CSI-MAC `REG_04` | `0x000c0000` |

Yet at 100 ms ECC/CRC/WC reached `2405/1/310`, with D3 `hs_err`; at five
seconds ECC reached 65536 and WC 9009. VI reported zero-line height underflow,
the probe returned 1, and no frame appeared. All six loads and reverse unloads
returned 0, no `soph_*` module remained, final taint equaled initial 4096, and
the frame file was absent. Thus direct receiver control-word configuration is
no longer the explanation for the stock/clean link difference. The next
bounded discriminator is read-only clock/reset and initialization-order state,
including possible RTOS-mediated prerequisites. Do not load stock modules or
open video admission on this evidence. Both volatile test module copies were
removed from `/tmp` and their absence verified.

## Prerequisite source audit and next discriminator (2026-09-20)

The two stock-only modules are not passive aliases for CIF configuration.
Pinned `rtos_cmdqu.c` registers the mailbox IRQ and initializes its hardware
spinlock; pinned `fast_image.c` sends `SYS_CMD_INFO_LINUX_INIT_DONE` and waits
for `SYS_CMD_INFO_RTOS_INIT_DONE`, then maps/allocates MCU shared buffers.
No direct CIF settle, lane-deskew or CSI clock-phase write was found in these
sources. This makes an RTOS-mediated prerequisite plausible, not established;
do not load the modules into CleanKVM merely because stock loads them.

CleanKVM has no readable `/sys/kernel/debug/clk/clk_summary` or
`/proc/clk/clk_summary`, and no video modules remained after the last test
(taint 4096). The next bounded diagnostic is an independently built,
default-off CIF readback that uses the driver's already mapped MMIO regions
to report the same six control words measured on stock immediately after
MIPI attribute setup and streaming enable: PHY4L `REG_08/REG_10`, PHY-TOP
`REG_80/REG_84/REG_88`, and CSI-MAC `REG_04`. It must make no extra register
write, expose no broad physical-memory interface, and retain normal load,
timeout, reverse-unload, taint and frame-absence gates. Only if the live
control words match stock should the inquiry move to clock/reset state and
the RTOS handshake.

The readback and its hardware result are recorded above; the earlier Podman
store issue was resolved by rebuilding the development environment.

## Corrected stock-equivalent clean test (2026-09-20)

After the SD-card switch, CleanKVM booted with kernel `5.10.4-tag-`, initial
taint 0, no `soph_*` modules or audio node, and probe SHA-256
`151287655f54f42f34b5c70ea73d27c58a48e2c54bc1f94374d5eee40846be45`.
The offline-built per-lane module was transferred to volatile `/tmp` and
verified at SHA-256
`9dd7410b4ae9651d098a3d0a82f0d9aba90878e8c50c628f92740ec84ae38ef6`.

One bounded run used the corrected stock-equivalent control settings:
`lane_phase=0,8,8,3,3`, `csi_ck_phase=0`, `hs_settle=8`,
`pn_swap=0,0,0,0,0`, device 0, lanes `2,4,3,1,0`. At T0 ECC/CRC/WC was
`35/0/28` with unknown decode; at T100ms it was `2763/65/1741`, decoded
`raw8`, and D3 was idle. VI saw a height-underflow at zero lines and no FE0
or frame. At 5 seconds ECC masked at 65536, CRC 1400, WC 38521, decode
`raw10`. The probe failed to acquire a frame (`0xc00e8040`). All six modules
loaded/unloaded RC 0; no `soph_*` modules or frame remained, final taint 4096.
The source-backed LT status check afterward reported HDMI stable `0x55`,
1920×1080, MCU resume RC 0 and no open I2C FD.

Thus the previously missed deskew translation is corrected, but the clean
link still does not match stock's near-zero counters. Do not sweep settle or
bypass errors on this evidence. Next discriminate stock prerequisite clock/
reset and initialization order from the clean runtime, using source-backed
read-only telemetry. Video remains fail-closed.

## Stock comparison card prepared (2026-09-20)

The sole attached removable USB card was resolved as `/dev/disk4` (32.0 GB,
pre-write CLEANKVM 16.8 MB FAT / 62.9 MB Linux layout). The retained stock
v1.4.2 archive passed `xz -t`, and decompression SHA-256 matched the previously
qualified image `23648088de59e043a1289c49c5103f829dc652604f8dd76db22dd8ff7aca6539`.
After unmount, exactly 1,627,390,464 source bytes were written. The card then
showed stock 16.8 MB FAT / 1.6 GB Linux layout. Its complete Linux partition
read back at SHA-256
`c285650ee41892c0cef5fbc371d3b13828f886a3ebb82a400ff06bbec94585a4`,
matching the retained stock source partition. The card was safely ejected and
has since been installed in the NanoKVM. The stock image is now booting; the previously established stock SSH login and
password apply. Capture only read-only stock receiver/clock state after HDMI
lock. Do not copy or load stock executables/modules into CleanKVM.

## Stock LT settle comparison (2026-09-19)

The SDK LT6911 profile specifies D-PHY `enable=1`, `hs_settle=8`, but that
setting alone did not reproduce the stock receiver's near-zero link errors.
With independently built patch-0014 MIPI module (SHA-256
`9dd7410b4ae9651d098a3d0a82f0d9aba90878e8c50c628f92740ec84ae38ef6`):

- Tuned lane/polarity/clock settings and settle 8: T0 ECC/CRC/WC `102/0/0`;
  T100ms `3689/0/166`, decode `raw8`; no line or frame completed. ECC masked
  at 65536, with WC 3038 and CRC 0.
- Stock-profile polarity and clock phase, but incorrectly translated per-lane deskew
  `0,3,3,8,8`, settle 8: T0 `41/0/0`; T100ms `4064/0/168`, decode `raw12`;
  no line or frame completed. ECC masked at 65536, WC 2711, CRC 8.
- Both runs used the unchanged standard interrupt mask; all six video modules
  loaded/unloaded RC 0, no `soph_*` remained, no frame file existed, and final
  kernel taint was 4096. Video remains fail-closed.

Patch 0014 interprets `lane_phase[0..4]` as individual clock/D0/D1/D2/D3
codes. Correction from live stock registers: the old grouped default
`{0,3,8,0,3}` translates to `0,8,8,3,3`, **not** `0,3,3,8,8`.
`LANE_IS_PORT1(x)` is true for physical IDs 3–5: clock 2 and data 1/0
share Port 0 (near code 3); data 4/3 are Port 1 (far code 8).
The earlier settle-8 run labeled “stock-profile” did not test stock deskew.
The stock LT header also declares `devno=1`, whereas the observed stock runtime
selected device 0, so the header is evidence for settle 8, not a complete
description of runtime state. Do not infer that settle 8 is the whole stock
configuration or sweep it further without a new discriminating hypothesis.

Next: obtain a read-only, same-register stock/clean comparison after HDMI lock
(D-PHY settle readback, lane deskew/force codes, polarity, clock phase, CSI
interrupt mask/status and relevant clock/reset state). Attribute the exact
runtime receiver differences before another driver mutation. Preserve the
stock artifacts as offline evidence only; do not load vendor binaries into
CleanKVM.

### Live stock receiver baseline (2026-09-20)

Stock boot reached SSH and authenticated web UI at `192.168.2.5`. Receiver
module build #30 reports device 0, lanes `2,4,3,1,0`, zero PN swaps,
`yuv422-8`, ECC/CRC/HDR/FIFO `0`, WC `1` through five 1-second samples.
Clock/lane states fluctuate between high-speed transfer and idle; web video
is black and UART reports image-read failures, so this does not demonstrate
successful stock frame capture in the present setup.

Six exact read-only 32-bit control register reads, addresses mapped from pinned
CIF source, returned: PHY4L `REG_08` `0x0a0d0308=0x00000002` (clock selection
2, PN swaps 0, clock phase 0); `REG_10` `0x0a0d0310=0x00000208` (settle 8);
PHY-TOP `REG_80` `0x0a0d0080=0x001b003f` (force deskew physical lanes
0,1,3,4, not 2), `REG_84` `0x0a0d0084=0x08000303` (physical codes 0..3 =
3,3,0,8), `REG_88` `0x0a0d0088=0x00002008` (physical code 4 = 8);
CSI MAC `REG_04` `0x0a0c2404=0x000c0000` (interrupt mask low byte 0).
No register was written. Compare the same control words on CleanKVM, then
test `lane_phase=0,8,8,3,3` under patch 0014 with settle 8, zero PN swaps,
and clock phase 0. The prior swapped-deskew test is not that control.

- Per-lane PHY deskew decoupling and single-variable sweeps on target hardware
  confirmed independent physical lane behavior (`image/patches/0014-cif-per-lane-deskew-diagnostic.patch`):
  - Port 0 Near Data Lanes: `D2=11` and `D3=12` (`lane_phase[3]=11`, `lane_phase[4]=12`)
    is strictly optimal (collapsed at D2=10/13 and D3=11/13).
  - Port 1 Far Data Lanes: unforced code 0 (`D0=0, D1=0`) is optimal (forcing D0=1 collapsed
    reception to 12 lines; forcing D1=1/2 collapsed to 75 and 16 lines).
  - Physical Clock Lane: setting `CLK=1` (`lane_phase[0]=1`) unlocked a new all-time record of
    **229 complete active video lines and 441,087 pixels** into `CSIBDG_A`. Setting `CLK=2`
    collapsed reception to 11 lines.
  - Settle Dynamics: `hs_settle=20` confirmed >80% CRC reduction (1,608 total), but truncates
    line streaming at 1 line. `hs_settle=16` remains required for multi-line streaming.
- Alternating Repeat Verification of Clock Deskew 0 vs 1:
  - A 3-pair alternating sequence confirmed a >2.4x upward shift in line acquisition distribution:
    - **Clock Code 0**: Mean **60.0 lines** (52, 57, 71 lines; all terminated via `frm height less than setting`).
    - **Clock Code 1**: Mean **146.3 lines** (133, 77, 229 lines; terminated via `frm width less than setting`).
  - Proved that the 152-line capture was not an outlier but part of a systematically superior distribution.
- Current Optimal Configuration Baseline:
  - `lane_phase=1,0,0,11,12` (`CLK=1, D0=0, D1=0, D2=11, D3=12`)
  - `csi_ck_phase=8`
  - `hs_settle=16`
  - `pn_swap=0,0,1,0,0`
- Frame Acquisition Termination Analysis:
  1. `CSIBDG_A CH0 frm width less than setting(1920)`: bit errors on the high-speed link
     prematurely truncate line packets, triggering `_vi_err_handler()` to drop the frame buffer
     and re-trigger preraw.
  2. CIF Software Interrupt Mask (`cif.c:2763-2777`): 1-bit ECC errors (correctable by Hamming code)
     accumulate at ~35,000/sec, overflowing the 16-bit software counter at 1.8s and writing `0xFF`
     to `CSI_INTR_MASK`.
- Next bounded steps:
  1. Prepare a time-limited, non-default diagnostic patch (0015) for CIF interrupt masking:
     evaluate whether masking only fatal error interrupts (or capping `errcnt_ecc` at `0xFFFF` without
     writing `0xFF` to `CSI_INTR_MASK`) allows continuous acquisition beyond 1.8 seconds.
  2. Inspect VI frame width error handling: evaluate whether isolated width errors in individual lines
     can be tolerated without discarding the full frame buffer during diagnostic acquisition.
  3. Keep video admission strictly closed until 1080 lines, `FRAME_DONE_FE0`, and a hashed
     4,147,200-byte planar-YUV422 frame are verified on target hardware, followed by clean
     reverse unload (`RC=0`, empty `/proc/modules`, taint 4096).

## Historical state and investigation timeline

- Independent boot, UART, Ethernet DHCP, Rust services, authenticated MCP, and
  fail-closed HID have passed their current development gates.
- The six quarantined video modules load through normal kernel version checks
  and unload cleanly. Generic media/audio exposure remains absent.
- LT6911 control responds only on I2C bus 4/address `0x2b`, but protected ID
  reads return `0000` before, during, and after a proven reset pulse.
- GPIO451 is confirmed as active-low GPIOB3 on ADC1. Its mux and sampled low/high
  reset behavior are correct.
- All ten MIPI RX pads are now assigned function 3 in U-Boot. Removing the
  conflicting CAM_MCLK1 assignment did not restore CSI activity.
- v21 accepted the generic SDK LT6911 configuration but reported every physical
  sample zero, all data lanes `hs_idle`, no high-speed clock, and unknown decode
  format.
- Offline stock-image inspection recovered the product-specific
  `sensor_cfg.ini.LT`: MIPI device 0, lane order `2,4,3,1,0`, no PN swaps, bus
  4, and no sensor MCLK. The SDK parser copies these values directly into its
  runtime sensor configuration. v21 instead used the generic adapter defaults
  for MIPI device 1, lanes `2,0,1,3,4`, and all PN swaps enabled.
- The stock hardware-init script only releases GPIO451 high and reloads the
  GPIO-I2C modules. It does not load bridge firmware, program EDID, or pulse
  reset. The later startup wrapper selects the LT config and starts opaque
  services; strings in `kvm_system` show hardware-version detection but no
  evidence that it automatically invokes the separate EDID updater.
- v22 booted with the exact stock product profile. The receiver accepted device
  0, lanes `2,4,3,1,0`, and zero PN swaps, but all physical samples remained
  zero, all data lanes remained `hs_idle`, `CK_HS` remained zero, and decode
  remained unknown. The product-profile mismatch is therefore ruled out.
- The remaining fault is constrained upstream of the SoC receiver: LT6911
  power/firmware state, HDMI input/HPD/EDID state, or the physical CSI path.
- v23 preserved the bridge's power-on state, performed no I2C transaction or
  reset pulse, and waited five seconds; CSI remained identically idle. Reset
  sequencing and the 500 ms v22 delay are therefore ruled out.
- macOS display enumeration while the HDMI cable was connected showed only the
  built-in display and `S34CG50`; NanoKVM did not appear as a display. This is
  direct host-side evidence that the device is not presenting usable HPD/EDID.
- Sipeed has now published the complete GPL-3.0 EDID utility source. Review of
  commit `bd070b2f5ce27d22618e8a1c8d2bdca543429882` closed the opaque-protocol
  blocker and identified the exact LT6911UXC programming sequence.
- CleanKVM v25 contains an independently compiled, bounded derivative for only
  `/dev/i2c-4`, address `0x2b`, and its embedded checksum-validated E21 payload.
  It is root-only, never starts automatically, reads before writing, requires
  `--program-e21`, verifies all 256 bytes, and resumes the MCU on exit.

## Historical next-work plan (superseded by the current checkpoint)

1. Boot v26 and run its bounded LT flash backup once. Preserve and hash the
   resulting 800,000-byte dump before any further bridge mutation, then map its
   structure/version indicators against the published stock read routine.
2. Determine whether reset release starts the embedded MCU and whether HPD has
   a separately documented control/status bit.
3. Only after the bridge emits a CSI clock should bounded VI allocation and
   one-frame copying be re-enabled.
4. Remove temporary reset telemetry after the diagnosis is closed and rebuild
   all admitted modules from the final source tree.
5. Continue Phase 3a separately: inventory the ADC/DAC paths, confirm actual
   microphone/speaker/HDMI-audio routing, and design bounded opt-in brokers.

The former upstream-source blocker is closed. Sipeed's published GPL-3.0 source
is pinned in `image/sources.lock`; the retained stock executable remains excluded.

Phase 3a evidence and its staged privacy boundary are now specified in
`docs/PHASE3A_AUDIO_ADMISSION.md`. The next independent build task is a minimal
enumeration-only audio profile and validator. USB Audio, PDM, external I2S, and
all PCM access remain unauthorized pending a separate hardware authorization.

The retained `E21_NanoKVM.bin` has passed offline structural validation: it is
exactly 256 bytes, begins with the standard EDID header, declares one extension
block, and both 128-byte checksums equal zero. Its SHA-256 is
`526e33a7d63d8deddbcbb00bc3b89a3a88fae1059f8d16a5b5b3fbd28742146e`.
It contains `NanoKVM` display strings. This validates the payload, not the opaque
programming procedure.

## Explicitly deferred

- Guessing or copying an opaque LT6911 register table.
- Enabling video or audio through MCP before local hardware admission.
- Restoring arbitrary physical-memory access or executing stock utilities.
- Treating the `0000` ID as proof of a specific silicon failure without a
  stock-state comparison.

## Prepared next image

- Path: `out/phase3-edid-v25/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256:
  `e9c18dfbadc571a33fea56310a811685a005e5d92784e9c6e4d538993bab33e4`
- Embedded root-only programmer SHA-256:
  `3f8b4ced84e4445a25c8a058e3d7e194f01af5b757021206e0367c45d0db0195`.
  It is mode `0500`, owned by root, and has no init entry.
- v24 was stopped before bridge mutation when its UART baseline revealed that
  packaging had inherited the prior audio kernel and lacked `/dev/i2c-4`.
  v25 combines the already verified v23 video FIT with the audited new rootfs.
- The resolved 32.0 GB external card was flashed with v25, exactly read back, and safely
  ejected. Boot FIT, rootfs, and FIP matched the audited artifacts. macOS FAT
  bookkeeping changed the whole read-back hash to
  `60050eca74c28d874f499a177f3a2f30a86069d68297e08a1d6fc345969959fe`;
  embedded-artifact verification passed and is authoritative.
- The controlled v25 boot passed the video-profile gate with taint zero,
  `/dev/i2c-4` root-only, zero loaded modules, no media/audio nodes, the exact
  installed programmer hash, and both Rust services live. The pre-write EDID
  was structurally invalid. The bounded operation returned zero and its full
  256-byte read-back matched E21. Final taint remained zero and no process held
  `/dev/i2c-4`. A complete physical power removal is now required before host
  HDMI enumeration and the status-only CSI gate.
- After full power removal and restart, macOS still enumerated only `S34CG50`
  and its internal panel. The post-EDID MIPI gate loaded and unloaded all six
  modules successfully, created no frame, and remained completely idle:
  physical samples all zero, four `hs_idle` data lanes, `CK_HS=0`, and unknown
  decode. Final taint was the expected out-of-tree-only 4096.
- Source-documented bridge reads sharpened the fault: UXC HDMI signal register
  bank `0x86`, address `0xa3` returned `0x00`, rather than upstream's `0x55`
  stable or `0x88` disappeared values. Bank `0x85` CSI Vactive and Hactive both
  returned `0x0000`. No I2C descriptor remained open. Valid persistent EDID is
  therefore insufficient; LT6911UXC firmware/execution or HPD control is now
  the leading boundary.
- Offline extraction of stock v1.4.2 found no separately identifiable
  LT6911UXC application payload or upgrade loader; ordinary stock startup
  depends on bridge-resident SPI firmware. Its stripped application contains a
  firmware-read diagnostic matching Sipeed's published source, but no upgrade
  path has been found. Exact acquisition and loader requirements are in
  `docs/LT6911_FIRMWARE_RECOVERY.md`.

## Prepared LT flash-backup image

- Path: `out/phase3-lt-backup-v26/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256:
  `53d9c4679f5443073564639d00307fcacbce35dd2c68f9df8411665a0e3a266b`
- v26 combines the hardware-proven v23 FIT with a newly audited root filesystem
  and unchanged FIP. Its independently compiled LT backup utility is mode
  `0500`, owned by root, has no init entry, and hashes to
  `24df815d4e32e003b37ad013de950cd49a73ecf2a0c64978a9c6dd1d4c05da6d`.
  It implements only the source-backed 24-bit SPI-read sequence, reads the
  800,000-byte stock reference range twice, suppresses differing output, and
  restores the embedded MCU after both passes and final cleanup.
- The freshly resolved removable target was `/dev/disk4`, 30,992,760,832 bytes.
  Flashing wrote exactly 79,692,288 bytes. Immediate sector-exact readback had
  SHA-256
  `2621c015585db5ff77efa06e675a1f3abbbade6fd0391904e1870df47d91a167`;
  macOS FAT bookkeeping changed the whole-image hash, while the embedded boot
  FIT, root filesystem, and FIP each matched the audited v26 artifacts. The card
  was safely ejected and is ready for the controlled boot.
- v26 booted cleanly with HDMI present, taint zero, zero loaded modules, and no
  open I2C descriptor. Its two passes agreed, but every one of the 800,000 bytes
  was the stale controller setup value `0x51`. SHA-256 was
  `4308ea800a49ddcac2214fc1b35e3afacd7903b8b21a7028b8d478dccb06372b`.
  This is rejected as a zero-entropy controller artifact, not firmware.
- v27 generalized the separately source-backed, hardware-proven 32-byte EDID
  read transaction to 24-bit addressing and added a low-diversity rejection
  gate. Image SHA-256 was
  `29ea8fe9b575d3b614a75fbfe7601d70923ebe6193044e3f04e9abe8756b8b26`;
  reader SHA-256 was
  `4ed79af80cd59f6adc636abdec7547f76f6155395b9abc07f26c09758a520de5`.
  Two unchanged hardware runs both rejected the result because the complete
  passes differed. Both cleaned up with taint zero and no open I2C descriptor.
- v28 adds only fail-closed mismatch telemetry: first differing address,
  differing-byte count, and 16-byte samples from each pass. It still creates no
  file on mismatch. Image SHA-256 is
  `5d150df4085a50ddb9fd541e399dafb41c9bc3ab9d96a90bfc0d2b00fdf13493`;
  reader SHA-256 is
  `63ac141aaa76d06dea2e6290d4292eee5d14d319a13f0771ca51efb4b2ed473a`.
  Its hardware run localized every mismatch to the first 32-byte transaction:
  all remaining 799,968 bytes agreed. This is consistent with stale pipeline
  data immediately after controller setup, not unstable resident flash.
- v29 discards one address-zero transaction after each controller setup and
  then rereads address zero as compared data. It adds no LT register write or
  flash mutation. Image SHA-256 is
  `01047bab4733d6f312b43f4192753d47db841c490f5b691aced370d7747285b6`;
  reader SHA-256 is
  `5a08f5ad68212d42e07877c8d1bac89b1ac90babb1d912334e573ecf61c8d607`.
  On hardware, both complete 800,000-byte reads agreed and passed the diversity
  gates (`unique=255`, `non_ff=46620`, `non_zero=797282`). Device and host
  SHA-256 both equal
  `e4b1bd5944264ab8f192025056f849558dd30d37d00ed404e47a59f3e4ba25a5`.
  Baseline and final kernel taint were zero, no modules were loaded, and no I2C
  descriptor remained open. Preserve this owner-only backup as evidence; next,
  perform offline structure/version analysis and compare read-only LT state
  against a known-working stock boot before proposing any initialization write.
- Offline structure analysis identifies identical `0x18120`-byte slots at
  offsets `0x00000` and `0x80000`, each hashing to
  `07df59ea6e7cc79f46840317abe0d48b61b11ba89ecd4718e389e677d7c1d11e`.
  Each contains `0x18000` bytes of apparent 8051 LT application code followed
  by the pinned, checksum-valid 256-byte E21 EDID and 32 zero bytes; all other
  surveyed space is erased. Next: obtain read-only runtime version/status from
  a known-working stock boot and correlate it with this preserved layout. Do
  not execute, redistribute, or program the extracted application bytes.
- Review of Sipeed's pinned current UXC polling source exposed a diagnostic
  error: status must be read while the embedded MCU is stopped with
  `0x80:0xee=1`, then the MCU must be resumed with `0x80:0xee=0`. The corrected
  hardware run reported stable HDMI `0x55`, CSI Vactive `0x0438`, and Hactive
  `0x0780`; cleanup resumed the MCU, leaked no I2C handle, and did not change
  taint. The resident firmware is running and recognizes a stable 1920x1080
  input. The earlier live `0x00`/zero-dimension result is invalidated.
- The next bounded MIPI run observed nonzero physical samples, `CK_HS=1`, and
  high-speed activity on digital lanes D2/D3, proving a bridge-to-SoC signal.
  It also reported `raw8` decode and accumulated ECC/CRC/word-count errors, so
  no frame was admitted. A repeat run remained CSI-idle for two seconds with
  unchanged counters, while a following corrected LT snapshot still reported
  stable 1920x1080. Next: build a timing-only diagnostic that correlates LT
  stop/read/resume with multiple MIPI samples before changing lane mapping.
- The timing-correlated v30 image is prepared and card-verified. Its probe takes
  receiver snapshots at 0, 100, 500, 1000, 2000, and 5000 ms after applying the
  already qualified stock receiver profile; it still exits before VI setup or
  frame allocation. Image SHA-256 is
  `dcd316b15a2d87bf75d86c805ee5e026a177f619b5398c1f1f27ba7f8983f147`.
  The 31.0 GB removable card was flashed, exactly read back, and safely ejected.
  Embedded boot FIT, rootfs, and FIP verification passed; macOS FAT bookkeeping
  changed only the whole-card readback hash to
  `7803c04ac5db693b774038db2a3faa4e1f17584428d89e183fad9374f44265fd`.
  Next: boot v30, run the corrected stop/read/resume LT status snapshot, run the
  bounded six-snapshot receiver probe once, and confirm module, node, frame,
  file-descriptor, and taint cleanup before interpreting timing or lane state.
- The v30 hardware gate passed containment and exposed a deterministic receiver
  transition. The corrected LT snapshot again reported HDMI `0x55` and
  1920x1080, with MCU resume, final taint zero, and no I2C FD. The receiver saw
  clock/high-speed activity at T0; ECC/WC counters rose from `16/0` at T0 to
  `4690/174` at 100 ms, `22689/981` at 500 ms, `44922/2012` at 1 s, and
  `65536/2894` at 2 s. Decode alternated `unknown`/`raw12` despite the configured
  YUV422 profile. Counters then remained fixed through 7 s while CK_HS and D2/D3
  activity persisted. No frame existed; every module unloaded; final taint was
  the expected out-of-tree bit 4096. This localizes the failure to receiver
  framing/lane interpretation rather than HDMI detection or delayed link start.
  Next: correlate the driver's decode/status fields with the pinned SDK source
  and admit only a bounded receiver-profile discriminator justified by that
  source; do not introduce an LT firmware write or frame allocation.
- Pinned-driver correlation confirms that `/proc/mipi-rx` `decode` is the
  receiver's observed `CSI_DECODE_FORMAT` bitset, not an echo of the configured
  datatype. It also confirms the apparent plateau: once any error count exceeds
  `0xffff`, the driver masks CSI error interrupts. The pinned SG200x LT6911
  profile documents four alternate lane maps beside the current map. v31 admits
  only those five source-listed maps, each in a fresh six-module load/unload
  cycle with bounded samples and no VI setup, frame allocation, or LT access.
  Probe SHA-256 is
  `3bd78f43f757026d4629cbce003499709c4484fde2eaae65d990fd81ba3aa26a`;
  image SHA-256 is
  `5e3915396bcb46f6232620010c8ea6ccbc6b3929424c0b9f62a2ae2e99b68fd9`.
  The resolved 32.0 GB card was flashed, embedded artifacts verified on
  readback, and safely ejected. Next: boot and run the five-profile discriminator
  once, rejecting every profile that accumulates ECC/header/word-count errors.
- The first v31 boot was stopped before loading a module because its baseline
  found inert `/dev/snd/*` nodes. `/proc/asound` was absent and no audio module
  was loaded: these were stale Buildroot target-tree nodes inherited from an
  earlier incremental audio build. The rootfs builder now removes generated
  audio nodes for every non-audio profile and verifies their absence inside the
  final ext4 image. The discriminator gate also reports an explicit pass/fail.
  Corrected v32 passed the new ext4 non-audio audit and all existing audits;
  image SHA-256 is
  `a8bc2d0394e6c0da012db8735969545b126dd02ac51d2f0607281a3601227c72`.
  v31 is rejected. The resolved 31.0 GB card was flashed with v32, its embedded
  boot FIT, rootfs, and FIP verified on raw readback, and safely ejected. Next:
  boot v32 and repeat the clean baseline before loading any module.
- v32 also failed closed before module loading. Live `/dev` was devtmpfs and the
  audio nodes were kernel-generated; artifact comparison proved its boot FIT
  exactly matched the Phase 3a audio FIT. Packaging had reused the last kernel
  built in the shared work volume despite validating the video configuration.
  v32 is rejected. The assembler now supports an expected boot-FIT hash gate.
  v33 combines the hardware-qualified video-only v30 FIT
  (`9e1c47425122175b0327dea8079a588a02f0018aa00806d28911d4a56562e4a7`)
  with the corrected audited v32 rootfs and unchanged FIP. Its complete image
  SHA-256 is
  `afdea272fec92417adbba90f8f09b7313afcbccfc595fbc9625f8b34e659b677`.
  The resolved 32.0 GB card was flashed; raw readback verified the embedded
  video-only FIT, rootfs, and FIP, and the FIT retained its required hash. The
  card was safely ejected. Next: boot v33 and require the explicit baseline gate
  before module loading.
- v33 passed its explicit non-audio baseline with the expected probe hash and
  zero taint. All five source-listed zero-PN-swap lane maps were tested from
  fresh module state. Profile 0 (`2,4,3,1,0`) entered high-speed transfer but
  saturated ECC and accumulated word-count errors while decode wandered.
  Profile 1 held one lane in `hs_err`; profiles 2 and 3 stayed idle/sync with no
  decode. Profile 4 (`3,1,2,4,0`) uniquely reached `Deskew=done`, but two lanes
  remained in `hs_sync` and no datatype decoded. No map is admitted. Every
  frame-absence and unload gate passed; final modules were empty and taint was
  the expected 4096. Next: evaluate the exact generic SDK candidate—device 1,
  lanes `2,0,1,3,4`, all PN swaps enabled—as a single bounded polarity/port
  discriminator before considering any broader permutation.
- v34 implements only that exact generic SDK candidate as profile 5: MIPI
  device 1, lanes `2,0,1,3,4`, all five PN swaps enabled, YUV422 8-bit, 600 MHz
  MAC clock, and HS settle 8. The UART harness runs only profile 5 in a fresh
  module cycle. Probe SHA-256 is
  `085d4a346440e22ba8825f7145777a802c1c6944b8b1414ecfc03ea431e22106`.
  The final image uses the hash-gated video-only FIT and has SHA-256
  `dfc6e5d15a94317d2ab0edc55eb058503a2935d4eb926767f51cad2bb6c49d40`.
  The resolved 31.0 GB card was flashed, embedded artifacts verified on raw
  readback, and safely ejected. Next: boot v34, require the non-audio baseline,
  run profile 5 once, and perform full reverse cleanup.
- v34 hardware execution rejects profile 5. The exact probe identity and
  non-audio baseline passed. MIPI device 1 accepted lanes `2,0,1,3,4` with all
  PN swaps enabled, but every sample from T0 through T5000 ms remained
  `hs_idle`, with `CK_HS=0`, idle deskew, unknown decode, and all protocol
  counters at zero. This proves that the active LT CSI stream is not routed to
  receiver device 1. The probe created no frame; all six modules unloaded with
  return code zero; the final module set was empty; final taint was the expected
  4096. Next: stop enumerating speculative lane maps. Boot the known-working
  stock image and capture source-attributable LT and receiver initialization
  state for differential analysis, without copying or retaining vendor
  executables in the clean image.

- Path: `out/phase3-mipi-status-v23/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256:
  `fd83345a8f3a2a1f84c50dcf41e7cd88b895445db55c68c85f673c75071b23c4`
- The build was performed with networking disabled. Source-cache verification,
  normalized video-foundation configuration, rootfs audit, identity ownership,
  diagnostic ownership, FIT hashes, embedded artifacts, and final image hashes
  all passed.
- The first post-boot card presented for verification was a separate v21 card;
  its installed probe and FIP hashes identified it unambiguously. The currently
  selected 32 GB card was then flashed with v22 and verified before ejection:
  its boot FIT, rootfs, and FIP matched the audited artifacts exactly.
- The controlled v22 boot matched the installed probe hash. Baseline taint was
  zero, Ethernet acquired `192.168.2.62`, both Rust services ran, HID nodes
  remained root-only, and no media modules or nodes were present.
- The explicitly approved status test loaded the six quarantined modules and
  confirmed the stock receiver profile. It exited before VI/frame allocation;
  all modules unloaded, no frame or media node remained, and final taint was
  only the expected out-of-tree bit 4096.
- v23 removes the protected-ID I2C transactions and reset pulse from the
  diagnostic. It preserves the LT6911 power-on state, configures only the stock
  SoC receiver profile, waits five seconds for HDMI negotiation, and exits
  before VI or frame allocation. Its card read-back verified the exact boot
  FIT, rootfs, and FIP before ejection.
- The approved v23 hardware test again reported all six physical samples zero,
  every data lane `hs_idle`, `CK_HS=0`, and unknown decode. All modules unloaded,
  no frame or media node remained, and final taint was 4096.

## Prepared Phase 3a image

- Path: `out/phase3a-audio-enumeration-v1/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256:
  `208a811401849a2504257bb5e641fd077f0ec3cc96eef7b071907b8141fbdaf7`
- The build ran with networking disabled and reused the established device
  identity and recovery credential. Source-cache verification, normalized
  configuration, rootfs audit, identity ownership, FIT hashes, embedded
  artifacts, and final image hashes passed.
- The kernel is module-free and admits only ALSA core/PCM, its mandatory timer
  support, the CV182XA ADC/DAC machine and codec drivers, internal I2S, and the
  required clock/reset plumbing. Media, USB sound/UAC, PDM, external codecs,
  MIDI, sequencer, and OSS compatibility remain disabled.
- No capture/playback utility or audio broker is installed. The first hardware
  gate is root-only enumeration over UART without opening a PCM device and
  requires fresh explicit authorization before flashing or booting this image.
- The resolved 32 GB removable card was flashed after explicit authorization.
  Exact-length read-back verified the embedded boot FIT, root filesystem, and
  FIP against the audited artifacts before safe ejection. macOS FAT bookkeeping
  changed the whole-device read-back hash to
  `3b3a8dd1c1e8d2d5f9327fb9a2408e4c097f0049cf7f95e4a593e9914bf5f6cf`;
  embedded-artifact verification passed and remains authoritative.
- The authorized hardware boot passed Phase 3a Stage A. ALSA enumerated root-only
  `cv182xaadc` capture (`pcmC0D0c`) and `cv182xadac` playback (`pcmC1D0p`)
  endpoints. Kernel taint was zero; no process held a sound FD; modules, media
  nodes, and UAC were absent. Ethernet acquired `192.168.2.64`, and both Rust
  services remained live. No PCM was opened.
- Next: implement a one-second, memory-only local capture probe with fixed
  format/byte bounds, structural metrics only, explicit cleanup/zeroization,
  and a post-run FD audit. Do not add raw-audio export or MCP audio access.
- Capture-v2 is now built, audited, flashed, read back, and safely ejected. Its
  image SHA-256 is
  `a60d95083c7d8f97181d02175005be85da563ba4b70e83ef3aff0f480e8399c2`.
  After insertion and boot, run its device-free self-test first, then one
  explicitly bounded capture and a post-run PCM-FD/taint/containment audit.

## Prepared stock differential reference

- The retained stock v1.4.2 reference was written to the resolved removable
  32.0 GB card. The source image is 1,627,390,464 bytes with SHA-256
  `23648088de59e043a1289c49c5103f829dc652604f8dd76db22dd8ff7aca6539`;
  its compressed archive passed integrity checking and expands to that exact
  digest.
- macOS FAT bookkeeping changed the whole written-span readback digest to
  `50a0d7750e229333f3fdf878ed889e0b9bd6bcf365fcf4a0e1b030d0edb941e3`.
  The complete 1,610,612,736-byte Linux partition matched source exactly at
  SHA-256
  `c285650ee41892c0cef5fbc371d3b13828f886a3ebb82a400ff06bbec94585a4`.
  The card was safely ejected.
- Next: boot with HDMI and UART attached and capture a read-only stock
  receiver/LT state baseline before changing any register or copying any vendor
  executable.
- That stock UART baseline completed. The stock path parsed device 0, lanes
  `2,4,3,1,0`, zero PN swaps, and 1920x1080—the same receiver values used by
  the clean probe. Unlike the clean result, `/proc/mipi-rx` remained stable at
  `yuv422-8`, `CK_HS=1`, D2/D3 high-speed transfer, zero ECC/CRC/header/FIFO
  errors, and only one word-count error across samples two seconds apart.
- Stock booted kernel `5.10.4-tag- #46` and receiver module build `#30`. The
  image retains a 122,408-byte `soph_mipi_rx_old.ko` but loads a distinct
  125,992-byte replacement `soph_mipi_rx.ko`, SHA-256
  `d40419159a4065de8b7693e0de59c477de36373f24fe164e50afb37d674ed755`.
  The clean v7 module is 124,272 bytes with SHA-256
  `8218302da9c43c046fad23b80c60b65889996c84aec430fd492cc8e7d4dbe9c0`.
  This localizes the next investigation to the receiver-driver/build delta and
  stock prerequisite load order. Do not copy or load either stock module into
  the clean image; recover the behavior from auditable source.

## Prepared reset-sequence image v35

- Path: `out/phase3-reset-sequence-v35/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256:
  `b2db1e88af8913cd7ba7e63fa9d52b1ec49938d89326e6fc1836b4aa7c21cf99`
- Embedded root-only probe SHA-256:
  `4f461d8f483f777d5674cc7c5fdcb2ac29092630bea143244ca5af847d76cc2a`.
- v35 implements the source-exact `SAMPLE_COMM_VI_StartMIPI` reset and clock
  sequence (`CVI_MIPI_RESET_SENSOR`, `CVI_MIPI_RESET_MIPI`,
  `CVI_MIPI_SET_DEV_ATTR`, `CVI_MIPI_ENABLE_SENSOR_CLOCK`, `usleep(20)`,
  `CVI_MIPI_UNRESET_SENSOR`) on profile 0 (device 0, lanes `2,4,3,1,0`, zero PN
  swaps).
- The resolved 31.0 GB removable card (`/dev/disk4`) was flashed with v35.
  Immediate raw readback verified the embedded boot FIT (`1ef26201...`), rootfs
  (`d8647fe7...`), and FIP (`96a0b414...`) against their audited artifacts.
  macOS FAT bookkeeping changed the whole-card readback hash to
  `84c7164ac8180691fc50b880e4ba5e51baf53d307a4ba10d27f8dee11589e875`;
  embedded-artifact verification passed. The card was safely ejected.
- Controlled hardware boot of v35 passed baseline containment: taint zero,
  root-only `/dev/i2c-4`, non-audio ext4 verified, probe SHA-256 confirmed, and
  both Rust services live. The corrected LT6911 snapshot reported stable HDMI
  `0x55` and 1920x1080 input with MCU resume and no leaked I2C descriptors.
- All six modules loaded normally. The probe executed profile 0 with the sensor
  reset/clock sequence. Digital data lanes D2/D3 entered active high-speed
  transfer (`hs_hst`) with clock `CK_HS=1`. Decoded format correctly recognized
  `yuv422-8` at T100ms, T500ms, and T2000ms. Error counters rose from `31` ECC /
  `1` WC at T0 to `5607/324` at 100ms, `27973/1634` at 500ms, and hit the
  `65536/3822` plateau at 2s. No frame was admitted; all six modules unloaded
  with return code zero; final module list was empty; final taint was 4096.
- Conclusion: Sensor reset and clock sequencing correctly aligns wire framing
  to `yuv422-8`, but protocol errors persist. Next: perform offline attribution
  of the stock replacement `soph_mipi_rx.ko` (build #30 vs clean v7) and
  evaluate prerequisite `soph_rtos_cmdqu` / `soph_fast_image` initialization to
  recover clean framing from source.

## Raw-frame probe candidate v36 hardware result

- Path: `out/phase3-raw-frame-v36/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256: `2198051a352397e6402903c30bb8f4a94b11b9fbdd9ab56f999e0b128e998fc2`
- Probe SHA-256: `4a8aa49e0a518e5125899937317ae3c233848e0a0f3fd0650966974dc7df51aa`
- Target SD card `/dev/rdisk4` (32.0 GB) flashed and verified: raw sector readback
  (`81fcb72a...`) passed byte-for-byte verification for embedded `boot.sd`
  (`1ef26201...`), `rootfs.ext4` (`bc9b2da7...`), and `fip.bin` (`96a0b414...`).
- Hardware boot under serial control passed baseline containment: taint zero,
  non-audio ext4 verified, probe SHA-256 confirmed, and LT6911 confirmed locked
  to HDMI input `0x86:0xa3 = 0x55` at 1920x1080 with MCU resume.
- All six quarantined modules loaded with return code 0 (`soph_sys`, `soph_base`,
  `soph_mipi_rx`, `soph_snsr_i2c`, `soph_vi`, `soph_vpss`).
- Probe execution results:
  - Moving `CVI_VI_SetDevAttr(0)` and `CVI_VI_EnableDev(0)` before MIPI setup
    successfully activated `clk_csi_mac0` and MAC FIFO drainage (`fifofull: 0`).
  - At T0, initial MIPI RX errors were dramatically lower than v35: `EccErr: 50`,
    `WcErr: 0`, `fifofull: 0`.
  - At T100ms, errors rose to `2961` ECC / `314` WC.
  - At ~2.1s, CIF logged: `mask the interrupt since err cnt is full: ecc = 65536, crc = 5, wc = 6741, fifo_full = 0`.
  - `base_get_chn_buffer` reported `Mod(VI) Grp(0) Chn(0), jobs wait(0) work(2) done(0)`.
  - `CVI_VI_GetChnFrame` timed out and failed with `0xc00e8040` (`EPERM` / `CVI_ERR_VI_BUF_EMPTY`).
- Reverse unload succeeded completely: all 6 modules unloaded with return code 0,
  zero remainder modules, post-taint was expected 4096, and no frame leaked.
- Critical Root Cause Discoveries:
  1. **Bridge Reset Disruption**: `ioctl(mipi_fd, CVI_MIPI_RESET_SENSOR, &mipi_dev)`
     pulsed GPIO pin 353 (active-low reset on LT6911) for ~358 µs. This forced the
     already-locked LT6911 bridge to reboot and renegotiate HDMI (~2s). The bus
     transients during bridge renegotiation flooded the CIF receiver with 65536
     errors, triggering automatic interrupt masking (`mask the interrupt since err cnt is full`).
     Stock `S15kvmhwd` explicitly leaves GPIO451 high without a reset pulse.
  2. **Unset `dphy.enable`**: `mipi.mipi_attr.dphy.hs_settle = 8` was set, but
     `mipi.mipi_attr.dphy.enable` was left 0. In `cif.c` line 596, `cif_set_hs_settle`
     is only called `if (attr->dphy.enable)`. The D-PHY receiver therefore never
     configured its high-speed settle timing register.
  3. **VI Error 0xc00e8040**: Decodes to `CVI_ID_VI` (0x0e) + `CVI_ERR_VI_BUF_EMPTY` (0x8040).
     Because CIF masked interrupts after error saturation, no completed buffers
     reached the `done` queue (`done(0)`), causing `GetChnFrame` to time out.

## Raw-frame probe candidate v37 hardware result

- Candidate v37 implemented the non-disruptive initialization in `image/phase3-raw-frame-probe.c`:
  - `clk_csi_mac0` activated by enabling VI Dev 0 before MIPI initialization (`fifofull: 0`).
  - Omitted `CVI_MIPI_RESET_SENSOR` to preserve the HDMI-locked LT6911 bridge state.
  - Enabled D-PHY settle configuration via `dphy.enable = 1` and `hs_settle = 16`.
  - Configured sensor geometry via `VI_IOCTL_SET_SNR_INFO` (`1920x1080`).
- Result: Receiver errors remained low, but `CVI_VI_GetChnFrame` failed with `0xc00e8040`.
  Kernel logs revealed `isp_err_chk:6573(): CSIBDG_A CH0 frm height less than setting(1080), pxl=0 lines=0`,
  followed by `_vi_err_retrig_preraw()` looping 31 times within 6 ms and terminating the pipe.

## VI driver short-circuit fix (`0011-vi-log-actual-csibdg-line-count.patch`)

- **Root Cause**: `isp_err_chk` in `vi.c` treated transient dimension checks (`CH0_FRAME_HEIGHT_LS_INT`)
  as fatal (`ret = -1`), causing `vi_irq_handler` to exit early before checking `FRAME_DONE_FE0`.
  In addition, `vi_err_wake_up_th` exhausted its 31 retry budget in milliseconds.
- **Fix Applied**:
  - Removed `vi_err_wake_up_th()` on dimension interrupts; set `ret = 0` (warning only).
  - Removed early return in `vi_irq_handler` so `FRAME_DONE_FE0` is processed when asserted.
  - Added telemetry for `CH0_DEBUG_0` (pxl), `CH0_DEBUG_1` (lines), `CH0_DEBUG_2` (VSYNC/trig).
- **Target Verification**:
  - Rebuilt offline; deployed `soph_vi.ko` (`bf729726...`).
  - The 31-retry loop was completely eliminated.
  - Frame start interrupts (`VISofCh0Cnt`) surged to **2,681 counts at 386 FPS**, confirming
    continuous high-speed frame synchronization.

## CIF D-PHY diagnostic unpacking patch (`0012-cif-fix-pad-ctrl-mapping-and-phy-state.patch`)

- **Root Cause**: `cif_get_csi_phy_state()` shifted `DBG_94` by 16, misaligning D0..D3 data lane
  states into `union mipi_phy_state` (aliasing D0 to D2).
- **Fix**: Correctly unpacked individual bitfields into `union mipi_phy_state`. Added pad PU/PD
  clear logging.

## Empirical hardware discriminator findings

1. **2-Lane MIPI Isolation (Profile 6: `2,1,0,-1,-1`, `hs_settle=24`)**:
   - Both Port 0 data lanes (physical 1 and 0) **locked solidly into `hs_hst`** with only 3 ECC errors.
   - Proves Port 0 receiver silicon, pad routing, and deskew are completely functional.
2. **Port 1 Physical Pin Order**:
   - Swapped Port 1 candidate `2,3,4,1,0` immediately entered `hs_err` on physical 4.
   - Confirms `2,4,3,1,0` (physical 4 and 3 as Port 1 far lanes) is the authentic wiring order.
3. **4-Lane Far-Lane Phase Sweep (`lane_phase=0,0,X,0,0` with `2,4,3,1,0`)**:
   - Swept Port 1 deskew (`lane_phase[2]`):
     - `X = 4`: D0 in `hs_err`
     - `X = 8`: D0 `hs_sync`, D1 `hs_sync`, D2 `hs_hst`, D3 `hs_hst` (clean near-lane lock!)
     - `X = 10`: D0 `hs_err` (5s), `preample` (100ms)
     - `X = 11`: D0 `hs_err`
     - `X = 12`: D0 advanced to `preample` (state 4, immediately preceding `hs_hst`)
     - `X = 13`: D0 in `preample`, D1 `hs_sync`, D2 `hs_hst`, D3 `alt_cal`
     - `X = 14`: D0 `hs_err`
     - `X = 16`: D0 `hs_err`
   - *Timing window*: The manual deskew window for Port 1 far lanes is centered at 12–13.
4. **Near-Lane Phase Evaluation (`lane_phase[1]`)**:
   - Vendor default `lane_phase[1] = 0x03` caused D2 to degrade to `hs_sync`. Zero delay
     (`lane_phase[1] = 0`) is optimal for Port 0 near lanes.
5. **Clock Delay Evaluation (`lane_phase[0]`)**:
   - Tested `lane_phase=1,0,12,0,0`: offset common clock delay, degrading near-lane reception.
6. **High-Speed Settle Evaluation (`hs_settle`)**:
   - Tested `hs_settle = 24` on 4-lane mode: at T0, D0 reached `preample` even at `lane_phase[2] = 8`.

## Working receiver hypothesis (not an admission finding)

- **4-Lane Byte Striping**: MIPI CSI-2 distributes packet bytes across the
  configured data lanes. The far lanes have not shown stable `hs_hst`, while
  the CSI bridge reports `pxl=0, lines=0` and no completed frame. This is
  consistent with incomplete lane reception, but does not uniquely establish
  why `FRAME_DONE_FE0` is absent.
- **Clock Phase Control (`CSI_CK_PHASE`)**:
  `REG_SENSOR_PHY_4L_08` defines this eight-bit field at bits 16..23.
  Whether changing it improves far-lane lock is unmeasured. v40 tests only
  that bounded hypothesis, starting from the SDK-preserving zero setting.
- **Independent Per-Lane Deskew**:
  `cif.c` currently forces the same deskew code onto both far lanes (`lane_phase[2]`).
  Enabling independent 8-bit deskew (`DESKEW_CODE3` and `DESKEW_CODE4`) allows fine-tuning
  physical lane 3 and 4 individually.

## Ordered next work

1. **Polarity Resolution Closed**: The complete 16-state polarity matrix `(D0, D1, D2, D3)`
   was tested on hardware. Two foundational conclusions are closed:
   - D0 and D3 must NOT be swapped (`pn_swap[1] = 0, pn_swap[4] = 0`). Inverting
     either destroys reception (D0=1 mutes link completely; D3=1 corrupts packet sync).
   - D1 (physical lane 3) is differentially inverted on the PCB. Setting
     `pn_swap=0,0,1,0,0` streamed 98 lines and 187,663 pixels of valid `yuv422-8` video.
2. **Resolve Error Accumulation Around `0,0,1,0,0`**:
   The line-98 truncation was caused by CIF error saturation (65536 ECC / 5805 WC
   errors after ~2.1s) triggering driver interrupt masking. Next bounded tests:
   - Sweep Port 1 delay `lane_phase[2]` across values 8, 10, 11, 12, 13, 14, 15
     with `pn_swap=0,0,1,0,0` to identify the phase delay that minimizes ECC errors.
   - Evaluate `hs_settle = 20, 24` on `pn_swap=0,0,1,0,0` to evaluate whether
     settle timing extends reception beyond line 98.
   - Test `csi_ck_phase` on `pn_swap=0,0,1,0,0` using the v40 diagnostic control.
3. **Capture a single 1080p frame only after a clean receiver gate**:
   - When error accumulation is eliminated, verify `CSIBDG_A` receives all 1080 lines and
     `FRAME_DONE_FE0` asserts.
   - `CVI_VI_GetChnFrame` will acquire `/run/cleankvm-frame.yuv422p` (4,147,200 bytes).
   - Validate file size and SHA-256 hash.
4. **Clean Reverse Unload Verification**:
   - Confirm all six modules unload in reverse order with RC=0.
   - Confirm the six test modules are absent from `/proc/modules`, no frame
     file remains, and taint matches baseline plus the expected out-of-tree bit (4096).

### Prepared clock-phase-only diagnostic module

- Output: `out/phase3-clock-phase-module-v1/soph_mipi_rx.ko` (ignored local
  artifact), SHA-256
  `2e6cdcb8e8ec0e0fafe580dd54f86eace67dfe1cf8c90b80d969673a70ca863f`.
- Built in `cleankvm-work-clock-phase-v1` with networking disabled, using the
  qualified v35 kernel build and a copy of the source that already carried the
  `0012` PHY-state diagnostic. The original source volume was not modified.
- ELF64 RISC-V relocatable; vermagic is
  `5.10.4-tag- preempt mod_unload riscv`. Its undefined-symbol set matches the
  preceding diagnostic receiver module (SHA-256 `382dd2dc...`). The v39
  retained image uses the same FIT hash as v35 (`1ef26201...`).
- This remains a quarantined diagnostic module, not an admitted video
  component. The target kernel, module baseline, load/unload return codes,
  and frame-file cleanup were verified for the two tests recorded below.

### SD image v40 and initial hardware result

- Path: `out/phase3-clock-phase-v40/cleankvm-phase1.img`, 79,692,288 bytes,
  SHA-256 `bcb1b845152ea5d5974a4bb0eec54a5f10e63e4a231fbabbd0afe8fbc057e1b4`.
- Network-disabled incremental rootfs build passed its config, identity,
  probe-ownership, non-audio-node, and complete six-module audits. The
  packaged receiver is `2e6cdcb8...`, patched VI is `bf729726...`, and
  root-only probe is `15128765...`; each was independently checked inside
  the final ext4. The FIT is the previously qualified video-only
  `1ef26201339516fdd40816bdade7ba4578a0c84c41c93e358a4ba2edd709a79b`.
- The SD resolver identified exactly one external removable target:
  `/dev/disk4`, USB, 30,992,760,832 bytes, existing `CLEANKVM` layout.
  After unmount, exactly 79,692,288 bytes were written. Raw read-back of the
  same span had SHA-256
  `5341ca5a0d3bfbdf2b06ac723012dd90f1e661b2e867a3329891d1429e329714`;
  macOS FAT bookkeeping changed the whole span, but embedded FIT, rootfs,
  and FIP each matched the audited artifacts. The card was safely ejected.
- v40 booted into the authenticated recovery login. Baseline: kernel
  `5.10.4-tag-`, taint 0, no `soph_*` modules, no `/dev/snd/controlC0`,
  receiver `2e6cdcb8...`, VI `bf729726...`, probe `15128765...`, and both
  Rust services running. All six quarantined modules loaded with RC=0 in
  each bounded test.
- Phase 0 (`csi_ck_phase=0`, `lane_phase=0,0,12,0,0`, settle 24): ECC/CRC/
  header/WC/FIFO counters all zero at T0, T100ms, and T5000ms; digital
  D0/D1/D2 `hs_hst`, D3 `hs_sync` or `hs_idle`, clock HS=1, decode unknown,
  VI start/completion counts zero. `CVI_VI_GetChnFrame` failed with
  `0xc00e8040`; no frame existed. All six unloads returned zero, no test
  module remained, final taint 4096, frame absent.
- Source-backed LT stop/read/resume between tests reported HDMI signal `0x55`,
  Vactive 1080, Hactive 1920, MCU resume RC=0, and no leaked I2C descriptor.
- Phase 8, with every other parameter fixed, also had zero protocol errors,
  D0/D1/D2 `hs_hst`, D3 `hs_sync` or `hs_idle`, unknown decode, zero VI
  starts/completions, and no frame. The module parameter read back 8 and
  `dmesg` logged `cif: diagnostic CSI_CK_PHASE=8`, confirming the write path.
  Reverse unload and frame-file cleanup passed identically.
- The current evidence does not support continuing a broad clock-phase
  sweep. Investigate digital D3/physical lane 0 mapping and polarity from
  auditable source and stock-state evidence before another hardware change.
- Immediate source cross-check: `cif_set_lane_id()` programs
  `CSI_LANE_D3_SEL` from the fifth `lane_id` entry, so the product map
  `2,4,3,1,0` selects physical lane 0 for digital D3. The 4-lane `DBG_94`
  register defines D3 state at bits 12..14, matching the corrected diagnostic
  unpacker. The D3 observation is therefore not merely the old bit-shift
  reporting bug.

### Complete 16-State Polarity (PN Swap) Empirical Evaluation (2026-09-19)

All 16 combinations of `pn_swap` across data lanes D0, D1, D2, D3 were tested
on target hardware using `cleankvm-lane-discriminator`. Each cycle maintained
complete fail-closed safety (all six modules unloaded cleanly with RC=0, taint 4096,
no residual processes or frame files).

#### 16-State Polarity Truth Table

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

#### Key Takeaways

1. **D1 Receiver Polarity Setting**: `pn_swap=0,0,1,0,0` is an empirically effective
   receiver-side setting that unlocks active video streaming (98 lines, 187,663 pixels,
   `yuv422-8` decode). This is a promising receiver configuration, though not yet
   proof of a PCB differential pair inversion since stock declared zero PN swaps (`0,0,0,0,0`).
2. **D0 and D3 Polarities Verified Non-Inverted**: Inverting D0 completely kills
   SOF and mutes the link across all 8 permutations. Inverting D3 breaks packet
   header decoding across all permutations. Both must remain `pn_swap=0`.
3. **Line-98 Truncation Cause**: Error saturation reached 65536 ECC / 5805 WC
   errors after ~2.1 seconds, triggering CIF interrupt masking (`cif.c:2763-2777`).

### Sequential Single-Variable Isolation Results (2026-09-19)

A strictly sequential investigation was performed around `pn_swap=0,0,1,0,0`:
1. **Baseline Confirmation (`lane_phase[2]=12`, `hs_settle=16`, `csi_ck_phase=0`)**:
   - Confirmed `yuv422-8` decode. Early telemetry: T0 EccErr=35; T100 EccErr=3576
     (35.4k errors/sec). At T100, D3 showed `hs_err`. Interrupt masking triggered at
     T=1.9s at 65,536 ECC / 5,951 WC errors after buffering 3 lines (5,760 px).
2. **Deskew Alone (10, 11, 13 near 12 at settle 16, ck_phase 0)**:
   - Deskew 10: T100 CRC dropped to 395, WC to 305; D3 stayed in `hs_idle` without `hs_err`;
     delivered 4 lines (6,578 px).
   - Deskew 11: Delivered **11 lines (20,530 px)** (`pxl=20530 line=11`).
   - Deskew 13: Higher WC errors (321 at T100); 0 lines.
   - Result: Deskew window is strictly peaked at 11–12.
3. **Settle Alone (20, 24 at deskew 12, ck_phase 0)**:
   - Settle 20: Slashed CRC errors by **>80%** (T100 CRC dropped from 469 to 84; final
     CRC dropped from 8,461 to 1,621). Delivered 2 lines (3,352 px).
   - Settle 24: Decode remained `unknown`, SOF=0. Settle 24 exceeds physical window (ruled out).
4. **Clock Phase Evaluation (`csi_ck_phase=8` at deskew 12, settle 16)**:
   - Kernel confirmed `CSI_CK_PHASE=8` write.
   - Delivered **76 lines and 145,920 pixels** into `CSIBDG_A` before interrupt masking!
5. **Driver Interrupt Masking Mechanism Identified**:
   - Upstream `cif.c:2763-2777` unconditionally executes `cif_mask_csi_int_sts(ctx, 0x1F)`
     when `errcnt_ecc > 0xFFFF`. Because 1-bit Hamming ECC errors increment on every packet
     header (~34,000/sec at 1080p30), the 16-bit software counter saturates in ~1.9 seconds,
     artificially halting frame delivery even while packets stream cleanly.

### Per-Lane Deskew & Clock Deskew Tuning (2026-09-19)

- Isolated independent physical lane deskew registers via Patch 0014 (`lane_phase[0..4]` maps to CLK, D0, D1, D2, D3).
- Setting physical clock lane deskew to code 1 (`lane_phase[0]=1`) reached **152 complete lines (293,248 px)**.
- Alternating repeat verification (3 pairs, 6 runs) proved that Clock Deskew 1 shifted mean line reception
  from 60.0 lines (Code 0) to 146.3 lines (Code 1), reaching an all-time record of **229 lines (441,087 px)**.
- Clock 0 failed consistently on vertical framing (`frm height less than setting`), whereas Clock 1 completed
  lines up to 229 before an isolated packet error caused `frm width less than setting(1920)`.

### Time-Limited Interrupt Mask Diagnostic (Patch 0015) (2026-09-19)

- Implemented non-default, bounded diagnostic patch (`image/patches/0015-cif-time-limited-interrupt-mask-diagnostic.patch`).
- Selective masking via `cif_mask_csi_int_bits` in `cif_drv.c`: When `cif_diagnostic_mask=1`, `errcnt_ecc > 65535`
  masks only bit 0 (`CIF_INT_STS_ECC_ERR_MASK`, `0x01`), keeping data packet reception active.
- Rate-limited to 150,000 IRQs, retained fatal error handling (`hdr > 0`, `fifo_full > 0`, `crc/wc > 65535`), and logged
  first link error.
- Compiled module `out/phase3-interrupt-mask-diagnostic-v1/soph_mipi_rx.ko` (SHA-256 `048366c3869f3639a61aa47b9bcdd55e135a38aa04a6dd0fe785be76893f3ca0`).
- **Control Run (`cif_diagnostic_mask=0`)**:
  - Stock behavior reproduced identically: receiver masked `0xFF` at 1.76s (`ecc=65536, crc=7788, wc=2422`).
- **Diagnostic Runs (`cif_diagnostic_mask=1`)**:
  - Reception **successfully continued past the 1.8-second mask point** for the full 5.0 seconds (`VISofCh0Cnt` reached 196–393).
  - Total counters grew past 65,535: `EccErr ~84,400`, `CrcErr ~31,500`, `WcErr ~9,780`.
  - However, **no run reached `FRAME_DONE_FE0` with 1080 valid lines**.
  - Telemetry revealed why: the first link error (`ecc=1, crc=1`) occurs within **120–173 µs** of receiver activation, and
    line truncation (`CSIBDG_A CH0 frm width less than setting(1920)`) occurs on every single frame within the first 3 to 18 lines.
  - **Conclusion**: The software interrupt mask is **not the root blocker**. Even with the receiver unmasked and streaming for
    5 seconds, physical-layer / MAC packet corruption on every frame prevents any frame from completing 1080 lines.

## Next Steps

1. **Investigate Physical-Layer Timing & CRC/WC Error Source**:
   - Because CRC and word-count errors occur right from the first packet (+120 µs) and on every frame (line 3–18),
     the physical link is suffering from bit errors or misaligned framing at the PHY/MAC boundary.
   - Investigate LT6911UXC MIPI output drive strength, pre-emphasis, or clock-lane timing parameters via I2C.
   - Investigate SG2002 CIF MIPI PHY termination, settle calibration, or clock/data relationship (e.g. fine settle or clock delay).
2. **Maintain Strict Fail-Closed Security**:
   - Do not admit video, enable MCP media endpoints, or auto-load video modules until 1080 valid lines, `FRAME_DONE_FE0`,
     and a hashed 4,147,200-byte planar-YUV422 frame are verified on clean target hardware.
   - All diagnostic runs must verify clean reverse module unloads (`RC=0`), clean `/proc/modules`, and taint `4096`.
