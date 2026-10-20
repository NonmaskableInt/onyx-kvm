# Network policy

## Phase 1 recovery image

The recovery image permits wired Ethernet on `eth0` and obtains its address,
route, and DNS configuration as a DHCP client. This supports package-free
hardware diagnostics, time-bounded artifact retrieval during development, and
future enrollment work.

Apart from the product-profile name responder described below, the image must
not start SSH, HTTP(S), mDNS, a DHCP server, Wi-Fi management,
remote logging, telemetry, or an update agent. A successful boot acceptance
test requires enumerating listening sockets and finding none after DHCP has
settled. Network failure must not prevent local serial boot or diagnostics.

## Name responder (product profile only)

`onyx-kvm-mdns` answers mDNS queries (UDP 5353, group 224.0.0.251) for the
device's own `<hostname>.local` name so clients can reach the console without
DNS or `/etc/hosts` entries. Its admitted behaviour is deliberately narrow:

- It answers `A`/`ANY` with the `eth0` IPv4 address and `AAAA` with a negative
  (NSEC) record, only for its own name. It advertises no services, sends no
  unsolicited packets, does not probe for or resolve name conflicts (the name
  embeds the MAC suffix), and rate-limits itself to 5 replies per second.
- It parses untrusted multicast packets with bounded, panic-free code (no name
  compression, at most 8 questions, std only, unsafe code forbidden) and runs as
  numeric UID/GID 901 with no passwd entry, no credentials and no file access.
- It binds to the multicast group address (the kernel drops unicast-addressed
  packets) and answers only sources inside the `eth0` subnet, so a spoofed
  off-link source cannot use it to reflect replies at a third party.
- It never learns anything from the network, so it cannot widen the HTTPS
  origin allow-list. Name resolution does not authenticate the device: clients
  still trust the device only through its pinned certificate and pairing.
- It is absent from non-product profiles; `audit-rootfs.sh` enforces both.

The listening-socket boot test therefore expects exactly two sockets after DHCP
settles in the product profile: TCP 8443 and UDP 5353.

## Later control-plane image

The Rust control plane will add one authenticated HTTPS listener. MCP and the
browser UI share that listener and authorization layer. Enrollment is local
and explicit; there are no factory credentials. Outbound DNS and HTTPS are
policy-controlled, while device-side unattended download-and-execute updates
remain prohibited.

Audio and video brokers never receive network devices or credentials. They
communicate only with the policy-enforcing Rust service over bounded local IPC.

