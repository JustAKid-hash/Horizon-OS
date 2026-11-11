//! Hardware and environment detection used to build a [`Plan`](crate::plan::Plan).

use std::fs;
use std::path::Path;
use std::process::Command;

/// Are we booted in UEFI mode?
pub fn is_uefi() -> bool {
    Path::new("/sys/firmware/efi").exists()
}

/// The microcode package matching this CPU.
pub fn cpu_ucode() -> &'static str {
    let text = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    if text.contains("GenuineIntel") {
        "intel-ucode"
    } else if text.contains("AuthenticAMD") {
        "amd-ucode"
    } else {
        "intel-ucode"
    }
}

#[derive(Debug, Clone)]
pub struct Disk {
    pub path: String,
    pub size: String,
    pub model: String,
}

/// Whole disks, from `lsblk`. Loop and optical devices are filtered out.
pub fn disks() -> Vec<Disk> {
    let out = match Command::new("lsblk")
        .args(["-dn", "-o", "NAME,SIZE,TYPE,MODEL"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut disks = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let size = it.next().unwrap_or("?").to_string();
        let kind = it.next().unwrap_or("?").to_string();
        let model = it.collect::<Vec<_>>().join(" ");
        if kind != "disk" {
            continue;
        }
        if name.starts_with("loop") || name.starts_with("sr") || name.starts_with("zram") {
            continue;
        }
        disks.push(Disk {
            path: format!("/dev/{name}"),
            size,
            model,
        });
    }
    disks
}

/// Is this a Horizon OS live medium (as opposed to an installed system)?
pub fn on_live_medium() -> bool {
    Path::new("/run/archiso").exists()
}
