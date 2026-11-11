# Horizon OS — Build Plan

> A Linux distribution built around the Linux kernel, with pacman + AUR support,
> custom kernel support, starting as a CLI OS and growing into a full GUI desktop
> with selectable desktop environments (KDE Plasma, GNOME, Cinnamon, Hyprland, XFCE).

**Status:** Phase 0 complete; Phase 1a complete — the Horizon ISO (`horizon-2026.11.11-x86_64.iso`, label `HORIZON_202611`) builds and boots to a `[root@horizon ~]#` shell under UEFI (OVMF), verified headlessly with the OCR boot test. Phase 1b in progress — `horizon` CLI and `horizon-install` TUI installer written and building; signed `[horizon]` repository machinery in place.
**Repo:** `D:\Albarr\VSC-Projects\Horizon-OS`
**Build host:** Arch Linux WSL2 distro (`wsl -d archlinux`)
**Test target:** QEMU VM with WHPX acceleration, UEFI (OVMF)

---

## 1. Confirmed decisions

| Decision | Choice |
|---|---|
| Base strategy | **Hybrid** — Arch-based rootfs so pacman/AUR work immediately, then progressively replace components with our own Rust implementations while keeping pacman compatibility |
| Kernel for v1 | **Both, selectable** — Arch stock `linux`, Arch `linux-lts`, and our `horizon-kernel` (built from `linux-7.3.4.tar.xz`) |
| Where it runs | **QEMU VM first** (WHPX acceleration), bare metal later |
| Code scope | **Custom tooling around stock Arch** — own CLI, own TUI installer, own kernel manager, own branding and desktop meta-packages; pacman and the AUR are used exactly as upstream ships them |

---

## 2. The core technical insight

**Pacman is a frontend. `libalpm` is the engine.**

Pacman is a C program that links `libalpm` — the library that actually parses
`.pkg.tar.zst`, resolves dependencies, and maintains the database at
`/var/lib/pacman`. The AUR does not talk to pacman at all; it talks to `makepkg`
and the package format.

**Decision (revised): pacman and the AUR are used exactly as upstream Arch ships
them.** Horizon OS does not reimplement either — they work very well, and every
Arch package, `PKGBUILD`, `makepkg` and AUR helper keeps working unmodified.
Horizon's own code is the CLI, the TUI installer, the kernel manager, the
branding and the desktop meta-packages.

Consequences:

- No `horizon-pm`, no `horizon-aur`. The system is always a normal Arch system.
- Our code layers on top of pacman rather than beside it, so there is nothing to
  keep in sync with `libalpm`.
- The AUR, `makepkg`, `PKGBUILD`s and every Arch package keep working permanently.

This is the difference between a distro that *replaces* pacman (breaking the AUR)
and one that *reimplements* it (keeping the AUR). It is the single most important
design decision in the project.

---

## 3. Environment audit (verified)

| Component | Status |
|---|---|
| Arch Linux WSL2 distro | Present, runs as root, systemd active, `pacman 7.1.0` / `libalpm 16.0.1` |
| `archiso` | Installed (92-1) |
| `arch-install-scripts` | Installed (32-1) |
| `mkinitcpio` | Installed (42.2-1) → `/usr/sbin/mkinitcpio` |
| `base`, `base-devel`, `binutils`, `sudo`, `rsync` | Installed |
| `dosfstools`, `e2fsprogs`, `squashfs-tools` | Installed |
| `edk2-ovmf` | Installed (202608-1) |
| Network → archlinux.org / aur.archlinux.org / mirrors | All HTTP 200 |
| Rust | 1.99.1 (cargo 1.99.1) |
| GCC (MSYS2 UCRT64) | 16.1.0 |
| QEMU | 11.0.50, `whpx` accelerator confirmed working |
| xorriso | 1.5.6 |
| OVMF firmware (Windows) | `C:\Program Files\qemu\share\edk2-x86_64-code.fd` + `edk2-i386-vars.fd` |
| `/proc/config.gz` | Readable (50,653 bytes) — reference kernel config |
| WSL resources | 8 CPUs, 7.7 GB RAM |
| Free space | 1.9 TB inside WSL, 56 GB on `D:` |
| **Missing** | `git`, `grub`, `efibootmgr` (installed in Phase 0); no AUR helper yet |
| Kernel source | `D:\Albarr\VSC-Projects\Horizon-OS\linux-7.3.4.tar.xz` — valid, 102,366 files |

