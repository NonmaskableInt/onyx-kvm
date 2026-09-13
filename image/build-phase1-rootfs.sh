#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v5}
download_volume=${CLEANKVM_DOWNLOAD_VOLUME:-cleankvm-buildroot-dl-v2}
incremental=${CLEANKVM_INCREMENTAL_ROOTFS:-0}
credential_hash_file=${CLEANKVM_RECOVERY_PASSWORD_HASH_FILE:-}
identity_dir=${CLEANKVM_IDENTITY_DIR:-}
config_profile=${CLEANKVM_CONFIG_PROFILE:-phase1}
video_module_dir=${CLEANKVM_VIDEO_MODULE_DIR:-$repo_root/out/phase3-video-modules-v1}
raw_frame_probe=${CLEANKVM_RAW_FRAME_PROBE:-}
audio_capture_probe=${CLEANKVM_AUDIO_CAPTURE_PROBE:-}
edid_programmer=${CLEANKVM_EDID_PROGRAMMER:-}
lt_backup=${CLEANKVM_LT_BACKUP:-}
identity_source=
set --
if test -n "$identity_dir"; then
    test -d "$identity_dir"
    identity_source=/identity
    set -- -v "$identity_dir:/identity:ro"
fi
if test -n "$raw_frame_probe"; then
    test -f "$raw_frame_probe"
    set -- "$@" -v "$raw_frame_probe:/raw-frame-probe:ro"
fi
if test -n "$audio_capture_probe"; then
    test -f "$audio_capture_probe"
    set -- "$@" -v "$audio_capture_probe:/audio-capture-probe:ro"
fi
if test -n "$edid_programmer"; then
    test -f "$edid_programmer"
    set -- "$@" -v "$edid_programmer:/edid-programmer:ro"
fi
if test -n "$lt_backup"; then
    test -f "$lt_backup"
    set -- "$@" -v "$lt_backup:/lt-backup:ro"
fi
credential_hash=!
if test -n "$credential_hash_file"; then
    test -f "$credential_hash_file"
    credential_hash=$(tr -d '\n' <"$credential_hash_file")
    case "$credential_hash" in
        '$6$'*) ;;
        *) printf '%s\n' 'invalid recovery credential hash file' >&2; exit 1 ;;
    esac
fi

