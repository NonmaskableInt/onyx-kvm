# Stock NanoKVM Lite v1.4.3 baseline

**Observed:** 2026-09-03
**Image:** `2026-06-10-1_4_3.img`
**Application:** 2.4.3
**Collector schema:** `cleankvm-evidence-v2`
**Reviewed archive SHA-256:** `c94c0fcbb5e8d15ccf43fde3034e337b14085130672dda7b2d7bc0ece524f963`
**Archive location:** temporary/off-repository; reviewed but not retained in Git

No device serial, MAC address, hostname suffix, IP address, password, token,
or device key is retained here.

## Confirmed platform

- Device tree model: `LicheeRv Nano`
- Compatible: `cvitek,cv181x`
- Architecture: RISC-V 64-bit, `rv64imafdvcsu`, `sv39` MMU
- Kernel: vendor Linux 5.10.4, preemptible build dated 2025-04-17
- Linux-visible memory: 161,788 KiB; no swap configured
- USB device controller: `4340000.usb`, configured
- Host-observed USB identity: Sipeed NanoKVM `3346:1009`, USB 2.0 high speed

## Storage and boot

- Boot device: `mmcblk0`, approximately 30 GB
- Partition 1: 16 MiB VFAT mounted read-write at `/boot`
- Partition 2: approximately 7.6 GiB ext4 root mounted read-write
- Partition 3: approximately 21.2 GiB exFAT data mounted read-write
- Kernel command line uses `/dev/mmcblk0p2`, serial console at 115200 baud,
  `rootwait`, and `loglevel=0`.
- No read-only root, A/B system slots, or evident verified-root mechanism was
  observed in this baseline.

## Audio

- ALSA capture card: `cv182xa_adc`, one capture PCM device.
- ALSA playback card: `cv182xa_dac`, one playback PCM device.
- Device nodes for both capture and playback were present.
- Signal quality and physical routing remain untested. The result confirms
  kernel exposure, not that HDMI or USB audio is routed.

## Networking and exposed services

- Physical Ethernet and USB gadget interfaces were both up.
- Plain HTTP listened on all addresses through `NanoKVM-Server`.
- HTTPS was not listening.
- mDNS and NTP listened on multiple interfaces.
- A DHCP server listened on all IPv4 addresses rather than an interface-local
  address. This deserves isolation testing before any trusted-LAN use.
- SSH was disabled initially and temporarily enabled for this evidence run.
- The stock web interface accepted the documented factory credentials and
  required a password change.

## Loaded vendor surface

The runtime loaded out-of-tree/proprietary-marked modules for wireless,
monitoring, image/video codecs, TPU, watchdog, region/overlay, video output,
warp/transform, VPSS, video input, sensor I2C, MIPI RX, fast-image, RTOS command
queue, and base/system support. Exact source and license correspondence remains
to be established.

Notable modules included `soph_vc_driver`, `soph_vcodec`, `soph_vpss`,
`soph_vi`, `soph_mipi_rx`, `soph_rtos_cmdqu`, `soph_base`, and `soph_sys`.
Wireless modules were loaded even though this Lite setup used Ethernet; their
necessity must be tested and they should be removed from the clean image unless
required by the actual board variant.

## Collection defect and disposition

Version 1 of the collector redacted credential-shaped lines but did not redact
addresses embedded in the socket-listener report. That first raw archive was
rejected for repository inclusion. Version 2 replaced socket addresses with
`[REDACTED ADDRESS]`; its transferred checksum matched the checksum calculated
on the device, and a post-transfer scan found no IP address, MAC address,
credential marker, device key, serial, UUID, or token. This reviewed summary
retains only architectural facts.

## Still required

- Capture the complete serial boot log and offline partition hashes.
- Record exact PCB and HDMI-to-CSI markings.
- Test HDMI modes, disconnect behavior, USB HID descriptors, microphone,
  speaker, HDMI audio, USB Audio Class, watchdog, reset pin, thermals, and power.
- Map every loaded vendor module and userspace multimedia artifact to pinned
  source, license, and reproducible build output.