---

## 4. Repository layout

```
Horizon-OS/
├── profile/                  archiso profile — the live ISO
│   ├── profiledef.sh
│   ├── packages.x86_64
│   ├── airootfs/             branding: os-release, issue, motd, hostname
│   └── efiboot/              systemd-boot, kernel selection menu
├── packages/                 PKGBUILDs → signed [horizon] repo
│   ├── horizon-branding/
│   ├── horizon-cli/
│   ├── horizon-install/
│   ├── horizon-kernel/       ← builds linux-7.3.4
│   ├── horizon-kernel-lts/   ← wrapper over Arch linux-lts
│   └── horizon-desktop-{plasma,gnome,cinnamon,hyprland,xfce}/
├── src/                      Rust workspace (all original code)
│   ├── horizon/              the distro CLI
│   ├── horizon-install/      TUI installer (ratatui)
│   ├── horizon-kernel/       kernel build/install/switch
│   └── horizon-hwd/          hardware detection — C
├── config/
│   ├── kernel/               Horizon defconfig + fragments
│   ├── mkinitcpio/
│   ├── pacman/
│   └── systemd/
├── scripts/
│   ├── bootstrap-wsl.sh
│   ├── build-iso.sh
│   ├── create-profile-symlinks.ps1
│   ├── run-qemu.ps1
│   ├── test-boot.ps1
│   └── ocr-screenshot.ps1
├── iso/                      build output (gitignored)
└── docs/
```

### Component roadmap — Arch component → Horizon replacement

| Layer | v1 (works immediately) | Horizon version | Language |
|---|---|---|---|
| Package manager | `pacman` (unchanged) | — | — |
| AUR helper | `paru`/`yay` (unchanged) | — | — |
| Installer | `archinstall` | `horizon-install` (TUI) | Rust |
| Kernel management | manual | `horizon-kernel` | Rust |
| Hardware detection | udev/hwdb | `horizon-hwd` | C |
| Desktop config | manual | `horizon-settings` | C++/Qt |
| Init (PID 1) | `systemd` | **deferred — see risks** | C |

### Language allocation

- **Rust** — distro CLI, TUI installer, kernel manager, daemon. The bulk of original code.
- **C** — `horizon-hwd` hardware detection (sysfs/DMI), custom `mkinitcpio` hooks, kernel patches/config work.
- **C++** — only from Phase 3: `horizon-settings` (Qt6/Kirigami). Not added before it is needed.

---

## 5. Kernel strategy

Multi-kernel from Phase 2. All kernels selectable at install time and from the
systemd-boot menu.

| Kernel | Source | Role |
|---|---|---|
| `linux` | Arch stock | Default, guaranteed boot |
| `linux-lts` | Arch LTS | Fallback |
| `horizon-kernel` | `linux-7.3.4.tar.xz`, `-horizon` localversion | Opt-in custom kernel |

**Config strategy:** base the Horizon kernel config on **Arch's official kernel
config** (so hardware compatibility matches Arch out of the box), then layer
Horizon fragments on top (branding, optional `-march` tuning, user choices).
`/proc/config.gz` from the WSL kernel serves as a reference only — it is a
WSL-specific config and lacks general hardware drivers.

**Tooling:** `horizon kernel list | build | install | switch | remove`.

---

## 6. Phases

### Phase 0 — Foundation & pipeline proof ✅
- Install `git`, `grub`, `efibootmgr`, and remaining build deps in the Arch WSL distro.
- Create `~/horizon` build root on the WSL ext4 filesystem (do **not** build on
  `/mnt/d` — the 9p filesystem makes kernel builds crawl). Repo stays on `D:` as
  the source of truth and is rsynced across before builds.
- Scaffold the repository structure.
- Build a **stock Arch ISO** with `mkarchiso` and boot it in QEMU + OVMF.
- **Deliverable:** unmodified Arch ISO boots in QEMU. Pipeline proven before any custom work. ✅

  Done. `archlinux-2026.11.11-x86_64.iso` builds (1.3 GB, BIOS syslinux + UEFI
  systemd-boot) and boots to a `root@archiso` shell under UEFI/OVMF, verified
  headlessly by `scripts/test-boot.ps1 -Ocr`.