"$engine" run --rm --arch amd64 --network none \
    -v "$work_volume:/work:rw" \
    -v "$download_volume:/downloads:ro" \
    -v "$repo_root:/repo:ro" \
    "$@" \
    -w /work/sdk \
    -e TMPDIR=/work/sdk/.tmp \
    -e CLEANKVM_INCREMENTAL_ROOTFS="$incremental" \
    -e CLEANKVM_RECOVERY_PASSWORD_HASH="$credential_hash" \
    -e CLEANKVM_IDENTITY_SOURCE="$identity_source" \
    -e CLEANKVM_CONFIG_PROFILE="$config_profile" \
    -e CLEANKVM_VIDEO_MODULE_DIR=/video-modules \
    -e CLEANKVM_RAW_FRAME_PROBE="${raw_frame_probe:+/raw-frame-probe}" \
    -e CLEANKVM_AUDIO_CAPTURE_PROBE="${audio_capture_probe:+/audio-capture-probe}" \
    -e CLEANKVM_EDID_PROGRAMMER="${edid_programmer:+/edid-programmer}" \
    -e CLEANKVM_LT_BACKUP="${lt_backup:+/lt-backup}" \
    -v "$video_module_dir:/video-modules:ro" \
    "$builder" bash -lc '
        set -e
        cd /
        sha256sum -c /downloads/SHA256SUMS
        cd /work/sdk
        mkdir -p "$TMPDIR"
        case "$CLEANKVM_CONFIG_PROFILE" in
            phase1) /repo/image/validate-phase1-config.sh /work/sdk ;;
            phase2-hid) /repo/image/validate-phase2-hid-config.sh /work/sdk ;;
            phase3-discovery) /repo/image/validate-phase3-discovery-config.sh /work/sdk ;;
            phase3-video-foundation) /repo/image/validate-phase3-video-foundation-config.sh /work/sdk ;;
            phase3a-audio-enumeration) /repo/image/validate-phase3a-audio-enumeration-config.sh /work/sdk ;;
            *) printf "unknown configuration profile: %s\n" "$CLEANKVM_CONFIG_PROFILE" >&2; exit 1 ;;
        esac
        source build/cvisetup.sh
        defconfig sg2002_licheervnano_sd
        if [ -n "$CLEANKVM_RAW_FRAME_PROBE" ]; then
            install -D -m 0500 "$CLEANKVM_RAW_FRAME_PROBE" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-raw-frame-probe
            grep -qF "/usr/libexec/cleankvm-raw-frame-probe f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-raw-frame-probe f 0500 0 0 - - - - -" \
                    >> cleankvm/phase1-devices.txt
        fi
        if [ -n "$CLEANKVM_AUDIO_CAPTURE_PROBE" ]; then
            install -D -m 0500 "$CLEANKVM_AUDIO_CAPTURE_PROBE" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-audio-capture-probe
            grep -qF "/usr/libexec/cleankvm-audio-capture-probe f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-audio-capture-probe f 0500 0 0 - - - - -" \
                    >> cleankvm/phase1-devices.txt
        fi
        if [ -n "$CLEANKVM_EDID_PROGRAMMER" ]; then
            install -D -m 0500 "$CLEANKVM_EDID_PROGRAMMER" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-lt6911uxc-edid-programmer
            grep -qF "/usr/libexec/cleankvm-lt6911uxc-edid-programmer f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-lt6911uxc-edid-programmer f 0500 0 0 - - - - -" \
                    >> cleankvm/phase1-devices.txt
        fi
        if [ -n "$CLEANKVM_LT_BACKUP" ]; then
            install -D -m 0500 "$CLEANKVM_LT_BACKUP" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-lt6911uxc-flash-backup
            grep -qF "/usr/libexec/cleankvm-lt6911uxc-flash-backup f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-lt6911uxc-flash-backup f 0500 0 0 - - - - -" \
                    >> cleankvm/phase1-devices.txt
        fi
        if [ "$CLEANKVM_INCREMENTAL_ROOTFS" != 1 ]; then
            make -C buildroot clean
        fi
        make -C buildroot cvitek_SG200X_musl_riscv64_defconfig \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64 \
            "BR2_STRIP_EXCLUDE_FILES=cleankvmd cleankvm-memtest cleankvm-hid-broker"
        # The vendor Buildroot/BusyBox combination does not reliably preserve
        # GNU make jobserver descriptors through its nested configuration
        # makes. A single job is slower but deterministic and fail-closed.
        make -C buildroot -j1 \
            BR2_DL_DIR=/downloads \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64 \
            "BR2_STRIP_EXCLUDE_FILES=cleankvmd cleankvm-memtest cleankvm-hid-broker"
        # Incremental Buildroot finalization may retain the prior target-tree
        # mode even though the filesystem device table correctly applies 0500.
        # Normalize both views so the pre-image audit cannot disagree with the
        # generated ext4 ownership audit.
        if [ -n "$CLEANKVM_EDID_PROGRAMMER" ]; then
            chmod 0500 /work/sdk/buildroot/output/target/usr/libexec/cleankvm-lt6911uxc-edid-programmer
        fi
        if [ -n "$CLEANKVM_LT_BACKUP" ]; then
            chmod 0500 /work/sdk/buildroot/output/target/usr/libexec/cleankvm-lt6911uxc-flash-backup
        fi
        hid_broker=
        video_modules=
        if [ "$CLEANKVM_CONFIG_PROFILE" = phase2-hid ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = phase3-discovery ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = phase3-video-foundation ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = phase3a-audio-enumeration ]; then
            hid_broker=/repo/out/phase2-rust/cleankvm-hid-broker
        fi
        if [ "$CLEANKVM_CONFIG_PROFILE" = phase3-video-foundation ]; then
            video_modules="$CLEANKVM_VIDEO_MODULE_DIR"
        fi
        /repo/image/audit-rootfs.sh /work/sdk/buildroot/output/target \
            /repo/out/phase2-rust/cleankvmd \
            /repo/out/phase2-rust/cleankvm-memtest \
            "$CLEANKVM_RECOVERY_PASSWORD_HASH" \
            "$CLEANKVM_IDENTITY_SOURCE" \
            "$hid_broker" \
            "$video_modules" \
            "$CLEANKVM_RAW_FRAME_PROBE" \
            "$CLEANKVM_AUDIO_CAPTURE_PROBE" \
            "$CLEANKVM_EDID_PROGRAMMER" \
            "$CLEANKVM_LT_BACKUP"
        if [ -n "$CLEANKVM_IDENTITY_SOURCE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            passwd_image=$($debugfs -R "cat /etc/passwd" "$root_image" 2>/dev/null)
            group_image=$($debugfs -R "cat /etc/group" "$root_image" 2>/dev/null)
            key_stat=$($debugfs -R "stat /etc/cleankvm/tls-private-key.pkcs8.der" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$passwd_image" | \
                grep -q "^cleankvm:x:900:900:.*:/nonexistent:/sbin/nologin$"
            printf "%s\n" "$group_image" | grep -q "^cleankvm:x:900:"
            printf "%s\n" "$key_stat" | grep -q "Mode: *0640"
            printf "%s\n" "$key_stat" | grep -q "User: *0 *Group: *900"
            printf "%s\n" "finished ext4 identity ownership audit passed"
        fi
        if [ -n "$CLEANKVM_RAW_FRAME_PROBE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/cleankvm-raw-frame-probe" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 raw-frame probe ownership audit passed"
        fi
        if [ -n "$CLEANKVM_AUDIO_CAPTURE_PROBE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/cleankvm-audio-capture-probe" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 audio capture probe ownership audit passed"
        fi
        if [ -n "$CLEANKVM_EDID_PROGRAMMER" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/cleankvm-lt6911uxc-edid-programmer" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 EDID programmer ownership audit passed"
        fi
        if [ -n "$CLEANKVM_LT_BACKUP" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/cleankvm-lt6911uxc-flash-backup" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
        fi
    '
