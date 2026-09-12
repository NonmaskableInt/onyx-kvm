#!/bin/sh
set -eu

sdk_dir=${1:?usage: replace-ddr-compat.sh SDK_DIRECTORY}
blob="$sdk_dir/fsbl/test/cv181x/ddr_param.bin"
vendor_sha=5b7d44720997f018621fe4a462111b2d53954a5d1e2a794c83dd1dd6516eebaa
replacement_sha=076a27c79e5ace2a3d47f9dd2e83e4ff6ea8872b3c2218f66c92b89b55f36560

test -f "$blob" || {
    printf 'missing DDR compatibility input: %s\n' "$blob" >&2
    exit 1
}

actual_sha=$(sha256sum "$blob" | cut -d ' ' -f 1)
test "$actual_sha" = "$vendor_sha" || {
    printf 'refusing unexpected DDR input: %s\n' "$actual_sha" >&2
    exit 1
}

# SG2002 BL2 requires a nonempty DDR entry so the vendor packer emits FIP2,
# but the audited DDR initializer does not interpret its contents. Preserve one
# complete FIP alignment block while eliminating the opaque high-entropy bytes.
truncate -s 0 "$blob"
truncate -s 512 "$blob"

test "$(wc -c <"$blob")" -eq 512
actual_sha=$(sha256sum "$blob" | cut -d ' ' -f 1)
test "$actual_sha" = "$replacement_sha" || {
    printf 'DDR replacement verification failed: %s\n' "$actual_sha" >&2
    exit 1
}

printf 'installed deterministic DDR compatibility block: sha256:%s\n' \
    "$replacement_sha"
