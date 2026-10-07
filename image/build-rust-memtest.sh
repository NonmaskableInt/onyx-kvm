#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${ONYX_KVM_RUST_ARTIFACT_DIR:-${CLEANKVM_RUST_ARTIFACT_DIR:-"$repo_root/out/phase2-rust"}}
binary="$repo_root/target/riscv64gc-unknown-linux-musl/release/onyx-kvm-memtest"

cargo rustc --manifest-path "$repo_root/Cargo.toml" --release \
    --target riscv64gc-unknown-linux-musl --package onyx-kvm-memtest --offline -- \
    -C linker=rust-lld -C link-self-contained=yes -C target-feature=+crt-static

description=$(file "$binary")
case "$description" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'unexpected onyx-kvm-memtest binary: %s\n' "$description" >&2
        exit 1
        ;;
esac

mkdir -p "$artifact_dir"
install -m 0755 "$binary" "$artifact_dir/onyx-kvm-memtest"
cp -f "$artifact_dir/onyx-kvm-memtest" "$artifact_dir/cleankvm-memtest"
(cd "$artifact_dir" && shasum -a 256 onyx-kvm-memtest >MEMTEST_SHA256SUMS)
printf 'verified static RISC-V memory tester: %s/onyx-kvm-memtest\n' "$artifact_dir"
