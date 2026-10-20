# Onyx KVM Operator Guide

This guide covers initial setup, enrollment, pairing, daily operations, feature
gate management, maintenance, and recovery for the Onyx KVM (NanoKVM Lite hardware).

## System Overview

Onyx KVM is a secure, source-auditable KVM-over-IP appliance running on the
SG2002 RISC-V SoC. It provides:
- Low-latency WebRTC/MJPEG/H.264 video streaming over HTTPS.
- USB HID mouse and keyboard redirection.
- MCP (Model Context Protocol) interface for autonomous and assistive agents.
- Fail-closed admission boundaries for all subsystems.
- Read-only root filesystem (`ro,relatime`) protecting runtime integrity.

---

## Known limits

- **No host power or reset control.** The device cannot press the host's RESET
  or power/reboot buttons (no GPIO support). It can wake a sleeping host with a
  USB key press. Rebooting the KVM does not reboot the host. Use the host's own
  controls or a separate power switch to recover a hung machine.
- **NanoKVM Lite only.** There is no support for the HUD board of the more
  advanced NanoKVM models.

## 1. First Boot & Setup Claim

### Factory / Unconfigured State
On a freshly flashed microSD card or after a factory reset:
- The device obtains an IP address via DHCP on the wired Ethernet port.
- No PIN hash or pairing certificate exists.
- The web interface (`https://<device-ip>:8443/`) presents a first-time setup claim dialog.

### Claiming the Device
1. Connect to the device web interface over HTTPS: `https://<device-ip>:8443/`.
2. Accept the self-signed appliance certificate (or install your custom CA).
3. Enter an administrative PIN:
   - Minimum length: 6 digits (maximum 8 digits).
   - Enter an optional human-readable name for the device.
4. Click **Claim Device**.
   - The daemon writes the salted SHA-256 PIN hash to `/boot/onyx-kvm-pin.hash` and `/run/onyx-kvm/pin.hash`.
   - The daemon mints a least-privilege console session token (`kvm:read kvm:video kvm:control kvm:audio-*`).
   - The claim is first-come trust-on-first-use: whoever boots and sets up the
     device owns it, and the `Origin` header is client-supplied, so it is not a
     defence against a hostile LAN client. Claim a new device on an isolated
     network or use the UART ceremony (section 2). Malformed claims are counted;
     after 5 the endpoint refuses claims until the daemon restarts.
   - Physical access owns the device: anyone who can write the SD card can
     reset it, enable dev mode on a development image, or change feature gates.

---

## 2. Pairing Ceremony

For out-of-band enrollment or issuing automation / agent tokens:

### Local UART Pairing
1. Attach a 3.3V USB-UART adapter to the NanoKVM Lite serial pins (115200 baud, 8N1).
2. Log in to the serial console.
3. Run `pair-device.sh SESSION_TOKEN [6_DIGIT_PIN]` on the device (it only stages
   state in `/run/onyx-kvm`; it is not a network client). Without a PIN it
   generates a random 6-digit one and prints it. The staged code is single use,
   and three wrong attempts wipe it.
4. On the client, open `https://<device-ip>:8443/` and enter the code, or
   `POST /pair` with `{"code": "<code>"}`.
5. The client receives the staged session token. A PIN-only pairing (no staged
   ceremony) returns a least-privilege console token without `kvm:admin`.

Operator (`kvm:admin`) tokens are issued only through the physical UART ceremony.

---

## 3. Subsystem Feature Gates

Onyx KVM enforces fail-closed hardware admission boundaries. Individual
subsystems can be enabled or disabled dynamically and persist across reboots:

| Feature Gate | Default | Hardware Effect | Persistence Location |
|---|---|---|---|
| `video` | Enabled | LT6911 HDMI receiver & VI/VENC encoder | `/boot/onyx-kvm-features.json` |
| `hid` | Enabled | USB OTG gadget endpoints (`/dev/hidg0`, `/dev/hidg1`) | `/boot/onyx-kvm-features.json` |
| `speaker` | Disabled | On-board fixed-frequency tone generator | `/boot/onyx-kvm-features.json` |
| `mic` | Disabled | Line-in capture broker (metrics-only in Phase 3a) | `/boot/onyx-kvm-features.json` |

### Modifying Feature Gates via Web UI
In the web console drawer under **Hardware Feature Gates**, toggle individual switches.
Changes are applied immediately and written to `/boot/onyx-kvm-features.json`.

### Modifying Feature Gates via HTTPS API
Send an authenticated `POST /features` request:
```sh
curl -k -X POST https://<device-ip>:8443/features \
  -H "Authorization: Bearer <CONSOLE_OR_OPERATOR_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{"video": true, "hid": true, "speaker": false, "mic": false}'
```

---

## 4. Factory Reset & Recovery

### Factory Reset via MicroSD Boot Partition
To wipe credentials and return the appliance to an unconfigured state:
1. Power off the device and insert the microSD card into an administrative computer.
2. Create an empty file named `RESET` or `FACTORY_RESET` on the FAT boot partition:
   ```sh
   touch /Volumes/boot/RESET
   ```
3. Eject the card and boot the Onyx KVM.
4. The early init scripts detect the flag, wipe `/boot/onyx-kvm-pin.hash` and active pairings, remove the flag file, and reboot into the unconfigured state.

There is no network factory-reset endpoint. Reset requires physical access to
the card or the serial console.

---

## 5. Development Mode Policy

Development tooling (interactive Dropbear SSH server and arbitrary process execution)
is **excluded by default** from release/product images (`apply-product-profile.sh`): the payload, the init script and the `kvm.dev.exec` / `kvm.system.dev_mode` code (cargo feature `dev-tools`) are all absent.

For development builds (`ONYX_KVM_DEV_TOOLS=1` for both the rootfs and `build-rust-daemon.sh`, plus `ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1` while B1 is open):
1. Place your authorized public key in `/boot/ssh_authorized_keys`.
2. Create `/boot/DEV_MODE`.
3. Boot the device.
4. Dropbear SSH will start on port 22, authenticated strictly by the provided public key (root password login remains disabled).
5. The `kvm.dev.exec` MCP tool becomes active for authorized operator tokens.
