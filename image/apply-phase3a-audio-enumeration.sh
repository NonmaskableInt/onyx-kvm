#!/bin/sh
set -eu

sdk_dir=${1:?usage: apply-phase3a-audio-enumeration.sh SDK_DIRECTORY}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
kernel_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig"
marker='# Onyx KVM Phase 3a audio-enumeration generated configuration'
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

test -f "$kernel_defconfig"
grep -qF '# Onyx KVM Phase 2 HID generated configuration' "$kernel_defconfig"

mkdir -p "$temp_dir/kernel"
KCONFIG_CONFIG="$temp_dir/kernel/.config" \
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m \
    -O "$temp_dir/kernel" "$kernel_defconfig" \
    "$repo_root/image/configs/phase1-kernel.fragment" \
    "$repo_root/image/configs/phase2-hid-kernel.fragment" \
    "$repo_root/image/configs/phase3a-audio-enumeration-kernel.fragment"
cp "$temp_dir/kernel/.config" "$kernel_defconfig"
grep -qF "$marker" "$kernel_defconfig" || printf '\n%s\n' "$marker" >>"$kernel_defconfig"

printf '%s\n' 'Phase-3a audio-enumeration profile applied'
