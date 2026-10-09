#!/bin/sh
set -eu

if [ "$#" -lt 2 ]; then
    printf 'usage: verify-update.sh BUNDLE_DIR PUBLIC_KEY_PATH\n' >&2
    exit 1
fi

bundle_dir="$1"
public_key="$2"

manifest="$bundle_dir/manifest.json"
signature="$bundle_dir/signature.bin"
payload="$bundle_dir/payload.ext4"

test -d "$bundle_dir" || { printf 'error: bundle dir not found: %s\n' "$bundle_dir" >&2; exit 2; }
test -f "$manifest" || { printf 'error: manifest not found: %s\n' "$manifest" >&2; exit 2; }
test -f "$signature" || { printf 'error: signature not found: %s\n' "$signature" >&2; exit 2; }
test -f "$payload" || { printf 'error: payload not found: %s\n' "$payload" >&2; exit 2; }
test -f "$public_key" || { printf 'error: public key not found: %s\n' "$public_key" >&2; exit 2; }

# Parse and validate product
product=$(grep '"product":' "$manifest" | sed 's/.*"product":[ ]*"\([^"]*\)".*/\1/')
test "$product" = "onyx-kvm" || test "$product" = "cleankvm" || {
    printf 'error: invalid product in manifest: %s\n' "$product" >&2
    exit 3
}

# Determine public key format (PEM or raw 32-byte Ed25519 key)
key_size=$(wc -c <"$public_key" | tr -d ' ')
temp_der=
if [ "$key_size" -eq 32 ]; then
    temp_der="$bundle_dir/.temp_pubkey_$$.der"
    trap 'rm -f "$temp_der"' EXIT HUP INT TERM
    # Reconstruct ASN.1 DER header (12 bytes: 302a300506032b6570032100) + 32-byte raw key
    printf '\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00' > "$temp_der"
    cat "$public_key" >> "$temp_der"
    verify_opts="-keyform DER -inkey $temp_der"
else
    verify_opts="-inkey $public_key"
fi

# Verify signature
openssl pkeyutl -verify -rawin -pubin $verify_opts -in "$manifest" -sigfile "$signature" >/dev/null || {
    printf 'error: cryptographic signature verification failed\n' >&2
    exit 4
}

# Verify payload size
expected_size=$(grep '"payload_size":' "$manifest" | tr -dc '0-9')
actual_size=$(wc -c <"$payload" | tr -d ' ')
test "$expected_size" -eq "$actual_size" || {
    printf 'error: payload size mismatch: expected %s, got %s\n' "$expected_size" "$actual_size" >&2
    exit 5
}

# Verify payload SHA-256
expected_sha256=$(grep '"payload_sha256":' "$manifest" | sed 's/.*"payload_sha256":[ ]*"\([^"]*\)".*/\1/')
actual_sha256=$(shasum -a 256 "$payload" | awk '{print $1}')
test "$expected_sha256" = "$actual_sha256" || {
    printf 'error: payload SHA-256 digest mismatch: expected %s, got %s\n' "$expected_sha256" "$actual_sha256" >&2
    exit 6
}

# If SHA256SUMS is present, verify it too
if [ -f "$bundle_dir/SHA256SUMS" ]; then
    (cd "$bundle_dir" && shasum -a 256 -c SHA256SUMS >/dev/null 2>&1) || {
        printf 'error: SHA256SUMS check failed\n' >&2
        exit 7
    }
fi

version=$(grep '"version":' "$manifest" | sed 's/.*"version":[ ]*"\([^"]*\)".*/\1/')
printf 'Verified signed offline update bundle successfully:\n'
printf '  Product:        %s\n' "$product"
printf '  Version:        %s\n' "$version"
printf '  Payload Size:   %s bytes\n' "$actual_size"
printf '  Payload SHA256: %s\n' "$actual_sha256"
printf '  Signature:      VERIFIED (Ed25519)\n'
