#!/bin/sh
set -eu

sdk_dir=${1:?usage: verify-sdk-snapshot.sh SDK_DIRECTORY}
expected_revision=d4003f15b35d43ad4842f427050ab2bba0114fa5

fail() {
    printf 'SDK snapshot verification failed: %s\n' "$1" >&2
    exit 1
}

digest_manifest() {
    git -C "$sdk_dir" ls-tree -r "$expected_revision" -- "$1" |
        sha256sum | awk '{print $1}'
}

check_tree() {
    name=$1
    path=$2
    expected_tree=$3
    expected_digest=$4
    actual_tree=$(git -C "$sdk_dir" rev-parse "$expected_revision:$path")
    test "$actual_tree" = "$expected_tree" ||
        fail "$name tree is $actual_tree, expected $expected_tree"
    actual_digest=$(digest_manifest "$path")
    test "$actual_digest" = "$expected_digest" ||
        fail "$name archive is $actual_digest, expected $expected_digest"
    printf 'verified %-12s %s\n' "$name" "$actual_digest"
}

command -v sha256sum >/dev/null 2>&1 || fail "sha256sum is required"
test "$(git -C "$sdk_dir" rev-parse HEAD)" = "$expected_revision" ||
    fail "wrong SDK revision"
test -z "$(git -C "$sdk_dir" status --porcelain --untracked-files=no)" ||
    fail "tracked SDK files are modified"

sdk_digest=$(git -C "$sdk_dir" archive --format=tar "$expected_revision" |
    sha256sum | awk '{print $1}')
test "$sdk_digest" = 1d5bc6cb9b67a33b19346236b3d0bf894ee55d07161e8c23385d8fe767238036 ||
    fail "whole SDK archive digest does not match"
printf 'verified %-12s %s\n' sdk "$sdk_digest"

check_tree linux linux_5.10 c189779ef3df19ccc9cf9953fd467d74ccd9eca9 \
    18d6e3e9420c314de507ee3ea32c21049f447923dbf7b57faa4199f66509c036
check_tree buildroot buildroot 9cb5e5b8472461a2afbd735b410b0a4aad14c34f \
    6a0d4f00adaad78d42e3da56ec5a2acf2daff52bc707bfff6bd5ee31fe15b663
check_tree opensbi opensbi 21bbe9fa4ada20c2e8fa8c9ca09396584e0ea656 \
    643e650b4c9c2eb5ab5bf5b71daeb94983b40686fb3f64da0f3d9d61d686aeb6
check_tree uboot u-boot-2021.10 dc2947e802dfd796dc8c3b4400e5ad5e12ee0caa \
    53826962c879b87501e23b0567624d126616a0002c94aa1b48636be6ed8298aa
check_tree fsbl fsbl a7ff72edc7332a1abab12b298aef97fa093a24a3 \
    f7c6c98018ae536cc5dca65efa113f98d966453a012fd42e12599e4c8b270bc8
check_tree middleware middleware cc28c8372568501f33ee2eab0130b33d72dc8c44 \
    a1ef5acf8fe8f8314d26b7e77a52e2feb0ac2bff89f9e56eac3e6225a623496e
