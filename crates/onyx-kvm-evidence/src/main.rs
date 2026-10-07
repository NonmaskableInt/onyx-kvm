//! Read-only hardware evidence collection for `NanoKVM` Lite devices.

use std::env;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: &str = "onyx-kvm-evidence-v1";

struct Probe {
    name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

const PROBES: &[Probe] = &[
    Probe {
        name: "kernel.txt",
        program: "uname",
        args: &["-srvmo"],
    },
    Probe {
        name: "mounts.txt",
        program: "mount",
        args: &[],
    },
    Probe {
        name: "partitions.txt",
        program: "cat",
        args: &["/proc/partitions"],
    },
    Probe {
        name: "modules.txt",
        program: "cat",
        args: &["/proc/modules"],
    },
    Probe {
        name: "listeners.txt",
        program: "ss",
        args: &["-lntup"],
    },
    Probe {
        name: "usb-state.txt",
        program: "sh",
        args: &[
            "-c",
            "for f in /sys/class/udc/*/state; do printf '%s: ' \"$(basename \"$(dirname \"$f\")\")\"; cat \"$f\"; done",
        ],
    },
];

const FILES: &[(&str, &str)] = &[
    ("cpuinfo.txt", "/proc/cpuinfo"),
    ("meminfo.txt", "/proc/meminfo"),
    ("cmdline.txt", "/proc/cmdline"),
    (
        "device-tree-model.txt",
        "/sys/firmware/devicetree/base/model",
    ),
    (
        "device-tree-compatible.bin",
        "/sys/firmware/devicetree/base/compatible",
    ),
    ("alsa-cards.txt", "/proc/asound/cards"),
    ("alsa-pcm.txt", "/proc/asound/pcm"),
];

fn main() {
    if let Err(error) = run() {
        eprintln!("onyx-kvm-evidence: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let destination = parse_destination(env::args_os().skip(1))?;
    create_new_directory(&destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;

    write_file(destination.join("SCHEMA"), SCHEMA_VERSION.as_bytes()).map_err(display_io)?;
    let mut index = File::create(destination.join("INDEX.txt")).map_err(display_io)?;
    writeln!(index, "schema={SCHEMA_VERSION}").map_err(display_io)?;
    writeln!(
        index,
        "privacy=unique identifiers and credential-shaped values redacted"
    )
    .map_err(display_io)?;

    for (name, source) in FILES {
        match fs::read(source) {
            Ok(bytes) => {
                let sanitized = sanitize_bytes(&bytes);
                write_file(destination.join(name), &sanitized).map_err(display_io)?;
                writeln!(index, "{name}\tfile\tok\t{source}").map_err(display_io)?;
            }
            Err(error) => {
                writeln!(index, "{name}\tfile\tunavailable\t{source}: {error}")
                    .map_err(display_io)?;
            }
        }
    }

    for probe in PROBES {
        match Command::new(probe.program).args(probe.args).output() {
            Ok(output) => {
                let mut combined = output.stdout;
                combined.extend_from_slice(&output.stderr);
                write_file(destination.join(probe.name), &sanitize_bytes(&combined))
                    .map_err(display_io)?;
                writeln!(
                    index,
                    "{}\tcommand\texit={}\t{} {}",
                    probe.name,
                    output.status,
                    probe.program,
                    probe.args.join(" ")
                )
                .map_err(display_io)?;
            }
            Err(error) => {
                writeln!(
                    index,
                    "{}\tcommand\tunavailable\t{}: {error}",
                    probe.name, probe.program
                )
                .map_err(display_io)?;
            }
        }
    }

    collect_named_entries(
        &destination,
        "network-interfaces.txt",
        "/sys/class/net",
        &["type", "carrier", "operstate", "mtu"],
    )?;
    collect_named_entries(&destination, "sound-devices.txt", "/sys/class/sound", &[])?;

    println!("Evidence written to {}", destination.display());
    println!("Review every file before moving it to a repository.");
    Ok(())
}

fn parse_destination<I, T>(mut arguments: I) -> Result<PathBuf, String>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    let destination = arguments
        .next()
        .ok_or_else(|| "usage: onyx-kvm-evidence OUTPUT_DIRECTORY".to_owned())?;
    if arguments.next().is_some() {
        return Err("usage: onyx-kvm-evidence OUTPUT_DIRECTORY".to_owned());
    }
    let path = PathBuf::from(destination.as_ref());
    if path.as_os_str().is_empty() || path == Path::new("/") {
        return Err("refusing unsafe output directory".to_owned());
    }
    Ok(path)
}

fn create_new_directory(path: &Path) -> io::Result<()> {
    fs::create_dir(path)
}

fn write_file(path: PathBuf, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents)
}

#[allow(clippy::needless_pass_by_value)]
fn display_io(error: io::Error) -> String {
    error.to_string()
}

fn sanitize_bytes(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let mut sanitized = String::with_capacity(text.len());
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if [
            "serial",
            "uuid",
            "device_key",
            "password",
            "token",
            "secret",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
        {
            sanitized.push_str("[REDACTED LINE]\n");
        } else {
            sanitized.push_str(&redact_network_identifiers(line));
            sanitized.push('\n');
        }
    }
    sanitized.into_bytes()
}

fn redact_network_identifiers(line: &str) -> String {
    line.split_inclusive(char::is_whitespace)
        .map(|part| {
            let token = part.trim();
            if looks_like_ipv4(token) || looks_like_colon_address(token) {
                part.replacen(token, "[REDACTED ADDRESS]", 1)
            } else {
                part.to_owned()
            }
        })
        .collect()
}

fn looks_like_ipv4(token: &str) -> bool {
    let candidate = token
        .split(':')
        .next()
        .unwrap_or(token)
        .trim_matches(|character: char| !character.is_ascii_digit() && character != '.');
    let octets: Vec<_> = candidate.split('.').collect();
    octets.len() == 4
        && octets.iter().all(|octet| {
            !octet.is_empty()
                && octet.len() <= 3
                && octet.chars().all(|character| character.is_ascii_digit())
                && octet.parse::<u8>().is_ok()
        })
}

fn looks_like_colon_address(token: &str) -> bool {
    let candidate = if let Some(bracketed) = token.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or(bracketed)
    } else {
        token
    }
    .trim_matches(|character: char| {
        !character.is_ascii_hexdigit() && character != ':' && character != '%'
    });
    candidate.matches(':').count() >= 2
        && candidate
            .chars()
            .all(|character| character.is_ascii_hexdigit() || matches!(character, ':' | '%'))
}

fn collect_named_entries(
    destination: &Path,
    output_name: &str,
    root: &str,
    attributes: &[&str],
) -> Result<(), String> {
    let mut output = String::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            writeln!(output, "unavailable: {error}").map_err(|error| error.to_string())?;
            return write_file(destination.join(output_name), output.as_bytes())
                .map_err(display_io);
        }
    };

    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        output.push_str(name);
        output.push('\n');
        for attribute in attributes {
            let value = fs::read_to_string(path.join(attribute))
                .unwrap_or_else(|_| "unavailable".to_owned());
            writeln!(output, "  {attribute}: {}", value.trim())
                .map_err(|error| error.to_string())?;
        }
    }
    write_file(destination.join(output_name), output.as_bytes()).map_err(display_io)
}

#[cfg(test)]
mod tests {
    use super::{parse_destination, sanitize_bytes};
    use std::ffi::OsString;

    #[test]
    fn redacts_unique_and_secret_lines() {
        let input = b"processor: 0\nSerial: abc\nTOKEN=value\nmodel: SG2002\n";
        let actual = String::from_utf8(sanitize_bytes(input)).unwrap_or_default();
        assert_eq!(
            actual,
            "processor: 0\n[REDACTED LINE]\n[REDACTED LINE]\nmodel: SG2002\n"
        );
    }

    #[test]
    fn rejects_root_destination() {
        assert!(parse_destination([OsString::from("/")].into_iter()).is_err());
    }

    #[test]
    fn redacts_network_addresses() {
        let input = b"tcp 192.168.2.5:22 [fe80::4ada:35ff:fe6e:44be]:123 48:da:35:6f:44:be\n";
        let actual = String::from_utf8(sanitize_bytes(input)).unwrap_or_default();
        assert_eq!(
            actual,
            "tcp [REDACTED ADDRESS] [REDACTED ADDRESS] [REDACTED ADDRESS]\n"
        );
    }
}
