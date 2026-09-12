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

## Ordered next work

1. Finish mapping every stock action affecting LT6911
   power, reset, clocks, EDID, HPD, interrupt pins, I2C, or CSI output to source
   where available, and prefer a read-only comparison against a stock boot.
2. Capture a read-only stock-boot comparison of bridge acknowledgement, HDMI
   source/HPD behavior, and `/proc/mipi-rx` with the same HDMI cable and source.
3. If a bridge write is required,
   admit only the smallest source-backed operation with before/after reads,
   UART visibility, time bounds, and guaranteed cleanup.
4. Only after the bridge emits a CSI clock should bounded VI allocation and
   one-frame copying be re-enabled.
5. Remove temporary reset telemetry after the diagnosis is closed and rebuild
   all admitted modules from the final source tree.
6. Continue Phase 3a separately: inventory the ADC/DAC paths, confirm actual
   microphone/speaker/HDMI-audio routing, and design bounded opt-in brokers.

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
