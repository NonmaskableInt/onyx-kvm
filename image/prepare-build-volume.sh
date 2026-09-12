#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
source_volume=${CLEANKVM_SOURCE_VOLUME:-cleankvm-sdk}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v5}
daemon_binary="$repo_root/out/phase2-rust/cleankvmd"
memtest_binary="$repo_root/out/phase2-rust/cleankvm-memtest"

test -f "$daemon_binary" || {
    printf '%s\n' 'missing target daemon; run image/build-rust-daemon.sh first' >&2
    exit 1
}
test -f "$memtest_binary" || {
    printf '%s\n' 'missing target memory tester; run image/build-rust-memtest.sh first' >&2
    exit 1
}

command -v "$engine" >/dev/null 2>&1 || {
    printf 'missing container engine: %s\n' "$engine" >&2
    exit 1
}

if "$engine" volume inspect "$work_volume" >/dev/null 2>&1; then
    printf 'refusing to overwrite existing build volume: %s\n' "$work_volume" >&2
    exit 1
fi

"$engine" volume create "$work_volume" >/dev/null
"$engine" run --rm --arch amd64 \
    -v "$source_volume:/source:ro" \
    -v "$work_volume:/work" \
    -v "$repo_root:/repo:ro" \
    --user 0:0 \
    "$builder" sh -lc '
        set -eu
        git clone --no-hardlinks /source/sdk /work/sdk
        git -C /work/sdk checkout --detach d4003f15b35d43ad4842f427050ab2bba0114fa5
        /repo/image/verify-sdk-snapshot.sh /work/sdk
        /repo/image/replace-ddr-compat.sh /work/sdk
        /repo/image/apply-phase1-config.sh /work/sdk \
            /repo/out/phase2-rust/cleankvmd \
            /repo/out/phase2-rust/cleankvm-memtest
        /repo/image/validate-phase1-config.sh /work/sdk
        chown -R 1000:1000 /work/sdk
    '

"$engine" run --rm --arch amd64 \
    -v "$work_volume:/work:ro" \
    "$builder" sh -lc '
        set -eu
        test -f /work/sdk/cleankvm/phase1-overlay/etc/network/interfaces
        test -x /work/sdk/cleankvm/phase1-overlay/usr/bin/cleankvmd
        test -x /work/sdk/cleankvm/phase1-overlay/usr/bin/cleankvm-memtest
        grep -qF "# CleanKVM Phase 1 generated configuration" \
            /work/sdk/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig
        grep -qF "# CleanKVM Phase 1 generated configuration" \
            /work/sdk/buildroot/configs/cvitek_SG200X_musl_riscv64_defconfig
        grep -qF "# CleanKVM Phase 1 generated configuration" \
            /work/sdk/build/boards/sg200x/sg2002_licheervnano_sd/u-boot/sg2002_licheervnano_sd_defconfig
        test "$(wc -c </work/sdk/fsbl/test/cv181x/ddr_param.bin)" -eq 512
        test "$(sha256sum /work/sdk/fsbl/test/cv181x/ddr_param.bin | cut -d " " -f 1)" = \
            076a27c79e5ace2a3d47f9dd2e83e4ff6ea8872b3c2218f66c92b89b55f36560
    '

printf 'prepared disposable build volume: %s\n' "$work_volume"
