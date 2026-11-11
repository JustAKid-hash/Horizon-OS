//! Horizon OS distribution CLI.
//!
//! A thin, opinionated front end over the system: it reports on the machine,
//! installs desktop environments from a catalog, and lists kernels. It
//! deliberately does **not** reimplement pacman or the AUR -- those stay exactly
//! as upstream Arch ships them, and Horizon OS is designed to work with them.

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = horizon::cli::Cli::parse();
    horizon::cli::dispatch(cli)
}
