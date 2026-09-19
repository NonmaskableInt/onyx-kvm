#!/usr/bin/env python3
"""
detect-target-disk.py - Removable SD card detection and target resolution tool.

Discovers external physical disks with size <= 32 GB (configurable ceiling) on
macOS, rejecting internal and virtual storage to prevent accidental host mutation.
Provides structured human-readable, JSON, and device path outputs for automated
and interactive flashing workflows.
"""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

# Default ceiling: 32 GiB (34,359,738,368 bytes) to accommodate standard 31.0 - 32.5 GB cards
DEFAULT_MAX_BYTES = 34_359_738_368


def run_cmd(cmd: List[str]) -> bytes:
    """Run a system command and return stdout bytes or raise an error."""
    try:
        proc = subprocess.run(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )
        return proc.stdout
    except subprocess.CalledProcessError as err:
        sys.stderr.write(f"Command {' '.join(cmd)} failed: {err.stderr.decode('utf-8', errors='replace')}\n")
        raise


def format_bytes(num_bytes: int) -> str:
    """Format bytes into a human-readable string (GB and GiB)."""
    gb = num_bytes / 1e9
    gib = num_bytes / (1024**3)
    return f"{gb:.1f} GB ({gib:.2f} GiB / {num_bytes:,} bytes)"


def get_all_disks_plist() -> Dict[str, Any]:
    """Retrieve full plist from diskutil list -plist."""
    raw = run_cmd(["diskutil", "list", "-plist"])
    return plistlib.loads(raw)


def get_disk_info_plist(disk_id: str) -> Dict[str, Any]:
    """Retrieve full plist from diskutil info -plist <disk>."""
    raw = run_cmd(["diskutil", "info", "-plist", disk_id])
    return plistlib.loads(raw)


def find_candidate_disks(max_bytes: int = DEFAULT_MAX_BYTES) -> List[Dict[str, Any]]:
    """Scan all disks and return candidate external whole physical disks <= max_bytes."""
    list_plist = get_all_disks_plist()
    whole_disks = list_plist.get("WholeDisks", [])

    candidates: List[Dict[str, Any]] = []

    # Map partitions from list_plist if available
    partitions_by_whole: Dict[str, List[Dict[str, Any]]] = {}
    for entry in list_plist.get("AllDisksAndPartitions", []):
        dev_id = entry.get("DeviceIdentifier")
        if dev_id:
            partitions_by_whole[dev_id] = entry.get("Partitions", [])

    for disk_id in sorted(whole_disks):
        try:
            info = get_disk_info_plist(disk_id)
        except Exception:
            continue

        # Strict Safety Filtering
        # 1. Must be whole disk
        if not info.get("WholeDisk", False):
            continue

        # 2. Must NOT be internal
        if info.get("Internal", True) or info.get("OSInternalMedia", False):
            continue

        # 3. Must be external / removable
        is_external = (
            info.get("RemovableMediaOrExternalDevice", False)
            or info.get("Removable", False)
            or info.get("DeviceLocation", "").lower() == "external"
        )
        if not is_external:
            continue

        # 4. Must NOT be virtual
        if info.get("VirtualOrPhysical", "").lower() == "virtual" or info.get("Virtual", False):
            continue

        # 5. Size check
        total_size = info.get("TotalSize", 0)
        if total_size <= 0 or total_size > max_bytes:
            continue

        # Extract volume names and mount points
        partitions = partitions_by_whole.get(disk_id, [])
        volume_names: List[str] = []
        mount_points: List[str] = []
        partition_summaries: List[Dict[str, Any]] = []

        for p in partitions:
            p_id = p.get("DeviceIdentifier", "")
            p_size = p.get("Size", 0)
            p_content = p.get("Content", "")
            p_vol = p.get("VolumeName")
            p_mount = p.get("MountPoint")

            if p_vol:
                volume_names.append(p_vol)
            if p_mount:
                mount_points.append(p_mount)

            partition_summaries.append({
                "identifier": p_id,
                "node": f"/dev/{p_id}",
                "size_bytes": p_size,
                "content": p_content,
                "volume_name": p_vol,
                "mount_point": p_mount,
            })

        # Also check top-level volume name (if disk has no partition table, e.g. superfloppy)
        top_vol = info.get("VolumeName")
        if top_vol and top_vol not in volume_names:
            volume_names.append(top_vol)
        top_mount = info.get("MountPoint")
        if top_mount and top_mount not in mount_points:
            mount_points.append(top_mount)

        candidate = {
            "identifier": disk_id,
            "node": f"/dev/{disk_id}",
            "raw_node": f"/dev/r{disk_id}",
            "size_bytes": total_size,
            "size_formatted": format_bytes(total_size),
            "media_name": info.get("MediaName", "Unknown"),
            "bus_protocol": info.get("BusProtocol", "Unknown"),
            "partition_scheme": info.get("Content", "None"),
            "removable": info.get("Removable", False),
            "volume_names": volume_names,
            "mount_points": mount_points,
            "partitions": partition_summaries,
        }
        candidates.append(candidate)

    return candidates


