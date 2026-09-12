#!/usr/bin/env python3
import hashlib
import pathlib
import struct
import sys


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


image_path, boot_path, rootfs_path, fip_path = map(pathlib.Path, sys.argv[1:])
image = image_path.read_bytes()
boot = boot_path.read_bytes()
rootfs = rootfs_path.read_bytes()
fip = fip_path.read_bytes()

if image[510:512] != b"\x55\xaa":
    raise SystemExit("invalid MBR signature")

parts = []
for index in range(2):
    entry = image[446 + index * 16 : 462 + index * 16]
    status, part_type = entry[0], entry[4]
    start, sectors = struct.unpack_from("<II", entry, 8)
    parts.append((status, part_type, start * 512, sectors * 512))

if parts[0][0] != 0x80 or parts[0][1] != 0x0C:
    raise SystemExit("partition 1 is not the SG2002-compatible boot partition")
if parts[1][1] != 0x83:
    raise SystemExit("partition 2 is not Linux")
if parts[0][2] != 512 or parts[0][3] != 32768 * 512:
    raise SystemExit("boot partition does not match the SG2002 sector-1 layout")
if parts[1][2] != 32769 * 512:
    raise SystemExit("root partition does not follow the SG2002 boot partition")

boot_partition = image[parts[0][2] : parts[0][2] + parts[0][3]]
root_partition = image[parts[1][2] : parts[1][2] + len(rootfs)]
if struct.unpack_from("<H", boot_partition, 14)[0] != 4:
    raise SystemExit("boot filesystem does not use four reserved sectors")
if digest(root_partition) != digest(rootfs):
    raise SystemExit("embedded root filesystem differs from audited artifact")
if boot_partition.count(boot) != 1:
    raise SystemExit("boot FIT is not present exactly once in FAT partition")
if boot_partition.count(fip) != 1:
    raise SystemExit("FIP is not present exactly once in FAT partition")
if b"BOOT    SD " not in boot_partition or b"FIP     BIN" not in boot_partition:
    raise SystemExit("expected FAT directory entries are absent")

print(f"image_sha256={digest(image)}")
print(f"image_bytes={len(image)}")
print(f"boot_partition_offset={parts[0][2]}")
print(f"root_partition_offset={parts[1][2]}")
print("embedded_artifacts=verified")
