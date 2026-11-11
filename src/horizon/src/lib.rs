//! Horizon OS distribution library.
//!
//! The `horizon` binary is a thin shell over this crate, and `horizon-install`
//! reuses the same desktop catalog and pacman helpers.

pub mod cli;
pub mod desktops;
pub mod doctor;
pub mod kernel;
pub mod meta;
pub mod pacman;
pub mod sysinfo;
pub mod ui;
