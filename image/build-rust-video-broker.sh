#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${CLEANKVM_RUST_ARTIFACT_DIR:-"$repo_root/out/phase3-rust"}
binary="$repo_root/target/riscv64gc-unknown-linux-musl/release/cleankvm-video-broker"

mkdir -p "$artifact_dir"
cd "$repo_root"
cargo rustc --release --target riscv64gc-unknown-linux-musl \
    --package cleankvm-video-broker --bin cleankvm-video-broker --offline -- \
    -C linker=rust-lld \
    -C link-self-contained=yes \
    -C target-feature=+crt-static

description=$(file "$binary")
case "$description" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'unexpected video broker binary: %s\n' "$description" >&2
        exit 1
        ;;
esac

install -m 0755 "$binary" "$artifact_dir/cleankvm-video-broker"
cd "$artifact_dir"
shasum -a 256 cleankvm-video-broker >cleankvm-video-broker.sha256
shasum -a 256 -c cleankvm-video-broker.sha256
printf 'verified static RISC-V video broker: %s/cleankvm-video-broker\n' "$artifact_dir"
