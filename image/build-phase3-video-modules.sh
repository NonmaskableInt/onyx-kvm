#!/bin/sh
set -eu

# Build the nine quarantined video kernel modules and the VPU firmware from
# pinned sources, offline, and write them with SHA256SUMS and BUILD-INFO.txt.
#
#   image/build-phase3-video-modules.sh OUTPUT_DIR
#
# Inputs
#   ONYX_KVM_WORK_VOLUME   a disposable volume prepared by prepare-build-volume.sh
#                          and apply-phase3-video-foundation.sh (patched SDK).
#                          It is modified: the kernel is built under
#                          linux_5.10/build/ and three osdrv directories are
#                          replaced.
#   ONYX_KVM_OSDRV_DIR     checkout of sophgo/osdrv at the revision pinned in
#                          sources.lock (default dl/sophgo-osdrv-sparse);
#                          patch 0028 must be applied (apply-osdrv-patch.sh).
#   ONYX_KVM_REFERENCE_MODULES
#                          optional directory of previously shipped modules to
#                          compare against.
#
# Provenance (verified 2026-10-19 against out/phase3-video-modules-v51):
#   soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss
#       built from the pinned SDK (osdrv/interdrv/v2) with patches 0002-0027;
#   mars_vcodec soph_jpeg cvi_vc_driver
#       built from sophgo/osdrv (interdrv/{vcodec,jpeg,cvi_vc_drv}); mars_vcodec
#       additionally takes vcodec/hal/mars from the SDK because the osdrv tree
#       has no mars HAL for it. The osdrv tree is a different generation from
#       the SDK layout, so it replaces the SDK directories of the same name.
#   coda980.bin
#       copied unmodified from the SDK ramdisk (no source exists).
# Kernel build metadata is pinned (KBUILD_BUILD_*) so repeated builds match.

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo_root/image/default-env.sh"

output_dir=${1:?usage: build-phase3-video-modules.sh OUTPUT_DIR}
osdrv_dir=${ONYX_KVM_OSDRV_DIR:-"$repo_root/dl/sophgo-osdrv-sparse"}
reference_dir=${ONYX_KVM_REFERENCE_MODULES:-}
engine=$CONTAINER_ENGINE

"$repo_root/image/verify-sources.sh"
"$repo_root/image/apply-osdrv-patch.sh" "$osdrv_dir"

"$engine" volume inspect "$ONYX_KVM_WORK_VOLUME" >/dev/null 2>&1 || {
    printf 'missing work volume: %s (run image/prepare-build-volume.sh)\n' \
        "$ONYX_KVM_WORK_VOLUME" >&2
    exit 1
}
"$engine" volume inspect "$ONYX_KVM_HOST_TOOLS_VOLUME" >/dev/null 2>&1 || {
    printf 'missing host-tools volume: %s\n' "$ONYX_KVM_HOST_TOOLS_VOLUME" >&2
    exit 1
}
test ! -e "$output_dir" || {
    printf 'refusing to overwrite %s\n' "$output_dir" >&2
    exit 1
}
mkdir -p "$output_dir"
output_dir=$(CDPATH= cd -- "$output_dir" && pwd)
osdrv_revision=$(git -C "$osdrv_dir" rev-parse HEAD)

reference_mount=
if test -n "$reference_dir"; then
    reference_mount="-v $reference_dir:/reference:ro"
fi