def filter_candidates(
    candidates: List[Dict[str, Any]],
    disk_filter: Optional[str] = None,
    label_filter: Optional[str] = None,
) -> List[Dict[str, Any]]:
    """Filter candidates by disk identifier/path or volume label."""
    filtered = candidates

    if disk_filter:
        clean_id = disk_filter.strip().replace("/dev/r", "").replace("/dev/", "")
        filtered = [c for c in filtered if c["identifier"] == clean_id]

    if label_filter:
        pattern = label_filter.strip().lower()
        filtered = [
            c for c in filtered
            if any(pattern in v.lower() for v in c["volume_names"])
        ]

    return filtered


def unmount_disk(disk_id: str) -> bool:
    """Unmount all volumes on the disk using diskutil unmountDisk."""
    clean_id = disk_id.strip().replace("/dev/r", "").replace("/dev/", "")
    try:
        out = run_cmd(["diskutil", "unmountDisk", clean_id])
        sys.stderr.write(out.decode("utf-8", errors="replace"))
        return True
    except Exception as err:
        sys.stderr.write(f"Failed to unmount {clean_id}: {err}\n")
        return False


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Detect and resolve external target SD cards (<= 32 GB) on macOS."
    )
    parser.add_argument(
        "--resolve",
        action="store_true",
        help="Resolve to a single target disk and output its path (fails closed if ambiguous).",
    )
    parser.add_argument(
        "--raw",
        action="store_true",
        help="When resolving, output raw character device node (e.g. /dev/rdisk4) instead of block device.",
    )
    parser.add_argument(
        "--id-only",
        action="store_true",
        help="When resolving, output disk identifier only (e.g. disk4).",
    )
    parser.add_argument(
        "--disk",
        type=str,
        help="Filter candidate by disk identifier or node (e.g. disk4 or /dev/disk4).",
    )
    parser.add_argument(
        "--label",
        type=str,
        help="Filter candidate by volume label substring (case-insensitive, e.g. CLEANKVM).",
    )
    parser.add_argument(
        "--max-gb",
        type=float,
        default=32.0,
        help="Maximum drive size in GB (default: 32.0; threshold allows up to 34.36 GB / 32 GiB).",
    )
    parser.add_argument(
        "--unmount",
        action="store_true",
        help="Unmount all volumes on the resolved disk before exiting.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output candidate information in JSON format.",
    )
    parser.add_argument(
        "--quiet",
        "-q",
        action="store_true",
        help="Suppress informational stderr messages.",
    )

    args = parser.parse_args()

    # Calculate byte ceiling: allow up to nominal GB + 7.5% margin or GiB equivalent
    max_bytes = int(args.max_gb * (1024**3))
    if max_bytes < DEFAULT_MAX_BYTES and args.max_gb == 32.0:
        max_bytes = DEFAULT_MAX_BYTES

    try:
        all_candidates = find_candidate_disks(max_bytes=max_bytes)
    except Exception as err:
        sys.stderr.write(f"Error querying system disks: {err}\n")
        return 3

    candidates = filter_candidates(all_candidates, disk_filter=args.disk, label_filter=args.label)

    # JSON Output Mode
    if args.json:
        if args.resolve:
            if len(candidates) != 1:
                sys.stderr.write(
                    f"Resolution error: expected 1 target, found {len(candidates)} candidates.\n"
                )
                print(json.dumps({"error": "ambiguous_or_missing", "candidates": candidates}, indent=2))
                return 2 if len(candidates) > 1 else 1
            res = candidates[0]
            if args.unmount:
                res["unmounted"] = unmount_disk(res["identifier"])
            print(json.dumps(res, indent=2))
            return 0
        else:
            print(json.dumps(candidates, indent=2))
            return 0

    # Resolution Mode
    if args.resolve:
        if len(candidates) == 0:
            if not args.quiet:
                sys.stderr.write("No external disk <= 32 GB matched the criteria.\n")
                if len(all_candidates) > 0:
                    sys.stderr.write(f"Note: {len(all_candidates)} external disk(s) found before filtering:\n")
                    for c in all_candidates:
                        sys.stderr.write(f"  - {c['node']} ({c['size_formatted']}) labels: {c['volume_names']}\n")
            return 1

        if len(candidates) > 1:
            sys.stderr.write(
                f"Safety check failed: multiple external target disks ({len(candidates)}) detected.\n"
                "To resolve unambiguously, specify --disk <id> or --label <name>:\n"
            )
            for c in candidates:
                sys.stderr.write(
                    f"  - {c['node']} (raw: {c['raw_node']}, size: {c['size_formatted']}, "
                    f"media: '{c['media_name']}', labels: {c['volume_names']})\n"
                )
            return 2

        resolved = candidates[0]

        if args.unmount:
            if not args.quiet:
                sys.stderr.write(f"Unmounting volumes on {resolved['node']}...\n")
            if not unmount_disk(resolved["identifier"]):
                return 3

        if args.raw:
            print(resolved["raw_node"])
        elif args.id_only:
            print(resolved["identifier"])
        else:
            print(resolved["node"])
        return 0

    # Default List Mode (Human-Readable Table)
    if len(candidates) == 0:
        print("No external disks <= 32 GB found.")
        return 0

    print(f"Found {len(candidates)} external candidate target disk(s) <= {args.max_gb} GB:")
    print("-" * 80)
    for idx, c in enumerate(candidates, 1):
        vols = ", ".join(c["volume_names"]) if c["volume_names"] else "None"
        mounts = ", ".join(c["mount_points"]) if c["mount_points"] else "Not mounted"
        print(f"[{idx}] Identifier:       {c['identifier']}")
        print(f"    Device Node:      {c['node']} (raw: {c['raw_node']})")
        print(f"    Disk Size:        {c['size_formatted']}")
        print(f"    Media / Protocol: {c['media_name']} via {c['bus_protocol']}")
        print(f"    Partition Scheme: {c['partition_scheme']}")
        print(f"    Volume Label(s):  {vols}")
        print(f"    Mount Point(s):   {mounts}")
        if c["partitions"]:
            print("    Partitions:")
            for p in c["partitions"]:
                p_label = f" '{p['volume_name']}'" if p['volume_name'] else ""
                p_mount = f" at {p['mount_point']}" if p['mount_point'] else ""
                print(f"      * {p['identifier']} ({p['content']}, {p['size_bytes']:,} bytes){p_label}{p_mount}")
        print("-" * 80)

    print("\nUsage tips:")
    print("  Resolve standard node: python3 image/detect-target-disk.py --resolve [--label <label> | --disk <id>]")
    print("  Resolve raw device:    python3 image/detect-target-disk.py --resolve --raw")
    print("  Resolve and unmount:   python3 image/detect-target-disk.py --resolve --unmount")
    print("  Structured JSON:       python3 image/detect-target-disk.py --json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
