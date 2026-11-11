//! The install engine: turns a [`Plan`] into a sequence of system commands.
//!
//! Everything here shells out to the standard Arch tools -- `sgdisk`, `mkfs.*`,
//! `pacstrap`, `genfstab`, `arch-chroot`. Horizon OS deliberately has no
//! partitioning or package code of its own, so an installed system is exactly an
//! Arch system plus Horizon branding and the packages the user chose.

use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::plan::Plan;
use crate::system;

/// Progress messages emitted while installing.
pub enum Event {
    Log(String),
    Progress(u16),
}

/// Run a plan to completion. `emit` receives log lines and progress updates.
pub fn run(plan: &Plan, emit: &mut dyn FnMut(Event)) -> Result<()> {
    let mut r = Runner {
        plan,
        emit,
        done: 0,
        total: 8,
    };
    r.install()
}

/// Remove duplicates while keeping the first occurrence's position, so the
/// pacstrap line reads in the order the sets were assembled.
fn dedup_preserving_order(v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

struct Runner<'a> {
    plan: &'a Plan,
    emit: &'a mut dyn FnMut(Event),
    done: u16,
    total: u16,
}

impl Runner<'_> {
    fn log(&mut self, line: impl Into<String>) {
        (self.emit)(Event::Log(line.into()));
    }

    fn phase(&mut self, title: &str) {
        self.done += 1;
        let pct = (self.done as u32 * 100 / self.total as u32).min(100) as u16;
        (self.emit)(Event::Progress(pct));
        (self.emit)(Event::Log(format!("==> {title}")));
    }

    fn cmd<S: AsRef<str>>(&mut self, argv: &[S]) -> Result<()> {
        let parts: Vec<String> = argv.iter().map(|s| s.as_ref().to_string()).collect();
        let pretty = parts.join(" ");
        self.log(format!("$ {pretty}"));
        if self.plan.dry_run {
            return Ok(());
        }
        let status = Command::new(&parts[0])
            .args(&parts[1..])
            .status()
            .with_context(|| format!("running `{pretty}`"))?;
        if !status.success() {
            bail!("command failed: {pretty}");
        }
        Ok(())
    }

    /// Like [`cmd`](Self::cmd) but a non-zero exit is not fatal.
    fn cmd_ok<S: AsRef<str>>(&mut self, argv: &[S]) {
        let parts: Vec<String> = argv.iter().map(|s| s.as_ref().to_string()).collect();
        let pretty = parts.join(" ");
        self.log(format!("$ {pretty}   (failure ignored)"));
        if self.plan.dry_run {
            return;
        }
        let _ = Command::new(&parts[0]).args(&parts[1..]).status();
    }

    fn output<S: AsRef<str>>(&mut self, argv: &[S]) -> Result<String> {
        let parts: Vec<String> = argv.iter().map(|s| s.as_ref().to_string()).collect();
        let pretty = parts.join(" ");
        self.log(format!("$ {pretty}"));
        if self.plan.dry_run {
            return Ok(String::new());
        }
        let out = Command::new(&parts[0])
            .args(&parts[1..])
            .output()
            .with_context(|| format!("running `{pretty}`"))?;
        if !out.status.success() {
            bail!("command failed: {pretty}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    fn install(&mut self) -> Result<()> {
        let disk = self.plan.disk.clone();
        let uefi = self.plan.uefi;
        let efi = self.plan.efi_partition();
        let root = self.plan.root_partition();
        let kernel = self.plan.kernel.clone();
        let ucode = system::cpu_ucode().to_string();

        self.log(format!("Horizon OS -> {disk}"));
        self.log(format!(
            "Firmware: {}",
            if uefi { "UEFI" } else { "BIOS (legacy)" }
        ));

        // 1 -- clear anything already mounted from the target.
        self.phase("Unmounting anything using the target disk");
        self.cmd_ok(&["umount", "-R", "/mnt"]);
        self.cmd_ok(&["swapoff", "-a"]);

        // 2 -- partition.
        self.phase("Partitioning");
        self.cmd(&["sgdisk", "--zap-all", &disk])?;
        if uefi {
            self.cmd(&[
                "sgdisk",
                "-n",
                "1:0:+512M",
                "-t",
                "1:ef00",
                "-c",
                "1:EFI system partition",
                &disk,
            ])?;
        } else {
            self.cmd(&[
                "sgdisk",
                "-n",
                "1:0:+1M",
                "-t",
                "1:ef02",
                "-c",
                "1:BIOS boot partition",
                &disk,
            ])?;
        }
        self.cmd(&[
            "sgdisk",
            "-n",
            "2:0:0",
            "-t",
            "2:8300",
            "-c",
            "2:Horizon OS root",
            &disk,
        ])?;
        self.cmd_ok(&["partprobe", &disk]);
        self.cmd_ok(&["udevadm", "settle"]);

        // 3 -- filesystems.
        self.phase("Creating filesystems");
        if uefi {
            self.cmd(&["mkfs.fat", "-F32", "-n", "EFI", &efi])?;
        }
        self.cmd(&["mkfs.ext4", "-F", "-L", "horizon", &root])?;

        // 4 -- mount.
        self.phase("Mounting the new system");
        self.cmd(&["mount", &root, "/mnt"])?;
        if uefi {
            self.cmd(&["mount", "--mkdir", &efi, "/mnt/boot"])?;
        }

        // 5 -- base system.
        self.phase("Installing packages (pacstrap)");
        let mut packages: Vec<String> = vec![
            "base".into(),
            kernel.clone(),
            "linux-firmware".into(),
            ucode.clone(),
            "sudo".into(),
            "nano".into(),
            "man-db".into(),
            "networkmanager".into(),
            "grub".into(),
        ];
        if uefi {
            packages.push("efibootmgr".into());
        }
        if let Some(desktop) = &self.plan.desktop {
            if let Ok(catalog) = horizon::desktops::Catalog::load() {
                packages.extend(catalog.full_packages(desktop, true));
            }
        }
        // The base set and the shared desktop set overlap (sudo, nano,
        // networkmanager, man-db). pacman tolerates repeats, but a clean argv is
        // easier to read in the log and in --dry-run output.
        let packages = dedup_preserving_order(packages);
        let mut argv: Vec<String> = vec!["pacstrap".into(), "-K".into(), "/mnt".into()];
        argv.extend(packages.iter().cloned());
        self.cmd(&argv)?;

        // 5b -- carry the Horizon CLI over from the live medium, if present.
        if std::path::Path::new("/usr/bin/horizon").exists() {
            self.log("carrying /usr/bin/horizon into the new system".to_string());
            if !self.plan.dry_run {
                let _ = std::fs::create_dir_all("/mnt/usr/bin");
                let _ = std::fs::copy("/usr/bin/horizon", "/mnt/usr/bin/horizon");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        "/mnt/usr/bin/horizon",
                        std::fs::Permissions::from_mode(0o755),
                    );
                }
            }
        }

        // 6 -- fstab.
        self.phase("Writing /etc/fstab");
        let fstab = self.output(&["genfstab", "-U", "/mnt"])?;
        if !self.plan.dry_run {
            let mut f = fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open("/mnt/etc/fstab")
                .context("opening /mnt/etc/fstab")?;
            use std::io::Write;
            f.write_all(fstab.as_bytes())?;
        }

        // 7 -- configuration inside the new system.
        self.phase("Configuring the new system");
        let script = self.setup_script(&disk, uefi);
        let users = format!(
            "root:{pw}\n{user}:{pw}\n",
            pw = self.plan.password,
            user = self.plan.username
        );
        if !self.plan.dry_run {
            fs::write("/mnt/root/horizon-setup.sh", script)
                .context("writing /mnt/root/horizon-setup.sh")?;
            fs::write("/mnt/root/horizon-users.txt", users)
                .context("writing /mnt/root/horizon-users.txt")?;
        }
        self.cmd_ok(&["chmod", "600", "/mnt/root/horizon-users.txt"]);
        self.cmd(&["arch-chroot", "/mnt", "bash", "/root/horizon-setup.sh"])?;
        self.cmd(&[
            "arch-chroot",
            "/mnt",
            "sh",
            "-c",
            "chpasswd < /root/horizon-users.txt",
        ])?;
        self.cmd_ok(&[
            "rm",
            "-f",
            "/mnt/root/horizon-setup.sh",
            "/mnt/root/horizon-users.txt",
        ]);

        // 8 -- done.
        self.phase("Unmounting");
        self.cmd_ok(&["umount", "-R", "/mnt"]);

        self.log("");
        self.log(format!(
            "Installed Horizon OS on {disk} (root {}, boot {})",
            root,
            if uefi { efi.as_str() } else { "BIOS/MBR" }
        ));
        Ok(())
    }

    /// The bash script that runs inside `arch-chroot`.
    fn setup_script(&self, disk: &str, uefi: bool) -> String {
        let hostname = &self.plan.hostname;
        let username = &self.plan.username;

        let dm = match &self.plan.desktop {
            Some(d) if !d.dm_service.is_empty() => format!(
                "systemctl enable {}\nsystemctl set-default graphical.target",
                d.dm_service
            ),
            _ => "systemctl set-default multi-user.target".to_string(),
        };

        let bootloader = if uefi {
            "grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=Horizon --recheck"
                .to_string()
        } else {
            format!("grub-install --target=i386-pc --recheck {disk}")
        };

        let mut s = String::new();
        let _ = writeln!(s, "#!/usr/bin/env bash");
        let _ = writeln!(s, "set -euo pipefail");
        let _ = writeln!(
            s,
            "# Generated by horizon-install. Runs inside the new system."
        );
        let _ = writeln!(s, "ln -sf /usr/share/zoneinfo/UTC /etc/localtime");
        let _ = writeln!(s, "hwclock --systohc");
        let _ = writeln!(s, "echo 'en_US.UTF-8 UTF-8' > /etc/locale.gen");
        let _ = writeln!(s, "locale-gen");
        let _ = writeln!(s, "echo 'LANG=en_US.UTF-8' > /etc/locale.conf");
        let _ = writeln!(s, "echo '{hostname}' > /etc/hostname");
        let _ = writeln!(s, "cat > /etc/hosts <<'EOF'");
        let _ = writeln!(s, "127.0.0.1   localhost");
        let _ = writeln!(s, "::1         localhost");
        let _ = writeln!(s, "127.0.1.1   {hostname}.localdomain {hostname}");
        let _ = writeln!(s, "EOF");
        let _ = writeln!(s, "useradd -m -G wheel -s /bin/bash '{username}'");
        let _ = writeln!(
            s,
            "echo '%wheel ALL=(ALL:ALL) ALL' > /etc/sudoers.d/10-wheel"
        );
        let _ = writeln!(s, "chmod 440 /etc/sudoers.d/10-wheel");
        let _ = writeln!(s, "systemctl enable NetworkManager.service");
        let _ = writeln!(s, "systemctl enable bluetooth.service || true");
        let _ = writeln!(s, "{dm}");
        let _ = writeln!(s, "mkinitcpio -P");
        let _ = writeln!(s, "{bootloader}");
        let _ = writeln!(s, "grub-mkconfig -o /boot/grub/grub.cfg");
        // Branding. NOTE: /etc/os-release is owned by the `filesystem` package;
        // a `horizon-release` package that owns it properly arrives with the
        // signed [horizon] repository.
        let _ = writeln!(s, "cat > /etc/os-release <<'EOF'");
        let _ = writeln!(s, "NAME=\"Horizon OS\"");
        let _ = writeln!(s, "PRETTY_NAME=\"Horizon OS\"");
        let _ = writeln!(s, "ID=horizon");
        let _ = writeln!(s, "ID_LIKE=arch");
        let _ = writeln!(s, "BUILD_ID=rolling");
        let _ = writeln!(s, "ANSI_COLOR=\"38;2;23;147;209\"");
        let _ = writeln!(
            s,
            "HOME_URL=\"https://github.com/JustAKid-hash/Horizon-OS\""
        );
        let _ = writeln!(
            s,
            "DOCUMENTATION_URL=\"https://github.com/JustAKid-hash/Horizon-OS/tree/main/docs\""
        );
        let _ = writeln!(
            s,
            "SUPPORT_URL=\"https://github.com/JustAKid-hash/Horizon-OS/issues\""
        );
        let _ = writeln!(
            s,
            "BUG_REPORT_URL=\"https://github.com/JustAKid-hash/Horizon-OS/issues\""
        );
        let _ = writeln!(s, "EOF");
        let _ = writeln!(s, "cat > /etc/motd <<'EOF'");
        let _ = writeln!(s);
        let _ = writeln!(s, "  Welcome to Horizon OS 0.1");
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "  Pacman and the AUR work exactly as on Arch. `horizon --help` lists"
        );
        let _ = writeln!(
            s,
            "  the Horizon tools; `horizon desktop list` shows more desktops."
        );
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "  Documentation : https://github.com/JustAKid-hash/Horizon-OS"
        );
        let _ = writeln!(s);
        let _ = writeln!(s, "EOF");
        s
    }
}
