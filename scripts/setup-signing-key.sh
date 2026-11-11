#!/usr/bin/env bash
# Generate (or reuse) the GPG key that signs the [horizon] repository and the
# Horizon packages, then export the public half into packages/horizon-keyring/
# so it can ship in the keyring package and on the live medium.
#
#   bash scripts/setup-signing-key.sh
#
# Run once per build host. The private key stays in ~/.gnupg and is never
# committed; only the public key is exported.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEYRING_DIR="$REPO/packages/horizon-keyring"

KEY_NAME="Horizon OS"
KEY_EMAIL="horizon@horizon-os.org"
KEY_COMMENT="Horizon OS package signing key"

mkdir -p "$KEYRING_DIR"

# --- 1. find an existing key, or generate one --------------------------------
KEY_FP="$(gpg --batch --with-colons --list-secret-keys "$KEY_EMAIL" 2>/dev/null \
          | awk -F: '/^fpr:/ {print $10; exit}')"

if [[ -z "$KEY_FP" ]]; then
  echo "==> No signing key found; generating a new RSA 4096 key"
  cat > /tmp/horizon-key-params <<EOF
%no-protection
Key-Type: RSA
Key-Length: 4096
Subkey-Type: RSA
Subkey-Length: 4096
Name-Real: $KEY_NAME
Name-Comment: $KEY_COMMENT
Name-Email: $KEY_EMAIL
Expire-Date: 0
%commit
EOF
  gpg --batch --generate-key /tmp/horizon-key-params
  rm -f /tmp/horizon-key-params
  KEY_FP="$(gpg --batch --with-colons --list-secret-keys "$KEY_EMAIL" 2>/dev/null \
            | awk -F: '/^fpr:/ {print $10; exit}')"
fi

if [[ -z "$KEY_FP" ]]; then
  echo "error: could not obtain a signing key fingerprint" >&2
  exit 1
fi

echo "==> Signing key fingerprint: $KEY_FP"

# --- 2. export the public half ------------------------------------------------
gpg --batch --yes --armor --export "$KEY_FP" > "$KEYRING_DIR/horizon.asc"
gpg --batch --yes --export "$KEY_FP" > "$KEYRING_DIR/horizon.gpg"
printf '%s\n' "$KEY_FP" > "$KEYRING_DIR/horizon-trusted"

# --- 3. remember the key id for build-repo.sh --------------------------------
echo "$KEY_FP" > "$REPO/packages/.horizon-key-id"

echo
echo "==> Exported:"
echo "    $KEYRING_DIR/horizon.asc      (armored public key)"
echo "    $KEYRING_DIR/horizon.gpg      (binary keyring)"
echo "    $KEYRING_DIR/horizon-trusted  (trusted key ids)"
echo
echo "Next: run scripts/build-repo.sh to build and sign the [horizon] repository."
