#!/bin/sh
set -eu

# Read-only Phase 3/3a hardware inventory. The script deliberately performs no
# I2C transactions, GPIO exports, module loads, mixer changes, or PCM opens.

printf '%s\n' 'ONYX_KVM_PHASE3_EVIDENCE_V1'
printf 'MODEL='
tr -d '\000' </proc/device-tree/model 2>/dev/null || printf '%s' unavailable
printf '\n'
printf 'COMPATIBLE='
tr '\000' ',' </proc/device-tree/compatible 2>/dev/null || printf '%s' unavailable
printf '\n'

printf '%s\n' '[I2C_ADAPTERS]'
for name in /sys/class/i2c-adapter/i2c-*/name; do
    test -r "$name" || continue
    printf '%s=' "${name%/name}"
    sed -n '1p' "$name"
done

printf '%s\n' '[MEDIA_DEVICE_NODES]'
for node in /dev/video* /dev/media* /dev/v4l-subdev* /dev/cvi-* /dev/snd/*; do
    test -e "$node" && printf '%s\n' "$node"
done

printf '%s\n' '[MEDIA_MODULES]'
awk '$1 ~ /^(soph_|cvi|snd|videodev|v4l2)/ { print $1 }' /proc/modules 2>/dev/null || true

printf '%s\n' '[ALSA_CARDS]'
if test -r /proc/asound/cards; then
    sed -n '1,80p' /proc/asound/cards
else
    printf '%s\n' absent
fi
printf '%s\n' '[ALSA_PCM]'
if test -r /proc/asound/pcm; then
    sed -n '1,80p' /proc/asound/pcm
else
    printf '%s\n' absent
fi

printf '%s\n' '[RESERVED_MEMORY_NODES]'
if test -d /proc/device-tree/reserved-memory; then
    find /proc/device-tree/reserved-memory -mindepth 1 -maxdepth 1 -type d \
        -exec basename '{}' \; | LC_ALL=C sort
else
    printf '%s\n' absent
fi

printf '%s\n' '[RELEVANT_DT_STATUS]'
find /proc/device-tree -type f -name status 2>/dev/null | while IFS= read -r status; do
    case "$status" in
        *audio*|*sound*|*adc*|*dac*|*i2s*|*pdm*|*mipi*|*video*|*vpss*|*venc*|*vi@*)
            printf '%s=' "${status%/status}"
            tr -d '\000' <"$status"
            printf '\n'
            ;;
    esac
done
printf '%s\n' 'ONYX_KVM_PHASE3_EVIDENCE_END'
