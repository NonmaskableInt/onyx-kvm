# Phase 3a audio and microphone admission

Updated: 2026-10-05

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

## Current v51 hardware-capture candidate

The Stage B proof has been rebuilt on the current authenticated web/MCP and HID
foundation as `out/phase3a-web-capture-v51/cleankvm-phase1.img` (79,692,288
bytes, SHA-256
`f7725d253b1739eca6c077bb632b54207cf3261badb5c17640c3eb0559e123a7`).
The embedded FIT, rootfs, and FIP hashes are respectively `1b504a2c...`,
`9295c98e...`, and `6e6d3b5c...`; all passed offline verification. The installed
root-only capture probe matches the previously audited binary (SHA-256
`55f10830fa0589c75d597f328a94b40a5105f64124d019b50254dab68c34c652`).

The reusable SDK workspace initially retained the Phase 3 video broker from an
earlier overlay. The rootfs audit failed closed and no image was accepted.
`image/build-phase1-rootfs.sh` now removes every optional probe plus video
broker, startup script, and quarantine-module directory from both overlay and
incremental target before restoring only explicitly selected artifacts. The
corrected v51 rootfs audit and audio-probe ownership audit passed.

The v51 image was flashed to the sole external removable target (`/dev/disk4`,
USB, 30,992,760,832 bytes) and safely ejected after exact-span read-back. macOS
FAT bookkeeping changed the whole-span hash to
`8b934bf8d6b11af1c1541a96db80622b7ad3ce2837526a78046f7dff9837bbed`,
but embedded FIT, rootfs, and FIP verification passed against the audited v51
artifacts.

The remaining authorized hardware operation is limited to Stage B capture
metrics: establish a cold-boot baseline with no video modules and both audio
gates absent, invoke the probe once locally as root, verify the one-second bound
and structural metrics, then confirm no process retains a sound device. No raw
PCM export, audio playback, HDMI-audio work, UAC exposure, MCP audio tool, or
web audio control is admitted by this image.

## Stage B v51 hardware result

The flashed v51 image cold-booted at `192.168.2.99` with kernel taint `0`,
`/dev/mem` absent, loadable-module support disabled at the kernel boundary,
video/microphone/speaker gates absent, no media nodes, no UAC function, and no
process holding a sound-device descriptor. The ADC and DAC enumerated as
`cv182xaadc` card 0 and `cv182xadac` card 1; all five sound nodes were owned by
root with mode `0600`. The installed capture probe matched the audited SHA-256
`55f10830fa0589c75d597f328a94b40a5105f64124d019b50254dab68c34c652`,
and its internal zeroization/math self-test passed.

One explicitly authorized local capture completed successfully in about 1.01
seconds with `CAPTURE_RC=0`: 16,000 mono S16LE frames, 32,000 bytes, 16 kHz,
peak absolute sample 32,767, mean absolute sample 10,990, integer RMS 13,098,
zero clipped samples, and zero zero-valued samples. No raw sample was written or
sent. After return, the PCM status endpoint was unavailable (closed), a complete
process-FD scan found no sound-device handle, no probe process remained, and no
audio/PCM/WAV output file existed under `/run` or `/tmp`. Kernel taint remained
`0`; both audio gates remained absent.

Stage B therefore passes its bounded local microphone proof. This admits the
physical capture path only as evidence; it does not enable background capture,
network retrieval, MCP/web audio tools, playback, HDMI audio, or UAC. Stage C
playback remains a separate boundary and must first establish amplifier/mute
routing and a safe inactive state.

## Stage C routing attribution and v52 candidate

The exact compiled v51/v52 device tree enables the internal SG2002 DAC but
contains neither `mute-gpio-l` nor `mute-gpio-r`. The inherited QFN description
defines only right-channel mute on GPIO E2, and the LicheeRV Nano board DTS
explicitly deletes it. Consequently, the pinned DAC driver's `muteAmp()` has no
valid external GPIO on this board. Its reliable shutdown boundary is internal:
the `DAC Playback MUTE` control plus clearing DAC and I2S receive enable during
PCM stop/close.

Official Sipeed hardware documentation identifies an onboard PA capable of
driving a speaker below 1 W. The official schematic identifies it as AW8010A
U11 with hardware-biased enable, confirming there is no software amplifier
shutdown line to recover through the current DT. Stage C must therefore remain
short and conservative and must restore the internal mute before PCM close.

A separate root-only probe implements that bounded test. It selects card 1,
device 0; asserts both internal mute channels; selects the inverted ALSA
volume's lowest nonzero DAC setting; emits a one-second, 1 kHz mono S16LE tone
at 16 kHz with peak amplitude 512 (1.56% of full scale); drains; reasserts mute;
closes the PCM; and zeroizes its locked 32,000-byte buffer. A four-second alarm
bounds driver stalls, and every cleanup path attempts safe mute. The probe is a
static stripped RISC-V ELF (SHA-256
`b9b6d69a9d30d056fc9642117ea2708f273565262a53377074d7a7f9d7b0c256`),
installed root-only (`0500`) and absent from init.

