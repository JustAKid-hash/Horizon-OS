#!/usr/bin/env bash
# Build the signed [horizon] package repository.
#
#   bash scripts/build-repo.sh
#
# Produces a pacman repository at $HORIZON_REPO (default /root/horizon/repo)
# containing horizon-keyring, horizon-release, horizon-cli and one
# horizon-desktop-* meta-package per entry in src/horizon/data/desktops.toml.
#
# Prerequisites: run scripts/setup-signing-key.sh once first.
set -euo pipefail
export LC_ALL=C

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="${HORIZON_BUILD_ROOT:-/root/horizon}"
# The builder user needs to traverse the build root and write to the repo dir.
# /root itself is 700 by default; relax to 711 (traverse only) so the builder
# can reach /root/horizon without being able to list /root's contents.
chmod 711 /root 2>/dev/null || true
chmod 755 "$BUILD" 2>/dev/null || true
SRC="$BUILD/src"
RUST_TARGET="$BUILD/target"
PKGROOT="$BUILD/pkgbuilds"
OUT_REPO="${HORIZON_REPO:-$BUILD/repo}"
BUILDER="builder"

if [[ $EUID -ne 0 ]]; then
  echo "error: build-repo.sh must run as root (it uses su to the 'builder' user)" >&2
  exit 1
fi

echo "==> Repo source : $REPO"
echo "==> Build root  : $BUILD"
echo "==> Output repo : $OUT_REPO"

# --- 1. signing key ---------------------------------------------------------
KEY_ID_FILE="$REPO/packages/.horizon-key-id"
if [[ ! -f "$KEY_ID_FILE" ]]; then
  echo "==> No signing key recorded; running setup-signing-key.sh"
  bash "$REPO/scripts/setup-signing-key.sh"
fi
KEY_FP="$(cat "$KEY_ID_FILE")"
echo "==> Signing key: $KEY_FP"

# make sure the build host pacman trusts it (needed to verify during ISO builds)
if ! pacman-key --list-keys "$KEY_FP" >/dev/null 2>&1; then
  echo "==> Importing Horizon key into the build host pacman keyring"
  pacman-key --init >/dev/null 2>&1 || true
  pacman-key --add "$REPO/packages/horizon-keyring/horizon.asc" >/dev/null 2>&1 || true
  pacman-key --lsign-key "$KEY_FP" >/dev/null 2>&1 || true
fi

# --- 2. build the Rust workspace -------------------------------------------
echo "==> Building Rust workspace"
bash "$REPO/scripts/build-rust.sh" >/dev/null

HORIZON_BIN="$RUST_TARGET/release/horizon"
HORIZON_INSTALL_BIN="$RUST_TARGET/release/horizon-install"
if [[ ! -x "$HORIZON_BIN" ]]; then
  echo "error: horizon binary not found at $HORIZON_BIN" >&2
  exit 1
fi

# --- 3. stage package sources on ext4 (9p is too slow / not writable) -------
echo "==> Staging package sources"
rm -rf "$PKGROOT"
mkdir -p "$PKGROOT"
rsync -a "$REPO/packages/" "$PKGROOT/"
chown -R "$BUILDER:$BUILDER" "$PKGROOT"

# stage the prebuilt binaries where horizon-cli's PKGBUILD expects them
cp "$HORIZON_BIN" "$PKGROOT/horizon-cli/horizon"
cp "$HORIZON_INSTALL_BIN" "$PKGROOT/horizon-cli/horizon-install"
chown "$BUILDER:$BUILDER" "$PKGROOT/horizon-cli/horizon" "$PKGROOT/horizon-cli/horizon-install"

# ensure keyring files are present (they are, but be explicit)
cp "$REPO/packages/horizon-keyring/horizon.asc"  "$PKGROOT/horizon-keyring/horizon.asc"
cp "$REPO/packages/horizon-keyring/horizon.gpg"  "$PKGROOT/horizon-keyring/horizon.gpg"
cp "$REPO/packages/horizon-keyring/horizon-trusted" "$PKGROOT/horizon-keyring/horizon-trusted"
chown "$BUILDER:$BUILDER" "$PKGROOT/horizon-keyring"/horizon.*

# --- 4. generate desktop meta-package PKGBUILDs -----------------------------
echo "==> Generating desktop meta-package PKGBUILDs"
for id in plasma gnome cinnamon hyprland xfce; do
  mkdir -p "$PKGROOT/horizon-desktop-$id"
  "$HORIZON_BIN" desktop gen-pkgbuild "$id" "$PKGROOT/horizon-desktop-$id" >/dev/null
done
chown -R "$BUILDER:$BUILDER" "$PKGROOT"

# --- 5. build every package -------------------------------------------------
echo "==> Building packages"
PACKAGES=(horizon-keyring horizon-release horizon-cli)
for id in plasma gnome cinnamon hyprland xfce; do
  PACKAGES+=("horizon-desktop-$id")
done

rm -rf "$OUT_REPO"
mkdir -p "$OUT_REPO"
chown "$BUILDER:$BUILDER" "$OUT_REPO"

for p in "${PACKAGES[@]}"; do
  echo "    -> $p"
  su "$BUILDER" -c "cd '$PKGROOT/$p' && makepkg --nodeps --noconfirm --clean" >/dev/null
  # move the built artifact into the repo
  mv "$PKGROOT/$p"/*.pkg.tar.* "$OUT_REPO/"
done

# --- 6. create and sign the repository database ----------------------------
echo "==> Creating repository database"
cd "$OUT_REPO"
REPO_NAME="horizon"
# repo-add --sign signs the db with the default gpg key
LC_ALL=C repo-add --sign "$REPO_NAME.db.tar.gz" ./*.pkg.tar.* >/dev/null

# repo-add --sign produces .db.tar.gz.sig; also create the conventional .db symlink
ln -sf "$REPO_NAME.db.tar.gz" "$REPO_NAME.db"
ln -sf "$REPO_NAME.db.tar.gz.sig" "$REPO_NAME.db.sig"
ln -sf "$REPO_NAME.files.tar.gz" "$REPO_NAME.files" 2>/dev/null || true

echo
echo "==> Repository ready: $OUT_REPO"
ls -1 "$OUT_REPO"
echo
echo "Signing key fingerprint: $KEY_FP"
echo "Add to pacman.conf:"
echo "  [horizon]"
echo "  SigLevel = Required DatabaseOptional"
echo "  Server = file://$OUT_REPO"
