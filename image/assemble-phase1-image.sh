#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${CLEANKVM_ARTIFACT_DIR:-"$repo_root/out/phase1"}
expected_boot_sha256=${CLEANKVM_EXPECTED_BOOT_SHA256:-}

test -f "$artifact_dir/boot.sd"
test -f "$artifact_dir/rootfs.ext4"
test -f "$artifact_dir/fip.bin"

if test -n "$expected_boot_sha256"; then
    actual_boot_sha256=$(shasum -a 256 "$artifact_dir/boot.sd" | cut -d ' ' -f 1)
    test "$actual_boot_sha256" = "$expected_boot_sha256" || {
        printf 'unexpected boot FIT: expected %s, got %s\n' \
            "$expected_boot_sha256" "$actual_boot_sha256" >&2
        exit 1
    }
fi

img_name=${ONYX_KVM_IMAGE_NAME:-onyx-kvm-phase1.img}

python3 "$repo_root/image/build-sd-image.py" \
    "$artifact_dir/$img_name" \
    "$artifact_dir/boot.sd" \
    "$artifact_dir/rootfs.ext4" \
    "$artifact_dir/fip.bin"
python3 "$repo_root/image/verify-sd-image.py" \
    "$artifact_dir/$img_name" \
    "$artifact_dir/boot.sd" \
    "$artifact_dir/rootfs.ext4" \
    "$artifact_dir/fip.bin"
if [ "$img_name" != "cleankvm-phase1.img" ]; then
    ln -sf "$img_name" "$artifact_dir/cleankvm-phase1.img"
fi
(cd "$artifact_dir" && \
    shasum -a 256 boot.sd rootfs.ext4 fip.bin phase1.its "$img_name" > SHA256SUMS && \
    shasum -a 256 -c SHA256SUMS)

printf 'verified development SD image: %s/%s\n' "$artifact_dir" "$img_name"
