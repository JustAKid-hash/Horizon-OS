//! Kernel listing.
//!
//! Phase 2 adds `horizon kernel build | install | switch | remove` and the
//! `horizon-kernel` package. For now this only reports what is in `/boot`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::ui;

fn running_release() -> String {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn cmd_list() -> Result<()> {
    let boot = Path::new("/boot");
    let mut versions: BTreeSet<String> = BTreeSet::new();

    let entries = match fs::read_dir(boot) {
        Ok(e) => e,
        Err(_) => {
            println!(
                "{}",
                ui::yellow("/boot is not readable; cannot list kernels.")
            );
            return Ok(());
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(v) = name.strip_prefix("vmlinuz-") {
            versions.insert(v.to_string());
        } else if let Some(rest) = name.strip_prefix("initramfs-") {
            if let Some(v) = rest.strip_suffix(".img") {
                versions.insert(v.to_string());
            }
        }
    }

    let running = running_release();

    println!("{}", ui::bold("Kernels installed in /boot"));
    println!();
    if versions.is_empty() {
        println!("  {}", ui::dim("None found."));
        return Ok(());
    }

    println!(
        "  {}  {}  {}  status",
        ui::pad("Version", 28),
        ui::pad("kernel", 7),
        ui::pad("initramfs", 10)
    );
    for v in &versions {
        let has_vmlinuz = boot.join(format!("vmlinuz-{v}")).exists();
        let has_initramfs = boot.join(format!("initramfs-{v}.img")).exists();
        let status = if *v == running {
            ui::green("running")
        } else {
            ui::dim("")
        };
        println!(
            "  {}  {}  {}  {}",
            ui::pad(v, 28),
            ui::pad(if has_vmlinuz { "yes" } else { "-" }, 7),
            ui::pad(if has_initramfs { "yes" } else { "-" }, 10),
            status
        );
    }

    println!();
    println!(
        "  {}",
        ui::dim("Building and switching kernels arrives with Phase 2 (horizon-kernel).")
    );
    Ok(())
}
