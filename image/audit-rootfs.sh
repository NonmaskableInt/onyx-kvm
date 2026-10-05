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
test -d "$rootfs" || {
    printf 'not a root filesystem directory: %s\n' "$rootfs" >&2
    exit 1
}

for path in \
    usr/sbin/sshd usr/sbin/dropbear usr/sbin/avahi-daemon \
    usr/sbin/dnsmasq usr/sbin/udhcpd usr/sbin/wpa_supplicant \
    usr/bin/mender usr/bin/mosh usr/bin/rauc usr/bin/swupdate \
    usr/bin/ffmpeg usr/bin/gst-launch-1.0 usr/bin/arecord usr/bin/aplay \
    usr/bin/cleankvm-hid-hold-test \
    usr/bin/cleankvm-media-broker usr/bin/cleankvm-audio-capture-broker \
    usr/bin/cleankvm-audio-playback-broker
do
    if test -e "$rootfs/$path"; then
        printf 'prohibited Phase-1 executable: /%s\n' "$path" >&2
        exit 1
    fi
done

if test -z "$expected_video_broker" && test -e "$rootfs/usr/bin/cleankvm-video-broker"; then
    printf '%s\n' 'prohibited Phase-1 executable: /usr/bin/cleankvm-video-broker' >&2
    exit 1
fi

test -f "$rootfs/etc/network/interfaces" || {
    printf '%s\n' 'missing wired network configuration' >&2
    exit 1
}
grep -q '^iface eth0 inet dhcp$' "$rootfs/etc/network/interfaces" || {
    printf '%s\n' 'eth0 is not configured as a DHCP client' >&2
    exit 1
}
test -x "$rootfs/usr/sbin/cleankvm-wait-carrier" || {
    printf '%s\n' 'missing bounded Ethernet carrier wait helper' >&2
    exit 1
}
grep -qF 'pre-up /usr/sbin/cleankvm-wait-carrier eth0 10' \
    "$rootfs/etc/network/interfaces" || {
    printf '%s\n' 'missing bounded pre-DHCP carrier wait' >&2
    exit 1
}

test ! -e "$rootfs/etc/init.d/S03cleankvm-enroll" || {
    printf '%s\n' 'runtime recovery enrollment must not be present' >&2
    exit 1
}
if test -n "$expected_identity"; then
    test -x "$rootfs/etc/init.d/S50cleankvmd" || {
        printf '%s\n' 'missing authenticated HTTPS service' >&2
        exit 1
    }
    grep -qF 'start-stop-daemon -S -b -m -p "$pidfile" -c 900:900 -x "$daemon"' \
        "$rootfs/etc/init.d/S50cleankvmd" || {
        printf '%s\n' 'HTTPS service does not drop to uid/gid 900' >&2
        exit 1
    }
    test "$(wc -c <"$rootfs/etc/cleankvm/oauth-issuer-public-key.raw" | tr -d ' ')" -eq 32
    cmp "$expected_identity/tls-certificate.der" "$rootfs/etc/cleankvm/tls-certificate.der"
    cmp "$expected_identity/tls-private-key.pkcs8.der" \
        "$rootfs/etc/cleankvm/tls-private-key.pkcs8.der"
    cmp "$expected_identity/oauth-issuer-public-key.raw" \
        "$rootfs/etc/cleankvm/oauth-issuer-public-key.raw"
fi
case "$expected_password_hash" in
    '!'|'$6$'*) ;;
    *) printf '%s\n' 'invalid expected recovery password hash' >&2; exit 1 ;;
esac
test "$(awk -F: '$1 == "root" { print $2 }' "$rootfs/etc/shadow")" = "$expected_password_hash" || {
    printf '%s\n' 'root credential does not match the provisioned hash' >&2
    exit 1
}

test -x "$rootfs/usr/bin/cleankvmd" || {
    printf '%s\n' 'missing executable /usr/bin/cleankvmd' >&2
    exit 1
}
test -s "$rootfs/usr/bin/cleankvmd" || {
    printf '%s\n' 'installed /usr/bin/cleankvmd is empty' >&2
    exit 1
}
cmp "$expected_daemon" "$rootfs/usr/bin/cleankvmd" || {
    printf '%s\n' 'installed cleankvmd does not match the verified build' >&2
    exit 1
}
case "$(file "$rootfs/usr/bin/cleankvmd")" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf '%s\n' 'installed cleankvmd is not a static stripped RISC-V ELF' >&2
        exit 1
        ;;
esac

test -s "$rootfs/usr/bin/cleankvm-memtest" || {
    printf '%s\n' 'missing or empty /usr/bin/cleankvm-memtest' >&2
    exit 1
}
cmp "$expected_memtest" "$rootfs/usr/bin/cleankvm-memtest" || {
    printf '%s\n' 'installed cleankvm-memtest does not match the verified build' >&2
    exit 1
}
case "$(file "$rootfs/usr/bin/cleankvm-memtest")" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf '%s\n' 'installed cleankvm-memtest is not a static stripped RISC-V ELF' >&2
        exit 1
        ;;
esac

