#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo_root/image/default-env.sh"

engine=$CONTAINER_ENGINE
builder=$ONYX_KVM_BUILDER_TAG
work_volume=$ONYX_KVM_WORK_VOLUME
host_tools_volume=$ONYX_KVM_HOST_TOOLS_VOLUME
output_dir=${ONYX_KVM_EDID_PROGRAMMER_OUTPUT_DIR:-$repo_root/out/lt6911uxc-edid-programmer-v2}

mkdir -p "$output_dir"
test ! -e "$output_dir/onyx-kvm-lt6911uxc-edid-programmer"
"$engine" run --rm --arch amd64 --network none \
    -v "$work_volume:/work:rw" \
    -v "$host_tools_volume:/host-tools:ro" \
    -v "$repo_root:/repo:ro" \
    -v "$output_dir:/output:rw" -w /work/sdk "$builder" bash -lc '
        set -e
        gcc_bin=/host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin
        if [ ! -x "$gcc_bin/riscv64-unknown-linux-musl-gcc" ]; then
            gcc_bin=$(find /work/sdk/host-tools -type f -name riscv64-unknown-linux-musl-gcc -exec dirname {} + | head -n 1)
        fi
        test -x "$gcc_bin/riscv64-unknown-linux-musl-gcc"
        "$gcc_bin/riscv64-unknown-linux-musl-gcc" -Os -static -fPIE \
          -fstack-protector-strong -D_FORTIFY_SOURCE=2 \
          -Wall -Wextra -Werror /repo/image/lt6911uxc-edid-programmer.c \
          -o /output/onyx-kvm-lt6911uxc-edid-programmer
        "$gcc_bin/riscv64-unknown-linux-musl-strip" /output/onyx-kvm-lt6911uxc-edid-programmer
        file /output/onyx-kvm-lt6911uxc-edid-programmer
    '
sha256sum "$output_dir/onyx-kvm-lt6911uxc-edid-programmer" >"$output_dir/SHA256SUMS"
printf 'built offline bounded EDID programmer: %s\n' "$output_dir/onyx-kvm-lt6911uxc-edid-programmer"
