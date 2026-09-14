# Next steps

Updated: 2026-09-13

## Current state

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

## Ordered next work

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
  Next: flash v33 and require the explicit baseline gate before module loading.

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
