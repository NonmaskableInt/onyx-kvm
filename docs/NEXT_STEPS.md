# Next steps

Updated: 2026-10-19

This is the newest-first hardware and release log. Read the top entry before
continuing bring-up, and update it when evidence closes a hypothesis.

## Current state (read this first)

- **Newest hardware-verified image:** development candidate v86 (entry below).
- **Last hardware-accepted image before the rename:** product release candidate v80 (entry below).
  Hardware findings through v80 use the earlier `cleankvm` names where they
  quote paths, hashes and log output; those records are intentionally unchanged.
- **Naming standardized to `onyx-kvm` after v80** (see [NAMING.md](NAMING.md)).
  The legacy fallbacks, aliases and symlinks are gone, so the next candidate
  (v81) is a rebuild of every on-target artifact, not a repackage of v80. It
  needs a fresh work volume, rebuilt probes, modules and brokers, the rootfs
  audit, a removable-media identity check, flash and embedded-artifact
  read-back, and re-pairing of clients.
- **Open items and recommendations:** [OPEN_ITEMS.md](OPEN_ITEMS.md). The most
  important are the network setup-claim flow (S1), the microSD trust boundary
  (S2), development tooling shipping in product builds (S3), the unenforced
  `PENDING-REVIEW` source gate (B1) and the non-reproducible work volume (B2).
- **Next bounded experiment:** none pending on hardware. The v81 rebuild above is
  the next step and needs no new hypothesis.

## Development Candidate v86: Boot-Partition Log Made Dev-Only [BOOTED — mDNS AND IP ORIGIN CONFIRMED]

v86 is v85 with the `S51onyx-kvm-mdns` boot-partition status log gated on
`usr/lib/onyx-kvm/dev/mdns-debug`, which only `ONYX_KVM_DEV_TOOLS=1` builds
create. `boot.sd`, `fip.bin` and `phase1.its` are unchanged. Still a development
build (unresolved licenses, OPEN_ITEMS B1).

- **Flash (2026-10-20):** `/dev/disk4` re-resolved (only external disk, removable
  USB, 30,992,760,832 bytes); 79,692,288 bytes written and read back;
  `embedded_artifacts=verified`. `rootfs.ext4` `832fe4d6...f4b545`; image
  `287ae05a...b752e651`.
- **Booted as** `onyx-kvm-6055de` at 192.168.1.246. A direct mDNS query returned
  `A` 192.168.1.246 and an NSEC answer for AAAA; `onyx-kvm-6055de.local` and the
  IP both serve `/setup/status`; the IP origin is accepted (409, not 403).
- **Operator pass (2026-10-20):** all console paths (video, HID, microphone,
  pairing) reported operational through the browser on v86.
- **Not checked:** that `onyx-kvm-mdns.log` is absent from the boot partition
  (the card was not pulled), socket enumeration, and UART evidence.

## Development Candidate v85: Name Responder and Origin Fix, Verified on Hardware [BOOTED — mDNS AND IP ORIGIN CONFIRMED]

v85 is v82 plus the `S50onyx-kvmd` wait for `eth0` and the product-only
`onyx-kvm-mdns` responder (see [NETWORK_POLICY.md](NETWORK_POLICY.md)). Only the
rootfs differs: `boot.sd`, `fip.bin` and `phase1.its` are byte-identical to v82.
Still a development build (unresolved licenses, OPEN_ITEMS B1).

- **Identity and flash (2026-10-20):** `/dev/disk4`, removable USB, 30,992,760,832
  bytes, the only external disk; written to `/dev/rdisk4` (79,692,288 bytes) and
  read back with `bs=512 count=155649`. `verify-sd-image.py`:
  `embedded_artifacts=verified`. `rootfs.ext4` `010c9006...742c255b`; image
  `8fcb1f3c...ca0dbd1be`.
- **IP origin:** the daemon accepted `https://192.168.1.245:8443` as a setup
  origin (HTTP 409 "already configured", not 403), so `eth0` had its address when
  `S50` built the origin list. Confirmed on v83, v84 and v85.
- **mDNS:** `onyx-kvm-cacf0f.local` resolves on the client. A direct legacy
  query returned an `A` answer with 192.168.1.245 and an NSEC (no AAAA) answer.
