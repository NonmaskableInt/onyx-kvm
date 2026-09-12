#!/bin/sh
set -eu

output_dir=${1:?usage: generate-recovery-credential.sh OUTPUT_DIRECTORY}
credential_file="$output_dir/recovery-credential.txt"
hash_file="$output_dir/recovery-password.sha512"

umask 077
mkdir -p "$output_dir"
test ! -e "$credential_file"
test ! -e "$hash_file"

password=$(openssl rand -base64 24 | tr -d '\n')
salt=$(openssl rand -hex 12)
password_hash=$(printf '%s\n' "$password" | openssl passwd -6 -salt "$salt" -stdin)
case "$password_hash" in
    '$6$'*) ;;
    *) printf '%s\n' 'failed to create SHA-512 recovery password hash' >&2; exit 1 ;;
esac

printf 'username=root\npassword=%s\n' "$password" >"$credential_file"
printf '%s\n' "$password_hash" >"$hash_file"
chmod 0600 "$credential_file" "$hash_file"
unset password password_hash salt

printf 'generated owner-only recovery credential: %s\n' "$credential_file"
