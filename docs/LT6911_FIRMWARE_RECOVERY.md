# LT6911UXC firmware recovery

Updated: 2026-09-13

## Finding

NanoKVM's stock v1.4.2 root filesystem contains no LT6911UXC application
firmware and no firmware loader. Offline extraction found only the E21 EDID,
`libsns_lt6911.so`, and normal application code. The stock startup script
selects the bridge variant and releases reset; it does not load bridge firmware.
The normal design therefore boots an embedded MCU from firmware already stored
in the LT6911UXC's attached/on-chip SPI flash.

The clean v25 test programmed and byte-verified E21, then removed power fully.
The bridge still did not assert a usable HDMI sink, reported signal state
`0x00`, reported zero CSI dimensions, and emitted no MIPI clock. This is
consistent with absent, corrupt, incompatible, or non-running bridge firmware,
but does not distinguish those cases.

## Public material reviewed

- Sipeed NanoKVM commit `bd070b2f5ce27d22618e8a1c8d2bdca543429882`
  publishes an EDID programmer but no LT6911UXC firmware or firmware loader.
- Stock image `20260123_NanoKVM_Rev1_4_2.img` contains no LT6911 firmware
  payload or loader.
- Espressif's Apache-2.0 driver identifies a compatible LT6911 through MCU-stop,
  bank `0xe1`, registers `0x00..0x01`, expecting PID `0x2102`; it does not load
  firmware.
- ZHAW's GPL-2.0 driver requires firmware supplied directly by Lontium and does
  not provide a payload or loader.
- Richard Antalik's reverse-engineering repository contains a 10,007-byte dump
  from an LT6911C display board, SHA-256
  `c4675ca95aafaba94d686464ea9ac237c61c61b15a3f38cf9c7b560144c63e55`.
  It is the wrong bridge variant/use case, has no declared redistribution
  license, and its author states that the upload procedure is unknown. It is
  rejected for NanoKVM.
- Lontium's public EVB guide uses its proprietary
  `Upgrade_Flash_For_Application` PC utility and a USB-to-I2C adapter. It does
  not publish the application payload or external-host flash protocol.

## Required recovery package

Before bridge flash mutation, obtain all of:

1. The exact NanoKVM Lite/Cube-B LT6911UXC application firmware supporting the
   board's four-lane YUV422 CSI mapping and E21 EDID/HPD behavior.
2. An immutable version identifier, payload SHA-256, and redistribution terms.
3. A documented I2C flash protocol or corresponding loader source, including
   erase geometry, address bounds, page size, timing, status/error decoding,
   watchdog/MCU handling, and reset/power-cycle sequence.
4. A read-back verification method and a documented recovery path for an
   interrupted erase/write.
5. Expected post-flash chip ID, firmware version, HDMI signal-state, HPD, CSI
   dimensions, output format, lane count, and MIPI clock registers.

The CleanKVM loader must be independently compiled, accept only the pinned
payload, back up readable bridge state before erase, refuse unexpected silicon,
write only the documented flash range, compare every programmed byte, restore
MCU/watchdog state on failure, remain root-only and local-only, and never start
automatically or appear through MCP.

## Current safety state

No bridge application firmware erase or write has been attempted. Only the
source-backed E21 EDID area was changed and verified. The incompatible public
LT6911C dump and proprietary Windows updater remain unexecuted and excluded.