- **Defect found and fixed along the way:** v83 and v84 did not answer. The
  `/boot/onyx-kvm-mdns.log` status record read `state: exited` with empty stderr.
  Cause: BusyBox `start-stop-daemon -c` requires a passwd entry and UID 901 had
  none. Fix: the `onyx-kvm-mdns` user/group (UID/GID 901, `nologin`) in
  `phase1-users.txt`, audited in the rootfs build.
- **Hostname and address change on every reflash.** The MAC is stored in
  `/boot/onyx-kvm-mac.txt`; reflashing wipes it, so the device takes a new MAC,
  hostname (`onyx-kvm-<last six hex digits>`) and DHCP lease (f406c8, b6baae,
  cf20e7, cacf0f across v82-v85). Pairings do not survive a reflash.
- **Diagnostic:** `S51onyx-kvm-mdns` writes a bounded status record
  (arguments, running/exited, 512 bytes of stderr) to `/boot/onyx-kvm-mdns.log`.
  Consider removing it before release.
- **Not done:** socket enumeration on the booted device (expected: TCP 8443 and
  UDP 5353 only); no UART, so no taint/module/node capture.

**Next bounded experiment:** none pending on hardware. Remaining release work is
the four license grants (B1) and the optional removal of the boot-partition
status record.

## Development Candidate v82: Rebuilt From Pinned Sources, Flashed, Read Back [BOOTED — VIDEO STREAMING CONFIRMED]

Built from a fresh work volume with `ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1`
(four components lack a license grant, so this image is **not releasable**; see
[OPEN_ITEMS.md](OPEN_ITEMS.md) B1). Product profile, no dev tools.

- **Removable-media identity (2026-10-19):** `detect-target-disk.py` found one
  external candidate, `/dev/disk4`, 31.0 GB (30,992,760,832 bytes), USB mass
  storage, previously labelled `ONYXKVM`. Unmounted, written to `/dev/rdisk4`
  (`dd bs=1m`, 79,692,288 bytes), read back with `bs=512 count=155649`.
- **Hashes:** image `a050562c...2308ae9`; `boot.sd` `f5216efc...11555b3`;
  `rootfs.ext4` `5c74ac92...a2a88c`; `fip.bin` (258,560 bytes) `37248202...187ae74`.
- **Read-back:** `verify-sd-image.py` on the card read-back reports
  `embedded_artifacts=verified` (FIT, FIP, rootfs). The whole-image hash
  differs (`6eeb675c...d1d111e`), as expected from FAT bookkeeping after the
  card was mounted; it does not replace the embedded check.
- **Rootfs audit:** passed after fixing `build-phase1-rootfs.sh`, which had left
  `S35onyx-kvm-devmode` in the product rootfs.
- **Modules:** eight of nine rebuilt modules match the shipped v51 set
  (five byte-identical, three identical except the kernel build stamp).
  `soph_mipi_rx` differs; the rebuilt one carries extra diagnostic parameters
  whose defaults have not been checked.
- **FIP:** differs from v80 (BL2 hash; `LOADER_2ND` 101,888 vs 101,376 bytes).
  Cause unconfirmed (suspected longer `onyx-kvm` strings).
- **Boot chain:** `image/build-phase1-bootchain.sh` now builds U-Boot, the FIP
  and the dtb. Rerun on the v82 volume it reproduced the shipped `fip.bin`
  (`37248202...187ae74`) exactly, so the FIP is reproducible.

**Hardware boot (2026-10-20, no UART available):**
- The card booted: the new FIP, kernel and rootfs came up, DHCP gave
  `192.168.1.242`, and `GET /setup/status` returned
  `{"configured":false,"name":"onyx-kvm-f406c8"}` before the claim.
- The operator completed the setup claim and the console streams HDMI video,
  so the rebuilt `soph_mipi_rx` and the other eight rebuilt modules work with
  the current LT6911 path. Taint, loaded modules and nodes were not captured
  because there was no UART; the evidence is the working stream only.
