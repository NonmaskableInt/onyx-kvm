# HID security boundary

Phase 2 introduces keyboard and relative-pointer output without giving the
network-facing daemon access to a USB gadget device. The design has two
processes:

- `cleankvmd` terminates HTTPS, validates the OAuth token and MCP lifecycle,
  and applies the shared authorization policy as unprivileged UID/GID 900.
- `cleankvm-hid-broker` has no network listener. It accepts only a local Unix
  socket and writes fixed-size reports to `/dev/hidg0` and `/dev/hidg1`. The
  socket is owned by `root:cleankvm` with mode `0660`, and every connection is
  independently checked with Linux `SO_PEERCRED`; only the dedicated
  `cleankvm` UID 900 is authorized.

The broker protocol is intentionally non-extensible at this stage: a five-byte
magic/version/type/length header followed by exactly eight keyboard bytes or
four pointer bytes. Oversized, truncated, unknown, or version-mismatched frames
are rejected. On client disconnect the broker writes zero reports to release
all keys and pointer buttons.

Authorization is repeated at the privilege boundary. The broker maps its own
operation type to the `kvm:control` scope and consults a local mutation-enable
marker for every report. The marker defaults absent, so merely compromising or
misconfiguring the network daemon cannot turn on mutation. The broker and fixed
ConfigFS gadget are installed for hardware qualification. The MCP HID tools
are exposed only to an unexpired, audience-bound grant containing
`kvm:control` and the explicit mutation claim. Even then, the broker rejects
every report while the local marker is absent. The marker is absent after every
boot; its production creation mechanism must be local/physical, boot-bounded,
and independently audited.

Kernel support is isolated in `image/configs/phase2-hid-kernel.fragment` and
validated against the pinned vendor Linux 5.10 tree. It enables only DWC2
peripheral mode, ConfigFS, libcomposite, and the HID gadget function. Host-side
USB, generic input/HID parsing, and serial, network, storage, audio, and video
gadget functions remain disabled. The vendor DTS declares the active controller
at `0x04340000` as `cvitek,cv182x-usb` and defaults it to OTG; the Phase-2 board
layer in `image/configs/phase2-hid-board.dtsi` explicitly narrows `dr_mode` to
`peripheral`. The Phase-1 recovery profile is unchanged.

Before deployment this boundary still requires:

1. A physical/local, boot-bounded mutation-enable mechanism.
2. Hardware tests for report delivery, disconnect release, stuck
   keys, malformed frames, daemon compromise, reboot, and power loss.

The controller-to-USB-C mapping, deterministic fixed HID descriptors,
cold-boot enumeration, and peer-identity enforcement have been completed. The
remaining hardware tests must retain the fail-closed default and use only a
deliberately enabled development boot.
