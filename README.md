# Horizon OS

A Linux distribution built on the Arch base, with pacman + AUR support, selectable
kernels (including a custom `horizon-kernel` built from Linux source), and its own
Rust tooling — growing from a CLI system into a full GUI desktop.

See [`docs/PLAN.md`](docs/PLAN.md) for the full design and rationale.

**Status:** Phase 1a complete (Horizon-branded ISO builds and boots); Phase 1b in
progress (Rust `horizon` CLI and `horizon-install` TUI installer written and
building; signed `[horizon]` repository machinery in place). Pacman and the AUR are
used exactly as upstream ships them.

## Quick start

```powershell
# 1. Install the build toolchain into the Arch Linux WSL2 distro (once)
wsl -d archlinux -e bash "/mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/bootstrap-wsl.sh"

# 2. Build the ISO (~15 min)
wsl -d archlinux -e bash "/mnt/d/Albarr/VSC-Projects/Horizon-OS/scripts/build-iso.sh"

# 3. Boot it in QEMU
pwsh -File scripts\run-qemu.ps1
```

The live medium autologins `root` on tty1, so you land at a `[root@horizon ~]#`
prompt. See [Building](#building) and [Testing](#testing) for the details.

## Status

- [x] **Phase 0** — Foundation: build environment ready; stock Arch ISO builds and boots to a `root@archiso` shell under UEFI (OVMF) in QEMU, verified headlessly
- [x] **Phase 1a** — Horizon archiso profile: own branding (ISO name/label, hostname, `os-release`, MOTD, boot menus) on a trimmed package set; the resulting ISO builds and boots to a `[root@horizon ~]#` shell under UEFI (OVMF), verified headlessly
- [~] **Phase 1b** — Horizon CLI OS v0.1 (`horizon` CLI, TUI installer, signed `[horizon]` repo, desktop meta-packages)
- [ ] **Phase 2** — Multi-kernel support (`linux`, `linux-lts`, `horizon-kernel`)
- [ ] **Phase 3** — GUI polish (settings app, first-run wizard)
- [ ] **Phase 4** — Repository hosting, polish and releases

## Layout

| Path | Purpose |
|---|---|
| `profile/` | archiso profile — builds the live ISO |
| `packages/` | our PKGBUILDs, published to the signed `[horizon]` repo |
| `src/` | Rust workspace — all original code |
| `config/` | kernel defconfig, mkinitcpio, pacman, systemd |
| `scripts/` | bootstrap / build / test automation |
| `iso/` | build output (gitignored) |
| `LICENSE` | MIT |

## Requirements

To build: Windows with WSL2, an Arch Linux WSL2 distro, and ~10 GB free inside it.
`scripts/bootstrap-wsl.sh` installs everything else (`archiso`, `mkarchiso`,
`grub`, `efibootmgr`, `syslinux`, `base-devel`, …).

To test: QEMU for Windows with the WHPX accelerator, and `pwsh` (PowerShell 7).
On Linux, `qemu-system-x86_64` plus `edk2-ovmf` works just as well — only the
PowerShell test harness is Windows-specific.

## Building

Builds run inside the Arch Linux WSL2 distro, because `mkarchiso`, `pacstrap` and
`makepkg` are native there. Do **not** build on `/mnt/d` — the 9p filesystem is far
too slow for squashfs and kernel work. `scripts/build-iso.sh` mirrors the repo into
`~/horizon` on the WSL ext4 filesystem automatically.

```powershell
# Your clone, in Windows form and in WSL form (the scripts locate the repo
# root from their own path, so this only has to point at the real clone).
$repo    = 'D:\Albarr\VSC-Projects\Horizon-OS'
$wslRepo = '/mnt/d/Albarr/VSC-Projects/Horizon-OS'

# 1. Set up the build host (mkarchiso deps, grub, efibootmgr, syslinux, ...)
wsl -d archlinux -e bash "$wslRepo/scripts/bootstrap-wsl.sh"

# 2. Build the ISO (writes iso\horizon-<date>-x86_64.iso)
wsl -d archlinux -e bash "$wslRepo/scripts/build-iso.sh"

# 3. Boot it
pwsh -File "$repo\scripts\run-qemu.ps1"
```

Until `profile/` contains the Horizon profile, `build-iso.sh` falls back to the
stock `releng` profile shipped with archiso.

### The profile

`profile/` is a full archiso profile derived from Arch's `releng`, rebranded and
trimmed. `mkarchiso` substitutes `%INSTALL_DIR%`, `%ARCH%` and `%ARCHISO_UUID%`
in the bootloader configs, so `install_dir="horizon"` in `profiledef.sh` rebrands
every path inside the ISO automatically.

| What | Value |
|---|---|
| ISO name / label | `horizon` / `HORIZON_<YYYYMM>` |
| Volume layout | `/horizon/boot/x86_64/...` |
| Live hostname | `horizon` (so the prompt is `[root@horizon ~]#`) |
| `/etc/os-release` | `NAME="Horizon OS"`, `ID=horizon`, `ID_LIKE=arch` |
| Boot menus | `Horizon OS install medium (...)` (systemd-boot, GRUB, syslinux) |

The package list is Arch's `releng` list minus the server/forensics/cloning tools
a desktop distribution does not need (bind, cloud-init, clonezilla, nmap, openvpn,
testdisk, …) plus `base-devel` and `git` so AUR packages can be built in the live
environment. `cloud-init` is dropped, so its five `cloud-init.target.wants/`
symlinks are not shipped.

`airootfs` contains ~27 symlinks (systemd unit enablement, `/etc/localtime`,
`/etc/resolv.conf`, a `/dev/null` mask). Git records them as mode `120000`, which
only restores as real symlinks when `core.symlinks=true` and the machine has
Developer Mode on (or the checkout is elevated). If a fresh clone ever lands
without them:

```powershell
pwsh -File scripts\create-profile-symlinks.ps1
```

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

A pass looks like the Horizon MOTD and a `[root@horizon ~]#` prompt in the OCR
output. `scripts/ocr-screenshot.ps1` can also be run on any screenshot on its own;
it upscales the image and uses the OCR engine built into Windows, so it needs no
extra tooling.

## Running on real hardware

The ISO is a hybrid image: it boots from a USB stick under UEFI **and** under
legacy BIOS. Write it with any raw-image writer — on Windows, Rufus in **DD mode**
or `dd` from the WSL side:

```bash
# WSL, with the USB stick at /dev/sdX  (double-check this — it is destructive)
sudo dd if=/root/horizon/out/horizon-*.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

Then boot the machine from the stick. The systemd-boot / GRUB / syslinux menu
offers **Horizon OS install medium** and a speakup screen-reader variant. The live
medium ships `horizon-install`, a TUI installer that partitions, pacstraps and
configures a system (optionally with a desktop environment); nothing is written
to disk until you confirm on the final screen.

## Notes for contributors

- Everything in `profile/airootfs/` is Linux-side content; keep it LF. `.gitattributes`
  enforces that.
- The 27 symlinks under `profile/airootfs/` are load-bearing. If a checkout loses
  them, `scripts/create-profile-symlinks.ps1` puts them back.
- Builds are slow because of squashfs xz compression (a few minutes) and, later,
  kernel compiles (~30–45 min at `-j8`). Prefer `-comp zstd` in `profiledef.sh`
  while iterating on branding if you don't care about final image size.

## License

[MIT](LICENSE) — Copyright (c) 2026 ZeroByte.

That covers Horizon OS's own code (the profile, the scripts, and the Rust tooling
under `src/`). The Arch Linux packages the ISO redistributes keep their own
licenses, and Horizon OS is not affiliated with or endorsed by Arch Linux.

