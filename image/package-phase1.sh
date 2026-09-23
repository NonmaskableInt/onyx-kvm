#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v5}
artifact_dir=${CLEANKVM_ARTIFACT_DIR:-"$repo_root/out/phase1"}

mkdir -p "$artifact_dir"

"$engine" run --rm --arch amd64 --network none \
    -v "$work_volume:/work:ro" \
    -v "$repo_root:/repo:ro" \
    -v "$artifact_dir:/artifacts:rw" \
    "$builder" bash -lc '
        set -eu
        sdk=/work/sdk
        stage=/tmp/cleankvm-package
        kernel="$sdk/linux_5.10/build/sg2002_licheervnano_sd/arch/riscv/boot/Image"
        kernel_config="$sdk/linux_5.10/build/sg2002_licheervnano_sd/.config"
        dtb="$sdk/linux_5.10/build/sg2002_licheervnano_sd/arch/riscv/boot/dts/cvitek/sg2002_licheervnano_sd.dtb"
        rootfs="$sdk/buildroot/output/images/rootfs.ext4"
        fip="$sdk/install/soc_sg2002_licheervnano_sd/fip.bin"
        mkimage=$(find "$sdk/u-boot-2021.10" "$sdk/host-tools" -type f -name mkimage -perm -111 -print -quit)

        test -n "$mkimage"
        test -f "$kernel"
        test -f "$kernel_config"
        test -f "$dtb"
        test -f "$rootfs"
        test -f "$fip"

        for symbol in \
            CONFIG_ARCH_CVITEK \
            CONFIG_SOC_THEAD \
            CONFIG_ARCH_CV181X \
            CONFIG_ARCH_CV181X_ASIC \
            CONFIG_CVITEK_PINCTRL_CV1835 \
            CONFIG_COMMON_CLK_CVITEK
        do
            grep -qx "$symbol=y" "$kernel_config"
        done

        rm -rf "$stage"
        mkdir -p "$stage"
        cp "$kernel" "$stage/Image"
        cp "$dtb" "$stage/sg2002_licheervnano_sd.dtb"
        cp /repo/image/phase1.its "$stage/phase1.its"
        "$mkimage" -f "$stage/phase1.its" "$stage/boot.sd"

        ! fdtget -l "$stage/boot.sd" /images | grep -q ramdisk
        ! fdtget -p "$stage/boot.sd" /configurations/config-sg2002_licheervnano_sd | grep -q ramdisk
        test "$(fdtget -t s "$stage/boot.sd" /images/kernel-1/hash-1 algo)" = sha256
        test "$(fdtget -t s "$stage/boot.sd" /images/fdt-sg2002_licheervnano_sd/hash-1 algo)" = sha256

        cp "$stage/boot.sd" /artifacts/boot.sd
        cp -L "$rootfs" /artifacts/rootfs.ext4
        cp "$fip" /artifacts/fip.bin
        cp /repo/image/phase1.its /artifacts/phase1.its
        cd /artifacts
        sha256sum boot.sd rootfs.ext4 fip.bin phase1.its > SHA256SUMS
        sha256sum -c SHA256SUMS
    '

printf 'verified Phase-1 partition artifacts: %s\n' "$artifact_dir"
