#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sdk_dir=${CLEANKVM_SDK_DIR:-"$repo_root/dl/LicheeRV-Nano-Build"}

"$repo_root/image/verify-sources.sh"

test "$(uname -s)" = Linux || {
    printf '%s\n' 'image builds require the pinned Linux builder environment' >&2
    exit 1
}

printf 'building from %s\n' "$sdk_dir"
"$repo_root/image/apply-phase1-config.sh" "$sdk_dir"