if test -n "$expected_hid_broker"; then
    test -s "$rootfs/usr/bin/cleankvm-hid-broker" || {
        printf '%s\n' 'missing or empty /usr/bin/cleankvm-hid-broker' >&2
        exit 1
    }
    cmp "$expected_hid_broker" "$rootfs/usr/bin/cleankvm-hid-broker" || {
        printf '%s\n' 'installed HID broker does not match the verified build' >&2
        exit 1
    }
    case "$(file "$rootfs/usr/bin/cleankvm-hid-broker")" in
        *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
        *) printf '%s\n' 'installed HID broker is not a static stripped RISC-V ELF' >&2; exit 1 ;;
    esac
    test -x "$rootfs/etc/init.d/S45cleankvm-hid-gadget"
    test -x "$rootfs/etc/init.d/S46cleankvm-hid-broker"
    cmp "$repo_root/image/phase2-hid-overlay/etc/init.d/S45cleankvm-hid-gadget" \
        "$rootfs/etc/init.d/S45cleankvm-hid-gadget" || {
        printf '%s\n' 'installed HID gadget init script does not match source overlay' >&2
        exit 1
    }
    cmp "$repo_root/image/phase2-hid-overlay/etc/init.d/S46cleankvm-hid-broker" \
        "$rootfs/etc/init.d/S46cleankvm-hid-broker" || {
        printf '%s\n' 'installed HID broker init script does not match source overlay' >&2
        exit 1
    }
    test ! -e "$rootfs/run/cleankvm/hid-enabled"
fi

if test -n "$expected_video_modules"; then
    installed_video="$rootfs/usr/lib/cleankvm/video-quarantine"
    test -d "$installed_video"
    set -- "$installed_video"/*.ko
    test "$#" -eq 6 || {
        printf '%s\n' 'video quarantine must contain exactly six modules' >&2
        exit 1
    }
    for name in soph_sys soph_base soph_mipi_rx soph_snsr_i2c soph_vi soph_vpss; do
        test -f "$installed_video/$name.ko"
        cmp "$expected_video_modules/$name.ko" "$installed_video/$name.ko"
        test "$(stat -c %a "$installed_video/$name.ko")" = 400
    done
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'video-quarantine\|soph_.*\.ko' '{}' + | grep -q .
fi

if test -n "$expected_raw_frame_probe"; then
    installed_probe="$rootfs/usr/libexec/cleankvm-raw-frame-probe"
    test -f "$installed_probe"
    cmp "$expected_raw_frame_probe" "$installed_probe"
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'cleankvm-raw-frame-probe' '{}' + | grep -q .
fi

if test -n "$expected_audio_capture_probe"; then
    installed_probe="$rootfs/usr/libexec/cleankvm-audio-capture-probe"
    test -f "$installed_probe"
    cmp "$expected_audio_capture_probe" "$installed_probe"
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'cleankvm-audio-capture-probe' '{}' + | grep -q .
fi

if test -n "$expected_audio_playback_probe"; then
    installed_probe="$rootfs/usr/libexec/cleankvm-audio-playback-probe"
    test -f "$installed_probe"
    cmp "$expected_audio_playback_probe" "$installed_probe"
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'cleankvm-audio-playback-probe' '{}' + | grep -q .
fi

if test -n "$expected_edid_programmer"; then
    installed_probe="$rootfs/usr/libexec/cleankvm-lt6911uxc-edid-programmer"
    test -f "$installed_probe"
    cmp "$expected_edid_programmer" "$installed_probe"
    test "$(stat -c %a "$installed_probe")" = 500
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'cleankvm-lt6911uxc-edid-programmer' '{}' + | grep -q .
fi

if test -n "$expected_lt_backup"; then
    installed_probe="$rootfs/usr/libexec/cleankvm-lt6911uxc-flash-backup"
    test -f "$installed_probe"
    cmp "$expected_lt_backup" "$installed_probe"
    test "$(stat -c %a "$installed_probe")" = 500
    ! find "$rootfs/etc/init.d" -type f -exec grep -l 'cleankvm-lt6911uxc-flash-backup' '{}' + | grep -q .
fi

if test -n "$expected_video_broker"; then
    test -s "$rootfs/usr/bin/cleankvm-video-broker" || {
        printf '%s\n' 'missing or empty /usr/bin/cleankvm-video-broker' >&2
        exit 1
    }
    cmp "$expected_video_broker" "$rootfs/usr/bin/cleankvm-video-broker" || {
        printf '%s\n' 'installed video broker does not match the verified build' >&2
        exit 1
    }
    case "$(file "$rootfs/usr/bin/cleankvm-video-broker")" in
        *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
        *) printf '%s\n' 'installed video broker is not a static stripped RISC-V ELF' >&2; exit 1 ;;
    esac
    test -x "$rootfs/etc/init.d/S47cleankvm-video-broker"
    cmp "$repo_root/image/phase3-video-overlay/etc/init.d/S47cleankvm-video-broker" \
        "$rootfs/etc/init.d/S47cleankvm-video-broker" || {
        printf '%s\n' 'installed video broker init script does not match source overlay' >&2
        exit 1
    }
    test ! -e "$rootfs/run/cleankvm/video-enabled"
fi

printf '%s\n' 'Phase-1 root filesystem audit passed'
