//! Minimal mDNS responder for the device's own `<hostname>.local` name.
//!
//! It answers `A` and `ANY` queries with the interface IPv4 address and
//! answers `AAAA` with a negative (NSEC) record. It advertises no services,
//! sends no unsolicited packets, performs no probing, and ignores everything
//! that is not a plain query for its own name. It holds no credentials and is
//! started as an unprivileged user.
//!
//! Only packets addressed to the multicast group (bound to the group address,
//! so the kernel drops unicast-addressed packets) and sent from the interface's
//! own subnet are answered, so a spoofed off-link source cannot use it to
//! reflect traffic at a third party.
//!
//! Usage: `onyx-kvm-mdns HOSTNAME IPV4 PREFIX_LENGTH`

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::process::ExitCode;
use std::time::{Duration, Instant};

const MDNS_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;
const MAX_PACKET: usize = 1500;
const MAX_QUESTIONS: u16 = 8;
const TTL_SECONDS: u32 = 120;
const MAX_RESPONSES_PER_SECOND: u32 = 5;

const TYPE_A: u16 = 1;
const TYPE_AAAA: u16 = 28;
const TYPE_NSEC: u16 = 47;
const TYPE_ANY: u16 = 255;
const CLASS_IN: u16 = 1;
const CACHE_FLUSH: u16 = 0x8000;

/// The lowercase DNS wire form of `<hostname>.local`.
fn wire_name(hostname: &str) -> Option<Vec<u8>> {
    let valid = !hostname.is_empty()
        && hostname.len() <= 63
        && !hostname.starts_with('-')
        && !hostname.ends_with('-')
        && hostname
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !valid {
        return None;
    }
    let mut name = Vec::new();
    name.push(u8::try_from(hostname.len()).ok()?);
    name.extend(hostname.bytes().map(|b| b.to_ascii_lowercase()));
    name.extend_from_slice(&[5, b'l', b'o', b'c', b'a', b'l', 0]);
    Some(name)
}

struct Question {
    qtype: u16,
    /// Start and end of this question in the packet, for echoing.
    span: (usize, usize),
}

/// Parse the question section, keeping only questions for `own_name`.
/// Returns `None` for anything malformed or not a plain query.
fn parse_questions(packet: &[u8], own_name: &[u8]) -> Option<Vec<Question>> {
    if packet.len() < 12 {
        return None;
    }
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    // QR must be 0 (query) and opcode must be 0.
    if flags & 0xF800 != 0 {
        return None;
    }
    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    if qdcount == 0 || qdcount > MAX_QUESTIONS {
        return None;
    }
    let mut pos = 12;
    let mut matches = Vec::new();
    for _ in 0..qdcount {
        let start = pos;
        // Names in questions are read without compression: a pointer is
        // rejected rather than followed.
        loop {
            let len = usize::from(*packet.get(pos)?);
            if len & 0xC0 != 0 {
                return None;
            }
            pos += 1 + len;
            if len == 0 {
                break;
            }
            if pos > packet.len() {
                return None;
            }
        }
        let name = packet.get(start..pos)?;
        let fixed = packet.get(pos..pos + 4)?;
        let qtype = u16::from_be_bytes([fixed[0], fixed[1]]);
        let qclass = u16::from_be_bytes([fixed[2], fixed[3]]) & !CACHE_FLUSH;
        pos += 4;
        if qclass == CLASS_IN && name.eq_ignore_ascii_case(own_name) {
            matches.push(Question {
                qtype,
                span: (start, pos),
            });
        }
    }
    Some(matches)
}

