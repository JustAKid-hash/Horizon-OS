//! Thin wrappers around the tools Horizon OS deliberately keeps unchanged:
//! `pacman`, `systemctl` and `sudo`. Nothing here reimplements them.

use std::collections::HashSet;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::ui;

/// Is `pacman` on PATH?
pub fn available() -> bool {
    Command::new("pacman")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Are we running as uid 0?
pub fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

/// Is `sudo` on PATH?
pub fn has_sudo() -> bool {
    Command::new("sudo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The argv prefix needed to run a privileged command, or an error explaining
/// why we cannot.
pub fn privilege_prefix() -> Result<Vec<String>> {
    if is_root() {
        Ok(Vec::new())
    } else if has_sudo() {
        Ok(vec!["sudo".to_string()])
    } else {
        bail!("this needs root, and neither root nor sudo is available -- re-run as root")
    }
}

/// Every installed package name (`pacman -Qq`).
pub fn installed() -> Result<HashSet<String>> {
    let out = Command::new("pacman")
        .arg("-Qq")
        .output()
        .context("running `pacman -Qq`")?;
    if !out.status.success() {
        bail!("`pacman -Qq` failed");
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().map(|l| l.trim().to_string()).collect())
}

/// Run an argv, or print it when `dry_run` is set.
pub fn run(argv: &[String], dry_run: bool) -> Result<()> {
    if dry_run {
        println!("  {} {}", ui::dim("would run:"), argv.join(" "));
        return Ok(());
    }
    println!("  {} {}", ui::dim("running:"), argv.join(" "));
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .status()
        .with_context(|| format!("running `{}`", argv.join(" ")))?;
    if !status.success() {
        bail!("command failed: {}", argv.join(" "));
    }
    Ok(())
}

/// `pacman -S --needed <packages>`, elevated when necessary.
pub fn install(packages: &[String], dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        bail!("no packages to install");
    }
    let mut argv = privilege_prefix()?;
    argv.push("pacman".into());
    argv.push("-S".into());
    argv.push("--needed".into());
    argv.extend(packages.iter().cloned());
    run(&argv, dry_run)
}

/// `pacman -Syu`.
pub fn cmd_update(dry_run: bool) -> Result<()> {
    if !available() {
        bail!("pacman is not installed");
    }
    println!("{}", ui::bold("Updating Horizon OS"));
    println!(
        "  {}",
        ui::dim("pacman and the AUR are used unmodified; this is a thin wrapper.")
    );
    println!();
    let mut argv = privilege_prefix()?;
    argv.push("pacman".into());
    argv.push("-Syu".into());
    run(&argv, dry_run)
}

/// `systemctl enable --now <unit>`.
pub fn enable_unit(unit: &str, dry_run: bool) -> Result<()> {
    let mut argv = privilege_prefix()?;
    argv.push("systemctl".into());
    argv.push("enable".into());
    argv.push("--now".into());
    argv.push(unit.into());
    run(&argv, dry_run)
}

/// Is a systemd unit enabled? Returns `None` when systemctl is unavailable.
pub fn unit_enabled(unit: &str) -> Option<bool> {
    Command::new("systemctl")
        .args(["is-enabled", "--quiet", unit])
        .status()
        .ok()
        .map(|s| s.success())
}

/// Is a systemd unit active? Returns `None` when systemctl is unavailable.
pub fn unit_active(unit: &str) -> Option<bool> {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .ok()
        .map(|s| s.success())
}
