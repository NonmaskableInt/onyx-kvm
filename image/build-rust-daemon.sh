#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${CLEANKVM_RUST_ARTIFACT_DIR:-"$repo_root/out/phase2-rust"}
binary="$repo_root/target/riscv64gc-unknown-linux-musl/release/cleankvmd"

mkdir -p "$artifact_dir"

cd "$repo_root"
cargo rustc --release --target riscv64gc-unknown-linux-musl \
    --package cleankvmd --offline -- \
    -C linker=rust-lld \
    -C link-self-contained=yes \
    -C target-feature=+crt-static

description=$(file "$binary")
case "$description" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'unexpected cleankvmd binary: %s\n' "$description" >&2
        exit 1
        ;;
esac

install -m 0755 "$binary" "$artifact_dir/cleankvmd"
cd "$artifact_dir"
shasum -a 256 cleankvmd >SHA256SUMS
shasum -a 256 -c SHA256SUMS
printf 'verified static RISC-V daemon: %s/cleankvmd\n' "$artifact_dir"
