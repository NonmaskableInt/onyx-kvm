#!/bin/sh
set -eu

rootfs=${1:?usage: audit-rootfs.sh ROOTFS_DIRECTORY EXPECTED_DAEMON EXPECTED_MEMTEST}
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
expected_daemon=${2:?usage: audit-rootfs.sh ROOTFS_DIRECTORY EXPECTED_DAEMON EXPECTED_MEMTEST}
expected_memtest=${3:?usage: audit-rootfs.sh ROOTFS_DIRECTORY EXPECTED_DAEMON EXPECTED_MEMTEST}
expected_password_hash=${4:-!}
expected_identity=${5:-}
expected_hid_broker=${6:-}
expected_video_modules=${7:-}
expected_raw_frame_probe=${8:-}
expected_audio_capture_probe=${9:-}
expected_edid_programmer=${10:-}
expected_lt_backup=${11:-}
expected_video_broker=${12:-}
expected_audio_playback_probe=${13:-}
expected_audio_capture_broker=${14:-}
expected_audio_playback_broker=${15:-}
test -d "$rootfs" || {
    printf 'not a root filesystem directory: %s\n' "$rootfs" >&2
    exit 1
}

for path in \
    usr/sbin/sshd usr/sbin/dropbear usr/sbin/avahi-daemon \
    usr/sbin/dnsmasq usr/sbin/udhcpd usr/sbin/wpa_supplicant \
    usr/bin/mender usr/bin/mosh usr/bin/rauc usr/bin/swupdate \
    usr/bin/ffmpeg usr/bin/gst-launch-1.0 usr/bin/arecord usr/bin/aplay \
    usr/bin/onyx-kvm-hid-hold-test usr/bin/cleankvm-hid-hold-test \
    usr/bin/onyx-kvm-media-broker usr/bin/cleankvm-media-broker
do
    if test -e "$rootfs/$path"; then
        printf 'prohibited Phase-1 executable: /%s\n' "$path" >&2
        exit 1
    fi
done

for audio_broker in audio-capture-broker audio-playback-broker; do
    expected=
    case "$audio_broker" in
        audio-capture-broker) expected=$expected_audio_capture_broker ;;
        audio-playback-broker) expected=$expected_audio_playback_broker ;;
    esac
    broker_bin=
    if test -f "$rootfs/usr/bin/onyx-kvm-$audio_broker"; then
        broker_bin="$rootfs/usr/bin/onyx-kvm-$audio_broker"
    elif test -f "$rootfs/usr/bin/cleankvm-$audio_broker"; then
        broker_bin="$rootfs/usr/bin/cleankvm-$audio_broker"
    fi
    if test -z "$expected"; then
        test -z "$broker_bin" || {
            printf 'unexpected audio broker: %s\n' "$broker_bin" >&2
            exit 1
        }
    else
        test -n "$broker_bin" && test -s "$broker_bin"
        cmp "$expected" "$broker_bin"
        case "$(file "$broker_bin")" in
            *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
            *) printf 'installed %s is not a static stripped RISC-V ELF\n' "$broker_bin" >&2; exit 1 ;;
        esac
    fi
done

if test -n "$expected_audio_capture_broker"; then
    if test -f "$rootfs/etc/init.d/S47onyx-kvm-audio-capture-broker"; then
        test -x "$rootfs/etc/init.d/S47onyx-kvm-audio-capture-broker"
        cmp "$repo_root/image/phase3a-audio-overlay/etc/init.d/S47onyx-kvm-audio-capture-broker" \
            "$rootfs/etc/init.d/S47onyx-kvm-audio-capture-broker"
    else
        test -x "$rootfs/etc/init.d/S47cleankvm-audio-capture-broker"
        cmp "$repo_root/image/phase3a-audio-overlay/etc/init.d/S47cleankvm-audio-capture-broker" \
            "$rootfs/etc/init.d/S47cleankvm-audio-capture-broker"
    fi
    test ! -e "$rootfs/run/onyx-kvm/audio-capture-enabled"
    test ! -e "$rootfs/run/cleankvm/audio-capture-enabled"
