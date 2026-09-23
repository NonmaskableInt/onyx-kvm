#!/bin/sh
set -eu

sdk_dir=${1:?usage: validate-phase1-config.sh SDK_DIRECTORY}
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM
kernel_out="$temp_dir/kernel"
buildroot_out="$temp_dir/buildroot"
uboot_out="$temp_dir/uboot"

mkdir -p "$kernel_out" "$buildroot_out" "$uboot_out"
cp "$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig" \
    "$kernel_out/.config"
touch "$kernel_out/.config"
make -s -C "$sdk_dir/linux_5.10" O="$kernel_out" ARCH=riscv olddefconfig
make -s -C "$sdk_dir/buildroot" O="$buildroot_out" \
    cvitek_SG200X_musl_riscv64_defconfig
cp "$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/u-boot/sg2002_licheervnano_sd_defconfig" \
    "$uboot_out/.config"
# The vendor U-Boot tree has a broad %.config make rule. Ensure the copied
# input is newer than its Kconfig files so Make does not mistake .config for a
# fragment target after a prior SDK clean.
touch "$uboot_out/.config"
make -s -C "$sdk_dir/u-boot-2021.10" O="$uboot_out" ARCH=riscv olddefconfig

assert_kernel() {
    key=$1
    value=$2
    if test "$value" = y; then
        expected="$key=y"
        if grep -qx "$expected" "$kernel_out/.config"; then
            return 0
        fi
        actual=$(grep -E "^($key=|# $key is not set)" "$kernel_out/.config" || true)
        printf 'kernel configuration mismatch: expected %s; found %s\n' \
            "$expected" "${actual:-<symbol absent>}" >&2
        return 1
    else
        # Kconfig may omit a symbol when a disabled parent makes it
        # unreachable. Both omission and an explicit "not set" mean n.
        if grep -Eq "^$key=[ym]$" "$kernel_out/.config"; then
            actual=$(grep -E "^$key=" "$kernel_out/.config")
            printf 'kernel configuration mismatch: expected %s disabled; found %s\n' \
                "$key" "$actual" >&2
            return 1
        fi
    fi
}

assert_buildroot_disabled() {
    key=$1
    expected="# $key is not set"
    if ! grep -qx "$expected" "$buildroot_out/.config"; then
        actual=$(grep -E "^($key=|# $key is not set)" "$buildroot_out/.config" || true)
        printf 'Buildroot configuration mismatch: expected %s; found %s\n' \
            "$expected" "${actual:-<symbol absent>}" >&2
        return 1
    fi
}

assert_uboot() {
    key=$1
    value=$2
    if test "$value" = y; then
        grep -qx "$key=y" "$uboot_out/.config" || {
            actual=$(grep -E "^($key=|# $key is not set)" "$uboot_out/.config" || true)
            printf 'U-Boot configuration mismatch: expected %s=y; found %s\n' \
                "$key" "${actual:-<symbol absent>}" >&2
            return 1
        }
    elif grep -Eq "^$key=[ym]$" "$uboot_out/.config"; then
        actual=$(grep -E "^$key=" "$uboot_out/.config")
        printf 'U-Boot configuration mismatch: expected %s disabled; found %s\n' \
            "$key" "$actual" >&2
        return 1
    fi
}

for setting in \
    CONFIG_ARCH_CVITEK=y \
    CONFIG_SOC_THEAD=y \
    CONFIG_ARCH_CV181X=y \
    CONFIG_ARCH_CV181X_ASIC=y \
    CONFIG_CVITEK_PINCTRL_CV1835=y \
    CONFIG_COMMON_CLK_CVITEK=y \
    CONFIG_MODULES=n \
    CONFIG_CFG80211=n \
    CONFIG_BT=n \
    CONFIG_SOUND=n \
    CONFIG_MEDIA_SUPPORT=n \
    CONFIG_FB=n \
    CONFIG_BACKLIGHT_CLASS_DEVICE=n \
    CONFIG_VT=n \
    CONFIG_INPUT=n \
    CONFIG_USB=n \
    CONFIG_USB_GADGET=n \
    CONFIG_USB_DWC2=n \
    CONFIG_USB_LIBCOMPOSITE=n \
    CONFIG_USB_CONFIGFS=n \
    CONFIG_HID=n \
    CONFIG_GPIO_SYSFS=n \
    CONFIG_I2C=n \
    CONFIG_I2C_CHARDEV=n \
    CONFIG_SPI=n \
    CONFIG_SPI_SPIDEV=n \
    CONFIG_STAGING=n \
    CONFIG_IPV6=n \
    CONFIG_NETFILTER=n \
    CONFIG_NFS_FS=n \
    CONFIG_WIREGUARD=n \
    CONFIG_TUN=n \
    CONFIG_VETH=n \
    CONFIG_VXLAN=n \
    CONFIG_MTD=n \
    CONFIG_SCSI=n \
    CONFIG_DEBUG_FS=n \
    CONFIG_MAGIC_SYSRQ=n \
    CONFIG_STMMAC_ETH=y \
    CONFIG_STRICT_KERNEL_RWX=y \
    CONFIG_STACKPROTECTOR_STRONG=y \
    CONFIG_SECURITY_DMESG_RESTRICT=y