- **Defect found:** a claim via `https://192.168.1.242:8443` was rejected with
  "forbidden origin for setup claim". `S50onyx-kvmd` builds the origin list once
  and only waited 5 s for `eth0`, so the IP origin was missing when DHCP was
  slower. The image also has no mDNS responder, so `<host>.local` does not
  resolve. Workaround used: an `/etc/hosts` entry on the client for the plain
  hostname origin. Fix: `S50onyx-kvmd` now waits up to 30 s for the address
  (needs a rootfs rebuild and reflash to take effect).
- Operator reports HID and microphone working on this build (2026-10-20). The
  microphone result is an operator observation: how it was enabled and what
  "working" covered (Phase 3a admits metrics-only capture) was not recorded.
- PIN pairing and MCP verified from the client (2026-10-20): `POST /pair` with
  the operator-supplied PIN returned a 4 h Bearer console token; MCP
  `initialize` negotiated `2025-11-25`; `tools/list` returned only
  `kvm.status`, `kvm.video.snapshot`, `kvm.keyboard.send`, `kvm.pointer.send`.
  `kvm.dev.exec` and `kvm.system.dev_mode` are absent, confirming the dev-tools
  compile-out in the product build. The token was not recorded or persisted.

**Next bounded experiment:** rebuild the rootfs with the `S50` fix, reflash, and
confirm the IP origin works on first boot; then exercise HID and MCP pairing.

## Product Release Candidate v80: Offline Base Image Packaging, Verified Rootfs Audits & MCP Navigation Acceptance [COMPLETED]

Addressed "Via MCP navigate the host chrome browser to news.google.com" and "Let's build an image for the SD card and verify good behavior":

1. **MCP Host Navigation & Visual Verification Acceptance**:
   - Paired against `POST /pair` using PIN `123456`, minting a valid token with `kvm:control`, `kvm:video`, and `kvm:read` scopes.
   - Initialized MCP session (`protocolVersion: "2025-11-25"`).
   - Inspected live 1080p60 H.264 stream: confirmed host was active with Google Chrome in the foreground.
   - Dispatched authenticated keyboard reports over MCP `kvm.keyboard.send`:
     - Focus Omnibox: `Cmd + L` (`key: "l", modifiers: ["cmd"]`)
     - Type URL: `text: "news.google.com"`
     - Navigate: `key: "Return"`
   - Visually verified navigation over live H.264 video frame read-back: confirmed Google Chrome navigated to `https://news.google.com/home?...` and rendered the "Google News - Your briefing" feed.

2. **Full Workspace & Binary Compilation**:
   - All 70 workspace unit tests pass (`cargo test --workspace --locked`).
   - Cross-compiled static RISC-V release daemon `onyx-kvmd` (`image/build-rust-daemon.sh`).
   - Cross-compiled static RISC-V video broker `onyx-kvm-video-broker` (`image/build-rust-video-broker.sh`).
   - Cross-compiled static RISC-V HID broker `onyx-kvm-hid-broker` (`image/build-rust-hid-broker.sh`).
   - Cross-compiled static RISC-V audio capture and playback brokers (`image/build-rust-audio-brokers.sh`).

3. **Complete Production Base SD Image Assembly (`product-v80`)**:
   - Built full product rootfs (`CLEANKVM_CONFIG_PROFILE=product CLEANKVM_INCREMENTAL_ROOTFS=1 ./image/build-phase1-rootfs.sh`).
   - Passed all 7 rootfs security audits:
     - Phase-1 root filesystem audit
     - ext4 identity ownership audit
     - ext4 raw-frame probe ownership audit
     - ext4 non-audio device-node audit
     - ext4 audio capture probe ownership audit
     - ext4 audio playback probe ownership audit
     - ext4 EDID programmer ownership audit
   - Packaged and assembled whole image via `package-phase1.sh` and `assemble-phase1-image.sh`.
   - Output artifacts in `out/product-v80/`:
     - `onyx-kvm-phase1.img`: 79,692,288 bytes, SHA-256 `e91429607c7bc9dcef45a2ab69fd1ea1d47af376246555b08f8ac78094558d0c`
     - `boot.sd`: 10,904,108 bytes, SHA-256 `f624df1de67a6a0b8c3a60af7de9f2e429828bd1383be562aba359b72000625b`
     - `rootfs.ext4`: 62,914,560 bytes, SHA-256 `b5a41ebde8c8c720bb8440932c308c0cda2aca2775cd0dec100b4f62474b30f9`
     - `fip.bin`: 258,048 bytes, SHA-256 `6e6d3b5c7a58c1a585bf1ea456c76bd7c91cfd33c76dfc67bd33b31cf86cd301`
     - `phase1.its`: SHA-256 `0110e80c8a2a3b5dbd0f1dfa744c0eddbd2604ce92314ca1282e450621d35cfa`
   - Embedded FIP, FIT, and rootfs verified with `verify-sd-image.py`.

