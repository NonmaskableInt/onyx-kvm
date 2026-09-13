#!/bin/sh
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
engine=${CONTAINER_ENGINE:-podman}
builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
work_volume=${CLEANKVM_WORK_VOLUME:-cleankvm-work-v22}
output_dir=${CLEANKVM_LT_BACKUP_OUTPUT_DIR:-$repo_root/out/lt6911uxc-flash-backup-v1}
mkdir -p "$output_dir"
test ! -e "$output_dir/cleankvm-lt6911uxc-flash-backup"
"$engine" run --rm --arch amd64 --network none \
  -v "$work_volume:/work:rw" -v "$repo_root:/repo:ro" \
  -v "$output_dir:/output:rw" -w /work/sdk "$builder" bash -lc '
    set -e
    source build/cvisetup.sh
    defconfig sg2002_licheervnano_sd >/dev/null
    riscv64-unknown-linux-musl-gcc -Os -static -fPIE \
      -fstack-protector-strong -D_FORTIFY_SOURCE=2 -Wall -Wextra -Werror \
      /repo/image/lt6911uxc-flash-backup.c \
      -o /output/cleankvm-lt6911uxc-flash-backup
    riscv64-unknown-linux-musl-strip /output/cleankvm-lt6911uxc-flash-backup
    file /output/cleankvm-lt6911uxc-flash-backup
  '
sha256sum "$output_dir/cleankvm-lt6911uxc-flash-backup" >"$output_dir/SHA256SUMS"
