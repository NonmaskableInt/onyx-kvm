#!/bin/sh
set -eu

sdk_dir=${1:?usage: apply-phase3-video-foundation.sh SDK_DIRECTORY [MODULE_DIR] [VIDEO_BROKER_BINARY]}
module_dir=${2:-}
video_broker_binary=${3:-}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
kernel_defconfig="$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig"
marker='# Onyx KVM Phase 3 video-foundation generated configuration'
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

test -f "$kernel_defconfig"
grep -qF '# Onyx KVM Phase 2 HID generated configuration' "$kernel_defconfig"

mkdir -p "$temp_dir/kernel"
KCONFIG_CONFIG="$temp_dir/kernel/.config" \
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m \
    -O "$temp_dir/kernel" "$kernel_defconfig" \
    "$repo_root/image/configs/phase3-video-foundation-kernel.fragment"
cp "$temp_dir/kernel/.config" "$kernel_defconfig"
grep -qF "$marker" "$kernel_defconfig" || printf '\n%s\n' "$marker" >>"$kernel_defconfig"

if test -n "$module_dir"; then
    test -f "$module_dir/SHA256SUMS"
    module_target="$sdk_dir/onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine"
    mkdir -p "$module_target"
    set -- "$module_dir"/*.ko
    if test "$#" -eq 9; then
        for name in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss soph_jpeg mars_vcodec cvi_vc_driver; do
            test -f "$module_dir/$name.ko"
            install -m 0444 "$module_dir/$name.ko" "$module_target/$name.ko"
        done
    elif test "$#" -eq 6; then
        for name in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss; do
            test -f "$module_dir/$name.ko"
            install -m 0444 "$module_dir/$name.ko" "$module_target/$name.ko"
        done
    else
        printf 'unexpected number of video modules: %d (expected 6 or 9)\n' "$#" >&2
        exit 1
    fi
    if test -f "$module_dir/coda980.bin"; then
        fw_target="$sdk_dir/onyx-kvm/phase1-overlay/usr/share/fw_vcodec"
        mkdir -p "$fw_target"
        install -m 0444 "$module_dir/coda980.bin" "$fw_target/coda980.bin"
    fi
    (cd "$module_dir" && sha256sum -c SHA256SUMS)
    cp "$repo_root/image/lock-root-password.sh" "$sdk_dir/onyx-kvm/lock-root-password.sh"
    chmod 0755 "$sdk_dir/onyx-kvm/lock-root-password.sh"
fi

if test -n "$video_broker_binary"; then
    test -f "$video_broker_binary"
    overlay_dir="$sdk_dir/onyx-kvm/phase1-overlay"
    cp -a "$repo_root/image/phase3-video-overlay/." "$overlay_dir/"
    install -m 0755 "$video_broker_binary" "$overlay_dir/usr/bin/onyx-kvm-video-broker"
fi

printf '%s\n' 'Phase-3 video-foundation profile applied'
