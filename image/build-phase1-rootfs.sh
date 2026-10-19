#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repo_root/image/default-env.sh"

"$repo_root/image/verify-sources.sh"

engine=$CONTAINER_ENGINE
builder=$ONYX_KVM_BUILDER_TAG
work_volume=$ONYX_KVM_WORK_VOLUME
download_volume=${ONYX_KVM_DOWNLOAD_VOLUME:-onyx-kvm-buildroot-dl-v50}
host_tools_volume=$ONYX_KVM_HOST_TOOLS_VOLUME
incremental=${ONYX_KVM_INCREMENTAL_ROOTFS:-0}
config_profile=${ONYX_KVM_CONFIG_PROFILE:-phase1}

default_credential_hash_file=
if test -f "$repo_root/out/product-v64-secrets/recovery-password.sha512"; then
    case "$config_profile" in
        product)
            default_credential_hash_file=$repo_root/out/product-v64-secrets/recovery-password.sha512
            ;;
    esac
fi
credential_hash_file=${ONYX_KVM_RECOVERY_PASSWORD_HASH_FILE:-$default_credential_hash_file}

default_identity_dir=
if test -d "$repo_root/out/product-v64-secrets/identity"; then
    case "$config_profile" in
        product)
            default_identity_dir=$repo_root/out/product-v64-secrets/identity
            ;;
    esac
fi
identity_dir=${ONYX_KVM_IDENTITY_DIR:-$default_identity_dir}
# Newest versioned artifact directory under out/ that contains NAME.
latest_artifact() {
    pattern=$1
    name=$2
    for dir in $(ls -d "$repo_root"/out/$pattern 2>/dev/null | sort -V -r); do
        if test -f "$dir/$name"; then
            printf '%s\n' "$dir/$name"
            return 0
        fi
    done
    return 0
}

default_video_module_dir=$repo_root/out/phase3-video-modules-v1
for candidate in v51 v48; do
    if test -d "$repo_root/out/phase3-video-modules-$candidate"; then
        case "$config_profile" in
            phase3-video-foundation|product)
                default_video_module_dir=$repo_root/out/phase3-video-modules-$candidate
                ;;
        esac
        break
    fi
done
video_module_dir=${ONYX_KVM_VIDEO_MODULE_DIR:-$default_video_module_dir}

default_raw_frame_probe=
default_audio_capture_probe=
default_audio_playback_probe=
default_edid_programmer=
case "$config_profile" in
    phase3-video-foundation|product)
        default_raw_frame_probe=$(latest_artifact 'phase3-raw-frame-probe-v*' onyx-kvm-raw-frame-probe)
        ;;
esac
case "$config_profile" in
    phase3a-audio-enumeration|product)
        default_audio_capture_probe=$(latest_artifact 'phase3a-capture-probe-v*' onyx-kvm-audio-capture-probe)
        default_audio_playback_probe=$(latest_artifact 'phase3a-playback-probe-v*' onyx-kvm-audio-playback-probe)
        ;;
esac
case "$config_profile" in
    product)
        default_edid_programmer=$(latest_artifact 'lt6911uxc-edid-programmer-v*' onyx-kvm-lt6911uxc-edid-programmer)
        ;;
esac
raw_frame_probe=${ONYX_KVM_RAW_FRAME_PROBE:-$default_raw_frame_probe}
audio_capture_probe=${ONYX_KVM_AUDIO_CAPTURE_PROBE:-$default_audio_capture_probe}
audio_playback_probe=${ONYX_KVM_AUDIO_PLAYBACK_PROBE:-$default_audio_playback_probe}
edid_programmer=${ONYX_KVM_EDID_PROGRAMMER:-$default_edid_programmer}
if test "$config_profile" = product; then
    # A product image without these helpers would silently lose video, audio
    # or EDID support, so fail before the container build starts.
    for required in "raw frame probe:$raw_frame_probe" \
        "audio capture probe:$audio_capture_probe" \
        "audio playback probe:$audio_playback_probe" \
        "EDID programmer:$edid_programmer"; do
        test -n "${required#*:}" || {
            printf 'product profile requires %s; build it first (see image/README.md)\n' "${required%%:*}" >&2
            exit 1
        }
    done
