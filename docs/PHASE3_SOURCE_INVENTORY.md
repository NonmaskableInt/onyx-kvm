# Phase 3 preliminary source inventory

This is a discovery inventory, not an allowlist. Paths are relative to the
quarantined pinned SG2002 SDK checkout. A component enters a Phase 3 image only
after its complete dependency and license closure is recorded.

## LT6911 and video path

- The board SDK configuration enables `CONFIG_SENSOR_LONTIUM_LT6911` but also
  enables several unrelated camera sensors. It is not a least-privilege media
  configuration. File SHA-256:
  `76e937fbe773c8b5e2b8ea2962ba93d7f285adcdecc81ee09725a54c00e0683a`.
- `middleware/v2/component/isp/sensor/cv183x/lontium_lt6911/` contains the
   SG2002-family sensor adapter. The control source names `/dev/i2c-3`, address
  `0x2b`, bank register `0xff`, chip-ID registers `0xa000`/`0xa001`, and expected
  ID `0x1605`. This is source evidence only; the actual Lite bus and address
  still require a non-mutating hardware probe.
- Initial LT6911 source hashes:
  - `lt6911_sensor_ctl.c`:
    `8776086b879aff5928b870d2e970ff1ae5202b8c87c409d819d8f2d44c8913da`
  - `lt6911_cmos.c`:
    `e67bf5bedc1aa3839bbc5938e8b57800dda22f9829060a2f43579a222c834c63`
  - `lt6911_cmos_param.h`:
    `e72b08991b530a26a6d369b30ef78ddd16fbe4c93324ea7de30d71912e6b920e`
- The similarly named `cv182x` adapter instead selects `/dev/i2c-4` and address
  `0x56`. That conflict is a warning against copying either setup into the
  image without hardware evidence.
- The generic LicheeRV Nano DTS has SHA-256
  `0797073574e69c994ab51116b8630eece4516fae9e90c104dc189d6046bdb0b5`.
  It exposes a MIPI-RX sensor reset on GPIO E1 but has no explicit LT6911 node,
  interrupt description, or audio route. It is insufficient PCB evidence.
- The reviewed stock baseline loaded `soph_vi`, `soph_vpss`, `soph_vcodec`,
  `soph_mipi_rx`, `soph_rtos_cmdqu`, `soph_base`, and `soph_sys`. Exact module,
  library, firmware, and license correspondence remains an admission blocker.

## Phase 3a audio path

- Stock evidence reported one `cv182xa_adc` capture PCM and one `cv182xa_dac`
  playback PCM. This proves Linux exposure, not the physical microphone,
  speaker, HDMI-audio, or USB-audio route.
- The SDK contains CVITEK ASoC ADC and DAC components. Initial hashes are:
  - `sound/soc/cvitek/cv182xadc.c`:
    `0b1c12adb3b0ef004f658a20307422b7e03907e44ef11003260d708c80ac1468`
  - `sound/soc/cvitek/cv182xdac.c`:
    `f09333d9c3facc2dd3540234f574f2b3ee55b9a48d189a71daaa9a1b88bc81dd`
  - `sound/soc/cvitek/cv182x_cv182xadc.c`:
    `df6229250ff354d664761378fe87367acf8c7dd14057c0f434ac8797132ebe89`
  - `sound/soc/cvitek/cv182x_cv182xdac.c`:
    `d8b2a06f8e25a3109a59130460c129099e3a8b8e8b1c42234630c074434e2506`
- The generic DTS enables the DAC but does not document a Lite-specific output
  load or mute GPIO, and it does not establish the microphone route.
- No current evidence establishes HDMI audio extraction or a routed USB Audio
  Class path. Both remain unsupported until signal-level tests pass.
- Sipeed documents an onboard analog microphone on the shared LicheeRV Nano
  design. Its NanoKVM documentation also says MIC/amplifier circuitry was
  retained for production compatibility while later software removed the
  drivers because of privacy concerns.
- The pinned product kernel configuration enables the CV182XA ADC/DAC machine
  and codec drivers, internal I2S clocking, generic USB sound, USB Audio, and
  UAC1. CleanKVM currently disables all of them.
- The product DTS enables the DAC and removes the reference mute GPIO. Its
  inherited QFN description retains the internal ADC/DAC cards while deleting
  PDM and two external-I2S paths. This supports a tightly scoped ADC/DAC-only
  enumeration image; it does not establish USB Audio or HDMI-audio routing.
- The staged admission and privacy requirements are recorded in
  `docs/PHASE3A_AUDIO_ADMISSION.md`. No PCM may be opened during the initial
  enumeration test.

## Next evidence gates

