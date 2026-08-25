//! Bao TUI: the terminal frontend. Consumers run the [`Tui`](tui::Tui) —
//! `Tui::run(addr)` for the overview, `Tui::run_attached(addr, session)` to
//! jump into one session. The surfaces, components, and status language are
//! internal.

pub mod error;

mod action;
mod components;
mod emu;
mod event;
mod keys;
mod overview;
mod signal;
mod state;
mod terminal;
mod theme;
mod tui;

pub use tui::Tui;
