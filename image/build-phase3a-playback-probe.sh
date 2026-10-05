#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v50}
host_tools_volume=${CLEANKVM_HOST_TOOLS_VOLUME:-cleankvm-host-tools}
source_archive=${TINYALSA_SOURCE_ARCHIVE:-/private/tmp/tinyalsa-v2.0.0.tar.gz}
output_dir=${CLEANKVM_AUDIO_PLAYBACK_PROBE_OUTPUT_DIR:-$repo_root/out/phase3a-playback-probe-v1}
expected=573ae0b2d3480851c1d2a12503ead2beea27f92d44ed47b74b553ba947994ef1

test -f "$source_archive"
test "$(shasum -a 256 "$source_archive" | cut -d' ' -f1)" = "$expected"
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
        tool=/host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin/riscv64-unknown-linux-musl-
        make -C src CROSS_COMPILE="$tool" CFLAGS="-Os -g0 -ffunction-sections -fdata-sections"
        "${tool}gcc" -Os -g0 -static \
            -fstack-protector-strong -D_FORTIFY_SOURCE=2 \
            -Wall -Wextra -Werror -I include \
            /repo/image/phase3a-playback-probe.c src/libtinyalsa.a \
            -Wl,--gc-sections -ldl -o /output/cleankvm-audio-playback-probe
        "${tool}strip" /output/cleankvm-audio-playback-probe
        "${tool}readelf" -h /output/cleankvm-audio-playback-probe > /output/ELF.txt
        ! "${tool}nm" -u /output/cleankvm-audio-playback-probe | grep .
    '

(cd "$output_dir" && sha256sum cleankvm-audio-playback-probe ELF.txt > SHA256SUMS && sha256sum -c SHA256SUMS)
printf 'built offline bounded audio playback probe: %s\n' "$output_dir/cleankvm-audio-playback-probe"
