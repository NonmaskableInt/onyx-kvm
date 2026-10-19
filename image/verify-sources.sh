#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

# The SDK has tracked paths that differ only by case, so a checkout on a
# case-insensitive host (macOS) always looks modified. Verify the clean
# case-sensitive checkout in the source volume instead, inside the builder.
if [ -z "${ONYX_KVM_SDK_DIR:-}" ] && [ "$(uname)" = "Darwin" ]; then
    . "$repo_root/image/default-env.sh"
    exec "$CONTAINER_ENGINE" run --rm --arch amd64 --network none \
        -v "$ONYX_KVM_SOURCE_VOLUME:/source:ro" -v "$repo_root:/repo:ro" \
        -e ONYX_KVM_SDK_DIR=/source/sdk \
        -e ONYX_KVM_ALLOW_UNRESOLVED_SOURCES="${ONYX_KVM_ALLOW_UNRESOLVED_SOURCES:-0}" \
        "$ONYX_KVM_BUILDER_TAG" sh /repo/image/verify-sources.sh
fi

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
test -d "$sdk_dir/.git" || fail "missing SDK checkout at $sdk_dir"

actual_sdk_revision=$(git -c safe.directory="*" -C "$sdk_dir" rev-parse HEAD)
test "$actual_sdk_revision" = "$expected_sdk_revision" ||
    fail "SDK revision is $actual_sdk_revision, expected $expected_sdk_revision"

if test -n "$(git -c safe.directory="*" -C "$sdk_dir" status --porcelain --untracked-files=no)"; then
    fail "SDK checkout has tracked modifications"
fi

"$repo_root/image/verify-sdk-snapshot.sh" "$sdk_dir"

unresolved=$(cat "$lock_file" "$builder_lock" | grep -v '^#' | grep -cE 'PENDING-REVIEW|NO-LICENSE-GRANT' || true)
if [ "$unresolved" -gt 0 ]; then
    if [ "${ONYX_KVM_ALLOW_UNRESOLVED_SOURCES:-0}" = "1" ]; then
        printf 'WARNING: %s lock entries lack a license grant or review; waived for a development build, not releasable\n' "$unresolved" >&2
    else
        fail "lock files contain $unresolved PENDING-REVIEW or NO-LICENSE-GRANT entries (development builds: ONYX_KVM_ALLOW_UNRESOLVED_SOURCES=1)"
    fi
fi

printf 'verified SDK %s\n' "$actual_sdk_revision"