fi
if test -n "$expected_audio_playback_broker"; then
    if test -f "$rootfs/etc/init.d/S48onyx-kvm-audio-playback-broker"; then
        test -x "$rootfs/etc/init.d/S48onyx-kvm-audio-playback-broker"
        cmp "$repo_root/image/phase3a-audio-overlay/etc/init.d/S48onyx-kvm-audio-playback-broker" \
            "$rootfs/etc/init.d/S48onyx-kvm-audio-playback-broker"
    else
        test -x "$rootfs/etc/init.d/S48cleankvm-audio-playback-broker"
        cmp "$repo_root/image/phase3a-audio-overlay/etc/init.d/S48cleankvm-audio-playback-broker" \
            "$rootfs/etc/init.d/S48cleankvm-audio-playback-broker"
    fi
    test ! -e "$rootfs/run/onyx-kvm/audio-output-enabled"
    test ! -e "$rootfs/run/cleankvm/audio-output-enabled"
fi

if test -z "$expected_video_broker"; then
    if test -e "$rootfs/usr/bin/onyx-kvm-video-broker" || test -e "$rootfs/usr/bin/cleankvm-video-broker"; then
        printf '%s\n' 'prohibited Phase-1 executable: video broker present' >&2
        exit 1
    fi
fi

test -f "$rootfs/etc/fstab" || {
    printf '%s\n' 'missing /etc/fstab configuration' >&2
    exit 1
}
grep -qE '^/dev/root[[:space:]]+/[[:space:]]+ext[24][[:space:]]+[^[:space:]]*ro' "$rootfs/etc/fstab" || {
    printf '%s\n' '/etc/fstab does not mount / as read-only (ro)' >&2
    exit 1
}
grep -qE '^tmpfs[[:space:]]+/tmp[[:space:]]+tmpfs[[:space:]]+[^[:space:]]*nosuid' "$rootfs/etc/fstab" || {
    printf '%s\n' '/etc/fstab does not mount /tmp on tmpfs with nosuid' >&2
    exit 1
}
grep -qE '^tmpfs[[:space:]]+/run[[:space:]]+tmpfs[[:space:]]+[^[:space:]]*nosuid' "$rootfs/etc/fstab" || {
    printf '%s\n' '/etc/fstab does not mount /run on tmpfs with nosuid' >&2
    exit 1
}

test -f "$rootfs/etc/network/interfaces" || {
    printf '%s\n' 'missing wired network configuration' >&2
    exit 1
}
grep -q '^iface eth0 inet dhcp$' "$rootfs/etc/network/interfaces" || {
    printf '%s\n' 'eth0 is not configured as a DHCP client' >&2
    exit 1
}
carrier_helper=
if test -f "$rootfs/usr/sbin/onyx-kvm-wait-carrier"; then
    carrier_helper="$rootfs/usr/sbin/onyx-kvm-wait-carrier"
elif test -f "$rootfs/usr/sbin/cleankvm-wait-carrier"; then
    carrier_helper="$rootfs/usr/sbin/cleankvm-wait-carrier"
fi
test -n "$carrier_helper" && test -x "$carrier_helper" || {
    printf '%s\n' 'missing bounded Ethernet carrier wait helper' >&2
    exit 1
}
grep -qE 'pre-up /usr/sbin/(onyx-kvm|cleankvm)-wait-carrier eth0 10' \
    "$rootfs/etc/network/interfaces" || {
    printf '%s\n' 'missing bounded pre-DHCP carrier wait' >&2
    exit 1
}
test -x "$rootfs/etc/init.d/S05ro-root" || {
    printf '%s\n' 'missing read-only root remount init script' >&2
    exit 1
}

test ! -e "$rootfs/etc/init.d/S03onyx-kvm-enroll" || {
    printf '%s\n' 'runtime recovery enrollment must not be present' >&2
    exit 1
}
test ! -e "$rootfs/etc/init.d/S03cleankvm-enroll" || {
    printf '%s\n' 'runtime recovery enrollment must not be present' >&2
    exit 1
}
if test -n "$expected_identity"; then
    init_daemon=
    if test -f "$rootfs/etc/init.d/S50onyx-kvmd"; then
        init_daemon="$rootfs/etc/init.d/S50onyx-kvmd"
    elif test -f "$rootfs/etc/init.d/S50cleankvmd"; then
        init_daemon="$rootfs/etc/init.d/S50cleankvmd"
    fi
    test -n "$init_daemon" && test -x "$init_daemon" || {
        printf '%s\n' 'missing authenticated HTTPS service' >&2
        exit 1
    }
    grep -qF 'start-stop-daemon -S -b -m -p "$pidfile" -c 900:900 -x "$daemon"' \
        "$init_daemon" || {
        printf '%s\n' 'HTTPS service does not drop to uid/gid 900' >&2
        exit 1
    }
    identity_dir=
    if test -d "$rootfs/etc/onyx-kvm"; then
        identity_dir="$rootfs/etc/onyx-kvm"
    elif test -d "$rootfs/etc/cleankvm"; then
        identity_dir="$rootfs/etc/cleankvm"
    fi
    test -n "$identity_dir"
    test "$(wc -c <"$identity_dir/oauth-issuer-public-key.raw" | tr -d ' ')" -eq 32
    cmp "$expected_identity/tls-certificate.der" "$identity_dir/tls-certificate.der"
    cmp "$expected_identity/tls-private-key.pkcs8.der" \
        "$identity_dir/tls-private-key.pkcs8.der"
    cmp "$expected_identity/oauth-issuer-public-key.raw" \
        "$identity_dir/oauth-issuer-public-key.raw"
    if test -f "$expected_identity/oauth-issuer-signing-key.raw"; then
        test "$(wc -c <"$identity_dir/oauth-issuer-signing-key.raw" | tr -d ' ')" -eq 32
        cmp "$expected_identity/oauth-issuer-signing-key.raw" \
            "$identity_dir/oauth-issuer-signing-key.raw"
    fi
