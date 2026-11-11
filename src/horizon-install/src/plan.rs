//! What the installer is going to do, independent of how it was decided.

use horizon::desktops::Desktop;

#[derive(Debug, Clone)]
pub struct Plan {
    /// Whole disk to erase and install onto, e.g. `/dev/sda` or `/dev/nvme0n1`.
    pub disk: String,
    /// Install an EFI system partition and an EFI bootloader.
    pub uefi: bool,
    /// Kernel package: `linux` or `linux-lts`.
    pub kernel: String,
    /// Desktop environment to install, or `None` for a CLI-only system.
    pub desktop: Option<Desktop>,
    pub hostname: String,
    pub username: String,
    /// Plain-text password, held only long enough to write it into the new
    /// system's `/etc/shadow` via `chpasswd`.
    pub password: String,
    /// Print the commands instead of running them.
    pub dry_run: bool,
}

impl Plan {
    /// The ESP partition path.
    pub fn efi_partition(&self) -> String {
        partition(&self.disk, 1)
    }

    /// The root partition path.
    pub fn root_partition(&self) -> String {
        partition(&self.disk, 2)
    }
}

/// `/dev/sda` + 1 -> `/dev/sda1`, but `/dev/nvme0n1` + 1 -> `/dev/nvme0n1p1`.
pub fn partition(disk: &str, n: u32) -> String {
    match disk.chars().last() {
        Some(c) if c.is_ascii_digit() => format!("{disk}p{n}"),
        _ => format!("{disk}{n}"),
    }
}

/// Reject hostnames and usernames that would break the generated script.
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}
