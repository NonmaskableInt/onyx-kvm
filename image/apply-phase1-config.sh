#!/bin/sh
set -eu

sdk_dir=${1:?usage: apply-phase1-config.sh SDK_DIRECTORY DAEMON_BINARY MEMTEST_BINARY}
daemon_binary=${2:?usage: apply-phase1-config.sh SDK_DIRECTORY DAEMON_BINARY MEMTEST_BINARY}
memtest_binary=${3:?usage: apply-phase1-config.sh SDK_DIRECTORY DAEMON_BINARY MEMTEST_BINARY}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
marker='# Onyx KVM Phase 1 generated configuration'

kernel_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig"
buildroot_defconfig="$sdk_dir/buildroot/configs/cvitek_SG200X_musl_riscv64_defconfig"
board_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/sg2002_licheervnano_sd_defconfig"
uboot_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/u-boot/sg2002_licheervnano_sd_defconfig"
overlay_dir="$sdk_dir/onyx-kvm/phase1-overlay"
busybox_fragment="$sdk_dir/onyx-kvm/phase1-busybox.fragment"
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

for path in "$kernel_defconfig" "$buildroot_defconfig" "$board_defconfig" "$uboot_defconfig"; do
    test -f "$path" || {
        printf 'missing SDK configuration: %s\n' "$path" >&2
        exit 1
    }
    if grep -qF "$marker" "$path"; then
        printf 'Phase-1 configuration already applied: %s\n' "$path" >&2
        exit 1
    fi
done

mkdir -p "$temp_dir/kernel" "$temp_dir/buildroot" "$temp_dir/uboot"
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m \
    -O "$temp_dir/kernel" "$kernel_defconfig" \
    "$repo_root/image/configs/phase1-kernel.fragment"
cp "$temp_dir/kernel/.config" "$kernel_defconfig"
printf '\n%s\n' "$marker" >>"$kernel_defconfig"

{
    cat "$repo_root/image/configs/phase1-buildroot.fragment"
    printf 'BR2_ROOTFS_OVERLAY="%s"\n' "$overlay_dir"
    printf 'BR2_PACKAGE_BUSYBOX_CONFIG_FRAGMENT_FILES="%s"\n' "$busybox_fragment"
    printf 'BR2_ROOTFS_POST_BUILD_SCRIPT="%s"\n' "$sdk_dir/onyx-kvm/lock-root-password.sh"
    printf 'BR2_ROOTFS_USERS_TABLES="%s"\n' "$sdk_dir/onyx-kvm/phase1-users.txt"
    printf 'BR2_ROOTFS_DEVICE_TABLE="%s"\n' "$sdk_dir/onyx-kvm/phase1-devices.txt"
} >"$temp_dir/buildroot.fragment"
# Unlike the kernel profile, the Buildroot profile is intentionally created
# from an empty baseline. The vendor defconfig enables hundreds of unrelated
# packages and is not a safe base for subtraction.
: >"$temp_dir/buildroot.empty"
"$sdk_dir/buildroot/support/kconfig/merge_config.sh" -m \
    -O "$temp_dir/buildroot" "$temp_dir/buildroot.empty" \
    "$temp_dir/buildroot.fragment"
cp "$temp_dir/buildroot/.config" "$buildroot_defconfig"
printf '\n%s\n' "$marker" >>"$buildroot_defconfig"

"$sdk_dir/u-boot-2021.10/scripts/kconfig/merge_config.sh" -m \
    -O "$temp_dir/uboot" "$uboot_defconfig" \
    "$repo_root/image/configs/phase1-uboot.fragment"
cp "$temp_dir/uboot/.config" "$uboot_defconfig"
printf '\n%s\n' "$marker" >>"$uboot_defconfig"

for symbol in \
    CONFIG_CP_EXT_WIRELESS \
    CONFIG_SENSOR_GCORE_GC4653 \
    CONFIG_SENSOR_OV_OV2685 \
    CONFIG_SENSOR_OV_OS04A10 \
    CONFIG_SENSOR_SMS_SC035GS \
    CONFIG_MIPI_PANEL_ZCT2133V1
do
    sed -i "/^${symbol}=/d; /^# ${symbol} is not set$/d" "$board_defconfig"
    printf '# %s is not set\n' "$symbol" >>"$board_defconfig"
done
printf '%s\n' "$marker" >>"$board_defconfig"

mkdir -p "$overlay_dir"
cp -a "$repo_root/image/overlay/." "$overlay_dir/"
cp "$repo_root/image/configs/phase1-busybox.fragment" "$busybox_fragment"
cp "$repo_root/image/lock-root-password.sh" "$sdk_dir/onyx-kvm/lock-root-password.sh"
cp "$repo_root/image/configs/phase1-users.txt" "$sdk_dir/onyx-kvm/phase1-users.txt"
cp "$repo_root/image/configs/phase1-devices.txt" "$sdk_dir/onyx-kvm/phase1-devices.txt"
chmod 0755 "$sdk_dir/onyx-kvm/lock-root-password.sh"
test -f "$daemon_binary" || {
    printf 'missing static Rust daemon: %s\n' "$daemon_binary" >&2
    exit 1
}
case "$(file "$daemon_binary")" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'invalid target daemon: %s\n' "$(file "$daemon_binary")" >&2
        exit 1
        ;;
esac
install -D -m 0755 "$daemon_binary" "$overlay_dir/usr/bin/onyx-kvmd"
test -f "$memtest_binary" || {
    printf 'missing static Rust memory tester: %s\n' "$memtest_binary" >&2
    exit 1
}
case "$(file "$memtest_binary")" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'invalid target memory tester: %s\n' "$(file "$memtest_binary")" >&2
        exit 1
        ;;
esac
install -D -m 0755 "$memtest_binary" "$overlay_dir/usr/bin/onyx-kvm-memtest"

for patch_file in "$repo_root"/image/patches/*.patch; do
    case "${patch_file##*/}" in
        0001-dwc2-cvitek-honor-dr-mode.patch) continue ;;
        # Targets the separate sophgo-osdrv source tree (interdrv/cvi_vc_drv),
        # not the pinned SDK checkout; apply it with image/apply-osdrv-patch.sh.
        0028-cvi-vc-driver-hardware-h264.patch) continue ;;
    esac
    patch -d "$sdk_dir" -p1 <"$patch_file"
done

printf '%s\n' 'Phase-1 fragments and wired-DHCP overlay applied'
