# Architecture

## Trust boundaries

```text
untrusted LAN / MCP client / browser
                |
        TLS + authentication
                |
     cleankvmd (unprivileged)
                |
      bounded Unix socket RPC
        /                 \
 HID broker       media adapters
 (privileged)    (video/audio sandboxes)
        \                 /
          Linux device APIs
                |
       SG2002 + attached host
```

`cleankvmd` parses all remote input but cannot open USB gadget, GPIO, block,
or multimedia devices. Privileged brokers accept typed, size-bounded requests
over Unix sockets and independently enforce authorization policy.

The media adapters are the only components permitted to load SG2002 vendor
multimedia or audio libraries. They emit encoded media into bounded
shared-memory rings. They receive no credentials and have no network access.
If source-complete upstream paths become available, the adapters can be
replaced without changing either public protocol.

Audio is a separately enabled capability, not an automatic companion to video.
Microphone capture, HDMI audio input, and speaker output have independent
policy switches. The onboard microphone defaults to disabled at the driver
and service layers. Enabling it produces a persistent UI indication and every
capture session is audited. A build-time option omits microphone support, and
the hardware guide will document a physical microphone-disable modification.

## MCP boundary

The remote endpoint uses MCP Streamable HTTP over TLS. Development images
provision a unique TLS leaf and a separate OAuth issuer verification key during
image assembly; the issuer private key remains with the operator. Access tokens
are audience-bound, short-lived, and checked for per-tool scopes on every
request. The daemon runs as a dedicated unprivileged account and OAuth metadata
is served as required by the MCP HTTP authorization specification.

Local recovery may enable an MCP stdio endpoint on a physical serial console.
It is never bridged to TCP and obtains its authority from the local process
environment.

Initial tools:

| Tool | Scope | Risk |
| --- | --- | --- |
| `kvm.status` | `kvm:read` | read-only |
| `kvm.video.snapshot` | `kvm:video` | exposes host screen |
| `kvm.audio.capture` | `kvm:audio-capture` | captures nearby conversation |
| `kvm.audio.play` | `kvm:audio-output` | emits sound from attached speaker |
| `kvm.keyboard.send` | `kvm:control` | changes attached host |
| `kvm.pointer.send` | `kvm:control` | changes attached host |
| `kvm.media.attach` | `kvm:media` | presents attacker-controlled storage |
| `kvm.device.reboot` | `kvm:admin` | interrupts KVM availability |

Mutating tools are disabled by default. The policy layer applies rate limits,
payload limits, an append-only audit record, and optional physical-presence or
interactive confirmation requirements. MCP descriptions and annotations are
advisory only; brokers enforce the policy.

MCP controls bounded audio actions; it is not the bulk media transport.
Continuous audio uses an authenticated media session whose short-lived grant
is created through the same policy layer. Tool results contain metadata or a
one-time resource handle, never unbounded inline audio.

## Image layout

- Boot partition: bootloader, kernel, device tree, signed boot manifest.
- Root A and Root B: read-only SquashFS system slots.
- State: encrypted or integrity-protected configuration, host keys, audit log.
- Recovery: minimal offline recovery environment.

The first usable image may begin with a single read-only root slot while A/B
update and verified boot are developed. It must not claim secure or verified
boot until the hardware root-of-trust and rollback behavior are proven on the
device.
