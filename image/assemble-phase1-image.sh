#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${CLEANKVM_ARTIFACT_DIR:-"$repo_root/out/phase1"}

test -f "$artifact_dir/boot.sd"
test -f "$artifact_dir/rootfs.ext4"
test -f "$artifact_dir/fip.bin"

python3 "$repo_root/image/build-sd-image.py" \
    "$artifact_dir/cleankvm-phase1.img" \
    "$artifact_dir/boot.sd" \
    "$artifact_dir/rootfs.ext4" \
    "$artifact_dir/fip.bin"
python3 "$repo_root/image/verify-sd-image.py" \
    "$artifact_dir/cleankvm-phase1.img" \
    "$artifact_dir/boot.sd" \
    "$artifact_dir/rootfs.ext4" \
    "$artifact_dir/fip.bin"
(cd "$artifact_dir" && \
    shasum -a 256 boot.sd rootfs.ext4 fip.bin phase1.its cleankvm-phase1.img > SHA256SUMS && \
    shasum -a 256 -c SHA256SUMS)

printf 'verified development SD image: %s/cleankvm-phase1.img\n' "$artifact_dir"
