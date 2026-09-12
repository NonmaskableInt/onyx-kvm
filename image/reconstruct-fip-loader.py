#!/usr/bin/env python3
"""Replace only LOADER_2ND in an existing CVITEK FIP image."""

import argparse
import importlib.util
from pathlib import Path
from types import SimpleNamespace


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("fiptool")
    parser.add_argument("source_fip")
    parser.add_argument("loader_2nd")
    parser.add_argument("output_fip")
    args = parser.parse_args()

    spec = importlib.util.spec_from_file_location("cvitek_fiptool", args.fiptool)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    fip = module.FIP()
    fip.read_fip(args.source_fip)
    loader = Path(args.loader_2nd).read_bytes()
    magic = fip.ldr_2nd_hdr["MAGIC"]
    if loader[magic.addr:magic.end] != module.LOADER_2ND_MAGIC_ORIG:
        raise SystemExit("replacement loader does not have BL33 magic")

    fip.body2["LOADER_2ND"].content = loader
    fip.compress_algo = "lzma"
    rebuilt = fip.make(SimpleNamespace(BLCP_2ND_RUNADDR="0", MONITOR_RUNADDR="0"))
    Path(args.output_fip).write_bytes(rebuilt)


if __name__ == "__main__":
    main()
