#!/bin/sh
set -eu

sdk_dir=${1:?usage: apply-phase2-hid.sh SDK_DIRECTORY HID_BROKER_BINARY}
broker_binary=${2:?usage: apply-phase2-hid.sh SDK_DIRECTORY HID_BROKER_BINARY}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
kernel_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig"
board_dir="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/dts_riscv"
board_dts="$board_dir/sg2002_licheervnano_sd.dts"
dwc2_source="$sdk_dir/linux_5.10/drivers/usb/dwc2/platform.c"
overlay_dir="$sdk_dir/onyx-kvm/phase1-overlay"
marker='# Onyx KVM Phase 2 HID generated configuration'
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

test -f "$kernel_defconfig"
test -f "$board_dts"
test -f "$dwc2_source"
test -d "$overlay_dir"
test -f "$broker_binary"
! grep -qF "$marker" "$kernel_defconfig"
! grep -qF 'phase2-hid-board.dtsi' "$board_dts"
! grep -qF "CVITEK's private PHY override defaults to host mode" "$dwc2_source"

mkdir -p "$temp_dir/kernel"
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m \
    -O "$temp_dir/kernel" "$kernel_defconfig" \
    "$repo_root/image/configs/phase2-hid-kernel.fragment"
cp "$temp_dir/kernel/.config" "$kernel_defconfig"
printf '\n%s\n' "$marker" >>"$kernel_defconfig"

cp "$repo_root/image/configs/phase2-hid-board.dtsi" "$board_dir/phase2-hid-board.dtsi"
printf '\n#include "phase2-hid-board.dtsi"\n' >>"$board_dts"
cp -a "$repo_root/image/phase2-hid-overlay/." "$overlay_dir/"
install -m 0755 "$broker_binary" "$overlay_dir/usr/bin/onyx-kvm-hid-broker"
patch -d "$sdk_dir/linux_5.10" -p1 < \
    "$repo_root/image/patches/0001-dwc2-cvitek-honor-dr-mode.patch"

printf '%s\n' 'Phase-2 fail-closed HID probe applied'
