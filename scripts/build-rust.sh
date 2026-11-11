#!/usr/bin/env bash
# Build the Horizon OS Rust workspace inside the Arch Linux WSL distro.
#
#   wsl -d archlinux -e bash /mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/build-rust.sh
#
# The repo lives on a 9p mount, which is far too slow for a Rust build, so the
# source is mirrored to ~/horizon/src on ext4 first and compiled there. Pass
# extra arguments straight through to cargo, e.g. `-- -p horizon`.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="${HORIZON_BUILD_ROOT:-$HOME/horizon}"
SRC="$BUILD/src"
TARGET="$BUILD/target"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install the toolchain with: pacman -S rust" >&2
  exit 1
fi

echo "==> Repo   : $REPO"
echo "==> Build  : $SRC"
mkdir -p "$SRC"

echo "==> Mirroring src/ to ext4 (9p is too slow to build on)"
rsync -a --delete --exclude target "$REPO/src/" "$SRC/"

cd "$SRC"
export CARGO_TARGET_DIR="$TARGET"
echo "==> cargo build --release --workspace $*"
cargo build --release --workspace "$@"

echo
echo "==> Binaries"
for b in horizon horizon-install; do
  if [[ -f "$TARGET/release/$b" ]]; then
    ls -lh "$TARGET/release/$b"
  fi
done
