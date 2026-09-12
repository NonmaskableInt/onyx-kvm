#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
tag=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}

command -v "$engine" >/dev/null 2>&1 || {
    printf 'missing container engine: %s\n' "$engine" >&2
    exit 1
}

"$engine" build --arch amd64 --pull=never --network=host \
    --tag "$tag" --file "$repo_root/image/builder/Containerfile" \
    "$repo_root/image/builder"

"$engine" image inspect --format '{{.Digest}} {{.Id}}' "$tag"

