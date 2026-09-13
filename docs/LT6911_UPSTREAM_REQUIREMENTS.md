# LT6911UXC upstream requirements

Updated: 2026-09-12

## Why this is a release blocker

CleanKVM has proved that the NanoKVM Lite LT6911UXC acknowledges on I2C bus 4
at address `0x2b`, its reset line works, the SoC MIPI pads are muxed correctly,
and the exact stock receiver profile is accepted. Both reset-pulse and
power-on-state tests report no CSI clock or lane activity. The attached Mac also
does not enumerate NanoKVM as a display, indicating absent usable HPD/EDID.

The retained stock image contains a valid 256-byte E21 EDID and an updater with
symbols, but not corresponding source. Executing or translating that opaque
binary would violate the independent-image and source-audit boundary.

## Public evidence

- Lontium's LT6911UXC product brief says automatic operation uses an integrated
  processor and embedded SPI flash, and that firmware updates use the
  configuration I2C interface. It documents an integrated EDID shadow but no
  register map or programming protocol:
  <https://www.lontiumsemi.com/UploadFiles/pdf/LT6911UXC_Product_Brief.pdf>
- Sipeed's NanoKVM FAQ instructs Cube/Lite users to run the binary
  `nanokvm_update_edid` with `E21_NanoKVM.bin` and then power-cycle:
  <https://github.com/sipeed/sipeed_wiki/blob/main/docs/hardware/en/kvm/NanoKVM/faq.md>
- The ZHAW open-source LT6911UXC driver explicitly requires separately flashed
  Lontium firmware and does not supply an EDID writer:
  <https://github.com/InES-HPMM/Lontium_lt6911uxc>
- The public reverse-engineering survey concludes that a complete register map
  and firmware/programmer are unavailable and recommends obtaining them from
  Lontium:
  <https://github.com/JakubVanek/lt6911uxc-hacks>

## Minimum acceptable upstream material

One of the following is required before CleanKVM writes bridge state:

1. Source code, under a compatible redistributable license, for the exact
   LT6911UXC EDID enable/write/readback/disable sequence used by NanoKVM Lite.
2. A public register-level programming specification from Lontium covering the
   same sequence, including access locking, shadow commit, HPD behavior, timing,
   failure recovery, and write endurance.
3. A signed, redistributable firmware artifact plus a documented source-built
   loader protocol and a reproducible method to identify its version and hash
   after programming.

The material must also identify the NanoKVM Lite bridge firmware version,
whether E21 is written to volatile shadow or persistent SPI, and whether HPD is
asserted automatically after a valid EDID commit.

## Admission test after source arrives

- Validate the E21 bytes and both EDID block checksums before opening I2C.
- Refuse every bus/address except bus 4/address `0x2b`.
- Snapshot documented status/version registers before mutation.
- Apply only the documented unlock, bounded 256-byte write, commit, readback,
  and lock sequence.
- Compare all 256 read-back bytes and fail closed on any mismatch.
- Power-cycle once, confirm macOS display enumeration, then run the status-only
  CSI gate before enabling VI or allocating a frame.
- Keep bridge programming local-only; do not expose it through MCP.