fi
lt_backup=${ONYX_KVM_LT_BACKUP:-}
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
if test -n "$audio_playback_probe"; then
    test -f "$audio_playback_probe"
    set -- "$@" -v "$audio_playback_probe:/audio-playback-probe:ro"
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
    -v "$host_tools_volume:/host-tools:ro" \
    -v "$repo_root:/repo:ro" \
    "$@" \
    -w /work/sdk \
    -e TMPDIR=/work/sdk/.tmp \
    -e ONYX_KVM_INCREMENTAL_ROOTFS="$incremental" \
    -e ONYX_KVM_RECOVERY_PASSWORD_HASH="$credential_hash" \
    -e ONYX_KVM_IDENTITY_SOURCE="$identity_source" \
    -e ONYX_KVM_CONFIG_PROFILE="$config_profile" \
    -e ONYX_KVM_VIDEO_MODULE_DIR=/video-modules \
    -e ONYX_KVM_RAW_FRAME_PROBE="${raw_frame_probe:+/raw-frame-probe}" \
    -e ONYX_KVM_AUDIO_CAPTURE_PROBE="${audio_capture_probe:+/audio-capture-probe}" \
    -e ONYX_KVM_AUDIO_PLAYBACK_PROBE="${audio_playback_probe:+/audio-playback-probe}" \
    -e ONYX_KVM_EDID_PROGRAMMER="${edid_programmer:+/edid-programmer}" \
    -e ONYX_KVM_LT_BACKUP="${lt_backup:+/lt-backup}" \
    -v "$video_module_dir:/video-modules:ro" \
    "$builder" bash -lc '
        set -e
        if [ ! -e /work/sdk/host-tools ]; then
            ln -s /host-tools/repo /work/sdk/host-tools
        fi
        test "$(readlink -f /work/sdk/host-tools)" = /host-tools/repo
        cd /
        sha256sum -c /downloads/SHA256SUMS
        cd /work/sdk
        mkdir -p "$TMPDIR"
        case "$ONYX_KVM_CONFIG_PROFILE" in
            phase1) /repo/image/validate-phase1-config.sh /work/sdk ;;
            phase2-hid) /repo/image/validate-phase2-hid-config.sh /work/sdk ;;
            phase3-discovery) /repo/image/validate-phase3-discovery-config.sh /work/sdk ;;
            phase3-video-foundation) /repo/image/validate-phase3-video-foundation-config.sh /work/sdk ;;
            phase3a-audio-enumeration) /repo/image/validate-phase3a-audio-enumeration-config.sh /work/sdk ;;
            product) /repo/image/validate-product-config.sh /work/sdk ;;
            *) printf "unknown configuration profile: %s\n" "$ONYX_KVM_CONFIG_PROFILE" >&2; exit 1 ;;
        esac
        source build/cvisetup.sh
        defconfig sg2002_licheervnano_sd
        # Reinstall every selected Rust artifact on every build. The SDK work
        # volume is reusable, so relying on a prior apply step can otherwise
        # retain a stale executable even when its source and verified output
        # have changed.
        cp /repo/image/lock-root-password.sh onyx-kvm/lock-root-password.sh
        chmod 0755 onyx-kvm/lock-root-password.sh
        cp /repo/image/configs/phase1-users.txt onyx-kvm/phase1-users.txt
        cp /repo/image/configs/phase1-devices.txt onyx-kvm/phase1-devices.txt
        cp -a /repo/image/overlay/. onyx-kvm/phase1-overlay/
        # Reusable work volumes may still hold pre-rename (cleankvm*) overlay
        # directories and target files from earlier builds. Remove them so the
        # rootfs audit only ever sees current artifacts.
        rm -rf cleankvm
        find buildroot/output/target -iname "*cleankvm*" -prune -exec rm -rf {} + 2>/dev/null || true

        daemonsrc=/repo/out/phase2-rust/onyx-kvmd
        install -m 0755 "$daemonsrc" onyx-kvm/phase1-overlay/usr/bin/onyx-kvmd

        memtestsrc=/repo/out/phase2-rust/onyx-kvm-memtest
        install -m 0755 "$memtestsrc" onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-memtest

        if [ "$ONYX_KVM_CONFIG_PROFILE" != phase1 ]; then
            cp -a /repo/image/phase2-hid-overlay/. onyx-kvm/phase1-overlay/
            hidsrc=/repo/out/phase2-rust/onyx-kvm-hid-broker
            install -m 0755 "$hidsrc" onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-hid-broker
        fi

        if [ "${ONYX_KVM_DEV_TOOLS:-0}" = "1" ] && [ -f /repo/out/dev-dropbear-v1/dropbear ]; then
            mkdir -p onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/dev
            install -m 0755 /repo/out/dev-dropbear-v1/dropbear onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/dev/dropbear
            if [ -e /repo/out/dev-dropbear-v1/dropbearkey ]; then
                cp -a /repo/out/dev-dropbear-v1/dropbearkey onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/dev/dropbearkey
            fi
            mkdir -p buildroot/output/target/usr/lib/onyx-kvm/dev 2>/dev/null || true
            install -m 0755 /repo/out/dev-dropbear-v1/dropbear buildroot/output/target/usr/lib/onyx-kvm/dev/dropbear 2>/dev/null || true
            if [ -e /repo/out/dev-dropbear-v1/dropbearkey ]; then
                cp -a /repo/out/dev-dropbear-v1/dropbearkey buildroot/output/target/usr/lib/onyx-kvm/dev/dropbearkey 2>/dev/null || true
            fi
        else
            rm -rf onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/dev buildroot/output/target/usr/lib/onyx-kvm/dev 2>/dev/null || true
            if [ "$profile" = "product" ]; then
                rm -f onyx-kvm/phase1-overlay/etc/init.d/S35onyx-kvm-devmode buildroot/output/target/etc/init.d/S35onyx-kvm-devmode 2>/dev/null || true
            fi
        fi
        # The SDK work volume is deliberately reusable, but Buildroot overlays
        # are additive. Remove every optional artifact from both the source
        # overlay and an incremental target before installing the artifacts
        # selected for this build. This prevents a profile transition (for
        # example, video to audio enumeration) from silently inheriting a
        # previously admitted component.
        for optional_path in \
            usr/libexec/onyx-kvm-raw-frame-probe \
            usr/libexec/onyx-kvm-audio-capture-probe \
            usr/libexec/onyx-kvm-audio-playback-probe \
            usr/libexec/onyx-kvm-lt6911uxc-edid-programmer \
            usr/libexec/onyx-kvm-lt6911uxc-flash-backup
        do
            rm -f "onyx-kvm/phase1-overlay/$optional_path"
            rm -f "buildroot/output/target/$optional_path"
        done
        sed -i \
            -e "\\|^/usr/libexec/onyx-kvm-raw-frame-probe f |d" \
            -e "\\|^/usr/libexec/onyx-kvm-audio-capture-probe f |d" \
            -e "\\|^/usr/libexec/onyx-kvm-audio-playback-probe f |d" \
            -e "\\|^/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer f |d" \
            -e "\\|^/usr/libexec/onyx-kvm-lt6911uxc-flash-backup f |d" \
            onyx-kvm/phase1-devices.txt
        if [ "$ONYX_KVM_CONFIG_PROFILE" != phase3-video-foundation ] && \
           [ "$ONYX_KVM_CONFIG_PROFILE" != product ]; then
            rm -f onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-video-broker
            rm -f onyx-kvm/phase1-overlay/etc/init.d/S47onyx-kvm-video-broker
            rm -rf onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine
            rm -f buildroot/output/target/usr/bin/onyx-kvm-video-broker
            rm -f buildroot/output/target/etc/init.d/S47onyx-kvm-video-broker
            rm -rf buildroot/output/target/usr/lib/onyx-kvm/video-quarantine
        else
            cp -a /repo/image/phase3-video-overlay/. onyx-kvm/phase1-overlay/
            videosrc=/repo/out/phase3-rust/onyx-kvm-video-broker
            install -m 0755 "$videosrc" onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-video-broker
            mkdir -p onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine
            for module in /video-modules/*.ko; do
                install -m 0444 "$module" \
                    "onyx-kvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine/$(basename "$module")"
            done
            if [ -f /video-modules/coda980.bin ]; then
                mkdir -p onyx-kvm/phase1-overlay/usr/share/fw_vcodec
                install -m 0444 /video-modules/coda980.bin onyx-kvm/phase1-overlay/usr/share/fw_vcodec/coda980.bin
            elif [ -f /work/sdk/ramdisk/rootfs/common_musl_riscv64/usr/share/fw_vcodec/coda980.bin ]; then
                mkdir -p onyx-kvm/phase1-overlay/usr/share/fw_vcodec
                install -m 0444 /work/sdk/ramdisk/rootfs/common_musl_riscv64/usr/share/fw_vcodec/coda980.bin onyx-kvm/phase1-overlay/usr/share/fw_vcodec/coda980.bin
            fi
        fi
        # Incremental profile changes must not carry generated device nodes from
        # an earlier audio image into a non-audio filesystem. These nodes are
        # not owned by an overlay or device table and otherwise survive in the
        # Buildroot target tree even when CONFIG_SOUND is disabled.
        if [ "$ONYX_KVM_CONFIG_PROFILE" != phase3a-audio-enumeration ] && \
           [ "$ONYX_KVM_CONFIG_PROFILE" != product ]; then
            rm -rf buildroot/output/target/dev/snd
            rm -f onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-audio-capture-broker
            rm -f onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-audio-playback-broker
            rm -f onyx-kvm/phase1-overlay/etc/init.d/S47onyx-kvm-audio-capture-broker
            rm -f onyx-kvm/phase1-overlay/etc/init.d/S48onyx-kvm-audio-playback-broker
            rm -f buildroot/output/target/usr/bin/onyx-kvm-audio-capture-broker
            rm -f buildroot/output/target/usr/bin/onyx-kvm-audio-playback-broker
            rm -f buildroot/output/target/etc/init.d/S47onyx-kvm-audio-capture-broker
            rm -f buildroot/output/target/etc/init.d/S48onyx-kvm-audio-playback-broker
        elif [ -f /repo/out/phase3a-rust/onyx-kvm-audio-capture-broker ]; then
            cp -a /repo/image/phase3a-audio-overlay/. onyx-kvm/phase1-overlay/
            capturesrc=/repo/out/phase3a-rust/onyx-kvm-audio-capture-broker
            playbacksrc=/repo/out/phase3a-rust/onyx-kvm-audio-playback-broker
            install -m 0755 "$capturesrc" \
                onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-audio-capture-broker
            install -m 0755 "$playbacksrc" \
                onyx-kvm/phase1-overlay/usr/bin/onyx-kvm-audio-playback-broker
        fi
        if [ -n "$ONYX_KVM_RAW_FRAME_PROBE" ]; then
            install -D -m 0500 "$ONYX_KVM_RAW_FRAME_PROBE" \
                onyx-kvm/phase1-overlay/usr/libexec/onyx-kvm-raw-frame-probe
            grep -qF "/usr/libexec/onyx-kvm-raw-frame-probe f 0500 0 0" \
                onyx-kvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/onyx-kvm-raw-frame-probe f 0500 0 0 - - - - -" \
                    >> onyx-kvm/phase1-devices.txt
        fi
        if [ -n "$ONYX_KVM_AUDIO_CAPTURE_PROBE" ]; then
            install -D -m 0500 "$ONYX_KVM_AUDIO_CAPTURE_PROBE" \
                onyx-kvm/phase1-overlay/usr/libexec/onyx-kvm-audio-capture-probe
            grep -qF "/usr/libexec/onyx-kvm-audio-capture-probe f 0500 0 0" \
                onyx-kvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/onyx-kvm-audio-capture-probe f 0500 0 0 - - - - -" \
                    >> onyx-kvm/phase1-devices.txt
        fi
        if [ -n "$ONYX_KVM_AUDIO_PLAYBACK_PROBE" ]; then
            install -D -m 0500 "$ONYX_KVM_AUDIO_PLAYBACK_PROBE" \
                onyx-kvm/phase1-overlay/usr/libexec/onyx-kvm-audio-playback-probe
            grep -qF "/usr/libexec/onyx-kvm-audio-playback-probe f 0500 0 0" \
                onyx-kvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/onyx-kvm-audio-playback-probe f 0500 0 0 - - - - -" \
                    >> onyx-kvm/phase1-devices.txt
        fi
        if [ -n "$ONYX_KVM_EDID_PROGRAMMER" ]; then
            install -D -m 0500 "$ONYX_KVM_EDID_PROGRAMMER" \
                onyx-kvm/phase1-overlay/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer
            grep -qF "/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer f 0500 0 0" \
                onyx-kvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer f 0500 0 0 - - - - -" \
                    >> onyx-kvm/phase1-devices.txt
        fi
        if [ -n "$ONYX_KVM_LT_BACKUP" ]; then
            install -D -m 0500 "$ONYX_KVM_LT_BACKUP" \
                onyx-kvm/phase1-overlay/usr/libexec/onyx-kvm-lt6911uxc-flash-backup
            grep -qF "/usr/libexec/onyx-kvm-lt6911uxc-flash-backup f 0500 0 0" \
                onyx-kvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/onyx-kvm-lt6911uxc-flash-backup f 0500 0 0 - - - - -" \
                    >> onyx-kvm/phase1-devices.txt
        fi
        if [ "$ONYX_KVM_INCREMENTAL_ROOTFS" != 1 ]; then
            make -C buildroot clean
        fi
        make -C buildroot cvitek_SG200X_musl_riscv64_defconfig \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64 \
            "BR2_STRIP_EXCLUDE_FILES=onyx-kvmd onyx-kvm-memtest onyx-kvm-hid-broker onyx-kvm-video-broker onyx-kvm-audio-capture-broker onyx-kvm-audio-playback-broker"
        # The vendor Buildroot/BusyBox combination does not reliably preserve
        # GNU make jobserver descriptors through its nested configuration
        # makes. A single job is slower but deterministic and fail-closed.
        make -C buildroot -j1 \
            BR2_DL_DIR=/downloads \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64 \
            "BR2_STRIP_EXCLUDE_FILES=onyx-kvmd onyx-kvm-memtest onyx-kvm-hid-broker onyx-kvm-video-broker onyx-kvm-audio-capture-broker onyx-kvm-audio-playback-broker"
        # Incremental Buildroot finalization may retain the prior target-tree
        # mode even though the filesystem device table correctly applies 0500.
        # Normalize both views so the pre-image audit cannot disagree with the
        # generated ext4 ownership audit.
        if [ -n "$ONYX_KVM_EDID_PROGRAMMER" ]; then
            chmod 0500 /work/sdk/buildroot/output/target/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer
        fi
        if [ -n "$ONYX_KVM_LT_BACKUP" ]; then
            chmod 0500 /work/sdk/buildroot/output/target/usr/libexec/onyx-kvm-lt6911uxc-flash-backup
        fi
        if [ -f /work/sdk/buildroot/output/target/usr/share/fw_vcodec/coda980.bin ]; then
            chmod 0444 /work/sdk/buildroot/output/target/usr/share/fw_vcodec/coda980.bin
        fi
        hid_broker=
        video_modules=
        video_broker=
        audio_capture_broker=
        audio_playback_broker=
        daemon_bin=/repo/out/phase2-rust/onyx-kvmd
        memtest_bin=/repo/out/phase2-rust/onyx-kvm-memtest
        if [ "$ONYX_KVM_CONFIG_PROFILE" = phase2-hid ] || \
           [ "$ONYX_KVM_CONFIG_PROFILE" = phase3-discovery ] || \
           [ "$ONYX_KVM_CONFIG_PROFILE" = phase3-video-foundation ] || \
           [ "$ONYX_KVM_CONFIG_PROFILE" = phase3a-audio-enumeration ] || \
           [ "$ONYX_KVM_CONFIG_PROFILE" = product ]; then
            hid_broker=/repo/out/phase2-rust/onyx-kvm-hid-broker
        fi
        if [ "$ONYX_KVM_CONFIG_PROFILE" = phase3-video-foundation ] || \
           [ "$ONYX_KVM_CONFIG_PROFILE" = product ]; then
            video_modules="$ONYX_KVM_VIDEO_MODULE_DIR"
            if [ -f /repo/out/phase3-rust/onyx-kvm-video-broker ]; then
                video_broker=/repo/out/phase3-rust/onyx-kvm-video-broker
            fi
        fi
        if [ "$ONYX_KVM_CONFIG_PROFILE" = phase3a-audio-enumeration ] || \
           [ "$ONYX_KVM_CONFIG_PROFILE" = product ]; then
            audio_capture_broker=/repo/out/phase3a-rust/onyx-kvm-audio-capture-broker
            audio_playback_broker=/repo/out/phase3a-rust/onyx-kvm-audio-playback-broker
        fi
        /repo/image/audit-rootfs.sh /work/sdk/buildroot/output/target \
            "$daemon_bin" \
            "$memtest_bin" \
            "$ONYX_KVM_RECOVERY_PASSWORD_HASH" \
            "$ONYX_KVM_IDENTITY_SOURCE" \
            "$hid_broker" \
            "$video_modules" \
            "$ONYX_KVM_RAW_FRAME_PROBE" \
            "$ONYX_KVM_AUDIO_CAPTURE_PROBE" \
            "$ONYX_KVM_EDID_PROGRAMMER" \
            "$ONYX_KVM_LT_BACKUP" \
            "$video_broker" \
            "$ONYX_KVM_AUDIO_PLAYBACK_PROBE" \
            "$audio_capture_broker" \
            "$audio_playback_broker" \
            "$ONYX_KVM_CONFIG_PROFILE"
        if [ -n "$ONYX_KVM_IDENTITY_SOURCE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            passwd_image=$($debugfs -R "cat /etc/passwd" "$root_image" 2>/dev/null)
            group_image=$($debugfs -R "cat /etc/group" "$root_image" 2>/dev/null)
            key_stat=$($debugfs -R "stat /etc/onyx-kvm/tls-private-key.pkcs8.der" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$passwd_image" | \
                grep -Eq "^onyx-kvm:x:900:900:.*:/nonexistent:/sbin/nologin$"
            printf "%s\n" "$group_image" | grep -Eq "^onyx-kvm:x:900:"
            printf "%s\n" "$key_stat" | grep -q "Mode: *0640"
            printf "%s\n" "$key_stat" | grep -q "User: *0 *Group: *900"
            printf "%s\n" "finished ext4 identity ownership audit passed"
        fi
        if [ -n "$ONYX_KVM_RAW_FRAME_PROBE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/onyx-kvm-raw-frame-probe" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 raw-frame probe ownership audit passed"
        fi
        if [ "$ONYX_KVM_CONFIG_PROFILE" != phase3a-audio-enumeration ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            ! $debugfs -R "stat /dev/snd/controlC0" "$root_image" 2>&1 | \
                grep -q "Inode:"
            ! $debugfs -R "stat /dev/snd/pcmC0D0c" "$root_image" 2>&1 | \
                grep -q "Inode:"
            printf "%s\n" "finished ext4 non-audio device-node audit passed"
        fi
        if [ -n "$ONYX_KVM_AUDIO_CAPTURE_PROBE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/onyx-kvm-audio-capture-probe" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 audio capture probe ownership audit passed"
        fi
        if [ -n "$ONYX_KVM_AUDIO_PLAYBACK_PROBE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/onyx-kvm-audio-playback-probe" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 audio playback probe ownership audit passed"
        fi
        if [ -n "$ONYX_KVM_EDID_PROGRAMMER" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/onyx-kvm-lt6911uxc-edid-programmer" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 EDID programmer ownership audit passed"
        fi
        if [ -n "$ONYX_KVM_LT_BACKUP" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/onyx-kvm-lt6911uxc-flash-backup" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
        fi
    '
