//! Rendering for the installer TUI.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Step, FIELD_HOSTNAME, FIELD_PW, FIELD_PW2, FIELD_USER};

fn block(title: &str) -> Block<'static> {
    Block::bordered().title(format!(" {title} "))
}

fn list_state(selected: usize, len: usize) -> ListState {
    let mut s = ListState::default();
    if len > 0 {
        s.select(Some(selected));
    }
    s
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(f.area());

    header(f, app, chunks[0]);
    match app.step {
        Step::Welcome => welcome(f, chunks[1]),
        Step::Disk => disk(f, app, chunks[1]),
        Step::Kernel => kernel(f, app, chunks[1]),
        Step::Desktop => desktop(f, app, chunks[1]),
        Step::Account => account(f, app, chunks[1]),
        Step::Confirm => confirm(f, app, chunks[1]),
        Step::Installing => installing(f, app, chunks[1]),
        Step::Finished => finished(f, app, chunks[1]),
    }
    footer(f, app, chunks[2]);
}

fn header(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(
            " Horizon OS installer ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            if app.uefi { "UEFI" } else { "BIOS" },
            Style::default().fg(Color::Green),
        ),
    ];
    if app.dry_run {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "DRY RUN",
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::bordered()),
        area,
    );
}

fn footer(f: &mut Frame, app: &App, area: Rect) {
    let keys = match app.step {
        Step::Welcome => "Enter continue   Esc quit",
        Step::Disk => "Up/Down choose   Enter continue   Esc quit",
        Step::Kernel => "Up/Down choose   Enter continue   Esc back",
        Step::Desktop => "Up/Down choose   Enter continue   Esc back",
        Step::Account => "Tab/Up/Down field   Enter continue   Esc back",
        Step::Confirm => "y erase and install   Esc back",
        Step::Installing => "installing, please wait...",
        Step::Finished => "Enter quit",
    };
    let text = if let Some(err) = &app.error {
        Line::from(Span::styled(
            format!("  {err}"),
            Style::default().fg(Color::Red),
        ))
    } else {
        Line::from(Span::styled(
            format!("  {keys}"),
            Style::default().fg(Color::DarkGray),
        ))
    };
    f.render_widget(Paragraph::new(text).block(Block::bordered()), area);
}

fn welcome(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Horizon OS installer",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  This installs Horizon OS onto a whole disk. It will:"),
        Line::from(""),
        Line::from("    - erase and repartition the disk you choose"),
        Line::from("    - install an Arch base system with pacman"),
        Line::from("    - optionally install a desktop environment"),
        Line::from("    - install and configure the GRUB bootloader"),
        Line::from(""),
        Line::from("  Pacman and the AUR are used exactly as upstream Arch ships them."),
        Line::from(""),
        Line::from(Span::styled(
            "  Nothing is written until you confirm on the final screen.",
            Style::default().fg(Color::Green),
        )),
    ];
    f.render_widget(Paragraph::new(text).block(block(" Welcome ")), area);
}

fn disk(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(4)])
        .split(area);

    let items: Vec<ListItem> = if app.disks.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  no installable disk found",
            Style::default().fg(Color::Red),
        )))]
    } else {
        app.disks
            .iter()
            .map(|d| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<14}", d.path), Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{:<8}", d.size), Style::default().fg(Color::Yellow)),
                    Span::raw(d.model.clone()),
                ]))
            })
            .collect()
    };

    let list = List::new(items)
        .block(block(" Target disk "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    f.render_stateful_widget(
        list,
        chunks[0],
        &mut list_state(app.disk_idx, app.disks.len()),
    );

    let warn = Paragraph::new(vec![
        Line::from(Span::styled(
            "  The chosen disk will be COMPLETELY ERASED.",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from("  Make sure it holds nothing you need."),
    ])
    .block(block(" Warning "));
    f.render_widget(warn, chunks[1]);
}

fn kernel(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(4)])
        .split(area);

    let items: Vec<ListItem> = app
        .kernels
        .iter()
        .map(|(pkg, label)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<12}", pkg), Style::default().fg(Color::Cyan)),
                Span::raw(*label),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(block(" Kernel "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    f.render_stateful_widget(
        list,
        chunks[0],
        &mut list_state(app.kernel_idx, app.kernels.len()),
    );

    let note = Paragraph::new(Line::from(
        "  Both kernels are Arch packages and can coexist; pick either from GRUB at boot.",
    ))
    .wrap(Wrap { trim: false })
    .block(block(" Note "));
    f.render_widget(note, chunks[1]);
}

fn desktop(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(7), Constraint::Length(6)])
        .split(area);

    let items: Vec<ListItem> = (0..app.desktops.len())
        .map(|i| ListItem::new(Line::from(app.desktop_label(i))))
        .collect();
    let list = List::new(items)
        .block(block(" Desktop environment "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    f.render_stateful_widget(
        list,
        chunks[0],
        &mut list_state(app.desktop_idx, app.desktops.len()),
    );

    let detail = match app.desktops.get(app.desktop_idx).and_then(|d| d.as_ref()) {
        None => vec![Line::from(
            "  A CLI-only system: base, kernel, pacman and the AUR.",
        )],
        Some(d) => vec![
            Line::from(vec![
                Span::styled("  session   ", Style::default().fg(Color::DarkGray)),
                Span::raw(d.session.clone()),
            ]),
            Line::from(vec![
                Span::styled("  login     ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} ({})", d.display_manager, d.dm_service)),
            ]),
            Line::from(vec![
                Span::styled("  packages  ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} plus the shared set", d.packages.len())),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {}", d.summary),
                Style::default().fg(Color::DarkGray),
            )),
        ],
    };
    f.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(block(" Details ")),
        chunks[1],
    );
}

