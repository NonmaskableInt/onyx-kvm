#!/bin/sh
set -eu

# Build the SG2002 boot chain inside the work volume, offline: U-Boot (and its
# mkimage), the FSBL/OpenSBI/U-Boot firmware package (fip.bin), and the device
# tree binary. package-phase1.sh consumes all three, so run this after the
# rootfs build and before packaging.
#
#   ONYX_KVM_WORK_VOLUME=<volume> image/build-phase1-bootchain.sh
#
# The FSBL sources have no license grant (OPEN_ITEMS B1); development builds
# need ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1.

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
        cd "$sdk"
        set +u
        source build/cvisetup.sh >/dev/null 2>&1
        defconfig sg2002_licheervnano_sd >/dev/null 2>&1
        set -u
        # Same out-of-tree layout as the kernel build, or the source tree is
        # left dirty and the next rootfs build refuses it.
        export KERNEL_OUTPUT_FOLDER RAMDISK_OUTPUT_FOLDER SYSTEM_OUT_DIR

        rm -f "install/soc_sg2002_licheervnano_sd/fip.bin"
        set +u
        build_uboot
        build_fsbl
        set -u
        # kernel-dts ends with a copy into the ramdisk workspace that fails
        # when the ramdisk is not staged. The dtb is already built by then, so
        # the outputs below are the check, not the exit status.
        ( cd build && make kernel-dts ) || true

        fip=$sdk/install/soc_sg2002_licheervnano_sd/fip.bin
        dtb=$sdk/linux_5.10/$KERNEL_OUTPUT_FOLDER/arch/riscv/boot/dts/cvitek/sg2002_licheervnano_sd.dtb
        mkimage=$(find "$sdk/u-boot-2021.10" "$sdk/host-tools" -type f -name mkimage -perm -111 -print -quit)
        test -s "$fip" || { echo "fip.bin was not produced" >&2; exit 1; }
        test -s "$dtb" || { echo "dtb was not produced" >&2; exit 1; }
        test -n "$mkimage" || { echo "mkimage was not produced" >&2; exit 1; }
        sha256sum "$fip" "$dtb"
    '
