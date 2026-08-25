//! Cross-component intents. `Noop` means "handled locally, nothing to route".

use bao_core::types::SessionId;

#[derive(Debug, Clone)]
pub enum Action {
    Noop,
    Quit,
    /// Attach + fullscreen the selected session.
    Open,
    /// Focus the selected session's terminal, docked.
    FocusTerminal,
    /// Sidebar cursor movement — routed from the keymap, applied by the rail.
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    First,
    Last,
    StartFilter,
    /// Leave the terminal: back to the rail (`⌃q`, or any key when ended).
    StepOut,
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
    /// A submitted prompt, already resolved to a concrete action.
    RenameSession(SessionId, Option<String>),
    CreateSession(Option<String>),
    /// The rm confirmation was accepted.
    Rm(SessionId),
}
