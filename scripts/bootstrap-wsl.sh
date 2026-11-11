#!/usr/bin/env bash
# Set up the Arch Linux WSL2 distro as the Horizon OS build host.
#
#   wsl -d archlinux -e bash /mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/bootstrap-wsl.sh
#
# The Arch WSL distro is the natural build host: mkarchiso, pacstrap, makepkg and
# libalpm are all native there, so pacman and AUR packages work without containers.
set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "error: run as root (the Arch WSL distro defaults to root)" >&2
  exit 1
fi

# mkarchiso dependencies, plus the pieces Horizon OS needs to build and boot ISOs.
PACMAN_PACKAGES=(
  # base toolchain
  base
  base-devel
  git
  rsync
  # ISO building (archiso + its dependencies)
  archiso
  arch-install-scripts
  mkinitcpio
  dosfstools
  e2fsprogs
  erofs-utils
  squashfs-tools
  libisoburn          # provides xorriso
  mtools
  libarchive
  gawk
  openssl
  zstd
  # bootloaders / firmware
  grub
  efibootmgr
  syslinux
  edk2-ovmf
)

echo "==> Syncing package databases and upgrading"
pacman -Syu --noconfirm

echo "==> Installing build dependencies"
pacman -S --needed --noconfirm "${PACMAN_PACKAGES[@]}"

echo "==> Creating build root at /root/horizon"
mkdir -p /root/horizon/{work,out,src,logs}

echo "==> Verifying toolchain"
for tool in mkarchiso pacstrap makepkg mksquashfs xorriso grub-mkstandalone; do
  if command -v "$tool" >/dev/null 2>&1; then
    printf '    ok      %s\n' "$tool"
  else
    printf '    MISSING %s\n' "$tool"
  fi
done

echo "==> Done. Build an ISO with scripts/build-iso.sh"