fn account(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(area);

    let field = |i: usize, label: &str, value: &str, mask: bool| {
        let shown = if mask {
            "*".repeat(value.chars().count())
        } else {
            value.to_string()
        };
        let focused = app.focus == i;
        let marker = if focused { ">" } else { " " };
        let style = if focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Line::from(vec![
            Span::styled(format!(" {marker} {:<10}", label), style),
            Span::raw(shown),
        ])
    };

    let text = vec![
        Line::from(""),
        field(FIELD_HOSTNAME, "hostname", &app.hostname, false),
        field(FIELD_USER, "username", &app.username, false),
        field(FIELD_PW, "password", &app.password, true),
        field(FIELD_PW2, "confirm", &app.password2, true),
    ];
    f.render_widget(Paragraph::new(text).block(block(" Account ")), chunks[0]);

    let note = Paragraph::new(vec![
        Line::from("  The first account is added to the wheel group and can use sudo."),
        Line::from("  Root gets the same password. Keep it lowercase a-z, 0-9, '-' or '_'."),
    ])
    .wrap(Wrap { trim: false })
    .block(block(" Note "));
    f.render_widget(note, chunks[1]);
}

fn confirm(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(area);

    let disk = app
        .selected_disk()
        .map(|d| format!("{} ({} {})", d.path, d.size, d.model))
        .unwrap_or_else(|| "none".into());
    let kernel = app.kernels[app.kernel_idx].0;
    let desktop = match app.desktops.get(app.desktop_idx).and_then(|d| d.as_ref()) {
        None => "none (CLI only)".to_string(),
        Some(d) => d.name.clone(),
    };

    let row = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("  {:<12}", k), Style::default().fg(Color::DarkGray)),
            Span::raw(v),
        ])
    };
    let text = vec![
        Line::from(""),
        row("disk", disk),
        row(
            "firmware",
            if app.uefi {
                "UEFI".into()
            } else {
                "BIOS (legacy)".to_string()
            },
        ),
        row("kernel", kernel.to_string()),
        row("desktop", desktop),
        row("hostname", app.hostname.clone()),
        row("user", app.username.clone()),
        Line::from(""),
        Line::from(Span::styled(
            "  Press y to erase the disk and install.",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
    ];
    f.render_widget(Paragraph::new(text).block(block(" Review ")), chunks[0]);

    let warn = Paragraph::new(Line::from(Span::styled(
        "  This is the point of no return for the target disk.",
        Style::default().fg(Color::Red),
    )))
    .block(block(" Warning "));
    f.render_widget(warn, chunks[1]);
}

fn installing(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    f.render_widget(
        Gauge::default()
            .block(block(" Progress "))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(app.progress),
        chunks[0],
    );

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app
        .log
        .iter()
        .rev()
        .take(visible)
        .rev()
        .map(|l| Line::from(l.clone()))
        .collect();
    f.render_widget(Paragraph::new(lines).block(block(" Log ")), chunks[1]);
}

fn finished(f: &mut Frame, app: &App, area: Rect) {
    let text = match &app.finished {
        Some(Ok(())) => vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Installation complete.",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("  Remove the installation medium and reboot."),
            Line::from("  At the login prompt, sign in with the account you created."),
        ],
        Some(Err(e)) => vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Installation failed.",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("  {e}")),
            Line::from(""),
            Line::from("  The target disk may be left part-way through."),
        ],
        None => vec![Line::from("  ...")],
    };
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(block(" Done ")),
        area,
    );
}
