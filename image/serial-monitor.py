#!/usr/bin/env python3
"""
serial-monitor.py - Serial monitor and diagnostic harness for NanoKVM Lite UART.
"""

from __future__ import annotations

import argparse
import os
import re
import select
import sys
import termios
import time
import tty
from typing import Optional


def open_serial(port: str = "/dev/cu.usbserial-112440", baudrate: int = 115200) -> int:
    fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    attrs = termios.tcgetattr(fd)
    tty.setraw(fd)
    attrs[4] = getattr(termios, f"B{baudrate}", termios.B115200)
    attrs[5] = getattr(termios, f"B{baudrate}", termios.B115200)
    attrs[0] = termios.ICRNL
    attrs[1] = 0
    attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
    attrs[3] = 0
    termios.tcsetattr(fd, termios.TCSANOW, attrs)
    return fd


def drain(fd: int) -> None:
    while True:
        r, _, _ = select.select([fd], [], [], 0.05)
        if not r:
            break
        os.read(fd, 1024)


def read_until(fd: int, pattern: str, timeout: float = 10.0) -> tuple[bool, str]:
    buf = ""
    start = time.time()
    while time.time() - start < timeout:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            chunk = os.read(fd, 4096).decode("utf-8", errors="replace")
            sys.stdout.write(chunk)
            sys.stdout.flush()
            buf += chunk
            if pattern in buf:
                return True, buf
    return False, buf


def login(fd: int, credential_path: str) -> bool:
    with open(credential_path, "r", encoding="utf-8") as f:
        content = f.read()
    m = re.search(r"^password=(.*)$", content, re.MULTILINE)
    if not m:
        sys.stderr.write("No password found in credential file\n")
        return False
    password = m.group(1).strip()

    drain(fd)
    # Check if already at root prompt
    os.write(fd, b"\r\n")
    time.sleep(0.3)
    ok, buf = read_until(fd, "#", timeout=1.0)
    if ok:
        return True

    # Trigger login prompt
    if "login:" not in buf:
        os.write(fd, b"\r\n")
        ok, buf = read_until(fd, "login:", timeout=3.0)
        if not ok:
            sys.stderr.write("Failed to see login prompt\n")
            return False

    time.sleep(0.2)
    os.write(fd, b"root\r\n")
    ok, buf = read_until(fd, "Password:", timeout=3.0)
    if not ok:
        sys.stderr.write("Failed to see Password prompt\n")
        return False

    time.sleep(0.2)
    os.write(fd, password.encode("ascii") + b"\r\n")
    ok, buf = read_until(fd, "#", timeout=5.0)
    if not ok:
        sys.stderr.write("Failed to authenticate into root shell\n")
        return False

    return True


def run_cmd(fd: int, cmd: str, timeout: float = 30.0) -> str:
    drain(fd)
    os.write(fd, cmd.encode("ascii") + b"\r\n")
    buf = ""
    start = time.time()
    while time.time() - start < timeout:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            chunk = os.read(fd, 4096).decode("utf-8", errors="replace")
            sys.stdout.write(chunk)
            sys.stdout.flush()
            buf += chunk
            if "\n# " in buf or buf.endswith("# "):
                break
    return buf


def push_file(fd: int, local_path: str, remote_path: str) -> bool:
    import base64
    with open(local_path, "rb") as f:
        data = f.read()
    b64 = base64.b64encode(data).decode("ascii")
    print(f"Uploading {local_path} ({len(data)} bytes) to {remote_path} via base64...")
    run_cmd(fd, f"rm -f {remote_path} {remote_path}.b64")
    chunk_size = 512
    for i in range(0, len(b64), chunk_size):
        chunk = b64[i:i+chunk_size]
        run_cmd(fd, f"printf '%s' '{chunk}' >> {remote_path}.b64", timeout=5.0)
    run_cmd(fd, f"base64 -d {remote_path}.b64 > {remote_path} && chmod 0755 {remote_path} && rm -f {remote_path}.b64")
    out = run_cmd(fd, f"sha256sum {remote_path}")
    print("Remote sha256:", out.strip())
    return True


