# LT6911UXC firmware recovery

Updated: 2026-09-13

## Finding

NanoKVM's stock v1.4.2 root filesystem contains no separately identifiable
LT6911UXC application firmware and no firmware-upgrade loader. Offline
extraction found the E21 EDID, `libsns_lt6911.so`, and stripped normal
application code. The stock startup script selects the bridge variant and
releases reset; it does not load bridge firmware.
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
- Stock image `20260123_NanoKVM_Rev1_4_2.img` contains no separately
  identifiable LT6911 firmware payload or upgrade loader. Its stripped
  `libkvm.so` contains a `lt6911_read_fw` diagnostic, so an embedded opaque
  payload cannot be ruled out solely from filenames or strings; no upgrade
  call or payload has been found.
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

## Stock behavioral reference, without execution

The extracted stock startup path copies `sensor_cfg.ini.LT` into place, then
launches the stripped `kvm_system` and server programs. The configuration is
bus 4, MIPI device 0, lane order `2,4,3,1,0`, no polarity swaps, no MCLK, and
60 fps. The hardware script leaves GPIO451 (the documented HDMI reset signal)
high; it does not pulse reset or program the LT during ordinary startup.

The stock `libkvm.so` symbol and string tables match Sipeed's published
`kvm_vision.cpp`, including `lt6911_enable`, `lt6911_disable`, status and
dimension reads, EDID access, and `lt6911_read_fw`. The UXC normal path stops
the embedded MCU through bank `0x80`, register `0xee = 1`, reads bank `0x86`
register `0xa3` for signal state and bank `0x85` registers `0xea`/`0xf0` for
CSI dimensions, then resumes the MCU with `0xee = 0`. It does not contain an
application-firmware upload in the published path.

The published `lt6911_read_fw` function uses three address bytes and surveys
49,999 16-byte records after address zero. The v26 reproduction returned the
setup byte `0x51` for every address in two passes. That zero-entropy output is a
stable controller artifact, not a firmware backup, and is rejected. The v26
reader SHA-256 was
`24df815d4e32e003b37ad013de950cd49a73ecf2a0c64978a9c6dd1d4c05da6d`;
its rejected output SHA-256 was
`4308ea800a49ddcac2214fc1b35e3afacd7903b8b21a7028b8d478dccb06372b`.

The corrected candidate instead generalizes the source-backed 32-byte EDID
read transaction already proven on this hardware. It changes only the three
address bytes across the same bounded 800,000-byte range, reads twice, requires
byte-identical results with at least four distinct byte values, uses an
exclusive fixed output path, and restores the MCU after each pass. It contains
no erase/program opcode and is never started automatically.

The v28 hardware evidence showed that exactly the first 32-byte transaction
differed while the remaining 799,968 bytes matched. The first samples were
`024d0190b00b7410f0222202554cffff` and
`00001e0000000000000000000000024d`. This localizes the problem to stale
pipeline data immediately after each controller setup.

v29 therefore adds one discarded address-zero read after setup, then rereads
address zero as the first compared block. It introduces no register operation
and no erase/program path. Its independently built image SHA-256 is
`01047bab4733d6f312b43f4192753d47db841c490f5b691aced370d7747285b6` and
the installed reader SHA-256 is
`5a08f5ad68212d42e07877c8d1bac89b1ac90babb1d912334e573ecf61c8d607`.
Both full hardware reads agreed. The accepted 800,000-byte result has 255
distinct values, 46,620 bytes other than `0xff`, and 797,282 nonzero bytes.
The device and owner-only host copy both hash to
`e4b1bd5944264ab8f192025056f849558dd30d37d00ed404e47a59f3e4ba25a5`.
The test started and ended with kernel taint zero, loaded no modules, left no
I2C descriptor open, and performed no flash mutation. This is now the preserved
resident-flash evidence baseline, not yet an authenticated vendor release.

Sipeed's pinned current UXC polling source establishes that status reads are
made only after stopping the embedded MCU (`0x80:0xee=1`) and are followed by a
resume (`0x80:0xee=0`). The earlier CleanKVM status harness omitted that state
transition, so its `0x00` signal and zero dimensions were not valid runtime
evidence. The corrected source-exact sequence returned signal `0x55`, Vactive
`0x0438`, and Hactive `0x0780`, proving the resident application is running and
recognizes stable 1920x1080 HDMI. The MCU resumed successfully, no I2C handle
remained open, and taint was unchanged. Firmware recovery/programming is no
longer the leading path; CSI-output timing and receiver synchronization are.

Offline, non-executing structure analysis finds two byte-identical slots at
flash offsets `0x00000` and `0x80000`. Each occupied span is `0x18120` bytes
and hashes to
`07df59ea6e7cc79f46840317abe0d48b61b11ba89ecd4718e389e677d7c1d11e`.
The first `0x18000` bytes contain 8051-style code and LT HDMI, MIPI, audio, and
firmware-version diagnostic strings. Bytes `0x18000..0x180ff` are a valid
two-block NanoKVM E21 EDID: both block checksums are zero and its SHA-256 is the
pinned published E21 hash
`526e33a7d63d8deddbcbb00bc3b89a3a88fae1059f8d16a5b5b3fbd28742146e`.
The remaining 32 bytes of each occupied span are zero. All bytes from
`0x18120..0x7ffff` and after `0x98120` through the surveyed range are erased
`0xff`. No extracted byte is committed, loaded, or executed.