The separately verified v52 image is
`out/phase3a-playback-v52/cleankvm-phase1.img` (79,692,288 bytes, SHA-256
`46e3116e5e011f1173ed7b40bb985d791f55caef0fd8d293d0f25b10d2c2e545`).
Its FIT (`1b504a2c...`), rootfs (`e1d6707c...`), and FIP (`6e6d3b5c...`)
passed embedded-artifact verification. The next bounded operation is SD
flash/read-back followed by cold-boot, probe self-test, one tone, and post-test
mute/PCM/file/taint verification. No playback broker or network operation is
present.

The image was then flashed to the confirmed prior CleanKVM card (`/dev/disk4`,
USB, 30,992,760,832 bytes, existing 16.8 MB `CLEANKVM` boot and 62.9 MB Linux
root layout). Exact-span read-back produced whole-image SHA-256
`aaafba1076b7e2d63d75aef01085d48de85fe96cb73e7b574649f328863e36d2`
after macOS FAT bookkeeping, while embedded FIT, rootfs, and FIP verification
passed against the audited v52 artifacts. The card was safely ejected.

## Stage C v52 hardware result

v52 cold-booted at `192.168.2.100` with taint `0`, loadable-module support
kernel-disabled, `/dev/mem` absent, and video/microphone/speaker gates absent.
The installed playback and capture probes matched their audited SHA-256 hashes;
the playback probe was root-owned and mode `0500`. Its internal tone-generation
and zeroization self-test passed.

The single authorized playback completed with `PLAYBACK_RC=0` in about 1.01
seconds: 16,000 mono S16LE frames (32,000 bytes), 16 kHz, 1 kHz tone, peak 512,
and inverted DAC control 31 (the lowest nonzero hardware setting). The probe's
successful return includes a completed drain and successful final safe-mute
write. Afterward the PCM status endpoint was unavailable (closed), a full
process-FD scan found no sound-device handle, no probe process remained, no
audio/PCM/WAV file existed under `/run` or `/tmp`, taint remained `0`, and both
audio gates remained absent.

Stage C passes the bounded local output proof. This admits the physical DAC/PA
path as evidence only; it does not authorize boot-time playback, arbitrary PCM,
background sessions, network-facing playback, UAC, or HDMI audio.

## Stage D implementation candidate (v53)

Two privileged Rust brokers now wrap the admitted Stage B/C probes without
expanding their hardware contracts. Capture and playback have separate sockets
(`/run/cleankvm/audio-capture.sock` and `/run/cleankvm/audio-output.sock`),
separate enable files, independent MCP scopes, and independent web controls.
Each broker accepts only UID 900, serves one request at a time, requires exactly
mono S16LE at 16 kHz for one second, and invokes a root-only fixed-function
probe. Capture returns metrics, never PCM. Playback accepts no caller-supplied
PCM and returns only a completion receipt for the admitted 1 kHz, peak-512 tone.

Both gates remain absent by default and are removed by their service stop
paths. MCP additionally requires `kvm:audio-capture` for capture and both
`kvm:audio-output` and an explicit mutation-enabled grant for playback. The
human web console calls the same MCP operations and displays `Active` for the
entire request.

Workspace tests and strict lints pass. The offline static RISC-V artifacts are:

- `cleankvmd`: `f25e309533a65abc63dd8f39e96c663ef3dce2a3d9b9df976f208ca4bd12c808`
- capture broker: `4276c86b5036bab37fc024ce735d11a3d8227318a2100dec64f6d801545eeb79`
- playback broker: `140e955e38f3dd93218c39e32ad1478a7c06f77050a98c062bacc8578b36b46f`

The verified v53 image is
`out/phase3a-audio-brokers-v53/cleankvm-phase1.img`, 79,692,288 bytes,
SHA-256 `1df82c71aeaee33f4dc53583218f980554383f75c737e4c4e4d43add594605d0`.
Its rootfs SHA-256 is
`31243d5bb6061b03e7860af514f2eabfd8c4bff320900970e9aa279f8bff7f6f`;
FIT and FIP remain `1b504a2c...` and `6e6d3b5c...`. Hardware/network admission
is still pending, so Stage D remains fail-closed until v53 read-back, cold-boot,
negative gate tests, independent authenticated operations, and post-operation
PCM/process/file/gate checks pass.

The v53 candidate was flashed to the uniquely resolved 30,992,760,832-byte
external physical CleanKVM card. Exactly 79,692,288 bytes were written and read
back. macOS FAT bookkeeping changed the whole-span SHA-256 to
`f63715da2d13333479370b90cfcfc1e0d8e69e4f00cfa80e9d37bf4249aea238`,
while the embedded FIT, rootfs, and FIP all matched the audited inputs. All
partitions were unmounted after verification. Cold-boot and independent MCP
audio admission remain pending.
