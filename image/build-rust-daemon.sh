#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
artifact_dir=${ONYX_KVM_RUST_ARTIFACT_DIR:-${CLEANKVM_RUST_ARTIFACT_DIR:-"$repo_root/out/phase2-rust"}}
binary="$repo_root/target/riscv64gc-unknown-linux-musl/release/onyx-kvmd"

mkdir -p "$artifact_dir"
temp_bin=$(mktemp -d)
trap 'rm -rf "$temp_bin"' EXIT HUP INT TERM

if ! command -v riscv64-linux-musl-gcc >/dev/null 2>&1 && \
   test -z "${CC_riscv64gc_unknown_linux_musl:-}"; then
    engine=${CONTAINER_ENGINE:-podman}
    builder=${CLEANKVM_BUILDER_TAG:-localhost/cleankvm-builder:ubuntu-22.04-amd64}
    cat >"$temp_bin/riscv64-linux-musl-gcc" <<EOF
#!/bin/sh
exec "$engine" run --rm --arch amd64 --network none \\
    -v cleankvm-host-tools:/host-tools:ro \\
    -v "$repo_root:$repo_root:rw" \\
    -v "$HOME/.cargo:$HOME/.cargo:ro" \\
    -w "\$(pwd)" \\
    "$builder" \\
    /host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin/riscv64-unknown-linux-musl-gcc "\$@"
EOF
    cat >"$temp_bin/riscv64-linux-musl-ar" <<EOF
#!/bin/sh
exec "$engine" run --rm --arch amd64 --network none \\
    -v cleankvm-host-tools:/host-tools:ro \\
    -v "$repo_root:$repo_root:rw" \\
    -v "$HOME/.cargo:$HOME/.cargo:ro" \\
    -w "\$(pwd)" \\
    "$builder" \\
    /host-tools/repo/gcc/riscv64-linux-musl-x86_64/bin/riscv64-unknown-linux-musl-ar "\$@"
EOF
    chmod 0755 "$temp_bin/riscv64-linux-musl-gcc" "$temp_bin/riscv64-linux-musl-ar"
    export CC_riscv64gc_unknown_linux_musl="$temp_bin/riscv64-linux-musl-gcc"
    export AR_riscv64gc_unknown_linux_musl="$temp_bin/riscv64-linux-musl-ar"
fi

cd "$repo_root"
cargo rustc --release --target riscv64gc-unknown-linux-musl \
    --package onyx-kvmd --offline -- \
    -C linker=rust-lld \
    -C link-self-contained=yes \
    -C target-feature=+crt-static

description=$(file "$binary")
case "$description" in
    *"ELF 64-bit"*"RISC-V"*"statically linked"*"stripped"*) ;;
    *)
        printf 'unexpected onyx-kvmd binary: %s\n' "$description" >&2
        exit 1
        ;;
esac

install -m 0755 "$binary" "$artifact_dir/onyx-kvmd"
cp -f "$artifact_dir/onyx-kvmd" "$artifact_dir/cleankvmd"
cd "$artifact_dir"
shasum -a 256 onyx-kvmd >SHA256SUMS
shasum -a 256 -c SHA256SUMS