4. **Target Disk Resolution, Flash & Verified Read-Back**:
   - Removable target disk uniquely resolved: `/dev/disk4` (31.0 GB USB mass storage, `ONYXKVM` volume label).
   - Unmounted and flashed 79,692,288 bytes to `/dev/rdisk4` in 4.34 seconds (18.4 MB/s).
   - Read back the exact 79,692,288-byte span (155,649 sectors).
   - Read-back span SHA-256: `b62ff29c7217d11c9a82ef05cdf5482407e147c2fed1b19d59d674189503fb50` (expected FAT bookkeeping difference).
   - Embedded FIP, FIT, and rootfs artifacts verified bit-for-bit against audited `product-v80` inputs via `verify-sd-image.py` (`embedded_artifacts=verified`).
   - Card safely ejected via `diskutil eject /dev/disk4`.

## Product Release Candidate v79: Host Wake Stream Resume, Probe Segfault Elimination & Remote Wakeup Hardening [COMPLETED]

Addressed "the host was asleep. Wake was used. Streaming did not resume. Stopping and Starting the stream did not change. A reboot followed by using wake stream appeared to work":

1. **Root Cause Analysis: USB Autosuspend, Probe NULL-Deref Segfault & Kernel Lockup**:
   - **USB Remote Wakeup Gate**: When the host OS enters USB suspend, Linux gadget `dwc2` enters L1/L2 sleep. Without bit 5 (`0xa0`) set in gadget config `bmAttributes` and `wakeup_on_write = 1` set on `/sys/kernel/config/usb_gadget/g1/functions/hid.*`, HID report writes failed with `-EAGAIN` (`-11`) and skipped remote wake signaling (`signalling skipped: gadget not in L1/L2 state`), leaving the host sleeping.
   - **Probe Buffer Cleanup Segfault**: In [`image/phase3-raw-frame-probe.c`](../image/phase3-raw-frame-probe.c), when the host goes to sleep and the probe encounters 8 consecutive timeouts (~2.5s), it broke out of its streaming loop. Immediately after the loop, the probe unconditionally executed:
     ```c
     memset(buf_a, 0, OUTPUT_LIMIT);
     memset(buf_b, 0, OUTPUT_LIMIT);
     memset(delta_buf, 0, 8 + 810 * 5122);
     free(buf_a); free(buf_b); free(delta_buf);
     ```
     In H.264 streaming mode, these raw delta buffers were never allocated (`NULL`). Calling `memset(NULL, 0, ...)` caused an instant userspace `SIGSEGV` *before* reaching the hardware cleanup block (`cleanup:`).
   - **Video Buffer (VB) Pool Exhaustion & Kernel Oops**: Because `cleanup:` was bypassed by the crash, `CVI_VI_DisableChn`, `CVI_SYS_Exit`, and `CVI_VB_Exit` were never called. The kernel VB pool 0 retained 2 blocks allocated to VI (`work(2)`). When subsequent probes were spawned, `CVI_VI_EnableChn` failed with `0xc00e8040` (Out of memory). Furthermore, `soph_vi.ko`'s `vi_destory_thread` attempted to stop an uninitialized thread pointer (`ERR_PTR(-11)`), triggering a kernel Oops (`store address misaligned in kthread_stop`) and wedging the task in uninterruptible sleep (`DW`) with `Fixing recursive fault but reboot is needed!`.
   - **Zombie Process Leak**: In [`crates/onyx-kvm-video-broker/src/main.rs`](../crates/onyx-kvm-video-broker/src/main.rs), when probe initialization failed while the MIPI clock was stopped, `stdout.read_exact` returned `Err(EOF)`, exiting via `?` without `child.wait()`. Repeated spawn attempts leaked up to 1,000 zombie processes under PID 223, hitting `pid_max` and preventing any new process forks.

