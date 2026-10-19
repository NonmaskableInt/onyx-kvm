use onyx_kvm_hid_broker::{MAX_FRAME_LEN, ReportBytes, decode_and_authorize};
use std::env;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_SOCKET: &str = "/run/onyx-kvm/hid.sock";
const DEFAULT_ENABLE: &str = "/run/onyx-kvm/hid-enabled";
const DEFAULT_KEYBOARD: &str = "/dev/hidg0";
const DEFAULT_POINTER: &str = "/dev/hidg1";
#[cfg(target_os = "linux")]
const ONYX_KVM_UID: u32 = 900;
const ONYX_KVM_GID: u32 = 900;

struct Config {
    socket: PathBuf,
    enable: PathBuf,
    keyboard: PathBuf,
    pointer: PathBuf,
}

fn main() -> ExitCode {
    let config = Config {
        socket: env_path("ONYX_KVM_HID_SOCKET", DEFAULT_SOCKET),
        enable: env_path("ONYX_KVM_HID_ENABLE", DEFAULT_ENABLE),
        keyboard: env_path("ONYX_KVM_HID_KEYBOARD", DEFAULT_KEYBOARD),
        pointer: env_path("ONYX_KVM_HID_POINTER", DEFAULT_POINTER),
    };
    match run(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("onyx-kvm-hid-broker: {error}");
            ExitCode::FAILURE
        }
    }
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name).map_or_else(|| PathBuf::from(default), PathBuf::from)
}

fn run(config: &Config) -> io::Result<()> {
    if let Some(parent) = config.socket.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, Permissions::from_mode(0o2770))?;
    }
    match fs::remove_file(&config.socket) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = UnixListener::bind(&config.socket)?;
    rustix::fs::chown(
        &config.socket,
        Some(rustix::fs::Uid::from_raw(0)),
        Some(rustix::fs::Gid::from_raw(ONYX_KVM_GID)),
    )
    .map_err(io::Error::from)?;
    fs::set_permissions(&config.socket, Permissions::from_mode(0o660))?;
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if let Err(error) = serve(stream, config) {
                    eprintln!("onyx-kvm-hid-broker: client rejected: {error}");
                }
            }
            Err(error) => eprintln!("onyx-kvm-hid-broker: accept failed: {error}"),
        }
    }
    Ok(())
}

fn serve(mut stream: UnixStream, config: &Config) -> io::Result<()> {
    authenticate_peer(&stream)?;
    let mut keyboard = open_device(&config.keyboard)?;
    let mut pointer = open_device(&config.pointer)?;
    let result = serve_frames(&mut stream, config, &mut keyboard, &mut pointer);
    let keyboard_release = keyboard.write_all(&[0_u8; 8]);
    let pointer_release = pointer.write_all(&[0_u8; 4]);
    result.and(keyboard_release).and(pointer_release)
}

#[cfg(target_os = "linux")]
fn authenticate_peer(stream: &UnixStream) -> io::Result<()> {
    let credentials = rustix::net::sockopt::socket_peercred(stream).map_err(io::Error::from)?;
    if credentials.uid.as_raw() != ONYX_KVM_UID {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("peer uid {} is not authorized", credentials.uid.as_raw()),
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn authenticate_peer(_stream: &UnixStream) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "peer credential authentication requires Linux",
    ))
}

fn open_device(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).open(path)
}

fn serve_frames(
    stream: &mut UnixStream,
    config: &Config,
    keyboard: &mut File,
    pointer: &mut File,
) -> io::Result<()> {
    loop {
        let mut header = [0_u8; 5];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        }
        let payload_len = usize::from(header[4]);
        if payload_len > MAX_FRAME_LEN - header.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized HID frame",
            ));
        }
        let mut frame = [0_u8; MAX_FRAME_LEN];
        frame[..header.len()].copy_from_slice(&header);
        stream.read_exact(&mut frame[header.len()..header.len() + payload_len])?;
        let enabled = config.enable.is_file();
        let report = decode_and_authorize(&frame[..header.len() + payload_len], enabled)
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        match report.bytes() {
            ReportBytes::Keyboard(bytes) => keyboard.write_all(&bytes)?,
            ReportBytes::Pointer(bytes) => pointer.write_all(&bytes)?,
        }
        stream.write_all(&[0])?;
    }
}
