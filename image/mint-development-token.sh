#!/bin/sh
set -eu

issuer_key=${1:?usage: mint-development-token.sh ISSUER_PRIVATE_KEY ISSUER AUDIENCE OUTPUT_FILE [read-only|hid-control|video-snapshot|audio-capture|audio-output|operator] [TTL_SECONDS]}
issuer=${2:?usage: mint-development-token.sh ISSUER_PRIVATE_KEY ISSUER AUDIENCE OUTPUT_FILE [read-only|hid-control|video-snapshot|audio-capture|audio-output|operator] [TTL_SECONDS]}
audience=${3:?usage: mint-development-token.sh ISSUER_PRIVATE_KEY ISSUER AUDIENCE OUTPUT_FILE [read-only|hid-control|video-snapshot|audio-capture|audio-output|operator] [TTL_SECONDS]}
output_file=${4:?usage: mint-development-token.sh ISSUER_PRIVATE_KEY ISSUER AUDIENCE OUTPUT_FILE [read-only|hid-control|video-snapshot|audio-capture|audio-output|operator] [TTL_SECONDS]}
profile=${5:-read-only}
ttl_seconds=${6:-300}

test -f "$issuer_key"
test ! -e "$output_file"
temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM
umask 077

encode() {
    openssl base64 -A | tr '+/' '-_' | tr -d '='
}

header=$(printf '%s' '{"alg":"EdDSA","typ":"at+jwt"}' | encode)
expiry=$(( $(date +%s) + ttl_seconds ))
case "$profile" in
    read-only)
        scope=kvm:read
        mutation=false
        ;;
    hid-control)
        scope='kvm:read kvm:control'
        mutation=true
        ;;
    video-snapshot)
        scope='kvm:read kvm:video'
        mutation=false
        ;;
    audio-capture)
        scope='kvm:read kvm:audio-capture'
        mutation=false
        ;;
    audio-output)
        scope='kvm:read kvm:audio-output'
        mutation=true
        ;;
    operator|all|full)
        scope='kvm:read kvm:video kvm:audio-capture kvm:audio-output kvm:control'
        mutation=true
        ;;
    *) printf 'unknown development token profile: %s\n' "$profile" >&2; exit 1 ;;
esac
claims=$(printf '{"iss":"%s","aud":"%s","exp":%s,"scope":"%s","onyx_kvm_mutation":%s,"cleankvm_mutation":%s}' \
    "$issuer" "$audience" "$expiry" "$scope" "$mutation" "$mutation" | encode)
signing_input="$header.$claims"
printf '%s' "$signing_input" >"$temporary_dir/signing-input"
signature=$(openssl pkeyutl -sign -rawin -inkey "$issuer_key" \
    -in "$temporary_dir/signing-input" | encode)
printf '%s.%s\n' "$signing_input" "$signature" >"$output_file"
chmod 0600 "$output_file"
unset header claims expiry signature signing_input scope mutation

printf 'minted owner-only five-minute development token: %s\n' "$output_file"