def run_discriminator(fd: int, credential_path: str, profile: int = 0, probe_path: str = "/usr/libexec/cleankvm-raw-frame-probe", probe_args: str = "", vi_path: Optional[str] = None, mipi_path: Optional[str] = None, mipi_params: str = "") -> bool:
    print("Logging into CleanKVM target...")
    if not login(fd, credential_path):
        return False

    print(f"\n--- Starting CleanKVM Lane Discriminator Test (Profile {profile}, Probe {probe_path}, Args: '{probe_args}', VI: '{vi_path}', MIPI: '{mipi_path}', MIPI_Params: '{mipi_params}') ---\n")
    modules = ["soph_sys", "soph_base", "soph_mipi_rx", "soph_snsr_i2c", "soph_vi", "soph_vpss"]
    reverse_modules = list(reversed(modules))
    module_dir = "/usr/lib/cleankvm/video-quarantine"

    run_cmd(fd, "echo CLEANKVM_LANE_DISCRIMINATOR_V1_BEGIN")
    run_cmd(fd, "test -r /proc/modules && test ! -e /dev/snd/controlC0 && echo BASELINE_GATE=passed || { echo BASELINE_GATE=failed; exit 91; }")
    run_cmd(fd, f"printf 'PROBE_SHA256='; sha256sum {probe_path} | cut -d' ' -f1; printf 'PRE_TAINT='; cat /proc/sys/kernel/tainted")

    run_cmd(fd, f"echo LANE_PROFILE_{profile}_BEGIN")
    run_cmd(fd, "dmesg -c > /dev/null")
    for mod in modules:
        params = ""
        if mod == "soph_vi":
            params = " vi_log_lv=0x7"
            if vi_path:
                mod_file = vi_path
            else:
                mod_file = f"{module_dir}/{mod}.ko"
        elif mod == "soph_mipi_rx":
            if mipi_params:
                params = f" {mipi_params}"
            if mipi_path:
                mod_file = mipi_path
            else:
                mod_file = f"{module_dir}/{mod}.ko"
        else:
            mod_file = f"{module_dir}/{mod}.ko"
        run_cmd(fd, f"printf '{mod}_SHA256='; sha256sum {mod_file} | cut -d' ' -f1")
        run_cmd(fd, f"insmod {mod_file}{params}; echo LOAD_{profile}_{mod}_RC=$?")

    # Run the probe (gives time for receiver output)
    probe_cmd = f"{probe_path} {profile} {probe_args}".strip()
    run_cmd(fd, f"{probe_cmd}; echo PROBE_{profile}_RC=$?", timeout=40.0)
    run_cmd(fd, "dmesg")
    run_cmd(fd, "ls -l /run/cleankvm-frame.yuv422p 2>/dev/null; sha256sum /run/cleankvm-frame.yuv422p 2>/dev/null; test -e /run/cleankvm-frame.yuv422p; echo FRAME_PRESENT_RC=$?")
    run_cmd(fd, "rm -f /run/cleankvm-frame.yuv422p; echo FRAME_CLEANUP_RC=$?")

    for mod in reverse_modules:
        run_cmd(fd, f"rmmod {mod}; echo UNLOAD_{profile}_{mod}_RC=$?")

    run_cmd(fd, f"printf 'PROFILE_{profile}_REMAINDER='; awk '$1 ~ /^soph_/ {{printf \"%s,\", $1}}' /proc/modules; echo")
    run_cmd(fd, f"echo LANE_PROFILE_{profile}_END")

    run_cmd(fd, "printf 'FINAL_LOADED='; awk '$1 ~ /^soph_/ {printf \"%s,\", $1}' /proc/modules; echo; printf 'POST_TAINT='; cat /proc/sys/kernel/tainted; test ! -e /run/cleankvm-frame.yuv422p; echo FINAL_FRAME_ABSENT_RC=$?")
    run_cmd(fd, "echo CLEANKVM_LANE_DISCRIMINATOR_V1_END")
    return True


