#!/bin/sh
set -eu

if [ "$#" -lt 3 ]; then
    printf 'usage: sign-update.sh PAYLOAD_PATH PRIVATE_KEY_PEM OUTPUT_BUNDLE_DIR [VERSION]\n' >&2
    exit 1
fi

payload_path="$1"
private_key="$2"
output_dir="$3"
version="${4:-v60}"

test -f "$payload_path" || {
    printf 'error: payload file not found: %s\n' "$payload_path" >&2
    exit 2
}
test -f "$private_key" || {
    printf 'error: private key file not found: %s\n' "$private_key" >&2
    exit 2
}

mkdir -p "$output_dir"
manifest="$output_dir/manifest.json"
signature="$output_dir/signature.bin"
bundle_payload="$output_dir/payload.ext4"

# Compute payload metadata
payload_size=$(wc -c <"$payload_path" | tr -d ' ')
payload_sha256=$(shasum -a 256 "$payload_path" | awk '{print $1}')
created_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Copy payload to bundle
cp -p "$payload_path" "$bundle_payload"

# Generate deterministic manifest
cat <<EOF >"$manifest"
{
  "product": "onyx-kvm",
  "version": "$version",
  "created_at": "$created_at",
  "payload_name": "payload.ext4",
  "payload_size": $payload_size,
  "payload_sha256": "$payload_sha256"
}
EOF

# Sign manifest with Ed25519 private key
openssl pkeyutl -sign -rawin -inkey "$private_key" -in "$manifest" -out "$signature"

# Generate bundle checksum file
(cd "$output_dir" && shasum -a 256 payload.ext4 manifest.json signature.bin > SHA256SUMS)

printf 'Signed offline update bundle created successfully at %s\n' "$output_dir"
printf '  Version:        %s\n' "$version"
printf '  Payload Size:   %s bytes\n' "$payload_size"
printf '  Payload SHA256: %s\n' "$payload_sha256"