do
    assert_kernel "${setting%=*}" "${setting#*=}"
done

for setting in \
    CONFIG_FIT=y \
    CONFIG_SHA256=y \
    CONFIG_CMD_BOOTM=y \
    CONFIG_CMD_MMC=y \
    CONFIG_CMD_FAT=y \
    CONFIG_MMC=y \
    CONFIG_ENV_IS_NOWHERE=y \
    CONFIG_DISTRO_DEFAULTS=n \
    CONFIG_NET=n \
    CONFIG_NET_RANDOM_ETHADDR=n \
    CONFIG_DM_ETH=n \
    CONFIG_ETH_DESIGNWARE=n \
    CONFIG_PHY_CVITEK=n \
    CONFIG_UDP_FUNCTION_FASTBOOT=n \
    CONFIG_FASTBOOT=n \
    CONFIG_FASTBOOT_FLASH=n \
    CONFIG_EFI_LOADER=n \
    CONFIG_CMD_BOOTEFI=n \
    CONFIG_CMD_CVI_UPDATE=n \
    CONFIG_FAT_WRITE=n \
    CONFIG_MMC_WRITE=n \
    CONFIG_LEGACY_IMAGE_FORMAT=n \
    CONFIG_CMD_IMPORTENV=n \
    CONFIG_CMD_EXPORTENV=n \
    CONFIG_CMD_SAVEENV=n \
    CONFIG_SAVEENV=n \
    CONFIG_DISPLAY=n \
    CONFIG_DM_VIDEO=n \
    CONFIG_BOOTLOGO=n \
    CONFIG_CMD_CVI_SD_UPDATE=n \
    CONFIG_CMD_MBR=n \
    CONFIG_MTD=n \
    CONFIG_SPI=n \
    CONFIG_LZ4=n \
    CONFIG_LZMA=n
do
    assert_uboot "${setting%=*}" "${setting#*=}"
done

grep -Eq 'CONFIG_BOOTCOMMAND[[:space:]]+"run sdboot"' \
    "$sdk_dir/u-boot-2021.10/include/configs/mars-asic.h"
if grep -E 'CONFIG_BOOTCOMMAND[[:space:]]+"run loadenvcmd' \
    "$sdk_dir/u-boot-2021.10/include/configs/mars-asic.h" >/dev/null; then
    printf '%s\n' 'U-Boot still imports unsigned uEnv.txt in its boot command' >&2
    exit 1
fi

grep -qx 'BR2_SYSTEM_DHCP="eth0"' "$buildroot_out/.config"
grep -qx 'BR2_TARGET_GENERIC_ROOT_PASSWD="\*"' "$buildroot_out/.config"
grep -qx 'BR2_ROOTFS_DEVICE_CREATION_DYNAMIC_DEVTMPFS=y' "$buildroot_out/.config"
for key in \
    BR2_PACKAGE_DROPBEAR \
    BR2_PACKAGE_OPENSSH \
    BR2_PACKAGE_AVAHI \
    BR2_PACKAGE_DNSMASQ \
    BR2_PACKAGE_WPA_SUPPLICANT \
    BR2_PACKAGE_ALSA_UTILS \
    BR2_PACKAGE_FFMPEG \
    BR2_PACKAGE_MOSH \
    BR2_PACKAGE_OPENCV4
do
    assert_buildroot_disabled "$key"
done

# Raw BR2_PACKAGE_* counts include architecture-capability symbols. Validate
# Buildroot's resolved package graph instead, including its required host tools.
make -s -C "$sdk_dir/buildroot" O="$buildroot_out" show-info \
    >"$temp_dir/packages.json"
python3 - "$temp_dir/packages.json" <<'PY'
import json
import sys

allowed = {
    "busybox", "host-acl", "host-attr", "host-autoconf", "host-automake",
    "host-e2fsprogs", "host-fakeroot", "host-libtool", "host-libzlib",
    "host-m4", "host-makedevs", "host-mkpasswd", "host-patchelf",
    "host-pkgconf", "host-skeleton", "host-util-linux", "host-zlib",
    "ifupdown-scripts", "initscripts", "musl-compat-headers",
    "rootfs-common", "rootfs-ext2", "rootfs-tar", "skeleton",
    "skeleton-init-common", "skeleton-init-sysv", "toolchain",
    "toolchain-external", "toolchain-external-custom", "urandom-scripts",
}
with open(sys.argv[1], encoding="utf-8") as stream:
    actual = set(json.load(stream))
if actual != allowed:
    print("Buildroot resolved package graph mismatch", file=sys.stderr)
    print("unexpected:", ", ".join(sorted(actual - allowed)) or "<none>", file=sys.stderr)
    print("missing:", ", ".join(sorted(allowed - actual)) or "<none>", file=sys.stderr)
    raise SystemExit(1)
PY

printf '%s\n' 'normalized Phase-1 configuration passed'
