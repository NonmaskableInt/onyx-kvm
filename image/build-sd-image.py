#!/usr/bin/env python3
import pathlib
import struct
import sys


SECTOR = 512
BOOT_START = 1
BOOT_SECTORS = 32768
ROOT_START = BOOT_START + BOOT_SECTORS
CLUSTER_SECTORS = 4
RESERVED_SECTORS = 4
FAT_SECTORS = 32
ROOT_ENTRIES = 512


def root_entry(name: bytes, cluster: int, size: int) -> bytes:
    if len(name) != 11:
        raise ValueError("FAT name must be exactly 11 bytes")
    entry = bytearray(32)
    entry[:11] = name
    entry[11] = 0x20
    struct.pack_into("<H", entry, 26, cluster)
    struct.pack_into("<I", entry, 28, size)
    return bytes(entry)


def make_fat16(files: list[tuple[bytes, bytes]]) -> bytes:
    volume = bytearray(BOOT_SECTORS * SECTOR)
    volume[0:3] = b"\xeb\x3c\x90"
    volume[3:11] = b"CLEANKVM"
    struct.pack_into("<H", volume, 11, SECTOR)
    volume[13] = CLUSTER_SECTORS
    struct.pack_into("<H", volume, 14, RESERVED_SECTORS)
    volume[16] = 2
    struct.pack_into("<H", volume, 17, ROOT_ENTRIES)
    struct.pack_into("<H", volume, 19, BOOT_SECTORS)
    volume[21] = 0xF8
    struct.pack_into("<H", volume, 22, FAT_SECTORS)
    struct.pack_into("<H", volume, 24, 32)
    struct.pack_into("<H", volume, 26, 2)
    struct.pack_into("<I", volume, 28, 0)
    volume[36] = 0x80
    volume[38] = 0x29
    struct.pack_into("<I", volume, 39, 0x434B564D)
    volume[43:54] = b"CLEANKVM   "
    volume[54:62] = b"FAT16   "
    volume[510:512] = b"\x55\xaa"

    fat = bytearray(FAT_SECTORS * SECTOR)
    struct.pack_into("<HH", fat, 0, 0xFFF8, 0xFFFF)
    root = bytearray(ROOT_ENTRIES * 32)
    data_start = (RESERVED_SECTORS + 2 * FAT_SECTORS + len(root) // SECTOR) * SECTOR
    cluster_bytes = CLUSTER_SECTORS * SECTOR
    next_cluster = 2

    for index, (name, content) in enumerate(files):
        count = (len(content) + cluster_bytes - 1) // cluster_bytes
        first = next_cluster
        for offset in range(count):
            cluster = first + offset
            successor = 0xFFFF if offset == count - 1 else cluster + 1
            struct.pack_into("<H", fat, cluster * 2, successor)
        destination = data_start + (first - 2) * cluster_bytes
        volume[destination : destination + len(content)] = content
        root[index * 32 : (index + 1) * 32] = root_entry(name, first, len(content))
        next_cluster += count

    if next_cluster >= (len(fat) // 2):
        raise SystemExit("boot files exceed FAT capacity")
    first_fat = RESERVED_SECTORS * SECTOR
    volume[first_fat : first_fat + len(fat)] = fat
    second_fat = (RESERVED_SECTORS + FAT_SECTORS) * SECTOR
    volume[second_fat : second_fat + len(fat)] = fat
    root_offset = (RESERVED_SECTORS + 2 * FAT_SECTORS) * SECTOR
    volume[root_offset : root_offset + len(root)] = root
    return bytes(volume)


output, boot_path, rootfs_path, fip_path = map(pathlib.Path, sys.argv[1:])
boot = boot_path.read_bytes()
rootfs = rootfs_path.read_bytes()
fip = fip_path.read_bytes()
root_sectors = (len(rootfs) + SECTOR - 1) // SECTOR
disk = bytearray((ROOT_START + root_sectors) * SECTOR)
disk[510:512] = b"\x55\xaa"

def partition(index: int, status: int, kind: int, start: int, sectors: int) -> None:
    offset = 446 + index * 16
    disk[offset] = status
    disk[offset + 1 : offset + 4] = b"\xfe\xff\xff"
    disk[offset + 4] = kind
    disk[offset + 5 : offset + 8] = b"\xfe\xff\xff"
    struct.pack_into("<II", disk, offset + 8, start, sectors)

partition(0, 0x80, 0x0C, BOOT_START, BOOT_SECTORS)
partition(1, 0x00, 0x83, ROOT_START, root_sectors)
fat = make_fat16([(b"FIP     BIN", fip), (b"BOOT    SD ", boot)])
disk[BOOT_START * SECTOR : (BOOT_START + BOOT_SECTORS) * SECTOR] = fat
disk[ROOT_START * SECTOR : ROOT_START * SECTOR + len(rootfs)] = rootfs
output.write_bytes(disk)
