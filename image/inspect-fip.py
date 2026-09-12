#!/usr/bin/env python3
import hashlib
import importlib.util
import json
import pathlib
import sys


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


repo = pathlib.Path(__file__).resolve().parents[1]
tool_path = repo / "dl/LicheeRV-Nano-Build/fsbl/plat/sg200x/fiptool.py"
spec = importlib.util.spec_from_file_location("sg200x_fiptool", tool_path)
if spec is None or spec.loader is None:
    raise SystemExit(f"cannot load FIP parser: {tool_path}")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


for argument in sys.argv[1:]:
    path = pathlib.Path(argument)
    fip = module.FIP()
    fip.read_fip(path)
    result = {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": sha256(path.read_bytes()),
        "param1": {name: entry.toint() for name, entry in fip.param1.items() if entry.type is int},
        "param2": {name: entry.toint() for name, entry in fip.param2.items() if entry.type is int},
        "components": {},
    }
    for name, entry in list(fip.body1.items()) + list(fip.body2.items()):
        content = bytes(entry.content)
        result["components"][name] = {
            "bytes": len(content),
            "sha256": sha256(content),
        }
    print(json.dumps(result, sort_keys=True))
