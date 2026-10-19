#!/bin/sh
set -eu

target_dir=${1:?Buildroot target directory is required}
shadow="$target_dir/etc/shadow"
temporary="$target_dir/etc/.shadow.onyx-kvm"

test -f "$shadow"
password_hash=${ONYX_KVM_RECOVERY_PASSWORD_HASH:-!}
case "$password_hash" in
    '!') ;;
    '$6$'*) ;;
    *)
        printf '%s\n' 'invalid Onyx KVM recovery password hash' >&2
        exit 1
        ;;
esac
awk -F: -v OFS=: -v password_hash="$password_hash" \
    '$1 == "root" { $2 = password_hash } { print }' \
    "$shadow" >"$temporary"
chmod 0600 "$temporary"
mv "$temporary" "$shadow"

identity_source=${ONYX_KVM_IDENTITY_SOURCE:-}
if test -n "$identity_source"; then
    test -f "$identity_source/tls-certificate.der"
    test -f "$identity_source/tls-private-key.pkcs8.der"
    test -f "$identity_source/oauth-issuer-public-key.raw"
    test "$(wc -c <"$identity_source/oauth-issuer-public-key.raw" | tr -d ' ')" -eq 32

    dir="$target_dir/etc/onyx-kvm"
    rm -rf "$dir"
    install -d -m 0750 "$dir"
    install -m 0644 "$identity_source/tls-certificate.der" \
        "$dir/tls-certificate.der"
    install -m 0640 "$identity_source/tls-private-key.pkcs8.der" \
        "$dir/tls-private-key.pkcs8.der"
    install -m 0644 "$identity_source/oauth-issuer-public-key.raw" \
        "$dir/oauth-issuer-public-key.raw"
    if test -f "$identity_source/oauth-issuer-signing-key.raw"; then
        install -m 0640 "$identity_source/oauth-issuer-signing-key.raw" \
            "$dir/oauth-issuer-signing-key.raw"
    fi
fi

dir="$target_dir/usr/lib/onyx-kvm/video-quarantine"
if test -d "$dir"; then
    set -- "$dir"/*.ko
    test "$#" -eq 6 -o "$#" -eq 9
    chmod 0400 "$@"
fi

if test -f "$target_dir/usr/share/fw_vcodec/coda980.bin"; then
    chmod 0444 "$target_dir/usr/share/fw_vcodec/coda980.bin"
fi
