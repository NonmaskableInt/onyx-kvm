#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v50}
download_volume=${CLEANKVM_DOWNLOAD_VOLUME:-cleankvm-buildroot-dl-v50}
host_tools_volume=${CLEANKVM_HOST_TOOLS_VOLUME:-cleankvm-host-tools}
incremental=${CLEANKVM_INCREMENTAL_ROOTFS:-0}
credential_hash_file=${CLEANKVM_RECOVERY_PASSWORD_HASH_FILE:-}
identity_dir=${CLEANKVM_IDENTITY_DIR:-}
config_profile=${CLEANKVM_CONFIG_PROFILE:-phase1}
default_video_module_dir=$repo_root/out/phase3-video-modules-v1
if test -d "$repo_root/out/phase3-video-modules-v48"; then
    case "$config_profile" in
        phase3-video-foundation|product)
            default_video_module_dir=$repo_root/out/phase3-video-modules-v48
            ;;
    esac
fi
video_module_dir=${CLEANKVM_VIDEO_MODULE_DIR:-$default_video_module_dir}

default_raw_frame_probe=
if test -f "$repo_root/out/phase3-raw-frame-probe-v50/cleankvm-raw-frame-probe"; then
    case "$config_profile" in
        phase3-video-foundation|product)
            default_raw_frame_probe=$repo_root/out/phase3-raw-frame-probe-v50/cleankvm-raw-frame-probe
            ;;
    esac
elif test -f "$repo_root/out/phase3-raw-frame-probe-v47/cleankvm-raw-frame-probe"; then
    case "$config_profile" in
        phase3-video-foundation|product)
            default_raw_frame_probe=$repo_root/out/phase3-raw-frame-probe-v47/cleankvm-raw-frame-probe
            ;;
    esac
elif test -f "$repo_root/out/phase3-raw-frame-probe-v46/cleankvm-raw-frame-probe"; then
    case "$config_profile" in
        phase3-video-foundation|product)
            default_raw_frame_probe=$repo_root/out/phase3-raw-frame-probe-v46/cleankvm-raw-frame-probe
            ;;
    esac
fi
raw_frame_probe=${CLEANKVM_RAW_FRAME_PROBE:-$default_raw_frame_probe}

default_audio_capture_probe=
if test -f "$repo_root/out/phase3a-capture-probe-v1/cleankvm-audio-capture-probe"; then
    case "$config_profile" in
        phase3a-audio-enumeration|product)
            default_audio_capture_probe=$repo_root/out/phase3a-capture-probe-v1/cleankvm-audio-capture-probe
            ;;
    esac
fi
audio_capture_probe=${CLEANKVM_AUDIO_CAPTURE_PROBE:-$default_audio_capture_probe}

default_audio_playback_probe=
if test -f "$repo_root/out/phase3a-playback-probe-v1/cleankvm-audio-playback-probe"; then
    case "$config_profile" in
        phase3a-audio-enumeration|product)
            default_audio_playback_probe=$repo_root/out/phase3a-playback-probe-v1/cleankvm-audio-playback-probe
            ;;
    esac
