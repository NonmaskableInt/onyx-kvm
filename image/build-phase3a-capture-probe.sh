#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo_root/image/default-env.sh"

engine=$CONTAINER_ENGINE
builder=$ONYX_KVM_BUILDER_TAG
work_volume=$ONYX_KVM_WORK_VOLUME
host_tools_volume=$ONYX_KVM_HOST_TOOLS_VOLUME
source_archive=$TINYALSA_SOURCE_ARCHIVE
output_dir=${ONYX_KVM_AUDIO_PROBE_OUTPUT_DIR:-$repo_root/out/phase3a-capture-probe-v1}
expected=573ae0b2d3480851c1d2a12503ead2beea27f92d44ed47b74b553ba947994ef1

if [ ! -f "$source_archive" ]; then
    printf 'error: tinyalsa source archive not found at %s\n' "$source_archive" >&2
    printf 'run explicit source fetch or place tinyalsa-v2.0.0.tar.gz at %s\n' "$source_archive" >&2
    exit 1
fi

actual_sha=$(shasum -a 256 "$source_archive" | cut -d' ' -f1)
if [ "$actual_sha" != "$expected" ]; then
    printf 'error: tinyalsa checksum mismatch: expected %s, got %s\n' "$expected" "$actual_sha" >&2
    exit 1
fi
mkdir -p "$output_dir"

"$engine" run --rm --arch amd64 --network none \
    -v "$work_volume:/work:ro" \
    -v "$host_tools_volume:/host-tools:ro" \
    -v "$repo_root:/repo:ro" \
    -v "$source_archive:/tinyalsa.tar.gz:ro" \
    -v "$output_dir:/output:rw" \
    "$builder" bash -lc '
        set -e
        stage=$(mktemp -d)
        trap '\''rm -rf "$stage"'\'' EXIT
        tar -xzf /tinyalsa.tar.gz -C "$stage" --strip-components=1
        cd "$stage"
        if [ -x /host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin/riscv64-unknown-linux-musl-gcc ]; then
            tool=/host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin/riscv64-unknown-linux-musl-
        else
            tool=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64/bin/riscv64-unknown-linux-musl-
        fi
        make -C src CROSS_COMPILE="$tool" CFLAGS="-Os -g0 -ffunction-sections -fdata-sections"
        "${tool}gcc" -Os -g0 -static \
            -fstack-protector-strong -D_FORTIFY_SOURCE=2 \
            -Wall -Wextra -Werror -I include \
            /repo/image/phase3a-capture-probe.c src/libtinyalsa.a \
            -Wl,--gc-sections -ldl -o /output/onyx-kvm-audio-capture-probe
        "${tool}strip" /output/onyx-kvm-audio-capture-probe
        "${tool}readelf" -h /output/onyx-kvm-audio-capture-probe > /output/ELF.txt
        ! "${tool}nm" -u /output/onyx-kvm-audio-capture-probe | grep .
    '

(cd "$output_dir" && sha256sum onyx-kvm-audio-capture-probe ELF.txt > SHA256SUMS && sha256sum -c SHA256SUMS)
printf 'built offline bounded audio capture probe: %s\n' "$output_dir/onyx-kvm-audio-capture-probe"
