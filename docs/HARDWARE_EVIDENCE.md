# Hardware evidence collection

Run this only on an isolated NanoKVM Lite whose stock image is treated as
untrusted. Connect the serial console for observation, connect HDMI/USB only to
a disposable test host, and do not attach it to a trusted LAN.

Build the statically linked collector for the target:

```sh
cargo build --release --target riscv64gc-unknown-linux-musl \
  --package onyx-kvm-evidence
```

Copy only the resulting binary to removable media. On the device, choose a new
output directory on that media:

```sh
./onyx-kvm-evidence /mnt/evidence/stock-board-1
```

The collector performs read-only probes. It refuses to use an existing output
directory and redacts lines shaped like serial numbers, UUIDs, device keys,
passwords, tokens, or secrets. It intentionally omits hostnames, interface
addresses, MAC addresses, configuration files, logs, and environment variables.

Review every captured file manually before committing it. The collector's
redaction is defense in depth, not a guarantee that an undocumented identifier
cannot occur in some other field.

## Manual evidence not collected automatically

- Exact PCB, module, and HDMI-to-CSI board markings and clear photographs.
- Full serial boot log from power-on through a stable login prompt.
- Partition image hashes acquired offline from the microSD card.
- USB descriptors observed by the disposable attached host.
- HDMI modes, disconnect/reconnect behavior, and encoded-frame validation.
- Microphone, speaker, HDMI audio, and USB Audio Class signal-path tests.
- Power draw, reset-pin behavior, watchdog reset, and thermal readings.

Record physical observations without device identifiers in a companion
`NOTES.md`. Keep the original evidence offline; commit a reviewed copy only.
