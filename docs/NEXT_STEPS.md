# Next steps

Updated: 2026-09-12

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

## Ordered next work

1. Recover or locate auditable source for the LT6911UXC EDID path. Sipeed's
   official NanoKVM FAQ instructs Cube/Lite users to run the separate
   `nanokvm_update_edid` utility with the 256-byte `E21_NanoKVM.bin` and then
   power-cycle, corroborating that EDID is persistent bridge state.
2. Validate the retained E21 payload structurally and map the updater's exact
   enable/write/readback/disable sequence. Do not execute the stock utility or
   translate an unexplained register table into CleanKVM.
3. If a bridge write is required,
   admit only the smallest source-backed operation with before/after reads,
   UART visibility, time bounds, and guaranteed cleanup.
4. Only after the bridge emits a CSI clock should bounded VI allocation and
   one-frame copying be re-enabled.
5. Remove temporary reset telemetry after the diagnosis is closed and rebuild
   all admitted modules from the final source tree.
6. Continue Phase 3a separately: inventory the ADC/DAC paths, confirm actual
   microphone/speaker/HDMI-audio routing, and design bounded opt-in brokers.

The public-source search did not find an auditable EDID writer. Public drivers
require pre-flashed Lontium firmware, while Lontium's product brief documents
the embedded MCU/SPI architecture but not its register protocol. The exact
material and admission criteria now live in `docs/LT6911_UPSTREAM_REQUIREMENTS.md`.
Bridge mutation is blocked on that upstream evidence; offline analysis and the
independent OS/control-plane work may continue.

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
