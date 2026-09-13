# Phase 3a audio and microphone admission

Updated: 2026-09-12

## Security decision

Microphone capture, local playback, HDMI-audio extraction, and USB Audio Class
are four separate capabilities. They remain absent by default and are not
implicitly admitted with video. Microphone capture is a privacy-sensitive
mutation boundary even when no network stream is active.

## Hardware and stock evidence

- Sipeed documents LicheeRV Nano with an onboard analog silicon microphone and
  standard ALSA recording/playback:
  <https://wiki.sipeed.com/hardware/en/lichee/RV_Nano/5_peripheral.html>.
- Sipeed's NanoKVM introduction says the KVM hardware retained MIC and amplifier
  circuits for production compatibility and that later application/firmware
  releases removed their drivers because of privacy concerns:
  <https://wiki.sipeed.com/nanokvm>.
- The SG2002 data sheet exposes a 16-bit codec, I2S, and DMIC capability. The
  LT6911UXC product brief separately exposes I2S and SPDIF digital audio output;
  this does not prove either signal is routed on NanoKVM Lite.
- The pinned board kernel configuration broadly enables ALSA, USB Audio,
  `SND_USB_AUDIO`, the CV182XA ADC/DAC machine and codec drivers, and UAC1.
- The board DTS explicitly enables the internal DAC and removes its reference
  mute GPIO. The inherited QFN description retains the internal ADC/DAC sound
  cards but deletes PDM and two external-I2S sound paths.
- The retained stock init script does not load separate audio modules. Prior
  stock runtime evidence exposed `cv182xa_adc` capture and `cv182xa_dac`
  playback PCMs, consistent with built-in drivers.
- CleanKVM's current normalized kernel profile sets `CONFIG_SND=n` and disables
  UAC1/UAC2. Hardware boots expose no ALSA cards or PCM devices.

## What is not yet established

- Whether the populated NanoKVM Lite revision actually includes the microphone
  and amplifier components described for the shared LicheeRV Nano design.
- Whether the LT6911UXC I2S/SPDIF outputs are physically routed to SG2002.
- Whether the internal DAC reaches a connector or only an unpopulated amplifier
  path.
- Required mute/power GPIOs and their safe inactive levels on this board.

## Staged admission

### A. Enumeration-only image

- Enable only ALSA core/PCM (including its mandatory timer support), the
  CV182XA ADC/DAC codec and machine drivers, and the required DMA/clock/reset
  support.
- Keep USB Audio, generic USB sound, PDM, external I2S endpoints, sequencer,
  MIDI, OSS compatibility, network audio, and all media/video modules disabled.
- Do not autoload or open either PCM. Expose nodes root-only and collect only
  card identity, supported formats/rates/channels, device-tree status, and mixer
  control names over UART.

### B. Local microphone proof

- Require explicit local authorization for each capture.
- Capture a fixed maximum of one second into locked, bounded memory; do not
  write persistent storage or open a network socket.
- Report only structural metrics initially (frames, clipping, RMS/noise floor),
  then zero the buffer. Exporting raw audio requires a separate approval.
- Confirm that capture stops, DMA is released, nodes remain root-only, and no
  background process retains the PCM.

### C. Local playback proof

- Treat DAC enablement independently from capture.
- Establish amplifier/mute routing first and default it muted.
- Use a bounded synthesized tone at conservative amplitude, then mute, drain,
  release, and clear buffers.

### D. USB and MCP exposure

- Do not add UAC until local capture/playback paths pass and the user explicitly
  selects the direction. USB microphone and speaker functions must be separable.
- MCP may report audio capability/status while disabled, but may not enable,
  capture, play, or retrieve audio until a dedicated authenticated broker and
  policy are admitted.
- Audio sessions require short leases, explicit direction, byte/time/rate
  bounds, one active client, cancellation, zeroization, and an audit event.

## Next implementation step

Create a normalized `phase3a-audio-enumeration` kernel fragment and validator.
Build it as a separate image from video diagnostics, inspect its normalized
diff, and audit that no UAC, USB sound, PDM, external I2S, or unrelated media
surface enters the root filesystem. Hardware loading or PCM access will require
fresh explicit approval.

## Enumeration-v1 hardware result

The explicitly authorized image booted successfully. Kernel taint remained
zero, no loadable module or media node existed, and no UAC function was present.
Ethernet acquired `192.168.2.64`; `cleankvmd` and the HID broker remained live.

ALSA enumerated `cv182xaadc` as card 0 with capture PCM `pcmC0D0c` and
`cv182xadac` as card 1 with playback PCM `pcmC1D0p`. Both controls, both PCMs,
and the mandatory timer node were owned by UID/GID 0 with mode `0600`. The
device tree exposed `sound_adc`, `sound_dac`, ADC/DAC register nodes, internal
I2S nodes, its master clock, and the I2S subsystem. A full process-FD audit
found no open `/dev/snd` node. No PCM was opened during either evidence pass.

Stage A therefore passes. Stage B remains a separate operation: implement and
offline-test a bounded local capture probe before any microphone PCM is opened.

The Stage B probe is now built from pinned TinyALSA v2.0.0 source. It is a
46,976-byte static, stripped RISC-V ELF installed root-only (`0500`), is not
referenced by init, and cannot be invoked by the unprivileged daemon. It fixes
capture to card 0/device 0, mono S16_LE at 16 kHz, exactly 16,000 frames and
32,000 bytes. A four-second alarm bounds a stalled driver. Output is limited to
frame/byte/format counts plus peak, mean-absolute, integer RMS, clipped-sample,
and zero-sample metrics. The locked sample buffer is explicitly zeroized and
unlocked on every normal or interrupted return; no sample is written or sent.

The resulting v2 image is `out/phase3a-capture-v2/cleankvm-phase1.img`, SHA-256
`a60d95083c7d8f97181d02175005be85da563ba4b70e83ef3aff0f480e8399c2`.
All offline rootfs, ownership, FIT, and embedded-artifact audits passed. Its
31 GB card read-back verified every embedded artifact before safe ejection;
macOS FAT bookkeeping produced whole read-back hash
`3f4d216e0067c554fcb9880be08973704451f45d489aa2624b4d5b4831fbd78b`.
