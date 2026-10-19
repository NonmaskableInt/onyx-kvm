#!/bin/sh
set -eu

# One-time local migration from the pre-rename "cleankvm" container resources
# to the "onyx-kvm" names used by the build scripts. It is additive: nothing
# is removed or overwritten, so the old resources keep working until you
# delete them yourself. Without --apply it only prints what it would do.
#
#   image/migrate-legacy-names.sh           # dry run
#   image/migrate-legacy-names.sh --apply   # retag builder, copy volumes

engine=${CONTAINER_ENGINE:-podman}
apply=0
case "${1:-}" in
    --apply) apply=1 ;;
    '') ;;
    *) printf 'usage: %s [--apply]\n' "$0" >&2; exit 2 ;;
esac

old_builder=localhost/cleankvm-builder:ubuntu-22.04-amd64
new_builder=localhost/onyx-kvm-builder:ubuntu-22.04-amd64

run() {
    if [ "$apply" -eq 1 ]; then
        "$@"
    else
        printf 'would run:'
        printf ' %s' "$@"
        printf '\n'
    fi
}

if "$engine" image inspect "$new_builder" >/dev/null 2>&1; then
    printf 'builder image already present: %s\n' "$new_builder"
elif "$engine" image inspect "$old_builder" >/dev/null 2>&1; then
    run "$engine" tag "$old_builder" "$new_builder"
else
    printf 'no builder image found; run image/build-builder.sh\n' >&2
fi

# Reusable inputs only. Work volumes are disposable: create a fresh one with
# image/prepare-build-volume.sh instead of copying an old one, because old
# work volumes carry the pre-rename overlay directory and config headers.
for suffix in sdk host-tools buildroot-dl-v50; do
    old=cleankvm-$suffix
    new=onyx-kvm-$suffix
    if "$engine" volume inspect "$new" >/dev/null 2>&1; then
        printf 'volume already present: %s\n' "$new"
        continue
    fi
    if ! "$engine" volume inspect "$old" >/dev/null 2>&1; then
        printf 'volume not found, skipping: %s\n' "$old"
        continue
    fi
    run "$engine" volume create "$new"
    run "$engine" run --rm --arch amd64 --network none --user 0:0 \
        -v "$old:/from:ro" -v "$new:/to" \
        "$new_builder" cp -a /from/. /to/
done

if [ "$apply" -eq 0 ]; then
    printf '\ndry run only; re-run with --apply to perform the steps above\n'
fi