### Phase 1a — Horizon profile and branding ✅
- Full archiso profile derived from `releng`, rebranded: ISO name `horizon`, label
  `HORIZON_<YYYYMM>`, volume layout `/horizon/...`, hostname `horizon`,
  `/etc/os-release` = `NAME="Horizon OS"`, own MOTD, and all three bootloaders
  retitled `Horizon OS install medium (...)`.
- Package list trimmed to a desktop distribution's needs (server/forensics/cloning
  tools dropped, `cloud-init` removed along with its five enablement symlinks) plus
  `base-devel` and `git` for in-live AUR builds.
- **Deliverable:** the ISO is Horizon-branded end to end and boots to `[root@horizon ~]#`. ✅
  Verified: `horizon-2026.11.11-x86_64.iso` (label `HORIZON_202611`, 358 packages)
  boots under UEFI/OVMF; the OCR of the console reads `Horizon OS (tty1)`,
  `horizon login: root (automatic login)` and the Horizon MOTD.

### Phase 1b — Horizon CLI OS v0.1
- Signed local `[horizon]` repository (`horizon-keyring`, `horizon-release`,
  `horizon-cli` and the `horizon-desktop-*` meta-packages).
- Rust workspace: `horizon` CLI + `horizon-install` TUI installer.
- Desktop environments as first-class citizens: `horizon desktop list|show|packages|status|install <id>` and `horizon-desktop-{plasma,gnome,cinnamon,hyprland,xfce}` meta-packages generated from a single catalog (`src/horizon/data/desktops.toml`).
- Horizon pacman config and mirrorlist (pacman and the AUR used unmodified).
- NetworkManager, user creation, sudo configuration.
- **Deliverable:** Horizon-branded ISO boots to a login TTY, installs to disk, offers a choice of desktop environment, pacman + AUR functional.

### Phase 2 — Multi-kernel support
- `horizon-kernel` PKGBUILD building `linux-7.3.4` with the Horizon config and
  `-horizon` localversion.
- `horizon kernel list | build | install | switch | remove`.
- Boot menu listing all installed kernels; kernel choice offered at install time.
- **Deliverable:** choose stock `linux`, `linux-lts`, or `horizon-kernel`.

### Phase 3 — GUI base and desktop polish
- Wayland/X11 stack, mesa, GPU drivers, SDDM/GDM (already pulled in by the Phase 1b desktop meta-packages).
- C++ `horizon-settings` (Qt6/Kirigami) for a unified settings app.
- First-run wizard, GUI installer.
- **Deliverable:** polished GUI with switchable desktop environments.

### Phase 4 — Hosting, polish and releases
- Repository hosting (public mirror) and package signing (already signed locally).
- GUI installer, first-run wizard.
- Release builds and ISO publishing.
- **Deliverable:** Horizon OS releases using its own tooling end to end.

---

## 7. Risks and honest caveats

- **Replacing systemd as PID 1 is not recommended.** It is the highest-risk,
  lowest-reward item: reimplementing service management, logging, cgroup handling
  and dependency ordering, while every package's unit files stop working. The
  recommendation is to keep systemd.
  This remains the user's call.
- Kernel compiles are ~30–45 minutes each at `-j8` (8 cores available in WSL).
- Full desktop environment installs are several GB.
- GUI/GPU validation is far better on bare metal than in QEMU.
- Building on `/mnt/d` is slow; builds must run from `~/horizon` in WSL ext4.

---

## 8. Immediate next step

**Phase 1b (in progress).** Remaining work:

1. Test `horizon-install` end to end in QEMU against a blank qcow2 (headless
   `--yes --force` path, then boot from disk).
2. Rebuild the ISO with the staged `[horizon]` repository and verify
   `horizon`/`horizon-install` work on the live medium.
3. Publish the repository (GitHub Pages or release assets) so installed systems
   can `pacman -S` Horizon packages.
4. `horizon-kernel` PKGBUILD building `linux-7.3.4` with the Horizon config.
