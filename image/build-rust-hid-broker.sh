#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${ONYX_KVM_RUST_ARTIFACT_DIR:-${CLEANKVM_RUST_ARTIFACT_DIR:-"$repo_root/out/phase2-rust"}}
binary="$repo_root/target/riscv64gc-unknown-linux-musl/release/onyx-kvm-hid-broker"

mkdir -p "$artifact_dir"
cd "$repo_root"
cargo rustc --release --target riscv64gc-unknown-linux-musl \
    --package onyx-kvm-hid-broker --bin onyx-kvm-hid-broker --offline -- \
    -C linker=rust-lld \
    -C link-self-contained=yes \
    -C target-feature=+crt-static

description=$(file "$binary")
case "$description" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'unexpected HID broker binary: %s\n' "$description" >&2
        exit 1
        ;;
esac

install -m 0755 "$binary" "$artifact_dir/onyx-kvm-hid-broker"
cp -f "$artifact_dir/onyx-kvm-hid-broker" "$artifact_dir/cleankvm-hid-broker"
cd "$artifact_dir"
shasum -a 256 onyx-kvm-hid-broker >onyx-kvm-hid-broker.sha256
shasum -a 256 -c onyx-kvm-hid-broker.sha256
printf 'verified static RISC-V HID broker: %s/onyx-kvm-hid-broker\n' "$artifact_dir"
