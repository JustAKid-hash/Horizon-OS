//! The desktop environment catalog and the commands that act on it.
//!
//! The catalog lives in `data/desktops.toml` and is embedded at compile time, so
//! the binary is self-contained and the same file drives both the CLI and the
//! generated `horizon-desktop-*` meta-packages.

use std::collections::HashSet;
use std::io::Write;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{pacman, ui};

const CATALOG: &str = include_str!("../data/desktops.toml");

#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub meta: Meta,
    #[serde(default)]
    pub desktop: Vec<Desktop>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub version: u32,
    #[serde(default)]
    pub common: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Desktop {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub marker: String,
    #[serde(default)]
    pub display_manager: String,
    #[serde(default)]
    pub dm_service: String,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Catalog {
    pub fn load() -> Result<Self> {
        toml::from_str(CATALOG).context("parsing the embedded desktop catalog")
    }

    pub fn find(&self, id: &str) -> Result<&Desktop> {
        if let Some(d) = self.desktop.iter().find(|d| d.id.eq_ignore_ascii_case(id)) {
            return Ok(d);
        }
        let known: Vec<&str> = self.desktop.iter().map(|d| d.id.as_str()).collect();
        bail!(
            "unknown desktop '{id}'. Known desktops: {}",
            known.join(", ")
        )
    }

    /// Every package a full install of `d` pulls in, de-duplicated, common first.
    pub fn full_packages(&self, d: &Desktop, include_common: bool) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if include_common {
            out.extend(self.meta.common.iter().cloned());
        }
        out.extend(d.packages.iter().cloned());
        dedup_preserving_order(out)
    }
}