fi
case "$expected_password_hash" in
    '!'|'$6$'*) ;;
    *) printf '%s\n' 'invalid expected recovery password hash' >&2; exit 1 ;;
esac
test "$(awk -F: '$1 == "root" { print $2 }' "$rootfs/etc/shadow")" = "$expected_password_hash" || {
    printf '%s\n' 'root credential does not match the provisioned hash' >&2
    exit 1
}

installed_daemon=
if test -f "$rootfs/usr/bin/onyx-kvmd"; then
    installed_daemon="$rootfs/usr/bin/onyx-kvmd"
elif test -f "$rootfs/usr/bin/cleankvmd"; then
    installed_daemon="$rootfs/usr/bin/cleankvmd"
fi
test -n "$installed_daemon" && test -x "$installed_daemon" || {
    printf '%s\n' 'missing executable daemon' >&2
    exit 1
}
test -s "$installed_daemon" || {
    printf '%s\n' 'installed daemon is empty' >&2
    exit 1
}
cmp "$expected_daemon" "$installed_daemon" || {
    printf '%s\n' 'installed daemon does not match the verified build' >&2
    exit 1
}
case "$(file "$installed_daemon")" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf '%s\n' 'installed daemon is not a static stripped RISC-V ELF' >&2
        exit 1
        ;;
esac

installed_memtest=
if test -f "$rootfs/usr/bin/onyx-kvm-memtest"; then
    installed_memtest="$rootfs/usr/bin/onyx-kvm-memtest"
elif test -f "$rootfs/usr/bin/cleankvm-memtest"; then
    installed_memtest="$rootfs/usr/bin/cleankvm-memtest"
fi
test -n "$installed_memtest" && test -s "$installed_memtest" || {
    printf '%s\n' 'missing or empty memtest' >&2
    exit 1
}
cmp "$expected_memtest" "$installed_memtest" || {
    printf '%s\n' 'installed memtest does not match the verified build' >&2
    exit 1
}
case "$(file "$installed_memtest")" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf '%s\n' 'installed memtest is not a static stripped RISC-V ELF' >&2
        exit 1
        ;;
esac

if test -n "$expected_hid_broker"; then
    installed_hid=
    if test -f "$rootfs/usr/bin/onyx-kvm-hid-broker"; then
        installed_hid="$rootfs/usr/bin/onyx-kvm-hid-broker"
    elif test -f "$rootfs/usr/bin/cleankvm-hid-broker"; then
        installed_hid="$rootfs/usr/bin/cleankvm-hid-broker"
    fi
    test -n "$installed_hid" && test -s "$installed_hid" || {
        printf '%s\n' 'missing or empty hid broker' >&2
        exit 1
    }
    cmp "$expected_hid_broker" "$installed_hid" || {
        printf '%s\n' 'installed HID broker does not match the verified build' >&2
        exit 1
    }
    case "$(file "$installed_hid")" in
        *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
        *) printf '%s\n' 'installed HID broker is not a static stripped RISC-V ELF' >&2; exit 1 ;;
    esac
    if test -f "$rootfs/etc/init.d/S45onyx-kvm-hid-gadget"; then
        test -x "$rootfs/etc/init.d/S45onyx-kvm-hid-gadget"
        cmp "$repo_root/image/phase2-hid-overlay/etc/init.d/S45onyx-kvm-hid-gadget" \
            "$rootfs/etc/init.d/S45onyx-kvm-hid-gadget"
    else
        test -x "$rootfs/etc/init.d/S45cleankvm-hid-gadget"
        cmp "$repo_root/image/phase2-hid-overlay/etc/init.d/S45cleankvm-hid-gadget" \
            "$rootfs/etc/init.d/S45cleankvm-hid-gadget"
    fi
    if test -f "$rootfs/etc/init.d/S46onyx-kvm-hid-broker"; then
        test -x "$rootfs/etc/init.d/S46onyx-kvm-hid-broker"
        cmp "$repo_root/image/phase2-hid-overlay/etc/init.d/S46onyx-kvm-hid-broker" \
            "$rootfs/etc/init.d/S46onyx-kvm-hid-broker"
    else
        test -x "$rootfs/etc/init.d/S46cleankvm-hid-broker"
        cmp "$repo_root/image/phase2-hid-overlay/etc/init.d/S46cleankvm-hid-broker" \
            "$rootfs/etc/init.d/S46cleankvm-hid-broker"
    fi
    test ! -e "$rootfs/run/onyx-kvm/hid-enabled"
    test ! -e "$rootfs/run/cleankvm/hid-enabled"
