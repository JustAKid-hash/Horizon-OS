# Horizon OS

A Linux distribution built on the Arch base, with pacman + AUR support, selectable
kernels (including a custom `horizon-kernel` built from Linux source), and its own
Rust tooling — growing from a CLI system into a full GUI desktop.

See [`docs/PLAN.md`](docs/PLAN.md) for the full design and rationale.

## Status

- [x] **Phase 0** — Foundation: build environment ready; stock Arch ISO builds and boots to a `root@archiso` shell under UEFI (OVMF) in QEMU, verified headlessly
- [ ] **Phase 1** — Horizon CLI OS v0.1 (branding, `horizon` CLI, TUI installer, AUR helper)
- [ ] **Phase 2** — Multi-kernel support (`linux`, `linux-lts`, `horizon-kernel`)
- [ ] **Phase 3** — GUI base + selectable desktops (Plasma, GNOME, Cinnamon, Hyprland, XFCE)
- [ ] **Phase 4** — `horizon-pm` (libalpm-backed package manager) + polish

## Layout

| Path | Purpose |
|---|---|
| `profile/` | archiso profile — builds the live ISO |
| `packages/` | our PKGBUILDs, published to the signed `[horizon]` repo |
| `src/` | Rust workspace — all original code |
| `config/` | kernel defconfig, mkinitcpio, pacman, systemd |
| `scripts/` | bootstrap / build / test automation |
| `iso/` | build output (gitignored) |

## Building

Builds run inside the Arch Linux WSL2 distro, because `mkarchiso`, `pacstrap` and
`makepkg` are native there. Do **not** build on `/mnt/d` — the 9p filesystem is far
too slow for squashfs and kernel work. `scripts/build-iso.sh` mirrors the repo into
`~/horizon` on the WSL ext4 filesystem automatically.

```powershell
$repo = 'D:\Albarr\VSC-Projects\Horizon-OS'

# 1. Set up the build host (mkarchiso deps, grub, efibootmgr, syslinux, ...)
wsl -d archlinux -e bash "/mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/bootstrap-wsl.sh"

# 2. Build the ISO
wsl -d archlinux -e bash "/mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/build-iso.sh"

# 3. Boot it
pwsh -File "$repo\scripts\run-qemu.ps1"
```

Until `profile/` contains the Horizon profile, `build-iso.sh` falls back to the
stock `releng` profile shipped with archiso.

## Testing

`scripts/run-qemu.ps1` boots the newest ISO in `iso/` under QEMU with WHPX
acceleration. UEFI (OVMF) is the default; pass `-Bios` to fall back to SeaBIOS.

```powershell
pwsh -File scripts\run-qemu.ps1                 # UEFI, 4G RAM, 4 vCPU
pwsh -File scripts\run-qemu.ps1 -Memory 8G      # more RAM
pwsh -File scripts\run-qemu.ps1 -Bios           # BIOS instead of UEFI
pwsh -File scripts\run-qemu.ps1 -Disk D:\vms\horizon.qcow2   # attach a disk
```

### Headless smoke test

`scripts/test-boot.ps1` is the automated gate: it boots the newest ISO with
`-display none`, waits, captures a screenshot through the QEMU monitor, converts
it to PNG, and shuts the VM down. Add `-Ocr` to also read the guest console back
as text, which is what makes the test self-verifying.

```powershell
pwsh -File scripts\test-boot.ps1 -Ocr            # boot, screenshot, OCR the console
pwsh -File scripts\test-boot.ps1 -WaitSeconds 300 -Ocr
pwsh -File scripts\test-boot.ps1 -KeepRunning    # leave the VM up
pwsh -File scripts\test-boot.ps1 -Bios -Ocr      # SeaBIOS path
```

A pass looks like the archiso banner and a `[root@archiso ~]#` prompt in the OCR
output. `scripts/ocr-screenshot.ps1` can also be run on any screenshot on its own;
it upscales the image and uses the OCR engine built into Windows, so it needs no
extra tooling.

