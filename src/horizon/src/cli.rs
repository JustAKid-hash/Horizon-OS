//! Command-line surface. Clap definitions live here; the work lives in the
//! sibling modules.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{desktops, doctor, kernel, meta, pacman, sysinfo};

#[derive(Debug, Parser)]
#[command(
    name = "horizon",
    version,
    about = "Horizon OS distribution CLI",
    long_about = "Horizon OS distribution CLI\n\nReports on the machine, installs desktop environments and lists kernels.\nPacman and the AUR are used exactly as upstream Arch ships them.",
    propagate_version = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show distribution and system information
    Info,
    /// Print the Horizon OS wordmark
    About,
    /// Check the health of the system
    Doctor,
    /// List the kernels installed in /boot
    Kernel,
    /// Update the system (wraps `pacman -Syu`)
    Update {
        /// Print the command instead of running it
        #[arg(long)]
        dry_run: bool,
    },
    /// Work with desktop environments
    #[command(subcommand)]
    Desktop(DesktopCmd),
}

#[derive(Debug, Subcommand)]
pub enum DesktopCmd {
    /// List the desktop environments Horizon OS can install
    List {
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Show one desktop in detail
    Show {
        /// Desktop id, as shown by `horizon desktop list`
        id: String,
    },
    /// Print the package list for a desktop, one per line
    Packages {
        /// Desktop id
        id: String,
        /// Include the packages shared by every desktop
        #[arg(long)]
        common: bool,
    },
    /// Show which desktops are installed on this machine
    Status,
    /// Install a desktop environment with pacman
    Install {
        /// Desktop id
        id: String,
        /// Show what would run, changing nothing
        #[arg(long)]
        dry_run: bool,
        /// Skip the packages shared by every desktop
        #[arg(long)]
        minimal: bool,
        /// Install the packages but do not enable the display manager
        #[arg(long)]
        no_display_manager: bool,
        /// Do not ask for confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Generate the meta-package PKGBUILD for a desktop
    #[command(name = "gen-pkgbuild")]
    GenPkgbuild {
        /// Desktop id
        id: String,
        /// Directory to write the PKGBUILD into
        out_dir: PathBuf,
    },
}

pub fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Info => sysinfo::cmd_info(),
        Command::About => sysinfo::cmd_about(),
        Command::Doctor => doctor::cmd_doctor(),
        Command::Kernel => kernel::cmd_list(),
        Command::Update { dry_run } => pacman::cmd_update(dry_run),
        Command::Desktop(cmd) => dispatch_desktop(cmd),
    }
}

fn dispatch_desktop(cmd: DesktopCmd) -> Result<()> {
    match cmd {
        DesktopCmd::List { json } => desktops::cmd_list(json),
        DesktopCmd::Show { id } => desktops::cmd_show(&id),
        DesktopCmd::Packages { id, common } => desktops::cmd_packages(&id, common),
        DesktopCmd::Status => desktops::cmd_status(),
        DesktopCmd::Install {
            id,
            dry_run,
            minimal,
            no_display_manager,
            yes,
        } => desktops::cmd_install(&id, dry_run, minimal, no_display_manager, yes),
        DesktopCmd::GenPkgbuild { id, out_dir } => meta::cmd_gen_pkgbuild(&id, &out_dir),
    }
}
