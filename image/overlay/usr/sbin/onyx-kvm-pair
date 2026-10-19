#!/bin/sh
set -eu

# Onyx KVM Local Pairing Ceremony Helper
# Stages a single-use pairing PIN and session token in /run/onyx-kvm.
# This script runs directly on target or as part of a physical console ceremony.

token_input=${1:-}
pin_input=${2:-}

run_dir=/run/onyx-kvm
mkdir -p "$run_dir"
chmod 2750 "$run_dir"

if [ -z "$pin_input" ]; then
    # Generate 6-digit cryptographic PIN from /dev/urandom
    pin=$(od -An -N3 -tu4 /dev/urandom | awk '{printf "%06d", $1 % 1000000}')
else
    pin="$pin_input"
fi

case "$pin" in
    [0-9][0-9][0-9][0-9][0-9][0-9]) ;;
    *)
        printf 'pairing PIN must be exactly 6 digits\n' >&2
        exit 1
        ;;
esac

if [ -z "$token_input" ]; then
    printf 'usage: pair-device.sh SESSION_TOKEN [6_DIGIT_PIN]\n' >&2
    exit 1
fi

# Write pairing files with strict ownership and permissions
printf '%s\n' "$pin" > "$run_dir/pairing-code"
chmod 0640 "$run_dir/pairing-code"
chown root:900 "$run_dir/pairing-code"

printf '%s\n' "$token_input" > "$run_dir/pending-session-token"
chmod 0640 "$run_dir/pending-session-token"
chown root:900 "$run_dir/pending-session-token"

printf '\n===================================================\n'
printf '           Onyx KVM Pairing Ceremony\n'
printf '===================================================\n'
printf 'Pairing PIN:  %s\n' "$pin"
printf 'Valid for:    Single-use, memory-only (/run)\n'
printf 'Throttle:     Maximum 3 attempts before revocation\n'
printf '===================================================\n'
printf 'Enter this PIN in the Onyx KVM web console to pair.\n\n'
