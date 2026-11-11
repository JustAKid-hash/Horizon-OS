//! Tiny ANSI colour helpers. No dependencies; honours `NO_COLOR`, `CLICOLOR_FORCE`
//! and whether stdout is a terminal.

use std::io::IsTerminal;

fn enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if let Ok(v) = std::env::var("CLICOLOR_FORCE") {
        if v != "0" {
            return true;
        }
    }
    std::io::stdout().is_terminal()
}

fn wrap(code: &str, s: &str) -> String {
    if enabled() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    wrap("1", s)
}
pub fn dim(s: &str) -> String {
    wrap("2", s)
}
pub fn red(s: &str) -> String {
    wrap("31", s)
}
pub fn green(s: &str) -> String {
    wrap("32", s)
}
pub fn yellow(s: &str) -> String {
    wrap("33", s)
}
pub fn blue(s: &str) -> String {
    wrap("34", s)
}
pub fn magenta(s: &str) -> String {
    wrap("35", s)
}
pub fn cyan(s: &str) -> String {
    wrap("36", s)
}

/// A short coloured status tag, padded to a constant width.
pub fn tag(level: Level) -> String {
    match level {
        Level::Ok => green("  ok  "),
        Level::Warn => yellow(" warn "),
        Level::Fail => red(" fail "),
        Level::Info => blue(" info "),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
    Info,
}

/// Left-align `s` into a field of `width` visible characters. Good enough for
/// the ASCII identifiers and names used here (no wide glyphs).
pub fn pad(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - n))
    }
}
