#!/bin/sh
set -eu

output_dir=${1:?usage: generate-development-identity.sh OUTPUT_DIRECTORY}
tls_key="$output_dir/tls-private-key.pem"
tls_certificate="$output_dir/tls-certificate.pem"
tls_certificate_der="$output_dir/tls-certificate.der"
tls_key_der="$output_dir/tls-private-key.pkcs8.der"
issuer_key="$output_dir/oauth-issuer-private-key.pem"
issuer_public_pem="$output_dir/oauth-issuer-public-key.pem"
issuer_public_raw="$output_dir/oauth-issuer-public-key.raw"
manifest="$output_dir/identity-sha256.txt"
temporary_der="$output_dir/.oauth-issuer-public-key.der"

umask 077
mkdir -p "$output_dir"
for path in "$tls_key" "$tls_certificate" "$issuer_key" \
    "$tls_certificate_der" "$tls_key_der" "$issuer_public_pem" \
    "$issuer_public_raw" "$manifest"
do
    test ! -e "$path" || {
        printf 'refusing to overwrite identity artifact: %s\n' "$path" >&2
        exit 1
    }
done

trap 'rm -f "$temporary_der"' EXIT HUP INT TERM

# A short-lived self-signed development leaf keeps this step independent of
# external PKI. Production images must replace it with an enrolled identity.
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 \
    -subj '/CN=cleankvm-recovery' \
    -addext 'subjectAltName=DNS:cleankvm-recovery' \
    -keyout "$tls_key" -out "$tls_certificate" >/dev/null 2>&1
openssl x509 -in "$tls_certificate" -outform DER -out "$tls_certificate_der"
openssl pkey -in "$tls_key" -outform DER -out "$tls_key_der"

# The issuer key stays with the operator. Only its raw Ed25519 public key is
# installed on the device for offline access-token verification.
openssl genpkey -algorithm ED25519 -out "$issuer_key" >/dev/null 2>&1
openssl pkey -in "$issuer_key" -pubout -out "$issuer_public_pem"
openssl pkey -pubin -in "$issuer_public_pem" -outform DER -out "$temporary_der"
test "$(wc -c <"$temporary_der" | tr -d ' ')" -eq 44
tail -c 32 "$temporary_der" >"$issuer_public_raw"
test "$(wc -c <"$issuer_public_raw" | tr -d ' ')" -eq 32

chmod 0600 "$tls_key" "$issuer_key"
chmod 0600 "$tls_key_der"
chmod 0644 "$tls_certificate" "$tls_certificate_der" "$issuer_public_pem" \
    "$issuer_public_raw"
(cd "$output_dir" && \
    shasum -a 256 tls-certificate.pem tls-certificate.der \
        oauth-issuer-public-key.pem oauth-issuer-public-key.raw \
        >"$(basename "$manifest")")
chmod 0644 "$manifest"

printf 'generated development TLS identity and OAuth issuer keypair: %s\n' "$output_dir"
printf '%s\n' 'private keys are owner-only and must not be copied into documentation'
