//! Horizon OS installer.
//!
//! Two front ends over the same engine:
//!
//! * the default ratatui TUI, for installing by hand from the live medium;
//! * `--yes`, an unattended path that takes every answer from the command line.
//!   It exists so the installer can be exercised in a VM and in CI, and it is
//!   also what a scripted bare-metal install would use.
//!
//! Both build the same [`Plan`] and hand it to [`engine::run`], so the two paths
//! cannot drift apart.

mod app;
mod engine;
mod plan;
mod system;
mod ui;

use std::io::Write;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use horizon::desktops::Catalog;

use crate::app::{App, Step};
use crate::plan::Plan;

/// Install Horizon OS onto a disk.
///
/// Run with no arguments for the interactive installer. Run with `--yes` to
/// install unattended, answering every question on the command line.
#[derive(Debug, Parser)]
#[command(
    name = "horizon-install",
    version,
    about = "Horizon OS installer",
    long_about = "Horizon OS installer\n\nWith no arguments this opens an interactive TUI. With --yes it installs \
                  unattended, taking the disk, kernel, desktop and account from the command line."
)]
struct Args {
    /// Install unattended, without the TUI. Requires --disk, --user and --password.
    #[arg(long)]
    yes: bool,

    /// Print the commands that would run, changing nothing.
    #[arg(long)]
    dry_run: bool,

    /// Skip the "you are not on the live medium" guard.
    #[arg(long)]
    force: bool,

    /// Disk to erase and install onto, e.g. /dev/sda or /dev/nvme0n1.
    #[arg(long, value_name = "DEVICE")]
    disk: Option<String>,

    /// Kernel package to install: linux or linux-lts.
    #[arg(long, value_name = "PKG", default_value = "linux")]
    kernel: String,

    /// Desktop environment id, as shown by `horizon desktop list`.
    #[arg(long, value_name = "ID")]
    desktop: Option<String>,

    /// Hostname for the new system.
    #[arg(long, value_name = "NAME", default_value = "horizon")]
    hostname: String,

    /// Name of the first user account.
    #[arg(long, value_name = "NAME")]
    user: Option<String>,

    /// Password for the first user and for root.
    #[arg(long, value_name = "PASSWORD")]
    password: Option<String>,

    /// Partition as BIOS/legacy even on a UEFI machine.
    #[arg(long)]
    bios: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.yes {
        headless(args)
    } else {
        if args.disk.is_some()
            || args.user.is_some()
            || args.password.is_some()
            || args.desktop.is_some()
        {
            eprintln!(
                "note: --disk/--user/--password/--desktop are only used with --yes; \
                 the TUI asks for them itself."
            );
        }
        tui(args)
    }
}

/// Guard against repartitioning the machine you are sitting on. The live medium
/// is the only place an install is expected to start from.
fn guard_live_medium(force: bool) -> Result<()> {
    if force || system::on_live_medium() {
        return Ok(());
    }
    bail!(
        "this does not look like the Horizon OS live medium (/run/archiso is missing). \
         Installing from a running system would erase the disk it is running from. \
         Re-run with --force if you are sure."
    )
}

/// Unattended install. Every question comes from the command line.
fn headless(args: Args) -> Result<()> {
    guard_live_medium(args.force)?;

    let disk = args
        .disk
        .clone()
        .context("--yes needs --disk, e.g. --disk /dev/sda")?;
    let username = args
        .user
        .clone()
        .context("--yes needs --user, e.g. --user albarr")?;
    let password = args.password.clone().context("--yes needs --password")?;

    if !plan::valid_name(&args.hostname) {
        bail!(
            "hostname '{}' is invalid: use lowercase a-z, 0-9, '-' or '_'",
            args.hostname
        );
    }
    if !plan::valid_name(&username) {
        bail!("username '{username}' is invalid: use lowercase a-z, 0-9, '-' or '_'");
    }
    if password.is_empty() {
        bail!("the password is empty");
    }
    if !matches!(args.kernel.as_str(), "linux" | "linux-lts") {
        bail!(
            "kernel '{}' is not supported here; use linux or linux-lts",
            args.kernel
        );
    }

    let desktop = match &args.desktop {
        None => None,
        Some(id) => {
            let catalog = Catalog::load()?;
            Some(catalog.find(id)?.clone())
        }
    };

    let uefi = !args.bios && system::is_uefi();

    let plan = Plan {
        disk: disk.clone(),
        uefi,
        kernel: args.kernel.clone(),
        desktop,
        hostname: args.hostname.clone(),
        username: username.clone(),
        password,
        dry_run: args.dry_run,
    };

    println!("Horizon OS installer");
    println!(
        "  disk       {} ({})",
        plan.disk,
        if uefi { "UEFI" } else { "BIOS (legacy)" }
    );
    println!("  kernel     {}", plan.kernel);
    println!(
        "  desktop    {}",
        plan.desktop
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "none (CLI only)".into())
    );
    println!("  hostname   {}", plan.hostname);
    println!("  user       {}", plan.username);
    if plan.dry_run {
        println!("  mode       dry run -- nothing will change");
    }
    println!();

    let mut emit = |ev: engine::Event| match ev {
        engine::Event::Log(line) => println!("{line}"),
        engine::Event::Progress(pct) => {
            print!("\r[{pct:>3}%]");
            let _ = std::io::stdout().flush();
        }
    };

    let result = engine::run(&plan, &mut emit);
    println!();

    match result {
        Ok(()) => {
            if plan.dry_run {
                println!("Dry run complete; nothing changed.");
            } else {
                println!("Installation complete. Remove the medium and reboot.");
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Interactive TUI install.
fn tui(args: Args) -> Result<()> {
    guard_live_medium(args.force)?;

    let mut app = App::new(args.dry_run, args.hostname.clone())?;
    if let Some(id) = &args.desktop {
        let catalog = Catalog::load()?;
        let _ = catalog.find(id)?; // validate, and fail early on a typo
        if let Some(pos) = app
            .desktops
            .iter()
            .position(|d| d.as_ref().map(|d| d.id.as_str()) == Some(id.as_str()))
        {
            app.desktop_idx = pos;
        }
    }

    let mut terminal = init_terminal()?;
    let result = run_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;
    result
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering the alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("creating the terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    while !app.quit {
        app.tick();
        terminal.draw(|f| ui::draw(f, app))?;

        // While installing, poll briefly so the log keeps up without a keypress.
        let timeout = if app.step == Step::Installing {
            Duration::from_millis(120)
        } else {
            Duration::from_millis(250)
        };

        if event::poll(timeout).context("polling for input")? {
            match event::read().context("reading an event")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}
