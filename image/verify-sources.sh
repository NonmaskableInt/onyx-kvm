#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
lock_file="$repo_root/image/sources.lock"
builder_lock="$repo_root/image/builder.lock"
sdk_dir=${ONYX_KVM_SDK_DIR:-"$repo_root/dl/LicheeRV-Nano-Build"}
expected_sdk_revision=d4003f15b35d43ad4842f427050ab2bba0114fa5

fail() {
    printf 'source verification failed: %s\n' "$1" >&2
    exit 1
}

test -f "$lock_file" || fail "missing image/sources.lock"
test -f "$builder_lock" || fail "missing image/builder.lock"
if grep -v '^#' "$lock_file" | grep -q 'PENDING-REVIEW'; then
    fail "sources.lock still contains PENDING-REVIEW entries"
fi
if grep -v '^#' "$builder_lock" | grep -q 'PENDING-REVIEW'; then
    fail "builder.lock still contains PENDING-REVIEW entries"
fi

if test -d "$sdk_dir/.git"; then
    actual_sdk_revision=$(git -C "$sdk_dir" rev-parse HEAD)
    test "$actual_sdk_revision" = "$expected_sdk_revision" ||
        fail "SDK revision is $actual_sdk_revision, expected $expected_sdk_revision"

    if [ "$(uname)" = "Darwin" ]; then
        printf 'note: Darwin host detected; case-insensitive APFS working tree status check skipped (checked in builder container)\n'
    else
        if test -n "$(git -C "$sdk_dir" status --porcelain --untracked-files=no)"; then
            fail "SDK checkout has tracked modifications"
        fi
        "$repo_root/image/verify-sdk-snapshot.sh" "$sdk_dir"
    fi
    printf 'verified SDK %s\n' "$actual_sdk_revision"
fi

printf 'source and lock verification passed\n'
