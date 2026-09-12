#!/bin/sh
set -eu

sdk_dir=${1:?usage: validate-phase2-hid-config.sh SDK_DIRECTORY}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM
kernel_out="$temp_dir/kernel"
mkdir -p "$kernel_out"

"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m -O "$kernel_out" \
    "$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig" \
    "$repo_root/image/configs/phase2-hid-kernel.fragment"
make -s -C "$sdk_dir/linux_5.10" O="$kernel_out" ARCH=riscv olddefconfig

assert_enabled() {
    grep -qx "$1=y" "$kernel_out/.config" || {
        actual=$(grep -E "^($1=|# $1 is not set)" "$kernel_out/.config" || true)
        printf 'HID kernel mismatch: expected %s=y; found %s\n' \
            "$1" "${actual:-<symbol absent>}" >&2
        exit 1
    }
}

assert_disabled() {
    if grep -Eq "^$1=[ym]$" "$kernel_out/.config"; then
        actual=$(grep -E "^$1=" "$kernel_out/.config")
        printf 'HID kernel mismatch: expected %s disabled; found %s\n' "$1" "$actual" >&2
        exit 1
    fi
}

for key in \
    CONFIG_USB_SUPPORT CONFIG_USB_GADGET CONFIG_USB_DWC2 \
    CONFIG_USB_DWC2_PERIPHERAL CONFIG_CONFIGFS_FS CONFIG_USB_LIBCOMPOSITE \
    CONFIG_USB_CONFIGFS CONFIG_USB_CONFIGFS_F_HID CONFIG_USB_F_HID
do
    assert_enabled "$key"
done

for key in \
    CONFIG_USB CONFIG_USB_DWC2_HOST CONFIG_USB_DWC2_DUAL_ROLE \
    CONFIG_INPUT CONFIG_HID CONFIG_USB_CONFIGFS_SERIAL CONFIG_USB_CONFIGFS_ACM \
    CONFIG_USB_CONFIGFS_NCM CONFIG_USB_CONFIGFS_ECM CONFIG_USB_CONFIGFS_RNDIS \
    CONFIG_USB_CONFIGFS_EEM CONFIG_USB_CONFIGFS_MASS_STORAGE \
    CONFIG_USB_CONFIGFS_F_FS CONFIG_USB_CONFIGFS_F_UAC1 \
    CONFIG_USB_CONFIGFS_F_UAC2 CONFIG_USB_CONFIGFS_F_MIDI \
    CONFIG_USB_CONFIGFS_F_UVC
do
    assert_disabled "$key"
done

printf '%s\n' 'normalized Phase-2 device-only HID configuration passed'