fi
audio_playback_probe=${CLEANKVM_AUDIO_PLAYBACK_PROBE:-$default_audio_playback_probe}

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
    -e CLEANKVM_INCREMENTAL_ROOTFS="$incremental" \
    -e CLEANKVM_RECOVERY_PASSWORD_HASH="$credential_hash" \
    -e CLEANKVM_IDENTITY_SOURCE="$identity_source" \
    -e CLEANKVM_CONFIG_PROFILE="$config_profile" \
    -e CLEANKVM_VIDEO_MODULE_DIR=/video-modules \
    -e CLEANKVM_RAW_FRAME_PROBE="${raw_frame_probe:+/raw-frame-probe}" \
    -e CLEANKVM_AUDIO_CAPTURE_PROBE="${audio_capture_probe:+/audio-capture-probe}" \
    -e CLEANKVM_AUDIO_PLAYBACK_PROBE="${audio_playback_probe:+/audio-playback-probe}" \
    -e CLEANKVM_EDID_PROGRAMMER="${edid_programmer:+/edid-programmer}" \
    -e CLEANKVM_LT_BACKUP="${lt_backup:+/lt-backup}" \
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
        case "$CLEANKVM_CONFIG_PROFILE" in
            phase1) /repo/image/validate-phase1-config.sh /work/sdk ;;
            phase2-hid) /repo/image/validate-phase2-hid-config.sh /work/sdk ;;
            phase3-discovery) /repo/image/validate-phase3-discovery-config.sh /work/sdk ;;
            phase3-video-foundation) /repo/image/validate-phase3-video-foundation-config.sh /work/sdk ;;
            phase3a-audio-enumeration) /repo/image/validate-phase3a-audio-enumeration-config.sh /work/sdk ;;
            product) /repo/image/validate-product-config.sh /work/sdk ;;
            *) printf "unknown configuration profile: %s\n" "$CLEANKVM_CONFIG_PROFILE" >&2; exit 1 ;;
        esac
        source build/cvisetup.sh
        defconfig sg2002_licheervnano_sd
        # Reinstall every selected Rust artifact on every build. The SDK work
        # volume is reusable, so relying on a prior apply step can otherwise
        # retain a stale executable even when its source and verified output
        # have changed.
        cp /repo/image/lock-root-password.sh cleankvm/lock-root-password.sh
        chmod 0755 cleankvm/lock-root-password.sh
        cp /repo/image/configs/phase1-users.txt cleankvm/phase1-users.txt
        cp /repo/image/configs/phase1-devices.txt cleankvm/phase1-devices.txt
        cp -a /repo/image/overlay/. cleankvm/phase1-overlay/
        rm -f cleankvm/phase1-overlay/etc/init.d/S50cleankvmd
        rm -f buildroot/output/target/etc/init.d/S50cleankvmd

        daemonsrc=/repo/out/phase2-rust/onyx-kvmd
        if [ ! -f "$daemonsrc" ]; then daemonsrc=/repo/out/phase2-rust/cleankvmd; fi
        install -m 0755 "$daemonsrc" cleankvm/phase1-overlay/usr/bin/onyx-kvmd
        ln -sf onyx-kvmd cleankvm/phase1-overlay/usr/bin/cleankvmd

        memtestsrc=/repo/out/phase2-rust/onyx-kvm-memtest
        if [ ! -f "$memtestsrc" ]; then memtestsrc=/repo/out/phase2-rust/cleankvm-memtest; fi
        install -m 0755 "$memtestsrc" cleankvm/phase1-overlay/usr/bin/onyx-kvm-memtest
        ln -sf onyx-kvm-memtest cleankvm/phase1-overlay/usr/bin/cleankvm-memtest

        if [ "$CLEANKVM_CONFIG_PROFILE" != phase1 ]; then
            cp -a /repo/image/phase2-hid-overlay/. cleankvm/phase1-overlay/
            rm -f cleankvm/phase1-overlay/etc/init.d/S45cleankvm-hid-gadget cleankvm/phase1-overlay/etc/init.d/S46cleankvm-hid-broker
            rm -f buildroot/output/target/etc/init.d/S45cleankvm-hid-gadget buildroot/output/target/etc/init.d/S46cleankvm-hid-broker
            hidsrc=/repo/out/phase2-rust/onyx-kvm-hid-broker
            if [ ! -f "$hidsrc" ]; then hidsrc=/repo/out/phase2-rust/cleankvm-hid-broker; fi
            install -m 0755 "$hidsrc" cleankvm/phase1-overlay/usr/bin/onyx-kvm-hid-broker
            ln -sf onyx-kvm-hid-broker cleankvm/phase1-overlay/usr/bin/cleankvm-hid-broker
        fi
        # The SDK work volume is deliberately reusable, but Buildroot overlays
        # are additive. Remove every optional artifact from both the source
        # overlay and an incremental target before installing the artifacts
        # selected for this build. This prevents a profile transition (for
        # example, video to audio enumeration) from silently inheriting a
        # previously admitted component.
        for optional_path in \
            usr/libexec/cleankvm-raw-frame-probe \
            usr/libexec/cleankvm-audio-capture-probe \
            usr/libexec/cleankvm-audio-playback-probe \
            usr/libexec/cleankvm-lt6911uxc-edid-programmer \
            usr/libexec/cleankvm-lt6911uxc-flash-backup
        do
            rm -f "cleankvm/phase1-overlay/$optional_path"
            rm -f "buildroot/output/target/$optional_path"
        done
        sed -i \
            -e "\\|^/usr/libexec/cleankvm-raw-frame-probe f |d" \
            -e "\\|^/usr/libexec/cleankvm-audio-capture-probe f |d" \
            -e "\\|^/usr/libexec/cleankvm-audio-playback-probe f |d" \
            -e "\\|^/usr/libexec/cleankvm-lt6911uxc-edid-programmer f |d" \
            -e "\\|^/usr/libexec/cleankvm-lt6911uxc-flash-backup f |d" \
            cleankvm/phase1-devices.txt
        if [ "$CLEANKVM_CONFIG_PROFILE" != phase3-video-foundation ] && \
           [ "$CLEANKVM_CONFIG_PROFILE" != product ]; then
            rm -f cleankvm/phase1-overlay/usr/bin/onyx-kvm-video-broker cleankvm/phase1-overlay/usr/bin/cleankvm-video-broker
            rm -f cleankvm/phase1-overlay/etc/init.d/S47onyx-kvm-video-broker cleankvm/phase1-overlay/etc/init.d/S47cleankvm-video-broker
            rm -rf cleankvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine cleankvm/phase1-overlay/usr/lib/cleankvm/video-quarantine
            rm -f buildroot/output/target/usr/bin/onyx-kvm-video-broker buildroot/output/target/usr/bin/cleankvm-video-broker
            rm -f buildroot/output/target/etc/init.d/S47onyx-kvm-video-broker buildroot/output/target/etc/init.d/S47cleankvm-video-broker
            rm -rf buildroot/output/target/usr/lib/onyx-kvm/video-quarantine buildroot/output/target/usr/lib/cleankvm/video-quarantine
        else
            cp -a /repo/image/phase3-video-overlay/. cleankvm/phase1-overlay/
            rm -f cleankvm/phase1-overlay/etc/init.d/S47cleankvm-video-broker
            rm -f buildroot/output/target/etc/init.d/S47cleankvm-video-broker
            videosrc=/repo/out/phase3-rust/onyx-kvm-video-broker
            if [ ! -f "$videosrc" ]; then videosrc=/repo/out/phase3-rust/cleankvm-video-broker; fi
            install -m 0755 "$videosrc" cleankvm/phase1-overlay/usr/bin/onyx-kvm-video-broker
            ln -sf onyx-kvm-video-broker cleankvm/phase1-overlay/usr/bin/cleankvm-video-broker
            mkdir -p cleankvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine
            mkdir -p cleankvm/phase1-overlay/usr/lib/cleankvm/video-quarantine
            for module in /video-modules/*.ko; do
                install -m 0444 "$module" \
                    "cleankvm/phase1-overlay/usr/lib/onyx-kvm/video-quarantine/$(basename "$module")"
                install -m 0444 "$module" \
                    "cleankvm/phase1-overlay/usr/lib/cleankvm/video-quarantine/$(basename "$module")"
            done
        fi
        # Incremental profile changes must not carry generated device nodes from
        # an earlier audio image into a non-audio filesystem. These nodes are
        # not owned by an overlay or device table and otherwise survive in the
        # Buildroot target tree even when CONFIG_SOUND is disabled.
        if [ "$CLEANKVM_CONFIG_PROFILE" != phase3a-audio-enumeration ] && \
           [ "$CLEANKVM_CONFIG_PROFILE" != product ]; then
            rm -rf buildroot/output/target/dev/snd
            rm -f cleankvm/phase1-overlay/usr/bin/onyx-kvm-audio-capture-broker cleankvm/phase1-overlay/usr/bin/cleankvm-audio-capture-broker
            rm -f cleankvm/phase1-overlay/usr/bin/onyx-kvm-audio-playback-broker cleankvm/phase1-overlay/usr/bin/cleankvm-audio-playback-broker
            rm -f cleankvm/phase1-overlay/etc/init.d/S47onyx-kvm-audio-capture-broker cleankvm/phase1-overlay/etc/init.d/S47cleankvm-audio-capture-broker
            rm -f cleankvm/phase1-overlay/etc/init.d/S48onyx-kvm-audio-playback-broker cleankvm/phase1-overlay/etc/init.d/S48cleankvm-audio-playback-broker
            rm -f buildroot/output/target/usr/bin/onyx-kvm-audio-capture-broker buildroot/output/target/usr/bin/cleankvm-audio-capture-broker
            rm -f buildroot/output/target/usr/bin/onyx-kvm-audio-playback-broker buildroot/output/target/usr/bin/cleankvm-audio-playback-broker
            rm -f buildroot/output/target/etc/init.d/S47onyx-kvm-audio-capture-broker buildroot/output/target/etc/init.d/S47cleankvm-audio-capture-broker
            rm -f buildroot/output/target/etc/init.d/S48onyx-kvm-audio-playback-broker buildroot/output/target/etc/init.d/S48cleankvm-audio-playback-broker
        elif [ -f /repo/out/phase3a-rust/onyx-kvm-audio-capture-broker ] || \
             [ -f /repo/out/phase3a-rust/cleankvm-audio-capture-broker ]; then
            cp -a /repo/image/phase3a-audio-overlay/. cleankvm/phase1-overlay/
            rm -f cleankvm/phase1-overlay/etc/init.d/S47cleankvm-audio-capture-broker cleankvm/phase1-overlay/etc/init.d/S48cleankvm-audio-playback-broker
            rm -f buildroot/output/target/etc/init.d/S47cleankvm-audio-capture-broker buildroot/output/target/etc/init.d/S48cleankvm-audio-playback-broker
            capturesrc=/repo/out/phase3a-rust/onyx-kvm-audio-capture-broker
            if [ ! -f "$capturesrc" ]; then capturesrc=/repo/out/phase3a-rust/cleankvm-audio-capture-broker; fi
            playbacksrc=/repo/out/phase3a-rust/onyx-kvm-audio-playback-broker
            if [ ! -f "$playbacksrc" ]; then playbacksrc=/repo/out/phase3a-rust/cleankvm-audio-playback-broker; fi
            install -m 0755 "$capturesrc" \
                cleankvm/phase1-overlay/usr/bin/onyx-kvm-audio-capture-broker
            ln -sf onyx-kvm-audio-capture-broker cleankvm/phase1-overlay/usr/bin/cleankvm-audio-capture-broker
            install -m 0755 "$playbacksrc" \
                cleankvm/phase1-overlay/usr/bin/onyx-kvm-audio-playback-broker
            ln -sf onyx-kvm-audio-playback-broker cleankvm/phase1-overlay/usr/bin/cleankvm-audio-playback-broker
        fi
        if [ -n "$CLEANKVM_RAW_FRAME_PROBE" ]; then
            install -D -m 0500 "$CLEANKVM_RAW_FRAME_PROBE" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-raw-frame-probe
            ln -sf cleankvm-raw-frame-probe \
                cleankvm/phase1-overlay/usr/libexec/onyx-kvm-raw-frame-probe
            grep -qF "/usr/libexec/cleankvm-raw-frame-probe f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-raw-frame-probe f 0500 0 0 - - - - -" \
                    >> cleankvm/phase1-devices.txt
        fi
        if [ -n "$CLEANKVM_AUDIO_CAPTURE_PROBE" ]; then
            install -D -m 0500 "$CLEANKVM_AUDIO_CAPTURE_PROBE" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-audio-capture-probe
            ln -sf cleankvm-audio-capture-probe \
                cleankvm/phase1-overlay/usr/libexec/onyx-kvm-audio-capture-probe
            grep -qF "/usr/libexec/cleankvm-audio-capture-probe f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-audio-capture-probe f 0500 0 0 - - - - -" \
                    >> cleankvm/phase1-devices.txt
        fi
        if [ -n "$CLEANKVM_AUDIO_PLAYBACK_PROBE" ]; then
            install -D -m 0500 "$CLEANKVM_AUDIO_PLAYBACK_PROBE" \
                cleankvm/phase1-overlay/usr/libexec/cleankvm-audio-playback-probe
            ln -sf cleankvm-audio-playback-probe \
                cleankvm/phase1-overlay/usr/libexec/onyx-kvm-audio-playback-probe
            grep -qF "/usr/libexec/cleankvm-audio-playback-probe f 0500 0 0" \
                cleankvm/phase1-devices.txt || \
                printf "%s\n" "/usr/libexec/cleankvm-audio-playback-probe f 0500 0 0 - - - - -" \
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
            "BR2_STRIP_EXCLUDE_FILES=onyx-kvmd onyx-kvm-memtest onyx-kvm-hid-broker onyx-kvm-video-broker onyx-kvm-audio-capture-broker onyx-kvm-audio-playback-broker cleankvmd cleankvm-memtest cleankvm-hid-broker cleankvm-video-broker cleankvm-audio-capture-broker cleankvm-audio-playback-broker"
        # The vendor Buildroot/BusyBox combination does not reliably preserve
        # GNU make jobserver descriptors through its nested configuration
        # makes. A single job is slower but deterministic and fail-closed.
        make -C buildroot -j1 \
            BR2_DL_DIR=/downloads \
            BR2_TOOLCHAIN_EXTERNAL_PATH=/work/sdk/host-tools/gcc/riscv64-linux-musl-x86_64 \
            "BR2_STRIP_EXCLUDE_FILES=onyx-kvmd onyx-kvm-memtest onyx-kvm-hid-broker onyx-kvm-video-broker onyx-kvm-audio-capture-broker onyx-kvm-audio-playback-broker cleankvmd cleankvm-memtest cleankvm-hid-broker cleankvm-video-broker cleankvm-audio-capture-broker cleankvm-audio-playback-broker"
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
        video_broker=
        audio_capture_broker=
        audio_playback_broker=
        daemon_bin=/repo/out/phase2-rust/onyx-kvmd
        if [ ! -f "$daemon_bin" ]; then daemon_bin=/repo/out/phase2-rust/cleankvmd; fi
        memtest_bin=/repo/out/phase2-rust/onyx-kvm-memtest
        if [ ! -f "$memtest_bin" ]; then memtest_bin=/repo/out/phase2-rust/cleankvm-memtest; fi
        if [ "$CLEANKVM_CONFIG_PROFILE" = phase2-hid ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = phase3-discovery ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = phase3-video-foundation ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = phase3a-audio-enumeration ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = product ]; then
            hid_broker=/repo/out/phase2-rust/onyx-kvm-hid-broker
            if [ ! -f "$hid_broker" ]; then hid_broker=/repo/out/phase2-rust/cleankvm-hid-broker; fi
        fi
        if [ "$CLEANKVM_CONFIG_PROFILE" = phase3-video-foundation ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = product ]; then
            video_modules="$CLEANKVM_VIDEO_MODULE_DIR"
            if [ -f /repo/out/phase3-rust/onyx-kvm-video-broker ]; then
                video_broker=/repo/out/phase3-rust/onyx-kvm-video-broker
            elif [ -f /repo/out/phase3-rust/cleankvm-video-broker ]; then
                video_broker=/repo/out/phase3-rust/cleankvm-video-broker
            fi
        fi
        if [ "$CLEANKVM_CONFIG_PROFILE" = phase3a-audio-enumeration ] || \
           [ "$CLEANKVM_CONFIG_PROFILE" = product ]; then
            audio_capture_broker=/repo/out/phase3a-rust/onyx-kvm-audio-capture-broker
            if [ ! -f "$audio_capture_broker" ]; then audio_capture_broker=/repo/out/phase3a-rust/cleankvm-audio-capture-broker; fi
            audio_playback_broker=/repo/out/phase3a-rust/onyx-kvm-audio-playback-broker
            if [ ! -f "$audio_playback_broker" ]; then audio_playback_broker=/repo/out/phase3a-rust/cleankvm-audio-playback-broker; fi
        fi
        /repo/image/audit-rootfs.sh /work/sdk/buildroot/output/target \
            "$daemon_bin" \
            "$memtest_bin" \
            "$CLEANKVM_RECOVERY_PASSWORD_HASH" \
            "$CLEANKVM_IDENTITY_SOURCE" \
            "$hid_broker" \
            "$video_modules" \
            "$CLEANKVM_RAW_FRAME_PROBE" \
            "$CLEANKVM_AUDIO_CAPTURE_PROBE" \
            "$CLEANKVM_EDID_PROGRAMMER" \
            "$CLEANKVM_LT_BACKUP" \
            "$video_broker" \
            "$CLEANKVM_AUDIO_PLAYBACK_PROBE" \
            "$audio_capture_broker" \
            "$audio_playback_broker"
        if [ -n "$CLEANKVM_IDENTITY_SOURCE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            passwd_image=$($debugfs -R "cat /etc/passwd" "$root_image" 2>/dev/null)
            group_image=$($debugfs -R "cat /etc/group" "$root_image" 2>/dev/null)
            key_stat=$($debugfs -R "stat /etc/onyx-kvm/tls-private-key.pkcs8.der" \
                "$root_image" 2>/dev/null)
            if [ -z "$key_stat" ]; then
                key_stat=$($debugfs -R "stat /etc/cleankvm/tls-private-key.pkcs8.der" \
                    "$root_image" 2>/dev/null)
            fi
            printf "%s\n" "$passwd_image" | \
                grep -Eq "^(onyx-kvm|cleankvm):x:900:900:.*:/nonexistent:/sbin/nologin$"
            printf "%s\n" "$group_image" | grep -Eq "^(onyx-kvm|cleankvm):x:900:"
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
        if [ "$CLEANKVM_CONFIG_PROFILE" != phase3a-audio-enumeration ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            ! $debugfs -R "stat /dev/snd/controlC0" "$root_image" 2>&1 | \
                grep -q "Inode:"
            ! $debugfs -R "stat /dev/snd/pcmC0D0c" "$root_image" 2>&1 | \
                grep -q "Inode:"
            printf "%s\n" "finished ext4 non-audio device-node audit passed"
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
        if [ -n "$CLEANKVM_AUDIO_PLAYBACK_PROBE" ]; then
            debugfs=/work/sdk/buildroot/output/host/sbin/debugfs
            root_image=/work/sdk/buildroot/output/images/rootfs.ext4
            probe_stat=$($debugfs -R "stat /usr/libexec/cleankvm-audio-playback-probe" \
                "$root_image" 2>/dev/null)
            printf "%s\n" "$probe_stat" | grep -q "Mode: *0500"
            printf "%s\n" "$probe_stat" | grep -q "User: *0 *Group: *0"
            printf "%s\n" "finished ext4 audio playback probe ownership audit passed"
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
