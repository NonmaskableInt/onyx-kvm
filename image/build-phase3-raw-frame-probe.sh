#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo_root/image/default-env.sh"

engine=$CONTAINER_ENGINE
builder=$ONYX_KVM_BUILDER_TAG
work_volume=$ONYX_KVM_WORK_VOLUME
host_tools_volume=$ONYX_KVM_HOST_TOOLS_VOLUME
output_dir=${ONYX_KVM_RAW_FRAME_OUTPUT_DIR:-$repo_root/out/phase3-raw-frame-probe-v1}

mkdir -p "$output_dir"
test ! -e "$output_dir/onyx-kvm-raw-frame-probe"

"$engine" run --rm --arch amd64 --network none \
    -v "$work_volume:/work:rw" \
    -v "$host_tools_volume:/host-tools:ro" \
    -v "$repo_root:/repo:ro" \
    -v "$output_dir:/output:rw" \
    -w /work/sdk \
    "$builder" bash -lc '
        set -e
        export PATH="/host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin:$PATH"
        source build/cvisetup.sh
        defconfig sg2002_licheervnano_sd >/dev/null
        test "$(riscv64-unknown-linux-musl-ar t middleware/v2/lib/libsys.a | wc -l)" -gt 5
        test "$(riscv64-unknown-linux-musl-ar t middleware/v2/lib/libvpu.a | wc -l)" -gt 5
        riscv64-unknown-linux-musl-gcc -Os -fPIE -pie \
            -fstack-protector-strong -D_FORTIFY_SOURCE=2 \
            -Wall -Wextra -Werror \
            -I middleware/v2/include \
            -I osdrv/interdrv/v2/include/common/uapi \
            -I osdrv/interdrv/v2/include/chip/mars/uapi \
            -I osdrv/interdrv/v2/include/common \
            -I osdrv/interdrv/v2/include/chip/mars \
            /repo/image/phase3-raw-frame-probe.c \
            middleware/v2/modules/vpu/src/cvi_vi.o \
            middleware/v2/modules/vpu/src/vi_ioctl.o \
            middleware/v2/modules/vpu/src/cvi_vpss.o \
            middleware/v2/modules/vpu/src/vpss_ioctl.o \
            -Wl,--gc-sections \
            -Wl,--start-group middleware/v2/lib/libsys.a -Wl,--end-group \
            -lpthread -latomic -Wl,-Map,/output/LINK-MAP.txt \
            -o /output/onyx-kvm-raw-frame-probe
        riscv64-unknown-linux-musl-strip /output/onyx-kvm-raw-frame-probe
        riscv64-unknown-linux-musl-readelf -d /output/onyx-kvm-raw-frame-probe \
            > /output/DYNAMIC.txt
        file /output/onyx-kvm-raw-frame-probe
    '

sha256sum "$output_dir/onyx-kvm-raw-frame-probe" >"$output_dir/SHA256SUMS"
printf 'built offline raw-frame probe: %s\n' "$output_dir/onyx-kvm-raw-frame-probe"
