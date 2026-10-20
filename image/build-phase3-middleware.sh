#!/bin/sh
set -eu

# Build the two vendor middleware libraries that the raw-frame probe links
# (middleware/v2 libsys.a, libvpu.a and the vpu objects) inside the work
# volume, offline. Run it after build-phase3-video-modules.sh, which builds the
# in-tree kernel and its headers, and before build-phase3-raw-frame-probe.sh.
#
#   ONYX_KVM_WORK_VOLUME=<volume> image/build-phase3-middleware.sh
#
# The middleware sources have no license grant (OPEN_ITEMS B1).

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo_root/image/default-env.sh"

"$repo_root/image/verify-sources.sh"

"$CONTAINER_ENGINE" run --rm --arch amd64 --network none \
    -v "$ONYX_KVM_WORK_VOLUME:/work" \
    -v "$ONYX_KVM_HOST_TOOLS_VOLUME:/host-tools:ro" \
    "$ONYX_KVM_BUILDER_TAG" bash -c '
        set -eu
        export PATH=/host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin:$PATH
        sdk=/work/sdk
        v2=$sdk/osdrv/interdrv/v2
        cd "$sdk"
        set +u
        source build/cvisetup.sh >/dev/null 2>&1
        defconfig sg2002_licheervnano_sd >/dev/null 2>&1
        set -u
        export KERNEL_OUTPUT_FOLDER
        kernel=$sdk/linux_5.10/$KERNEL_OUTPUT_FOLDER
        test -d "$kernel/riscv/usr/include/linux" || {
            echo "kernel headers missing; run build-phase3-video-modules.sh first" >&2
            exit 1
        }
        # The SDK installs its uapi headers next to the kernel headers; the
        # middleware includes them as <linux/cvi_*.h>.
        headers=$kernel/riscv/usr/include/linux
        cp -r "$v2/include/chip/mars/uapi/linux/." "$headers/"
        cp -r "$v2/include/common/uapi/linux/." "$headers/"
        ksrc=$sdk/linux_5.10
        cp "$ksrc/drivers/staging/android/uapi/ion.h" \
           "$ksrc/drivers/staging/android/uapi/ion_cvitek.h" \
           "$ksrc/include/uapi/linux/dma-buf.h" "$headers/"
        cd middleware/v2
        rm -f lib/libsys.a lib/libvpu.a
        mkdir -p lib include/isp
        # Each module Makefile reads $PWD, so build from inside its directory.
        # Only the static libraries are needed; the shared-library link step of
        # the default target fails in this toolchain (no libc to link against).
        for module in sys vpu; do
            ( cd "modules/$module" && make KERNEL_DIR="$kernel"                   "$sdk/middleware/v2/lib/lib$module.a" )
        done
        test "$(riscv64-unknown-linux-musl-ar t lib/libsys.a | wc -l)" -gt 5
        test "$(riscv64-unknown-linux-musl-ar t lib/libvpu.a | wc -l)" -gt 5
        sha256sum lib/libsys.a lib/libvpu.a
    '