2. **Resolution & Hardening Architecture**:
   - **USB Remote Wakeup**:
     - In [`image/phase2-hid-overlay/etc/init.d/S45onyx-kvm-hid-gadget`](../image/phase2-hid-overlay/etc/init.d/S45onyx-kvm-hid-gadget), set `bmAttributes = 0xa0` and enabled `wakeup_on_write = 1` on both `hid.keyboard` and `hid.pointer`.
     - In [`crates/onyx-kvm-mcp/src/lib.rs`](../crates/onyx-kvm-mcp/src/lib.rs), updated `parse_pointer_action` to safely accept signed integers (`as_i64()`, $-128..=127$ mapped to `(s as i8) as u8`) as well as unsigned.
   - **Probe Null-Deref Elimination**:
     - In [`image/phase3-raw-frame-probe.c`](../image/phase3-raw-frame-probe.c), added explicit null-checks (`if (buf_a) ...`, `if (buf_b) ...`, `if (delta_buf) ...`) before zeroing and freeing. Built offline probe binary `v55` (`36273821405ffe3f3c743eba455c2738a29a595179d8494ddde230943b5442cf`).
   - **Video Broker Child Process Reaping**:
     - In [`crates/onyx-kvm-video-broker/src/main.rs`](../crates/onyx-kvm-video-broker/src/main.rs), added explicit `child.kill()` and `child.wait()` on all probe spawn error paths and added 500ms spawn rate-limiting (`last_spawn_attempt`) to prevent rapid spawn loops when input signal is absent.
   - **Keep-Awake Timing Optimization**:
     - In [`crates/onyx-kvmd/src/web.rs`](../crates/onyx-kvmd/src/web.rs), reduced keep-awake interval from 30s to 5s (`idleMs >= 5000`), reliably beating the 10-15s display turn-off timeout common to standard host lockscreens.
   - **Video Capability Initialization**:
     - In [`image/phase3-video-overlay/etc/init.d/S47onyx-kvm-video-broker`](../image/phase3-video-overlay/etc/init.d/S47onyx-kvm-video-broker), ensured `PATH` includes `/sbin` and initialized `/run/onyx-kvm/video-enabled` gate file on daemon start.

3. **Live Hardware Verification & Test Results**:
   - All 70 Rust workspace unit tests pass (`cargo test --workspace --locked`).
   - Deployed probe `v55` and updated `onyx-kvmd` (`3bf60cc62938bc51ecab79f15a93272a1d71b2b5ad83d4993ba0fb0961c6b2e4`) to live board (`192.168.1.237`). Rootfs verified strictly `ro,relatime`.
   - Verified clean shutdown and restart: when signal drops (`A3=0x88`), probe exits cleanly via `close channel no. 0`, `vi_event_handler exit`, and `vpss_release` with zero memory leaks, zero zombies, and zero kernel Oopses.
   - Verified wake transition: host asleep (`A3=0x88`) -> wake pulse -> host woke up (`A3=0x55`) -> hardware pipeline resumed encoding at 60 FPS (`VIFPS: 60`, `VISofCh0Cnt: 1671+`) without requiring a board reboot.

## Product Release Candidate v78: Host Sleep Detection, Standby Stream Recovery & One-Click Wake [COMPLETED]

Addressed "the system seems to have a hard time when the host goes to sleep terminating the HDMI. We should be able to handle this condition":

1. **Root Cause Analysis: Host Sleep, Pointer Capture Ingestion & Kernel Wait Saturation**:
   - **Pointer Lock Swallowing**: When pointer lock preference was active (`pointerLockPref = true`) and the cursor uncaptured, `mousedown` events on the viewport unconditionally requested pointer lock and returned immediately (`return;`), while `mousemove` was ignored (`if (!locked && pointerLockPref) return;`). Consequently, clicking or moving the mouse on a dark, sleeping screen dispatched zero HID events to the target host. Users perceived the KVM as completely unresponsive.
   - **Kernel Timeout Saturation**: In [`image/phase3-raw-frame-probe.c`](../image/phase3-raw-frame-probe.c), frame acquisition blocked in the kernel for a full 1000ms per attempt (`CVI_VI_GetChnFrame(0, 0, &vi_frame, 1000)`). While the host display was asleep, the `soph_base` driver emitted continuous kernel diagnostic warnings every second (`base_get_chn_buffer:262(): Mod(VI) Grp(0) Chn(0), jobs wait(0) work(2) done(0)`), saturating kernel waitqueues.
   - **Missing Prominent Wake Controls**: The web interface lacked direct wake buttons or sleep indicators, requiring users to manually guess keystrokes.