fn dedup_preserving_order(v: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

/// The longest id, used to line up the list.
fn id_width(cat: &Catalog) -> usize {
    cat.desktop.iter().map(|d| d.id.len()).max().unwrap_or(4)
}

pub fn cmd_list(json: bool) -> Result<()> {
    let cat = Catalog::load()?;

    if json {
        let items: Vec<_> = cat
            .desktop
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id,
                    "name": d.name,
                    "summary": d.summary,
                    "session": d.session,
                    "display_manager": d.display_manager,
                    "dm_service": d.dm_service,
                    "marker": d.marker,
                    "packages": d.packages,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    let w = id_width(&cat);
    println!(
        "{}",
        ui::bold("Desktop environments available for Horizon OS")
    );
    println!();
    for d in &cat.desktop {
        println!(
            "  {}  {}  {}",
            ui::cyan(&ui::pad(&d.id, w)),
            ui::pad(&d.name, 13),
            ui::dim(&d.summary)
        );
    }
    println!();
    println!(
        "  {} {}",
        ui::dim("Install one with:"),
        ui::bold("horizon desktop install <id>")
    );
    Ok(())
}

pub fn cmd_show(id: &str) -> Result<()> {
    let cat = Catalog::load()?;
    let d = cat.find(id)?;

    println!("{} {}", ui::bold(&d.name), ui::dim(&format!("({})", d.id)));
    println!("  {}", d.summary);
    println!();
    println!("  {} {}", ui::pad("Session", 16), d.session);
    println!(
        "  {} {}  ({})",
        ui::pad("Display manager", 16),
        d.display_manager,
        d.dm_service
    );
    println!(
        "  {} {} + {} shared",
        ui::pad("Packages", 16),
        d.packages.len(),
        cat.meta.common.len()
    );
    println!();
    println!("  {}", ui::bold("Packages"));
    for p in &d.packages {
        println!("    {}", p);
    }
    if !d.notes.is_empty() {
        println!();
        println!("  {}", ui::bold("Notes"));
        for n in &d.notes {
            println!("    {}", ui::dim(n));
        }
    }
    Ok(())
}

pub fn cmd_packages(id: &str, common: bool) -> Result<()> {
    let cat = Catalog::load()?;
    let d = cat.find(id)?;
    for p in cat.full_packages(d, common) {
        println!("{p}");
    }
    Ok(())
}

pub fn cmd_status() -> Result<()> {
    let cat = Catalog::load()?;
    if !pacman::available() {
        bail!("pacman is not installed; cannot report desktop status");
    }
    let installed = pacman::installed()?;

    let w = id_width(&cat);
    println!("{}", ui::bold("Installed desktop environments"));
    println!();
    let mut any = false;
    for d in &cat.desktop {
        let present = installed.contains(&d.marker);
        // Colour after padding: ANSI escapes would otherwise count as visible
        // characters and break the alignment.
        let state = if present {
            any = true;
            ui::green(&ui::pad("installed", 13))
        } else {
            ui::dim(&ui::pad("not installed", 13))
        };
        let dm = if present && !d.dm_service.is_empty() {
            match pacman::unit_enabled(&d.dm_service) {
                Some(true) => format!("  {} {}", ui::dim("dm:"), ui::green(&d.dm_service)),
                Some(false) => format!("  {} {}", ui::dim("dm:"), ui::yellow("disabled")),
                None => String::new(),
            }
        } else {
            String::new()
        };
        println!(
            "  {}  {} {}  {}{}",
            ui::cyan(&ui::pad(&d.id, w)),
            state,
            ui::pad(&d.name, 13),
            ui::dim(&d.session),
            dm
        );
    }
    if !any {
        println!();
        println!(
            "  {}",
            ui::dim("No desktop environment is installed. This is a CLI-only system.")
        );
        println!(
            "  {} {}",
            ui::dim("Install one with:"),
            ui::bold("horizon desktop install <id>")
        );
    }
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

pub fn cmd_install(
    id: &str,
    dry_run: bool,
    minimal: bool,
    no_display_manager: bool,
    yes: bool,
) -> Result<()> {
    let cat = Catalog::load()?;
    let d = cat.find(id)?;

    if !pacman::available() {
        bail!("pacman is not installed; Horizon OS installs desktops through pacman");
    }

    let packages = cat.full_packages(d, !minimal);

    println!(
        "{} {}",
        ui::bold(&format!("Installing {}", d.name)),
        ui::dim(&format!("({})", d.id))
    );
    println!();
    if minimal {
        println!(
            "  {} minimal install -- the shared desktop packages (fonts, audio, network applet) are skipped",
            ui::yellow("note:")
        );
    }
    println!("  {} {}", ui::pad("Packages", 16), packages.len());
    println!("  {} {}", ui::pad("Display manager", 16), d.display_manager);
    if !dry_run {
        println!();
        println!("  {}", ui::dim("This runs pacman and changes the system."));
    }
    println!();

    if !yes && !dry_run && !confirm("Proceed?")? {
        println!("{}", ui::dim("Aborted; nothing changed."));
        return Ok(());
    }

    println!();
    pacman::install(&packages, dry_run)?;

    if !no_display_manager && !d.dm_service.is_empty() {
        println!();
        println!("{}", ui::bold("Enabling services"));
        pacman::enable_unit(&d.dm_service, dry_run)?;
        // A desktop needs a network manager and, usually, bluetooth.
        pacman::enable_unit("NetworkManager.service", dry_run)?;
        if packages.iter().any(|p| p == "bluez") {
            pacman::enable_unit("bluetooth.service", dry_run)?;
        }
    }

    println!();
    if dry_run {
        println!("{}", ui::yellow("Dry run complete; nothing changed."));
    } else {
        println!("{} {} is installed.", ui::green("Done."), d.name);
    }

    if !d.notes.is_empty() {
        println!();
        for n in &d.notes {
            println!("  {} {}", ui::dim("*"), n);
        }
    }
    if !dry_run {
        println!();
        println!(
            "  {}",
            ui::dim("Reboot to reach the login screen, or start the display manager now:")
        );
        println!(
            "    {}",
            ui::bold(&format!("sudo systemctl start {}", d.dm_service))
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_loads_five_desktops() {
        let cat = Catalog::load().unwrap();
        assert_eq!(cat.meta.version, 1);
        assert_eq!(cat.desktop.len(), 5);
        let ids: Vec<&str> = cat.desktop.iter().map(|d| d.id.as_str()).collect();
        for expected in ["plasma", "gnome", "cinnamon", "hyprland", "xfce"] {
            assert!(ids.contains(&expected), "missing desktop {expected}");
        }
    }

    #[test]
    fn find_is_case_insensitive() {
        let cat = Catalog::load().unwrap();
        assert!(cat.find("PLASMA").is_ok());
        assert!(cat.find("nope").is_err());
    }

    #[test]
    fn full_packages_dedupes() {
        let cat = Catalog::load().unwrap();
        let d = cat.find("plasma").unwrap();
        let pkgs = cat.full_packages(d, true);
        let mut seen = HashSet::new();
        for p in &pkgs {
            assert!(seen.insert(p.as_str()), "duplicate package {p}");
        }
        assert_eq!(pkgs[0], "xdg-desktop-portal");
    }

    #[test]
    fn full_packages_minimal_omits_common() {
        let cat = Catalog::load().unwrap();
        let d = cat.find("xfce").unwrap();
        let full = cat.full_packages(d, true);
        let minimal = cat.full_packages(d, false);
        assert!(full.len() > minimal.len());
        assert!(!minimal.contains(&"firefox".to_string()));
    }
}
