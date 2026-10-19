#!/bin/sh
set -eu

sdk_dir=${1:?usage: validate-product-config.sh SDK_DIRECTORY}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM
kernel_out="$temp_dir/kernel"
mkdir -p "$kernel_out"

KCONFIG_CONFIG="$kernel_out/.config" \
"$sdk_dir/linux_5.10/scripts/kconfig/merge_config.sh" -m -O "$kernel_out" \
    "$sdk_dir/build/boards/sg200x/sg2002_licheervnano_sd/linux/sg2002_licheervnano_sd_defconfig" \
    "$repo_root/image/configs/phase1-kernel.fragment" \
    "$repo_root/image/configs/phase2-hid-kernel.fragment" \
    "$repo_root/image/configs/phase3-video-foundation-kernel.fragment" \
    "$repo_root/image/configs/phase3a-audio-enumeration-kernel.fragment"
make -s -C "$sdk_dir/linux_5.10" O="$kernel_out" ARCH=riscv olddefconfig

assert_enabled() {
    grep -qx "$1=y" "$kernel_out/.config" || {
        actual=$(grep -E "^($1=|# $1 is not set)" "$kernel_out/.config" || true)
        printf 'product profile mismatch: expected %s=y; found %s\n' \
            "$1" "${actual:-<symbol absent>}" >&2
        exit 1
    }
}

assert_disabled() {
    if grep -Eq "^$1=[ym]$" "$kernel_out/.config"; then
        actual=$(grep -E "^$1=" "$kernel_out/.config")
        printf 'product profile mismatch: expected %s disabled; found %s\n' \
            "$1" "$actual" >&2
        exit 1
    fi
}

for key in \
    CONFIG_ARCH_CVITEK CONFIG_SOC_THEAD CONFIG_ARCH_CV181X CONFIG_ARCH_CV181X_ASIC \
    CONFIG_CVITEK_PINCTRL_CV1835 CONFIG_COMMON_CLK_CVITEK \
    CONFIG_MODULES CONFIG_MODULE_UNLOAD CONFIG_I2C CONFIG_I2C_CHARDEV \
    CONFIG_I2C_DESIGNWARE_PLATFORM CONFIG_STAGING CONFIG_ION CONFIG_ION_CVITEK \
    CONFIG_SOUND CONFIG_SND CONFIG_SND_PCM CONFIG_SND_SOC \
    CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM CONFIG_SND_SOC_CV182XA_CV182XAADC \
    CONFIG_SND_SOC_CV182XA_CV182XADAC CONFIG_SND_CV1835_I2S \
    CONFIG_SND_SOC_CV182XAADC CONFIG_SND_SOC_CV182XADAC \
    CONFIG_SND_SOC_CV1835_USE_AUDIO_PLL CONFIG_CV1835_I2S_SUBSYS
do
    assert_enabled "$key"
done

for key in \
    CONFIG_MODULE_FORCE_LOAD CONFIG_MODULE_FORCE_UNLOAD CONFIG_MEDIA_SUPPORT \
    CONFIG_SND_PROC_FS CONFIG_SND_SEQUENCER CONFIG_SND_OSSEMUL \
    CONFIG_SND_RAWMIDI CONFIG_SND_DRIVERS CONFIG_SND_SPI CONFIG_SND_USB \
    CONFIG_SND_USB_AUDIO CONFIG_SND_SOC_CV1835_ADAU1372 \
    CONFIG_SND_SOC_CV1835_CV1835PDM CONFIG_SND_SOC_CV182X_CV182XPDM \
    CONFIG_SND_SOC_CV1835PDM CONFIG_SND_SOC_CV1835_CONCURRENT_I2S \
    CONFIG_SND_SOC_CV183x_DUMMY_CARD CONFIG_SND_SOC_CV1835_LT9611 \
    CONFIG_USB_CONFIGFS_F_UAC1 CONFIG_USB_CONFIGFS_F_UAC2 \
    CONFIG_USB_CONFIGFS_F_MIDI CONFIG_USB_CONFIGFS_F_UVC CONFIG_DMABUF_HEAPS
do
    assert_disabled "$key"
done

if grep -Eq '=m$' "$kernel_out/.config"; then
    printf '%s\n' 'product profile mismatch: in-tree modules are not allowed' >&2
    grep -E '=m$' "$kernel_out/.config" >&2
    exit 1
fi

printf '%s\n' 'normalized combined Onyx KVM product configuration passed'
