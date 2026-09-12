#!/bin/sh
set -eu

sdk_dir=${1:?usage: apply-phase3-video-foundation.sh SDK_DIRECTORY}
module_dir=${2:-}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
kernel_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig"
marker='# CleanKVM Phase 3 video-foundation generated configuration'
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

test -f "$kernel_defconfig"
grep -qF '# CleanKVM Phase 2 HID generated configuration' "$kernel_defconfig"

mkdir -p "$temp_dir/kernel"
KCONFIG_CONFIG="$temp_dir/kernel/.config" \
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m \
    -O "$temp_dir/kernel" "$kernel_defconfig" \
    "$repo_root/image/configs/phase3-video-foundation-kernel.fragment"
cp "$temp_dir/kernel/.config" "$kernel_defconfig"
grep -qF "$marker" "$kernel_defconfig" || printf '\n%s\n' "$marker" >>"$kernel_defconfig"

if test -n "$module_dir"; then
    test -f "$module_dir/SHA256SUMS"
    module_target="$sdk_dir/cleankvm/phase1-overlay/usr/lib/cleankvm/video-quarantine"
    mkdir -p "$module_target"
    set -- "$module_dir"/*.ko
    test "$#" -eq 6
    for name in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss; do
        test -f "$module_dir/$name.ko"
        install -m 0444 "$module_dir/$name.ko" "$module_target/$name.ko"
    done
    (cd "$module_dir" && sha256sum -c SHA256SUMS)
    cp "$repo_root/image/lock-root-password.sh" "$sdk_dir/cleankvm/lock-root-password.sh"
    chmod 0755 "$sdk_dir/cleankvm/lock-root-password.sh"
fi

printf '%s\n' 'Phase-3 video-foundation profile applied'
