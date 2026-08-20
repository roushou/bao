//! Cross-component intents. `Noop` means "handled locally, nothing to route".

use bao_core::types::SessionId;
use crossterm::event::KeyEvent;

#[derive(Debug, Clone)]
pub enum Action {
    Noop,
    Quit,
    /// Attach + fullscreen the selected session.
    Open,
    /// Focus the selected session's terminal, docked.
    FocusTerminal,
    /// Verbs against the selected session.
    Resume,
    Stop,
    Remove,
    Rename,
    Create,
    OpenPalette,
    OpenHelp,
    /// The palette's selection was confirmed — resolve its entry.
    PaletteConfirm,
    /// A raw key while the terminal owns the keyboard.
    TerminalKey(KeyEvent),
    /// A submitted prompt, already resolved to a concrete action.
    RenameSession(SessionId, Option<String>),
    CreateSession(Option<String>),
    /// The rm confirmation was accepted.
    Rm(SessionId),
}
