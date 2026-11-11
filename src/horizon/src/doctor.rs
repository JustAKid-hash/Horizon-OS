//! `horizon doctor` -- a short list of health checks.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::pacman;
use crate::sysinfo::OsRelease;
use crate::ui::{self, Level};

struct Check {
    level: Level,
    name: String,
    detail: String,
}

fn curl_head(url: &str) -> Option<bool> {
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-m", "8", "-I", url])
        .output()
        .ok()?;
    Some(out.status.success())
}

fn free_space_on_root() -> Option<String> {
    let out = Command::new("df").args(["-h", "/"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() >= 4 {
        Some(format!("{} free of {}", cols[3], cols[1]))
    } else {
        None
    }
}

fn pending_updates() -> Option<usize> {
    let out = Command::new("pacman").args(["-Qu"]).output().ok()?;
    if !out.status.success() {
        // `pacman -Qu` exits non-zero when there is nothing to do.
        return Some(0);
    }
    Some(String::from_utf8_lossy(&out.stdout).lines().count())
}

fn checks() -> Vec<Check> {
    let mut out = Vec::new();

    // 1. Are we on Horizon OS?
    match OsRelease::read() {
        Some(r) if r.is_horizon() => out.push(Check {
            level: Level::Ok,
            name: "Distribution".into(),
            detail: format!("{} (id: horizon)", r.pretty_name),
        }),
        Some(r) => out.push(Check {
            level: Level::Warn,
            name: "Distribution".into(),
            detail: format!("{} -- not Horizon OS (id: {})", r.pretty_name, r.id),
        }),
        None => out.push(Check {
            level: Level::Warn,
            name: "Distribution".into(),
            detail: "no /etc/os-release".into(),
        }),
    }

    // 2. pacman.
    if pacman::available() {
        let n = pacman::installed().map(|s| s.len()).unwrap_or(0);
        out.push(Check {
            level: Level::Ok,
            name: "pacman".into(),
            detail: format!("present, {n} packages installed"),
        });
    } else {
        out.push(Check {
            level: Level::Fail,
            name: "pacman".into(),
            detail: "not found on PATH".into(),
        });
    }

    // 3. Database lock.
    if Path::new("/var/lib/pacman/db.lck").exists() {
        out.push(Check {
            level: Level::Warn,
            name: "pacman lock".into(),
            detail: "db.lck exists -- another pacman may be running".into(),
        });
    } else {
        out.push(Check {
            level: Level::Ok,
            name: "pacman lock".into(),
            detail: "clear".into(),
        });
    }

    // 4. systemd as PID 1.
    match fs::read_to_string("/proc/1/comm") {
        Ok(c) if c.trim() == "systemd" => out.push(Check {
            level: Level::Ok,
            name: "Init".into(),
            detail: "systemd (PID 1)".into(),
        }),
        Ok(c) => out.push(Check {
            level: Level::Warn,
            name: "Init".into(),
            detail: format!("PID 1 is {}", c.trim()),
        }),
        Err(_) => {}
    }

    // 5. Kernels in /boot.
    let kernels = fs::read_dir("/boot")
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("vmlinuz-"))
                .count()
        })
        .unwrap_or(0);
    if kernels > 0 {
        out.push(Check {
            level: Level::Ok,
            name: "Kernels".into(),
            detail: format!("{kernels} in /boot"),
        });
    } else {
        out.push(Check {
            level: Level::Fail,
            name: "Kernels".into(),
            detail: "no vmlinuz-* in /boot".into(),
        });
    }

    // 6. Disk space.
    if let Some(free) = free_space_on_root() {
        out.push(Check {
            level: Level::Info,
            name: "Disk".into(),
            detail: format!("/ has {free}"),
        });
    }

    // 7. Network to the Arch mirrors and the AUR (the two things Horizon OS relies on).
    if Command::new("curl").arg("--version").output().is_ok() {
        for (label, url) in [
            ("archlinux.org", "https://archlinux.org"),
            ("aur.archlinux.org", "https://aur.archlinux.org"),
        ] {
            match curl_head(url) {
                Some(true) => out.push(Check {
                    level: Level::Ok,
                    name: format!("Network {label}"),
                    detail: "reachable".into(),
                }),
                Some(false) => out.push(Check {
                    level: Level::Warn,
                    name: format!("Network {label}"),
                    detail: "unreachable".into(),
                }),
                None => {}
            }
        }
    }

    // 8. Pending updates.
    if let Some(n) = pending_updates() {
        out.push(Check {
            level: if n == 0 { Level::Ok } else { Level::Info },
            name: "Updates".into(),
            detail: if n == 0 {
                "up to date".into()
            } else {
                format!("{n} pending -- run `horizon update`")
            },
        });
    }

    out
}

pub fn cmd_doctor() -> Result<()> {
    println!("{}", ui::bold("Horizon OS health check"));
    println!();
    let mut worst = Level::Ok;
    for c in checks() {
        println!(
            "  {}  {}  {}",
            ui::tag(c.level),
            ui::pad(&c.name, 20),
            c.detail
        );
        worst = match (worst, c.level) {
            (_, Level::Fail) => Level::Fail,
            (Level::Ok, Level::Warn) => Level::Warn,
            (w, _) => w,
        };
    }
    println!();
    match worst {
        Level::Fail => println!("  {}", ui::red("One or more checks failed.")),
        Level::Warn => println!("  {}", ui::yellow("Completed with warnings.")),
        _ => println!("  {}", ui::green("All checks passed.")),
    }
    Ok(())
}