2. **One-Click Target Wake & Standby Stream Recovery Architecture**:
   - **Probe Fast Timeout & Standby Backoff**:
     - In [`image/phase3-raw-frame-probe.c`](../image/phase3-raw-frame-probe.c), reduced `acquire_output_frame` timeout from 1000ms to 200ms (12 frame intervals at 60 FPS).
     - Added a 250ms sleep backoff (`usleep(250000)`) on hardware frame acquisition failure. In standby, probe CPU load drops to near zero (<0.5%) and kernel warning logs are reduced.
     - Built updated offline probe binary `out/phase3-raw-frame-probe-v53/cleankvm-raw-frame-probe` and updated [`image/build-phase1-rootfs.sh`](../image/build-phase1-rootfs.sh).
   - **Dedicated Wake Controls in Web UI**:
     - In [`crates/onyx-kvmd/src/web.rs`](../crates/onyx-kvmd/src/web.rs), added an interactive `[Wake Target]` button directly inside `#stream-overlay` (`#stream-overlay-wake-btn`) and in the drawer Keyboard & Mouse section (`#wake-target-btn`).
     - Added `pointer-events: auto` to `.stream-overlay-box` with high-visibility accent styling (`.btn-wake`).
   - **Universal Standby Wake Ingestion**:
     - Added `wakeTarget()` which dispatches a Left Shift key tap (`report: [0x02, 0, 0, 0, 0, 0, 0, 0]` -> 60ms -> `[0, 0, 0, 0, 0, 0, 0, 0]`) followed by a relative mouse nudge (`[0, 1, 0, 0]` -> 60ms -> `[0, -1, 0, 0]`).
     - When `inStandby` is true, clicking anywhere on the viewport/overlay or pressing any key immediately invokes `wakeTarget()`, waking the sleeping target host instantly on the first click.
   - **"Keep Host Awake" (Caffeine / Jiggle) Setting**:
     - Added `#opt-keepawake` toggle in the settings drawer, persisted in `localStorage('onyx_keepawake')`.
     - When enabled, a background keepalive timer monitors user activity (`lastUserActivityTs`). If 30 seconds elapse without HID activity, it dispatches an imperceptible 1-pixel micro-nudge (`+1` then `-1`), keeping the target host OS display awake continuously when unattended.
   - **Seamless Hardware Recovery**:
     - When the target host wakes, the video pipeline immediately recovers. WebCodecs VideoDecoder receives keyframes, closes the overlay, and resumes 1080p60 H.264 playback without page refresh or reconnection.

3. **Live Hardware Verification & Test Results**:
   - Added unit test assertions in `crates/onyx-kvmd/src/web.rs` for wake buttons, keepawake persistence, and CSS classes. All 70 workspace unit tests pass (`cargo test --workspace --locked`).
   - Cross-compiled release binaries `onyx-kvmd` and `cleankvm-raw-frame-probe` v53, deployed to live board (`192.168.1.237`), and verified rootfs remounted strictly `ro,relatime`.
   - Verified standby detection: host display sleep triggered carrier unlock (`signalA3: 0x88`), emitted `STREAM_FLAG_STANDBY` frames at 500ms intervals, and kept memory stable.
   - Dispatched `wakeTarget()` via MCP: verified carrier re-locked (`signalA3: 0x55`, `csiWidth: 1920`, `csiHeight: 1080`, `VIFPS: 60`), and over 31,500 frames were ingested without a single kernel Oops or leak.

## Earlier Release Candidates & Hardware Investigation History

Release candidates prior to v78 and historical hardware investigation logs (v35 through v77, MIPI signal measurements, clock sweeps, and discriminator findings) have been archived in [docs/history/EXPERIMENT_HISTORY.md](history/EXPERIMENT_HISTORY.md).
