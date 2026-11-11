#!/usr/bin/env bash
# Build the Horizon OS live ISO.
#
#   wsl -d archlinux -e bash /mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/build-iso.sh
#
# The repository lives on the Windows filesystem (/mnt/d), which is far too slow for
# squashfs and kernel work. This script mirrors the profile into ~/horizon on the WSL
# ext4 filesystem, runs mkarchiso there, then copies the resulting ISO back into the
# repo's iso/ directory so Windows QEMU can boot it directly.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_ROOT="${BUILD_ROOT:-/root/horizon}"
PROFILE_NAME="${PROFILE_NAME:-releng}"

echo "==> Repo:       $REPO_DIR"
echo "==> Build root: $BUILD_ROOT"

if [[ "$REPO_DIR" == /mnt/* ]]; then
  echo "==> Note: repo is on a 9p mount; all real work happens in $BUILD_ROOT (ext4)"
fi

mkdir -p "$BUILD_ROOT"/{work,out,src,logs}

# Prefer the Horizon profile from the repo; fall back to stock archiso releng until
# profile/ is populated.
if [[ -d "$REPO_DIR/profile" ]] && [[ -n "$(ls -A "$REPO_DIR/profile" 2>/dev/null || true)" ]]; then
  echo "==> Using Horizon profile from repo"
  rsync -a --delete "$REPO_DIR/profile/" "$BUILD_ROOT/profile-horizon/"
  PROFILE="$BUILD_ROOT/profile-horizon"
else
  echo "==> profile/ is empty; falling back to stock archiso '$PROFILE_NAME' profile"
  if [[ ! -d "$BUILD_ROOT/profile-$PROFILE_NAME" ]]; then
    cp -r "/usr/share/archiso/configs/$PROFILE_NAME" "$BUILD_ROOT/profile-$PROFILE_NAME"
  fi
  PROFILE="$BUILD_ROOT/profile-$PROFILE_NAME"
fi

# Build the Rust tooling if needed, and stage it into the live medium.
if [[ ! -x "$BUILD_ROOT/target/release/horizon" ]]; then
  echo "==> Horizon binary missing; building Rust workspace"
  bash "$REPO_DIR/scripts/build-rust.sh"
fi
echo "==> Staging Horizon CLI into the live medium"
mkdir -p "$PROFILE/airootfs/usr/bin" "$PROFILE/airootfs/usr/share/horizon"
install -m755 "$BUILD_ROOT/target/release/horizon"         "$PROFILE/airootfs/usr/bin/horizon"
install -m755 "$BUILD_ROOT/target/release/horizon-install" "$PROFILE/airootfs/usr/bin/horizon-install"
if [[ -f "$REPO_DIR/packages/horizon-keyring/horizon.asc" ]]; then
  install -m644 "$REPO_DIR/packages/horizon-keyring/horizon.asc" "$PROFILE/airootfs/usr/share/horizon/horizon.asc"
fi

# Stage the signed [horizon] repository into the live medium (if it has been built).
if [[ -d "$BUILD_ROOT/repo" ]]; then
  echo "==> Staging [horizon] repository into the live medium"
  mkdir -p "$PROFILE/airootfs/usr/share/horizon"
  rsync -a --delete "$BUILD_ROOT/repo/" "$PROFILE/airootfs/usr/share/horizon/repo/"
  # Make the same path resolvable on the build host so mkarchiso's pacstrap
  # can fetch [horizon] packages during the ISO build.
  mkdir -p /usr/share/horizon
  if [[ ! -e /usr/share/horizon/repo ]]; then
    ln -s "$BUILD_ROOT/repo" /usr/share/horizon/repo
  fi
else
  echo "==> No [horizon] repo at $BUILD_ROOT/repo; skipping (run scripts/build-repo.sh to enable it)"
fi

echo "==> Cleaning work directory"
rm -rf "$BUILD_ROOT/work"
mkdir -p "$BUILD_ROOT/work"

LOG="$BUILD_ROOT/logs/mkarchiso-$(date +%Y%m%d-%H%M%S).log"
echo "==> Running mkarchiso (log: $LOG)"
mkarchiso -v -w "$BUILD_ROOT/work" -o "$BUILD_ROOT/out" "$PROFILE" 2>&1 | tee "$LOG"

echo "==> Build artifacts"
ls -lh "$BUILD_ROOT/out"

# Copy the ISO back to the repo so Windows-side QEMU can boot it from a fast path.
if [[ "$REPO_DIR" == /mnt/* ]]; then
  mkdir -p "$REPO_DIR/iso"
  shopt -s nullglob
  for f in "$BUILD_ROOT/out/"*.iso "$BUILD_ROOT/out/"*.sha512; do
    [[ -e "$f" ]] && cp -v "$f" "$REPO_DIR/iso/"
  done
  echo "==> Copied to $REPO_DIR/iso"
fi

echo "==> Boot it with: pwsh -File scripts/run-qemu.ps1"
