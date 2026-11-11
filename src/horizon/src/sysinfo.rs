//! System and distribution information, read from `/etc/os-release` and `/proc`.

use std::fs;

use anyhow::Result;

use crate::desktops::Catalog;
use crate::ui;

/// The fields of `/etc/os-release` that Horizon OS cares about.
#[derive(Debug, Default, Clone)]
pub struct OsRelease {
    pub name: String,
    pub pretty_name: String,
    pub id: String,
    pub id_like: String,
    pub version_id: String,
    pub image_version: String,
    pub home_url: String,
}

impl OsRelease {
    pub fn read() -> Option<Self> {
        let text = fs::read_to_string("/etc/os-release").ok()?;
        let mut rel = OsRelease::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"').to_string();
            match key.trim() {
                "NAME" => rel.name = value,
                "PRETTY_NAME" => rel.pretty_name = value,
                "ID" => rel.id = value,
                "ID_LIKE" => rel.id_like = value,
                "VERSION_ID" => rel.version_id = value,
                "IMAGE_VERSION" => rel.image_version = value,
                "HOME_URL" => rel.home_url = value,
                _ => {}
            }
        }
        Some(rel)
    }

    /// Is this a Horizon OS system (rather than plain Arch or something else)?
    pub fn is_horizon(&self) -> bool {
        self.id == "horizon"
    }
}

fn proc_sys_kernel(field: &str) -> String {
    fs::read_to_string(format!("/proc/sys/kernel/{field}"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn cpu_model() -> String {
    let text = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    text.lines()
        .find_map(|l| {
            l.strip_prefix("model name")
                .map(|v| v.trim_start_matches([':', ' ']).to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn cpu_count() -> usize {
    fs::read_to_string("/proc/cpuinfo")
        .map(|t| t.lines().filter(|l| l.starts_with("processor")).count())
        .unwrap_or(0)
}

fn mem_total_mib() -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(kb / 1024)
}

fn uptime_human() -> String {
    let Ok(text) = fs::read_to_string("/proc/uptime") else {
        return "unknown".to_string();
    };
    let secs = text
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn row(label: &str, value: &str) {
    println!("  {} {}", ui::pad(label, 16), value);
}

pub fn cmd_info() -> Result<()> {
    let rel = OsRelease::read();

    println!(
        "{}",
        ui::bold(
            &rel.as_ref()
                .map(|r| r.pretty_name.clone())
                .unwrap_or_else(|| "Unknown system".into())
        )
    );
    println!();

    match &rel {
        Some(r) => {
            let mut detail = format!("id: {}", r.id);
            if !r.id_like.is_empty() {
                detail.push_str(&format!(", like: {}", r.id_like));
            }
            row("Distribution", &format!("{}  ({})", r.name, detail));
            if !r.home_url.is_empty() {
                row("Homepage", &r.home_url);
            }
        }
        None => row("Distribution", "unknown (no /etc/os-release)"),
    }

    row("Kernel", &proc_sys_kernel("osrelease"));
    row("Architecture", std::env::consts::ARCH);
    row("Hostname", &hostname());
    row("Uptime", &uptime_human());
    row("CPU", &format!("{} x{}", cpu_model(), cpu_count()));
    if let Some(mib) = mem_total_mib() {
        row("Memory", &format!("{:.1} GiB", mib as f64 / 1024.0));
    }

    if crate::pacman::available() {
        if let Ok(pkgs) = crate::pacman::installed() {
            row("Packages", &format!("{} installed (pacman)", pkgs.len()));
        }
    } else {
        row("Packages", "pacman not found");
    }

    if let Ok(cat) = Catalog::load() {
        if let Ok(installed) = crate::pacman::installed() {
            let names: Vec<String> = cat
                .desktop
                .iter()
                .filter(|d| installed.contains(&d.marker))
                .map(|d| d.name.clone())
                .collect();
            if !names.is_empty() {
                row("Desktops", &names.join(", "));
            }
        }
    }

    Ok(())
}

pub fn cmd_about() -> Result<()> {
    let wordmark = [
        r"  ██╗  ██╗ ██████╗ ██████╗ ██╗███████╗ ██████╗ ███╗   ██╗",
        r"  ██║  ██║██╔═══██╗██╔══██╗██║╚══███╔╝██╔═══██╗████╗  ██║",
        r"  ███████║██║   ██║██████╔╝██║  ███╔╝ ██║   ██║██╔██╗ ██║",
        r"  ██╔══██║██║   ██║██╔══██╗██║ ███╔╝  ██║   ██║██║╚██╗██║",
        r"  ██║  ██║╚██████╔╝██║  ██║██║███████╗╚██████╔╝██║ ╚████║",
        r"  ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═══╝",
    ];
    for line in wordmark {
        println!("{}", ui::cyan(line));
    }
    println!();
    println!("  {} {}", ui::bold("Horizon OS"), ui::dim("0.1"));
    println!("  An Arch-based distribution with pacman + AUR support and");
    println!("  selectable desktop environments.");
    println!();
    println!(
        "  {}",
        ui::dim("https://github.com/JustAKid-hash/Horizon-OS")
    );
    Ok(())
}