/// Build the response for `packet`, or `None` when nothing should be sent.
/// `legacy` is true for a query from a port other than 5353, which gets a
/// unicast reply that echoes the ID and question.
fn build_response(packet: &[u8], own_name: &[u8], ip: Ipv4Addr, legacy: bool) -> Option<Vec<u8>> {
    let questions = parse_questions(packet, own_name)?;
    let wants_a = questions
        .iter()
        .any(|q| q.qtype == TYPE_A || q.qtype == TYPE_ANY);
    let wants_nsec = questions.iter().any(|q| q.qtype == TYPE_AAAA);
    if !wants_a && !wants_nsec {
        return None;
    }

    let mut out = Vec::with_capacity(96);
    let id = if legacy { &packet[0..2] } else { &[0, 0] };
    out.extend_from_slice(id);
    out.extend_from_slice(&[0x84, 0x00]); // response, authoritative
    let questions_echoed = u16::from(legacy);
    out.extend_from_slice(&questions_echoed.to_be_bytes());
    let answers = u16::from(wants_a) + u16::from(wants_nsec);
    out.extend_from_slice(&answers.to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    if legacy {
        let first = questions.first()?;
        out.extend_from_slice(&packet[first.span.0..first.span.1]);
    }
    if wants_a {
        out.extend_from_slice(own_name);
        out.extend_from_slice(&TYPE_A.to_be_bytes());
        out.extend_from_slice(&(CLASS_IN | CACHE_FLUSH).to_be_bytes());
        out.extend_from_slice(&TTL_SECONDS.to_be_bytes());
        out.extend_from_slice(&4u16.to_be_bytes());
        out.extend_from_slice(&ip.octets());
    }
    if wants_nsec {
        // The name exists but has no AAAA record.
        out.extend_from_slice(own_name);
        out.extend_from_slice(&TYPE_NSEC.to_be_bytes());
        out.extend_from_slice(&(CLASS_IN | CACHE_FLUSH).to_be_bytes());
        out.extend_from_slice(&TTL_SECONDS.to_be_bytes());
        let mut rdata = Vec::new();
        rdata.extend_from_slice(own_name); // next domain name
        rdata.extend_from_slice(&[0, 1, 0x40]); // window 0, bitmap: type A
        out.extend_from_slice(&u16::try_from(rdata.len()).ok()?.to_be_bytes());
        out.extend_from_slice(&rdata);
    }
    Some(out)
}

/// True when `source` is inside the `ip`/`prefix` subnet.
fn on_link(source: Ipv4Addr, ip: Ipv4Addr, prefix: u32) -> bool {
    let mask = match prefix {
        0 => return false,
        1..=31 => u32::MAX << (32 - prefix),
        _ => u32::MAX,
    };
    u32::from(source) & mask == u32::from(ip) & mask
}

fn run(hostname: &str, ip: Ipv4Addr, prefix: u32) -> Result<(), String> {
    let own_name = wire_name(hostname).ok_or("invalid hostname")?;
    let socket = UdpSocket::bind(SocketAddrV4::new(MDNS_GROUP, MDNS_PORT))
        .map_err(|e| format!("bind 5353: {e}"))?;
    socket
        .join_multicast_v4(&MDNS_GROUP, &ip)
        .map_err(|e| format!("join multicast group: {e}"))?;
    // std cannot select the outgoing multicast interface; the device has a
    // single interface, so replies leave through the default route.
    socket
        .set_multicast_ttl_v4(255)
        .map_err(|e| format!("set TTL: {e}"))?;
    socket
        .set_multicast_loop_v4(false)
        .map_err(|e| format!("disable loopback: {e}"))?;

    let mut window_start = Instant::now();
    let mut sent_in_window = 0u32;
    let mut buffer = [0u8; MAX_PACKET];
    loop {
        let Ok((length, source)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let SocketAddr::V4(source) = source else {
            continue;
        };
        if !on_link(*source.ip(), ip, prefix) {
            continue;
        }
        let legacy = source.port() != MDNS_PORT;
        let Some(response) = build_response(&buffer[..length], &own_name, ip, legacy) else {
            continue;
        };
        if window_start.elapsed() >= Duration::from_secs(1) {
            window_start = Instant::now();
            sent_in_window = 0;
        }
        if sent_in_window >= MAX_RESPONSES_PER_SECOND {
            continue;
        }
        sent_in_window += 1;
        let destination = if legacy {
            SocketAddr::V4(source)
        } else {
            SocketAddr::V4(SocketAddrV4::new(MDNS_GROUP, MDNS_PORT))
        };
        let _ = socket.send_to(&response, destination);
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(hostname), Some(address), Some(prefix), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        eprintln!("usage: onyx-kvm-mdns HOSTNAME IPV4 PREFIX_LENGTH");
        return ExitCode::from(2);
    };
    let (Ok(ip), Ok(prefix)) = (address.parse::<Ipv4Addr>(), prefix.parse::<u32>()) else {
        eprintln!("invalid IPv4 address or prefix length");
        return ExitCode::from(2);
    };
    if !(1..=32).contains(&prefix) {
        eprintln!("invalid IPv4 address or prefix length");
        return ExitCode::from(2);
    }
    match run(&hostname, ip, prefix) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("onyx-kvm-mdns: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 242);

    fn query(name: &[u8], qtype: u16, qclass: u16) -> Vec<u8> {
        let mut p = vec![0x12, 0x34, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        p.extend_from_slice(name);
        p.extend_from_slice(&qtype.to_be_bytes());
        p.extend_from_slice(&qclass.to_be_bytes());
        p
    }

    fn own() -> Vec<u8> {
        wire_name("onyx-kvm-f406c8").unwrap_or_default()
    }

    #[test]
    fn on_link_checks_the_subnet() {
        let ip = Ipv4Addr::new(192, 168, 1, 242);
        assert!(on_link(Ipv4Addr::new(192, 168, 1, 7), ip, 24));
        assert!(!on_link(Ipv4Addr::new(192, 168, 2, 7), ip, 24));
        assert!(!on_link(Ipv4Addr::new(8, 8, 8, 8), ip, 24));
        assert!(on_link(Ipv4Addr::new(192, 168, 1, 242), ip, 32));
        assert!(!on_link(Ipv4Addr::new(192, 168, 1, 7), ip, 0));
    }

    #[test]
    fn rejects_invalid_hostnames() {
        assert!(wire_name("").is_none());
        assert!(wire_name("-bad").is_none());
        assert!(wire_name("has.dot").is_none());
        assert!(wire_name(&"a".repeat(64)).is_none());
        assert!(wire_name("onyx-kvm-f406c8").is_some());
    }

    #[test]
    fn answers_a_query_for_own_name_case_insensitively() {
        let q = query(
            &wire_name("ONYX-KVM-F406C8").unwrap_or_default(),
            TYPE_A,
            0x8001,
        );
        let r = build_response(&q, &own(), IP, false).unwrap_or_default();
        assert_eq!(&r[0..2], &[0, 0]);
        assert_eq!(&r[2..4], &[0x84, 0x00]);
        assert_eq!(&r[6..8], &[0, 1]);
        assert_eq!(&r[r.len() - 4..], &IP.octets());
    }

    #[test]
    fn legacy_query_echoes_id_and_question() {
        let q = query(&own(), TYPE_A, CLASS_IN);
        let r = build_response(&q, &own(), IP, true).unwrap_or_default();
        assert_eq!(&r[0..2], &[0x12, 0x34]);
        assert_eq!(&r[4..6], &[0, 1]);
    }

    #[test]
    fn aaaa_gets_a_negative_record_without_an_address() {
        let q = query(&own(), TYPE_AAAA, CLASS_IN);
        let r = build_response(&q, &own(), IP, false).unwrap_or_default();
        assert_eq!(&r[6..8], &[0, 1]);
        let n = own().len();
        assert_eq!(&r[12 + n..14 + n], &TYPE_NSEC.to_be_bytes());
        assert!(!r.windows(4).any(|w| w == IP.octets()));
    }

    #[test]
    fn ignores_other_names_responses_and_other_classes() {
        let other = wire_name("someone-else").unwrap_or_default();
        assert!(build_response(&query(&other, TYPE_A, CLASS_IN), &own(), IP, false).is_none());
        assert!(build_response(&query(&own(), TYPE_A, 3), &own(), IP, false).is_none());
        assert!(build_response(&query(&own(), 16, CLASS_IN), &own(), IP, false).is_none());
        let mut response = query(&own(), TYPE_A, CLASS_IN);
        response[2] = 0x84;
        assert!(build_response(&response, &own(), IP, false).is_none());
    }

    #[test]
    fn malformed_packets_are_dropped_without_panicking() {
        assert!(build_response(&[], &own(), IP, false).is_none());
        assert!(build_response(&[0; 11], &own(), IP, false).is_none());
        let mut compressed = query(&own(), TYPE_A, CLASS_IN);
        compressed[12] = 0xC0;
        assert!(build_response(&compressed, &own(), IP, false).is_none());
        let truncated = query(&own(), TYPE_A, CLASS_IN);
        for cut in 0..truncated.len() {
            let _ = build_response(&truncated[..cut], &own(), IP, false);
        }
        let mut many = query(&own(), TYPE_A, CLASS_IN);
        many[5] = 9;
        assert!(build_response(&many, &own(), IP, false).is_none());
    }
}
