#!/usr/bin/env python3
import base64
import hashlib
import os
import re
import select
import sys
import termios
import time

SERIAL = "/dev/cu.usbserial-112440"
REMOTE_DUMP = "/run/onyx-kvm-lt6911-flash-800000.bin"

credential_path, log_path, dump_path = sys.argv[1:4]
credential = open(credential_path, encoding="utf-8").read()
match = re.search(r"^password=(.*)$", credential, re.MULTILINE)
if not match:
    raise SystemExit("credential file has no password field")
password = match.group(1)

fd = os.open(SERIAL, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
attrs = termios.tcgetattr(fd)
attrs[0] = attrs[1] = attrs[3] = 0
attrs[2] = termios.CLOCAL | termios.CREAD | termios.CS8
attrs[4] = attrs[5] = termios.B115200
termios.tcsetattr(fd, termios.TCSANOW, attrs)
termios.tcflush(fd, termios.TCIOFLUSH)
log = open(log_path, "wb")

def write(data):
    os.write(fd, data.encode())

def read_until(pattern, timeout, display=True):
    end = time.monotonic() + timeout
    data = bytearray()
    regex = re.compile(pattern)
    while time.monotonic() < end:
        ready, _, _ = select.select([fd], [], [], min(1, end - time.monotonic()))
        if not ready:
            continue
        chunk = os.read(fd, 65536)
        if not chunk:
            continue
        data.extend(chunk)
        log.write(chunk)
        log.flush()
        if display:
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
        if regex.search(data):
            return bytes(data)
    raise TimeoutError(f"UART timeout waiting for {pattern!r}")

def command(text, timeout=30, display=True):
    write(text + "\r")
    return read_until(rb"\r?\n# ", timeout, display)

try:
    time.sleep(1)
    write("\r")
    greeting = read_until(rb"(?:login:|\r?\n# )", 30)
    if b"login:" in greeting:
        write("root\r")
        read_until(rb"Password:", 10)
        write(password + "\r")
        password = ""
        credential = ""
        read_until(rb"\r?\n# ", 10)
    command("echo ONYX_KVM_LT_FLASH_BACKUP_V29_BEGIN")
    command("printf 'UPTIME='; cut -d' ' -f1 /proc/uptime; printf 'TAINT='; cat /proc/sys/kernel/tainted")
    command("printf 'BACKUP_SHA256='; sha256sum /usr/libexec/onyx-kvm-lt6911uxc-flash-backup | cut -d' ' -f1")
    command("test -c /dev/i2c-4 && test ! -e /dev/snd/controlC0 && test ! -e %s && echo CONTAINMENT_GATE=passed || { echo CONTAINMENT_GATE=failed; exit 91; }" % REMOTE_DUMP)
    command("printf 'BASELINE_MODULES='; awk 'END {print NR}' /proc/modules; printf 'BASELINE_I2C_FDS='; for p in /proc/[0-9]*/fd/*; do test \"$(readlink \"$p\" 2>/dev/null)\" = /dev/i2c-4 && printf '%%s,' \"$p\"; done; echo")
    result = command("/usr/libexec/onyx-kvm-lt6911uxc-flash-backup --dump-stock-range; printf 'BACKUP_RC=%s\\n' \"$?\"", 1500)
    command("printf 'FINAL_TAINT='; cat /proc/sys/kernel/tainted; printf 'FINAL_I2C_FDS='; for p in /proc/[0-9]*/fd/*; do test \"$(readlink \"$p\" 2>/dev/null)\" = /dev/i2c-4 && printf '%%s,' \"$p\"; done; echo")
    if not re.search(rb"BACKUP_RC=0(?:\r?\n)", result):
        raise RuntimeError("device rejected the flash read; no transfer attempted")
    status = command("printf 'DUMP_SHA256='; sha256sum %s | cut -d' ' -f1; printf 'DUMP_BYTES='; wc -c <%s" % (REMOTE_DUMP, REMOTE_DUMP))
    encoded = command("printf '\\nDUMP_BASE64_BEGIN\\n'; base64 %s; printf '\\nDUMP_BASE64_END\\n'" % REMOTE_DUMP, 300, False)
    encoded = encoded.replace(b"\r", b"")
    payload = encoded.split(b"DUMP_BASE64_BEGIN\n", 1)[1].split(b"\nDUMP_BASE64_END", 1)[0]
    dump = base64.b64decode(payload)
    if len(dump) != 800000:
        raise RuntimeError(f"decoded dump length is {len(dump)}, expected 800000")
    digest = hashlib.sha256(dump).hexdigest()
    remote = re.search(rb"DUMP_SHA256=([0-9a-f]{64})", status)
    if not remote or digest != remote.group(1).decode():
        raise RuntimeError("host dump hash does not match device hash")
    output = os.open(dump_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o400)
    os.write(output, dump)
    os.fsync(output)
    os.close(output)
    print(f"HOST_DUMP_BYTES={len(dump)} HOST_DUMP_SHA256={digest}")
    command("echo ONYX_KVM_LT_FLASH_BACKUP_V29_END")
finally:
    password = ""
    credential = ""
    log.close()
    os.close(fd)
