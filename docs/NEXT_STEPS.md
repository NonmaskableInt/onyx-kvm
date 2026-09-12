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
- The leading receiver hypothesis is now the incorrect product wiring profile,
  not a proven LT6911 firmware or power failure.

## Ordered next work

1. Flash and boot the completed status-only v22 image, then verify
   `/proc/mipi-rx` for a high-speed clock,
   nonzero physical samples, lane state, and decoded YUV422 format. Do not enter
   VI or allocate frame buffers yet.
3. If v22 remains idle, finish mapping every stock action affecting LT6911
   power, reset, clocks, EDID, HPD, interrupt pins, I2C, or CSI output to source
   where available, and prefer a read-only comparison against a stock boot.
4. If a bridge write is required,
   admit only the smallest source-backed operation with before/after reads,
   UART visibility, time bounds, and guaranteed cleanup.
5. Only after the bridge emits a CSI clock should bounded VI allocation and
   one-frame copying be re-enabled.
6. Remove temporary reset telemetry after the diagnosis is closed and rebuild
   all admitted modules from the final source tree.
7. Continue Phase 3a separately: inventory the ADC/DAC paths, confirm actual
   microphone/speaker/HDMI-audio routing, and design bounded opt-in brokers.

## Explicitly deferred

- Guessing or copying an opaque LT6911 register table.
- Enabling video or audio through MCP before local hardware admission.
- Restoring arbitrary physical-memory access or executing stock utilities.
- Treating the `0000` ID as proof of a specific silicon failure without a
  stock-state comparison.

## Prepared next image

- Path: `out/phase3-mipi-status-v22/cleankvm-phase1.img`
- Size: 79,692,288 bytes
- SHA-256:
  `7162fa38bf50bcb248a7b14f8fc58f8464d87d26ff287d43b7974b87c3f1b98e`
- The build was performed with networking disabled. Source-cache verification,
  normalized video-foundation configuration, rootfs audit, identity ownership,
  diagnostic ownership, FIT hashes, embedded artifacts, and final image hashes
  all passed.
