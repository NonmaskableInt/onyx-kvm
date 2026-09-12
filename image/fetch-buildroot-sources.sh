#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v4}
download_volume=${CLEANKVM_DOWNLOAD_VOLUME:-cleankvm-buildroot-dl-v2}

command -v "$engine" >/dev/null 2>&1 || {
    printf 'missing container engine: %s\n' "$engine" >&2
    exit 1
}

if ! "$engine" volume inspect "$download_volume" >/dev/null 2>&1; then
    "$engine" volume create "$download_volume" >/dev/null
    "$engine" run --rm --arch amd64 --user 0:0 \
        -v "$download_volume:/downloads" \
        "$builder" chown 1000:1000 /downloads
fi

# This is the only network-enabled image stage. Buildroot verifies downloads
# against its package .hash files where supplied; the complete resolved cache
# is additionally recorded for later fail-closed offline use.
"$engine" run --rm --arch amd64 \
    -v "$work_volume:/work:rw" \
    -v "$download_volume:/downloads:rw" \
    -v "$repo_root:/repo:ro" \
    -w /work/sdk \
    -e TMPDIR=/work/sdk/.tmp \
    "$builder" bash -lc '
        set -e
        mkdir -p "$TMPDIR"
        source build/cvisetup.sh
        defconfig sg2002_licheervnano_sd
        make -C buildroot clean
        make -C buildroot cvitek_SG200X_musl_riscv64_defconfig \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64
        make -C buildroot source \
            BR2_DL_DIR=/downloads \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64
        find /downloads -type f ! -name SHA256SUMS -print0 |
            sort -z |
            xargs -0 sha256sum > /downloads/SHA256SUMS
    '

"$engine" run --rm --arch amd64 \
    -v "$download_volume:/downloads:ro" \
    "$builder" sh -lc '
        set -eu
        test -s /downloads/SHA256SUMS
        cd /
        sha256sum -c /downloads/SHA256SUMS
    '

printf 'verified Buildroot source cache: %s\n' "$download_volume"
