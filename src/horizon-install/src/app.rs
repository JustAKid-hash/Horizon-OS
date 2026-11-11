//! Installer state machine.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::{bail, Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use horizon::desktops::{Catalog, Desktop};

use crate::engine;
use crate::plan::{self, Plan};
use crate::system;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Disk,
    Kernel,
    Desktop,
    Account,
    Confirm,
    Installing,
    Finished,
}

pub enum Msg {
    Log(String),
    Progress(u16),
    Done(Result<(), String>),
}

/// Fields on the account screen.
pub const FIELD_HOSTNAME: usize = 0;
pub const FIELD_USER: usize = 1;
pub const FIELD_PW: usize = 2;
pub const FIELD_PW2: usize = 3;
const FIELD_COUNT: usize = 4;

pub struct App {
    pub step: Step,
    pub dry_run: bool,
    pub uefi: bool,

    pub disks: Vec<system::Disk>,
    pub disk_idx: usize,

    /// (package, label)
    pub kernels: Vec<(&'static str, &'static str)>,
    pub kernel_idx: usize,

    /// Index 0 is "no desktop"; the rest mirror the catalog.
    pub desktops: Vec<Option<Desktop>>,
    pub desktop_idx: usize,

    pub hostname: String,
    pub username: String,
    pub password: String,
    pub password2: String,
    pub focus: usize,

    pub log: Vec<String>,
    pub progress: u16,
    pub finished: Option<Result<(), String>>,
    pub quit: bool,
    pub error: Option<String>,

    rx: Option<Receiver<Msg>>,
}

impl App {
    pub fn new(dry_run: bool, hostname: String) -> Result<Self> {
        let catalog = Catalog::load()?;
        let mut desktops: Vec<Option<Desktop>> = vec![None];
        desktops.extend(catalog.desktop.iter().cloned().map(Some));

        Ok(App {
            step: Step::Welcome,
            dry_run,
            uefi: system::is_uefi(),
            disks: system::disks(),
            disk_idx: 0,
            kernels: vec![
                ("linux", "linux -- stock Arch kernel"),
                ("linux-lts", "linux-lts -- long-term support"),
            ],
            kernel_idx: 0,
            desktops,
            desktop_idx: 0,
            hostname,
            username: String::new(),
            password: String::new(),
            password2: String::new(),
            focus: FIELD_USER,
            log: Vec::new(),
            progress: 0,
            finished: None,
            quit: false,
            error: None,
            rx: None,
        })
    }

    pub fn selected_disk(&self) -> Option<&system::Disk> {
        self.disks.get(self.disk_idx)
    }

    pub fn desktop_label(&self, i: usize) -> String {
        match self.desktops.get(i).and_then(|d| d.as_ref()) {
            None => "None -- a CLI-only system".to_string(),
            Some(d) => format!("{} -- {}", d.name, d.summary),
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        // Ctrl-C always quits, except mid-install.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.step != Step::Installing {
                self.quit = true;
            }
            return;
        }

        match self.step {
            Step::Welcome => {
                if matches!(key.code, KeyCode::Enter) {
                    self.step = Step::Disk;
                } else if key.code == KeyCode::Esc {
                    self.quit = true;
                }
            }
            Step::Disk => match key.code {
                KeyCode::Esc => self.quit = true,
                KeyCode::Up => self.disk_idx = self.disk_idx.saturating_sub(1),
                KeyCode::Down => {
                    if self.disk_idx + 1 < self.disks.len() {
                        self.disk_idx += 1;
                    }
                }
                KeyCode::Enter => {
                    if self.disks.is_empty() {
                        self.error = Some("no installable disk was found".into());
                    } else {
                        self.step = Step::Kernel;
                    }
                }
                _ => {}
            },
            Step::Kernel => match key.code {
                KeyCode::Esc => self.step = Step::Disk,
                KeyCode::Up => self.kernel_idx = self.kernel_idx.saturating_sub(1),
                KeyCode::Down => {
                    if self.kernel_idx + 1 < self.kernels.len() {
                        self.kernel_idx += 1;
                    }
                }
                KeyCode::Enter => self.step = Step::Desktop,
                _ => {}
            },
            Step::Desktop => match key.code {
                KeyCode::Esc => self.step = Step::Kernel,
                KeyCode::Up => self.desktop_idx = self.desktop_idx.saturating_sub(1),
                KeyCode::Down => {
                    if self.desktop_idx + 1 < self.desktops.len() {
                        self.desktop_idx += 1;
                    }
                }
                KeyCode::Enter => self.step = Step::Account,
                _ => {}
            },
            Step::Account => self.on_key_account(key),
            Step::Confirm => match key.code {
                KeyCode::Esc => self.step = Step::Account,
                KeyCode::Char('y') | KeyCode::Char('Y') => self.start_install(),
                _ => {}
            },
            Step::Installing => {}
            Step::Finished => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
                    self.quit = true;
                }
            }
        }
    }

    fn on_key_account(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.step = Step::Desktop,
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % FIELD_COUNT,
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = (self.focus + FIELD_COUNT - 1) % FIELD_COUNT
            }
            KeyCode::Enter => {
                self.error = None;
                self.step = Step::Confirm;
            }
            KeyCode::Backspace => {
                self.field_mut().pop();
            }
            KeyCode::Char(c) => {
                let f = self.field_mut();
                if f.len() < 64 {
                    f.push(c);
                }
            }
            _ => {}
        }
    }

    fn field_mut(&mut self) -> &mut String {
        match self.focus {
            FIELD_HOSTNAME => &mut self.hostname,
            FIELD_USER => &mut self.username,
            FIELD_PW => &mut self.password,
            _ => &mut self.password2,
        }
    }

    pub fn build_plan(&self) -> Result<Plan> {
        let disk = self
            .selected_disk()
            .map(|d| d.path.clone())
            .context("no disk selected")?;
        if !plan::valid_name(&self.hostname) {
            bail!("hostname must be lowercase letters, digits, '-' or '_'");
        }
        if !plan::valid_name(&self.username) {
            bail!("username must be lowercase letters, digits, '-' or '_'");
        }
        if self.password.is_empty() {
            bail!("the password is empty");
        }
        if self.password != self.password2 {
            bail!("the two passwords do not match");
        }
        Ok(Plan {
            disk,
            uefi: self.uefi,
            kernel: self.kernels[self.kernel_idx].0.to_string(),
            desktop: self.desktops[self.desktop_idx].clone(),
            hostname: self.hostname.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            dry_run: self.dry_run,
        })
    }

    fn start_install(&mut self) {
        let plan = match self.build_plan() {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e.to_string());
                self.step = Step::Account;
                return;
            }
        };
        self.error = None;
        self.log.clear();
        self.progress = 0;
        self.step = Step::Installing;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        thread::spawn(move || {
            let tx_events = tx.clone();
            let mut emit = move |ev: engine::Event| {
                let msg = match ev {
                    engine::Event::Log(l) => Msg::Log(l),
                    engine::Event::Progress(p) => Msg::Progress(p),
                };
                let _ = tx_events.send(msg);
            };
            let result = engine::run(&plan, &mut emit).map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Done(result));
        });
    }

    /// Drain messages from the install thread. Call once per UI frame.
    pub fn tick(&mut self) {
        let mut done = None;
        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok(Msg::Log(l)) => self.log.push(l),
                    Ok(Msg::Progress(p)) => self.progress = p,
                    Ok(Msg::Done(r)) => {
                        done = Some(r);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }
        if let Some(r) = done {
            self.finished = Some(r);
            self.step = Step::Finished;
            self.rx = None;
        }
    }
}
