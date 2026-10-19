#!/bin/sh
set -eu

# Apply patch 0028 (hardware H.264 cvi_vc_drv) to the pinned sophgo-osdrv
# source tree. That tree is separate from the pinned SDK, so
# apply-phase1-config.sh skips this patch.
#
#   image/apply-osdrv-patch.sh [OSDRV_DIR]
#
# OSDRV_DIR is a git checkout of https://github.com/sophgo/osdrv.git at the
# revision recorded in image/sources.lock (sparse checkouts are fine). It
# defaults to dl/sophgo-osdrv-sparse. The script is idempotent and offline.

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
osdrv_dir=${1:-${ONYX_KVM_OSDRV_DIR:-"$repo_root/dl/sophgo-osdrv-sparse"}}
patch_file="$repo_root/image/patches/0028-cvi-vc-driver-hardware-h264.patch"
expected_revision=$(awk -F' [|] ' '$1 == "sophgo-osdrv" { print $3 }' "$repo_root/image/sources.lock")

fail() {
    printf 'osdrv patch failed: %s\n' "$1" >&2
    exit 1
}

test -n "$expected_revision" || fail "sophgo-osdrv is not pinned in image/sources.lock"
test -d "$osdrv_dir/.git" || fail "missing osdrv checkout at $osdrv_dir"
actual_revision=$(git -C "$osdrv_dir" rev-parse HEAD)
test "$actual_revision" = "$expected_revision" ||
    fail "osdrv revision is $actual_revision, expected $expected_revision"

# The patch paths start with osdrv/, which is this repository's root, hence
# -p2 inside the checkout. git apply is exact; BSD patch is not reliable here.
apply="git -C $osdrv_dir apply -p2"
if $apply --reverse --check "$patch_file" >/dev/null 2>&1; then
    printf 'patch 0028 already applied to %s\n' "$osdrv_dir"
    exit 0
fi
$apply --check "$patch_file" || fail "patch 0028 does not apply cleanly (tree modified?)"
$apply "$patch_file"
printf 'patch 0028 applied to %s\n' "$osdrv_dir"
