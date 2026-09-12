# Phase 1 acceptance record

Status: hardware-qualified development checkpoint on 2026-09-06.

Canonical image: `out/phase2-dhcp-healing-dev/cleankvm-phase1.img`

SHA-256: `675af3f65b507cb1ddf81e06d93abadc8d59464f76661c72dc2a67f4dedff1c9`

## Passed gates

- The repository-owned assembler verified the MBR, FAT boot partition, Linux
  root partition, and byte-identical embedded FIT, FIP, and root filesystem.
- U-Boot verified the kernel and device-tree SHA-256 hashes on target hardware.
- Linux, the minimal Buildroot userspace, and the locked serial recovery prompt
  booted repeatedly from microSD.
- The Rust memory tester completed 600 passes over a 64 MiB allocation with no
  failures, followed by a normal verified reboot.
- U-Boot has no network stack or PHY driver. Its ASIC board code retains only
  the SG2002 internal EPHY power/reset and synthesized-ID bootstrap required
  before Linux MDIO discovery.
- Linux registered `bm-dwmac` with PHY `stmmac-0:00`, negotiated 100 Mbps
  full-duplex, and reported zero RX/TX hardware errors.
- Packet capture observed DHCP Discover, Offer, Request, and ACK. The target
  received `192.168.2.10` from macOS Internet Sharing and answered five ICMP
  probes across two checks with zero loss.
- A subsequent cold-boot test exposed a link-negotiation race in the original
  one-shot DHCP startup. The canonical image now waits briefly for carrier and
  configures `udhcpc` for five foreground attempts followed by two-second
  background retries. On its first hardware boot it retried without a power
  cycle, obtained `192.168.2.12`, and answered five ICMP probes with zero loss.
- The recovery root filesystem audit found none of the prohibited listeners,
  remote-login servers, discovery agents, Wi-Fi tools, update agents, or media
  and audio executables.

## Claim boundary

This checkpoint is a development recovery image, not a release. The pinned
vendor-derived FSBL and binary toolchains still need source/license closure,
independent compiler comparison, signed-boot root-of-trust validation, SBOM,
clean-builder reproduction, and power-failure testing. Video, HID, virtual
media, browser access, remote MCP, microphone, and audio remain disabled.

The Ethernet diagnostic image is retained separately at
`out/phase2-ethernet-diagnostic-dev`; its UART-only diagnostic script lives in
`image/diagnostics` and is deliberately absent from the canonical overlay.
