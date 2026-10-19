# LT6911UXC upstream requirements

Updated: 2026-09-13

## Resolution

Onyx KVM has proved that the NanoKVM Lite LT6911UXC acknowledges on I2C bus 4
at address `0x2b`, its reset line works, the SoC MIPI pads are muxed correctly,
and the exact stock receiver profile is accepted. Both reset-pulse and
power-on-state tests report no CSI clock or lane activity. The attached Mac also
does not enumerate NanoKVM as a display, indicating absent usable HPD/EDID.

Sipeed published the corresponding source under GPL-3.0 in the official NanoKVM
repository. Onyx KVM pins commit `bd070b2f5ce27d22618e8a1c8d2bdca543429882`;
the reference source SHA-256 is
`68829704293fbbb050f4ccb95b8bae7e891d7c71c4ff1d95ca64c7a66f93df06`.
This closes the protocol-source blocker. Hardware admission is still pending.

## Public evidence

- Lontium's LT6911UXC product brief says automatic operation uses an integrated
  processor and embedded SPI flash, and that firmware updates use the
  configuration I2C interface. It documents an integrated EDID shadow but no
  register map or programming protocol:
  <https://www.lontiumsemi.com/UploadFiles/pdf/LT6911UXC_Product_Brief.pdf>
- Sipeed's NanoKVM FAQ instructs Cube/Lite users to run the binary
  `nanokvm_update_edid` with `E21_NanoKVM.bin` and then power-cycle:
  <https://github.com/sipeed/sipeed_wiki/blob/main/docs/hardware/en/kvm/NanoKVM/faq.md>
- The official repository now includes the utility source and E21 payload:
  <https://github.com/sipeed/NanoKVM/tree/main/tools/nanokvm_update_edid>
- The ZHAW open-source LT6911UXC driver explicitly requires separately flashed
  Lontium firmware and does not supply an EDID writer:
  <https://github.com/InES-HPMM/Lontium_lt6911uxc>
- The public reverse-engineering survey concludes that a complete register map
  and firmware/programmer are unavailable and recommends obtaining them from
  Lontium:
  <https://github.com/JakubVanek/lt6911uxc-hacks>

## Source review outcome

The upstream UXC path suspends the MCU, erases the EDID area, checks and
acknowledges status `0xee`, writes eight 32-byte EDID pages plus a zero metadata
page, checks status again, and reads all eight pages back. Onyx KVM limits its
derivative to the hardware-established UXC endpoint and adds an exact argument
gate, pre-read, checksum validation, matching-payload no-op, byte comparison,
and unconditional MCU resume. Arbitrary payloads, other chip variants,
shell-based GPIO reset, and interactive mode were removed.

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