# shellcheck disable=SC2086
"$engine" run --rm --arch amd64 --network none \
    -v "$ONYX_KVM_WORK_VOLUME:/work" \
    -v "$ONYX_KVM_HOST_TOOLS_VOLUME:/host-tools:ro" \
    -v "$osdrv_dir:/osdrv:ro" \
    -v "$repo_root:/repo:ro" \
    -v "$output_dir:/output:rw" \
    $reference_mount \
    -e ONYX_KVM_OSDRV_REVISION="$osdrv_revision" \
    "$ONYX_KVM_BUILDER_TAG" bash -c '
        set -eu
        export PATH=/host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin:$PATH
        export KBUILD_BUILD_TIMESTAMP="Thu Jan  1 00:00:00 UTC 1970"
        export KBUILD_BUILD_USER=onyx-kvm KBUILD_BUILD_HOST=onyx-kvm KBUILD_BUILD_VERSION=1
        sdk=/work/sdk
        v2=$sdk/osdrv/interdrv/v2
        ko=$sdk/ko-out

        cd "$sdk"
        set +u
        source build/cvisetup.sh >/dev/null 2>&1
        defconfig sg2002_licheervnano_sd >/dev/null 2>&1
        set -u
        # Build the kernel out of tree, as the SDK does, so the source tree
        # stays clean for the rootfs build that follows in the same volume.
        export KERNEL_OUTPUT_FOLDER RAMDISK_OUTPUT_FOLDER SYSTEM_OUT_DIR
        kernel=$sdk/linux_5.10/$KERNEL_OUTPUT_FOLDER

        # 1. Kernel, needed for out-of-tree module builds.
        ( cd build && make kernel-build )

        # 2. Six modules from the pinned SDK.
        rm -rf "$ko"; mkdir -p "$ko/3rd"
        ( cd osdrv && make KERNEL_DIR="$kernel" INSTALL_DIR="$ko" \
              sys base cif snsr_i2c vi vpss )

        # 3. Three modules from the pinned sophgo-osdrv tree.
        for d in vcodec jpeg cvi_vc_drv; do
            if [ ! -d "$v2/$d.sdk-orig" ]; then mv "$v2/$d" "$v2/$d.sdk-orig"
            else rm -rf "$v2/$d"; fi
            cp -a "/osdrv/interdrv/$d" "$v2/$d"
        done
        rm -rf "$v2/vcodec/hal/mars"
        cp -a "$v2/vcodec.sdk-orig/hal/mars" "$v2/vcodec/hal/mars"
        for d in vcodec jpeg cvi_vc_drv; do
            ( cd "$v2/$d" && make CVIARCH=MARS CVIARCH_L=mars \
                  KERNEL_DIR="$kernel" all )
        done
        cp "$v2/vcodec/mars_vcodec.ko" "$v2/jpeg/soph_jpeg.ko" \
           "$v2/cvi_vc_drv/cvi_vc_driver.ko" "$ko/"

        # 4. Export exactly the nine modules plus the firmware.
        for m in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss \
                 soph_jpeg mars_vcodec cvi_vc_driver; do
            install -m 0644 "$ko/$m.ko" "/output/$m.ko"
        done
        install -m 0644 \
            "$sdk/ramdisk/rootfs/common_musl_riscv64/usr/share/fw_vcodec/coda980.bin" \
            /output/coda980.bin
        ( cd /output && sha256sum *.ko coda980.bin > SHA256SUMS )
        {
            printf "sdk_commit=%s\n" "$(git -C "$sdk" rev-parse HEAD)"
            printf "osdrv_commit=%s\n" "$ONYX_KVM_OSDRV_REVISION"
            printf "kbuild_timestamp=%s\n" "$KBUILD_BUILD_TIMESTAMP"
            printf "patches_sha256=\n"
            ( cd /repo/image/patches && sha256sum *.patch ) | sed "s/^/  /"
        } > /output/BUILD-INFO.txt

        # 5. Optional comparison with previously shipped modules. The kernel
        # build stamp embedded in some modules is ignored.
        if [ -d /reference ]; then
            for f in /output/*.ko; do
                n=$(basename "$f")
                if [ ! -f "/reference/$n" ]; then echo "compare $n: no reference"; continue; fi
                if cmp -s "$f" "/reference/$n"; then echo "compare $n: identical"
                elif diff <(strings -a "$f" | grep -v "^#[0-9]* PREEMPT") \
                          <(strings -a "/reference/$n" | grep -v "^#[0-9]* PREEMPT") \
                          >/dev/null; then echo "compare $n: identical except build stamp"
                else echo "compare $n: DIFFERENT"; fi
            done
        fi
    '

printf 'wrote %s\n' "$output_dir"
