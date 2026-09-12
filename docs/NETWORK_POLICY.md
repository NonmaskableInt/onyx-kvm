# Network policy

## Phase 1 recovery image

The recovery image permits wired Ethernet on `eth0` and obtains its address,
route, and DNS configuration as a DHCP client. This supports package-free
hardware diagnostics, time-bounded artifact retrieval during development, and
future enrollment work.

The image must not start SSH, HTTP(S), mDNS, a DHCP server, Wi-Fi management,
remote logging, telemetry, or an update agent. A successful boot acceptance
test requires enumerating listening sockets and finding none after DHCP has
settled. Network failure must not prevent local serial boot or diagnostics.

## Later control-plane image

The Rust control plane will add one authenticated HTTPS listener. MCP and the
browser UI share that listener and authorization layer. Enrollment is local
and explicit; there are no factory credentials. Outbound DNS and HTTPS are
policy-controlled, while device-side unattended download-and-execute updates
remain prohibited.

Audio and video brokers never receive network devices or credentials. They
communicate only with the policy-enforcing Rust service over bounded local IPC.

