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
   - Setup claim is rate-limited to 5 attempts to prevent brute-force attacks across the local subnet.

---

## 2. Pairing Ceremony

For out-of-band enrollment or issuing automation / agent tokens:

### Local UART Pairing
1. Attach a 3.3V USB-UART adapter to the NanoKVM Lite serial pins (115200 baud, 8N1).
2. Log in to the serial console (or run `onyx-kvm-pair`).
3. The script displays a short-lived 6-digit numeric pairing code on the console.
4. On the client machine or web browser:
   - Navigate to `https://<device-ip>:8443/`.
   - Enter the displayed pairing code and the device PIN.
5. Alternatively, run the pairing script on your management workstation:
   ```sh
   image/pair-device.sh https://<device-ip>:8443 <PAIRING_CODE> <PIN>
   ```
6. The client receives an Ed25519-signed JWT token granting access according to the requested role.

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

### Factory Reset via Authenticated API
An operator holding `kvm:admin` scope can issue a reset request:
```sh
curl -k -X POST https://<device-ip>:8443/factory-reset \
  -H "Authorization: Bearer <OPERATOR_TOKEN>"
```

---

## 5. Development Mode Policy

Development tooling (interactive Dropbear SSH server and arbitrary process execution)
is **excluded by default** from release/product images (`apply-product-profile.sh`).

For development builds (`ONYX_KVM_DEV_TOOLS=1`):
1. Place your authorized public key in `/boot/ssh_authorized_keys`.
2. Create `/boot/DEV_MODE`.
3. Boot the device.
4. Dropbear SSH will start on port 22, authenticated strictly by the provided public key (root password login remains disabled).
5. The `kvm.dev.exec` MCP tool becomes active for authorized operator tokens.