def run_lt_status(fd: int) -> bool:
    print("\n--- Querying LT6911 Bridge Status ---")
    run_cmd(fd, "test -c /dev/i2c-4 && printf 'TAINT=' && cat /proc/sys/kernel/tainted")
    run_cmd(fd, "i2cset -y 4 0x2b 0xff 0x80 && i2cset -y 4 0x2b 0xee 0x01 && echo MCU_STOP=passed")
    run_cmd(fd, "i2cset -y 4 0x2b 0xff 0x86 && printf 'HDMI_SIGNAL_A3=' && i2cget -y 4 0x2b 0xa3")
    run_cmd(fd, "i2cset -y 4 0x2b 0xff 0x85 && printf 'CSI_VACTIVE=' && i2ctransfer -y 4 w1@0x2b 0xf0 r2 && printf ' CSI_HACTIVE=' && i2ctransfer -y 4 w1@0x2b 0xea r2; echo")
    run_cmd(fd, "i2cset -y 4 0x2b 0xff 0x80; i2cset -y 4 0x2b 0xee 0x00; echo MCU_RESUME_RC=$?")
    run_cmd(fd, "printf 'FINAL_TAINT='; cat /proc/sys/kernel/tainted; printf 'I2C_FDS='; for p in /proc/[0-9]*/fd/*; do test \"$(readlink \"$p\" 2>/dev/null)\" = /dev/i2c-4 && printf '%s,' \"$p\"; done; echo")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Monitor NanoKVM Lite serial port.")
    parser.add_argument(
        "--port", default="/dev/cu.usbserial-112440", help="Serial port device path"
    )
    parser.add_argument(
        "--credentials", default="out/phase2-host-provisioned-dev/recovery-credential.txt", help="Credential file path"
    )
    parser.add_argument(
        "--read", type=float, default=0.0, help="Listen passively for N seconds"
    )
    parser.add_argument(
        "--prompt", action="store_true", help="Send newline and check current prompt"
    )
    parser.add_argument(
        "--cmd", type=str, help="Execute command on shell prompt"
    )
    parser.add_argument(
        "--run-discriminator", action="store_true", help="Run discriminator test"
    )
    parser.add_argument(
        "--profile", type=int, default=0, help="Lane profile index"
    )
    parser.add_argument(
        "--lt-status", action="store_true", help="Query LT6911 bridge status registers"
    )
    parser.add_argument(
        "--push-probe", type=str, help="Upload local probe binary to target /tmp/cleankvm-raw-frame-probe"
    )
    parser.add_argument(
        "--push-file", type=str, help="Upload local file to target"
    )
    parser.add_argument(
        "--push-dest", type=str, default="/tmp/soph_mipi_rx.ko", help="Remote destination for --push-file"
    )
    parser.add_argument(
        "--probe-path", type=str, default="/usr/libexec/cleankvm-raw-frame-probe", help="Path to probe executable on target"
    )
    parser.add_argument(
        "--probe-args", type=str, default="", help="Additional arguments to pass to probe"
    )
    parser.add_argument(
        "--vi-path", type=str, default=None, help="Custom soph_vi.ko module path on target"
    )
    parser.add_argument(
        "--mipi-path", type=str, default=None, help="Custom soph_mipi_rx.ko module path on target"
    )
    parser.add_argument(
        "--mipi-params", type=str, default="", help="Module parameters for soph_mipi_rx.ko"
    )
    args = parser.parse_args()

    try:
        fd = open_serial(args.port)
    except Exception as err:
        sys.stderr.write(f"Failed to open {args.port}: {err}\n")
        return 1

    try:
        if args.read > 0:
            read_until(fd, "___NEVER___", timeout=args.read)
            return 0

        if args.prompt:
            os.write(fd, b"\r\n")
            read_until(fd, "___NEVER___", timeout=1.5)
            return 0

        if args.push_probe:
            if os.path.exists(args.credentials):
                login(fd, args.credentials)
            ok = push_file(fd, args.push_probe, "/tmp/cleankvm-raw-frame-probe")
            return 0 if ok else 1

        if args.push_file:
            if os.path.exists(args.credentials):
                login(fd, args.credentials)
            ok = push_file(fd, args.push_file, args.push_dest)
            return 0 if ok else 1

        if args.run_discriminator:
            ok = run_discriminator(fd, args.credentials, profile=args.profile, probe_path=args.probe_path, probe_args=args.probe_args, vi_path=args.vi_path, mipi_path=args.mipi_path, mipi_params=args.mipi_params)
            return 0 if ok else 1

        if args.lt_status:
            if os.path.exists(args.credentials):
                login(fd, args.credentials)
            ok = run_lt_status(fd)
            return 0 if ok else 1

        if args.cmd:
            if os.path.exists(args.credentials):
                login(fd, args.credentials)
            out = run_cmd(fd, args.cmd)
            return 0

        # Default: read for 3 seconds
        read_until(fd, "___NEVER___", timeout=3.0)
        return 0
    finally:
        os.close(fd)


if __name__ == "__main__":
    sys.exit(main())
