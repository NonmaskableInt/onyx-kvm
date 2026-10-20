# Release notes (draft)

**Status: no release has been cut.** Every image built so far is a development
build that may not be redistributed. This file holds the statements a release
must carry; fill in the version, date and hashes when [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)
is complete.

## Licensing

Four vendor components ship in the image without a license grant:

1. **FSBL**: the Cvitek/Bitmain-authored files in `fsbl/plat/cv180x`,
   `fsbl/plat/cv181x` and `fsbl/lib/utils` (about 115 files).
2. **Middleware**: the sources linked into the raw-frame probe (`libsys.a` and
   four `middleware/v2/modules/vpu` objects); `sys/src/peri.c` is marked
   proprietary.
3. **`sophgo-osdrv`**: the source of the `soph_jpeg`, `mars_vcodec` and
   `cvi_vc_driver` modules, whose notices contradict their GPL declaration.
4. **`coda980.bin`**: the opaque VPU firmware blob.

Six more vendor kernel modules declare GPL in `MODULE_LICENSE` but carry no file
headers. A release needs a written grant for each of these from Sophgo/Cvitek,
or replacement of the parts. Evidence: [SDK_AUDIT.md](SDK_AUDIT.md); tracking:
[OPEN_ITEMS.md](OPEN_ITEMS.md) B1. The repository's own code is
GPL-3.0-or-later.

## Security posture and accepted risks

- **Setup claim (S1).** An unconfigured device is claimed by the first client
  that completes `/setup/claim` over the network (trust on first use). Whoever
  boots and sets up the device owns it. Claim it before connecting it to an
  untrusted network.
- **Physical access (S2, S3).** The microSD card holds the boot chain and root
  filesystem, so anyone with physical access to the device or card owns it.
- **Network surface.** The product image listens on TCP 8443 (HTTPS, web console,
  MCP) and UDP 5353 (mDNS answers for the device's own `<hostname>.local` name
  only; see [NETWORK_POLICY.md](NETWORK_POLICY.md)). There is no SSH, no DHCP
  server and no update agent. Name resolution does not authenticate the device;
  clients trust it through its certificate and pairing.
- **Development tooling.** `kvm.dev.exec` and `kvm.system.dev_mode` are compiled
  out of product builds, and the rootfs audit rejects dev payloads, the
  dev-mode init script and the mDNS boot-partition status log in a product
  image.
- **Audio.** Microphone capture is admitted as metrics-only and tone playback
  only; continuous audio stays disabled until Phase 4.

## Behaviour to expect

- Reflashing the card gives the device a new MAC address, hostname
  (`onyx-kvm-<last six hex digits>`) and DHCP lease, and discards all pairings.
  The MAC is stored in `/boot`.
- Without UART there is no admin token; the setup claim and PIN pairing mint
  console tokens only.