fi

if test -n "$expected_video_modules"; then
    installed_video=
    if test -d "$rootfs/usr/lib/onyx-kvm/video-quarantine"; then
        installed_video="$rootfs/usr/lib/onyx-kvm/video-quarantine"
    elif test -d "$rootfs/usr/lib/cleankvm/video-quarantine"; then
        installed_video="$rootfs/usr/lib/cleankvm/video-quarantine"
    fi
    test -n "$installed_video" && test -d "$installed_video"
    set -- "$installed_video"/*.ko
    if test "$#" -eq 9; then
        for name in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss soph_jpeg mars_vcodec cvi_vc_driver; do
            test -f "$installed_video/$name.ko"
            cmp "$expected_video_modules/$name.ko" "$installed_video/$name.ko"
            test "$(stat -c %a "$installed_video/$name.ko")" = 400
        done
        if test -f "$expected_video_modules/coda980.bin"; then
            test -f "$rootfs/usr/share/fw_vcodec/coda980.bin"
            cmp "$expected_video_modules/coda980.bin" "$rootfs/usr/share/fw_vcodec/coda980.bin"
            test "$(stat -c %a "$rootfs/usr/share/fw_vcodec/coda980.bin")" = 400 -o "$(stat -c %a "$rootfs/usr/share/fw_vcodec/coda980.bin")" = 444 -o "$(stat -c %a "$rootfs/usr/share/fw_vcodec/coda980.bin")" = 644
        fi
    elif test "$#" -eq 6; then
        for name in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss; do
            test -f "$installed_video/$name.ko"
            cmp "$expected_video_modules/$name.ko" "$installed_video/$name.ko"
            test "$(stat -c %a "$installed_video/$name.ko")" = 400
        done
    else
        printf 'video quarantine must contain six or nine modules, found %d\n' "$#" >&2
        exit 1
    fi
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'video-quarantine\|soph_.*\.ko' '{}' + | grep -q .
fi

if test -n "$expected_raw_frame_probe"; then
    installed_probe=
    if test -f "$rootfs/usr/libexec/onyx-kvm-raw-frame-probe"; then
        installed_probe="$rootfs/usr/libexec/onyx-kvm-raw-frame-probe"
    elif test -f "$rootfs/usr/libexec/cleankvm-raw-frame-probe"; then
        installed_probe="$rootfs/usr/libexec/cleankvm-raw-frame-probe"
    fi
    test -n "$installed_probe" && test -f "$installed_probe"
    cmp "$expected_raw_frame_probe" "$installed_probe"
    ! find "$rootfs/etc/init.d" -type f -exec grep -lE '(onyx-kvm|cleankvm)-raw-frame-probe' '{}' + | grep -q .
fi

if test -n "$expected_audio_capture_probe"; then
    installed_probe=
    if test -f "$rootfs/usr/libexec/onyx-kvm-audio-capture-probe"; then
        installed_probe="$rootfs/usr/libexec/onyx-kvm-audio-capture-probe"
    elif test -f "$rootfs/usr/libexec/cleankvm-audio-capture-probe"; then
        installed_probe="$rootfs/usr/libexec/cleankvm-audio-capture-probe"
    fi
    test -n "$installed_probe" && test -f "$installed_probe"
    cmp "$expected_audio_capture_probe" "$installed_probe"
    ! find "$rootfs/etc/init.d" -type f -exec grep -lE '(onyx-kvm|cleankvm)-audio-capture-probe' '{}' + | grep -q .
fi

if test -n "$expected_audio_playback_probe"; then
    installed_probe=
    if test -f "$rootfs/usr/libexec/onyx-kvm-audio-playback-probe"; then
        installed_probe="$rootfs/usr/libexec/onyx-kvm-audio-playback-probe"
    elif test -f "$rootfs/usr/libexec/cleankvm-audio-playback-probe"; then
        installed_probe="$rootfs/usr/libexec/cleankvm-audio-playback-probe"
    fi
    test -n "$installed_probe" && test -f "$installed_probe"
    cmp "$expected_audio_playback_probe" "$installed_probe"
    ! find "$rootfs/etc/init.d" -type f -exec grep -lE '(onyx-kvm|cleankvm)-audio-playback-probe' '{}' + | grep -q .
fi

if test -n "$expected_edid_programmer"; then
    installed_probe=
    if test -f "$rootfs/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer"; then
        installed_probe="$rootfs/usr/libexec/onyx-kvm-lt6911uxc-edid-programmer"
    elif test -f "$rootfs/usr/libexec/cleankvm-lt6911uxc-edid-programmer"; then
        installed_probe="$rootfs/usr/libexec/cleankvm-lt6911uxc-edid-programmer"
    fi
    test -n "$installed_probe" && test -f "$installed_probe"
    cmp "$expected_edid_programmer" "$installed_probe"
    test "$(stat -c %a "$installed_probe")" = 500
    ! find "$rootfs/etc/init.d" -type f -exec grep -lE '(onyx-kvm|cleankvm)-lt6911uxc-edid-programmer' '{}' + | grep -q .
fi

if test -n "$expected_lt_backup"; then
    installed_probe=
    if test -f "$rootfs/usr/libexec/onyx-kvm-lt6911uxc-flash-backup"; then
        installed_probe="$rootfs/usr/libexec/onyx-kvm-lt6911uxc-flash-backup"
    elif test -f "$rootfs/usr/libexec/cleankvm-lt6911uxc-flash-backup"; then
        installed_probe="$rootfs/usr/libexec/cleankvm-lt6911uxc-flash-backup"
    fi
    test -n "$installed_probe" && test -f "$installed_probe"
    cmp "$expected_lt_backup" "$installed_probe"
    test "$(stat -c %a "$installed_probe")" = 500
    ! find "$rootfs/etc/init.d" -type f -exec grep -lE '(onyx-kvm|cleankvm)-lt6911uxc-flash-backup' '{}' + | grep -q .
fi

if test -n "$expected_video_broker"; then
    installed_video_broker=
    if test -f "$rootfs/usr/bin/onyx-kvm-video-broker"; then
        installed_video_broker="$rootfs/usr/bin/onyx-kvm-video-broker"
    elif test -f "$rootfs/usr/bin/cleankvm-video-broker"; then
        installed_video_broker="$rootfs/usr/bin/cleankvm-video-broker"
    fi
    test -n "$installed_video_broker" && test -s "$installed_video_broker" || {
        printf '%s\n' 'missing or empty video broker' >&2
        exit 1
    }
    cmp "$expected_video_broker" "$installed_video_broker" || {
        printf '%s\n' 'installed video broker does not match the verified build' >&2
        exit 1
    }
    case "$(file "$installed_video_broker")" in
        *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
        *) printf '%s\n' 'installed video broker is not a static stripped RISC-V ELF' >&2; exit 1 ;;
    esac
    if test -f "$rootfs/etc/init.d/S47onyx-kvm-video-broker"; then
        test -x "$rootfs/etc/init.d/S47onyx-kvm-video-broker"
        cmp "$repo_root/image/phase3-video-overlay/etc/init.d/S47onyx-kvm-video-broker" \
            "$rootfs/etc/init.d/S47onyx-kvm-video-broker"
    else
        test -x "$rootfs/etc/init.d/S47cleankvm-video-broker"
        cmp "$repo_root/image/phase3-video-overlay/etc/init.d/S47cleankvm-video-broker" \
            "$rootfs/etc/init.d/S47cleankvm-video-broker"
    fi
    test ! -e "$rootfs/run/onyx-kvm/video-enabled"
    test ! -e "$rootfs/run/cleankvm/video-enabled"
fi

printf '%s\n' 'Phase-1 root filesystem audit passed'