1. Collect a read-only Phase 3 hardware inventory over UART: live DTS nodes,
   I2C adapter names, media/video device nodes, reserved-memory layout, loaded
   modules, ALSA card/PCM identities, and relevant GPIO labels.
2. Reconcile the result with PCB markings and the stock baseline without
   writing LT6911 or audio registers.
3. Construct minimal video-only and audio-only kernel fragments. Diff each
   normalized configuration against Phase 2 and reject unrelated subsystems.
4. Close all admitted vendor userspace libraries recursively before building a
   broker-linked image.

## Clean baseline hardware result

The Phase 2 v8 image was probed over authenticated local UART after Phase 3
started. It reported model `LicheeRv Nano`, compatible `cvitek,cv181x`, no I2C
adapters, no video/media/CVITEK media nodes, no relevant loaded modules, and no
ALSA cards or PCM devices. Reserved-memory nodes were limited to `cvifb`, `ion`,
and `mmode_resv0@80000000`. This proves the prior image remains a useful
zero-exposure baseline; it does not identify the physical LT6911 bus.

The next discovery image therefore adds only built-in DesignWare-platform I2C
and root-only `i2c-dev`. Media, sound, I2C slave, I2C mux, GPIO-I2C, UVC, and UAC
remain disabled. Its normalized kernel configuration is checked before build.

The resulting 79,692,288-byte discovery-v1 image has SHA-256
`4b49288f0c0fbad5b9cd11c85c587fb2adb50b65dbd0b7a7214a581cb5755b4d`.
Its root filesystem passed the existing credential, identity, executable, and
prohibited-media/audio audits. The kernel build compiled the DesignWare master,
common, platform, I2C core, and `i2c-dev` objects; it did not compile media or
sound objects.

The discovery-v1 hardware boot passed. Linux enumerated DesignWare adapters 0,
1, 3, and 4, mapped directly to device-tree controllers `i2c@04000000`,
`i2c@04010000`, `i2c@04030000`, and `i2c@04040000`; controller 2 remained
absent. Media nodes, ALSA cards/PCM devices, and relevant modules remained
absent, so the profile did not accidentally admit the media stack.

Offline inspection of retained stock image
`20260123_NanoKVM_Rev1_4_2.img.xz` resolves the earlier SDK ambiguity for this
product family. Its `S15kvmhwd` script probes bus 4 at address `0x2c` for the
LT6911C variant, then address `0x2b` for LT6911UXC. Its unexecuted, dynamically
linked `nanokvm_update_edid` ELF independently embeds `/dev/i2c-4` and names
both variants. This is strong board-software evidence for controller 4 and the
two 7-bit addresses. No vendor binary or script was copied into or executed by
CleanKVM.

An explicitly approved, narrowly bounded live receive-byte probe then checked
only bus 4 addresses `0x2b` and `0x2c`. Address `0x2b` acknowledged and `0x2c`
did not. Together with the two independent stock-image clues, this promotes
bus 4/address `0x2b` to the hardware target and identifies the populated
variant as LT6911UXC with high confidence. The probe did not scan any other
address and did not issue a register write. Reading the banked silicon-ID
registers would require state-changing bank/select writes and remains deferred.

Further read-only extraction recovered `/mnt/data/sensor_cfg.ini.LT`. It names
`LONTIUM_LT6911_2M_60FPS_8BIT` and selects bus 4, MIPI device 0, lane order
`2,4,3,1,0`, PN swaps `0,0,0,0,0`, and no MCLK. The reviewed SDK INI parser
parses each list element and stores it directly; the sensor setup later copies
those runtime values into the MIPI attributes. This product configuration takes
precedence over the generic LT6911 adapter defaults used by the v21 diagnostic.

The stock `S15kvmhwd` init path exports GPIO451, configures it as an output, and
leaves it high. It contains no bridge firmware load, EDID programming, or reset
pulse. The separate, unexecuted `nanokvm_update_edid` ELF contains LT6911C and
LT6911UXC EDID read/write routines, targets `/dev/i2c-4`, and toggles GPIO451,
but no reviewed normal-startup script invokes it. The stripped `kvm_system` ELF
references hardware-version detection and MIPI-driver replacement but yielded
no string evidence of an automatic EDID update. This evidence supports testing
the product lane profile before admitting any LT6911 register write.

The retained `/kvmapp/system/tool/E21_NanoKVM.bin` was extracted as data only
and validated offline. It is a structurally valid two-block EDID: 256 bytes,
standard `00ffffffffffff00` header, one declared extension, and a zero checksum
for each 128-byte block. SHA-256 is
`526e33a7d63d8deddbcbb00bc3b89a3a88fae1059f8d16a5b5b3fbd28742146e`.
The payload includes `NanoKVM` display descriptors. This closes payload
integrity but does not admit the stock updater or its unexplained register
sequence.
