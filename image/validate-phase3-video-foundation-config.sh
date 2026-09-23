#!/bin/sh
set -eu

sdk_dir=${1:?usage: validate-phase3-video-foundation-config.sh SDK_DIRECTORY}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM
kernel_out="$temp_dir/kernel"
mkdir -p "$kernel_out"

KCONFIG_CONFIG="$kernel_out/.config" \
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m -O "$kernel_out" \
    "$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig" \
    "$repo_root/image/configs/phase2-hid-kernel.fragment" \
    "$repo_root/image/configs/phase3-video-foundation-kernel.fragment"
make -s -C "$sdk_dir/linux_5.10" O="$kernel_out" ARCH=riscv olddefconfig

assert_enabled() {
    grep -qx "$1=y" "$kernel_out/.config" || {
        actual=$(grep -E "^($1=|# $1 is not set)" "$kernel_out/.config" || true)
        printf 'Phase 3 video-foundation mismatch: expected %s=y; found %s\n' \
            "$1" "${actual:-<symbol absent>}" >&2
        exit 1
    }
}

assert_disabled() {
    if grep -Eq "^$1=[ym]$" "$kernel_out/.config"; then
        actual=$(grep -E "^$1=" "$kernel_out/.config")
        printf 'Phase 3 video-foundation mismatch: expected %s disabled; found %s\n' \
            "$1" "$actual" >&2
        exit 1
    fi
}

for key in \
    CONFIG_ARCH_CVITEK CONFIG_SOC_THEAD CONFIG_ARCH_CV181X CONFIG_ARCH_CV181X_ASIC \
    CONFIG_CVITEK_PINCTRL_CV1835 CONFIG_COMMON_CLK_CVITEK \
    CONFIG_MODULES CONFIG_MODULE_UNLOAD CONFIG_I2C CONFIG_I2C_CHARDEV CONFIG_I2C_DESIGNWARE_PLATFORM \
    CONFIG_STAGING CONFIG_ION CONFIG_ION_CVITEK
do
    assert_enabled "$key"
done

for key in \
    CONFIG_MODULE_FORCE_LOAD CONFIG_MODULE_FORCE_UNLOAD \
    CONFIG_I2C_SLAVE CONFIG_I2C_SLAVE_EEPROM CONFIG_I2C_DESIGNWARE_SLAVE \
    CONFIG_I2C_MUX CONFIG_I2C_GPIO CONFIG_MEDIA_SUPPORT CONFIG_SOUND CONFIG_SND \
    CONFIG_USB_CONFIGFS_F_UAC1 CONFIG_USB_CONFIGFS_F_UAC2 CONFIG_USB_CONFIGFS_F_UVC \
    CONFIG_DMABUF_HEAPS
do
    assert_disabled "$key"
done

if grep -Eq '=m$' "$kernel_out/.config"; then
    printf '%s\n' 'Phase 3 video-foundation mismatch: in-tree modules are not allowed' >&2
    grep -E '=m$' "$kernel_out/.config" >&2
    exit 1
fi

printf '%s\n' 'normalized Phase-3 video-foundation configuration passed'
