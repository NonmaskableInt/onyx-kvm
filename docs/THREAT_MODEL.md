# Threat model

## Assets

- Keyboard, pointer, and virtual-media control of the attached host.
- Captured HDMI content and credentials displayed on it.
- Ambient audio captured by the onboard microphone and sound emitted locally.
- Device identity, operator credentials, signing keys, and audit records.
- Availability of the attached host and KVM.

## Adversaries

- An unauthenticated or authenticated-but-underprivileged LAN client.
- A malicious MCP client, browser, uploaded disk image, or update bundle.
- A compromised network-facing service.
- A compromised build dependency or vendor binary.
- An attacker with temporary access to the microSD card.

Physical access to the PCB, invasive chip attacks, and a malicious silicon or
mask ROM are initially out of scope. They remain documented residual risks.

## Required controls

1. No listening service before first-boot enrollment except a narrowly scoped
   local enrollment channel.
2. No shared default secret and no password authentication for SSH.
3. Memory-safe parsing in exposed services; unsafe Rust is forbidden in the
   workspace and confined to separately reviewed FFI adapters if unavoidable.
4. Capability separation between network, video, HID, media, and updater.
5. Bounded requests, timeouts, rate limits, and explicit authorization at both
   the gateway and privileged broker.
6. Signed updates; no device-side download-and-execute updater.
7. Pinned source revisions, recorded patches, SBOM, checksums, and isolated
   builds with network access disabled after dependency acquisition.
8. Audio capabilities are independently permissioned and default off.
   Microphone use has a persistent visible indicator, capture limits, audit
   events, and build-time and documented physical-disable options.
9. Audio buffers are memory-bounded, short-lived, excluded from crash dumps,
   and never retained unless an authenticated caller explicitly requests it.
10. Secrets never appear in MCP resources, tool results, logs, crash dumps, or
   image artifacts.

## Audio-specific abuse cases

- Silent ambient surveillance after a browser or MCP session ends.
- Cross-session leakage from stale audio buffers.
- Speaker abuse, including excessive volume or sustained nuisance playback.
- Crafted codecs or container files reaching privileged parsers.
- Confusion between the onboard microphone, HDMI audio, USB audio gadget, and
  optional speaker hardware.

The design therefore uses raw PCM at the privileged device boundary, performs
encoding in an unprivileged sandbox, zeroes reusable buffers, caps duration and
volume, and exposes the active physical source in every authorization prompt.

## Honest security claims

An independently built image does not by itself establish trust. Before a
release is described as hardened, we must verify the boot chain, enumerate
every firmware blob, reproduce the image from a clean builder, inspect exposed
ports, and test the final artifact on real NanoKVM Lite hardware.
