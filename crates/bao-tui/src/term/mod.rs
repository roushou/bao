//! The terminal model: two directions over one byte stream, meeting over the
//! modes the harness itself set.
//!
//! - **Decode** ([`decode`]/`Emu`): PTY bytes → screen. Parses output and
//!   records private-mode state (DECCKM, bracketed paste) as it goes.
//! - **Encode** ([`encode`]/`Encoder`): crossterm events → PTY bytes,
//!   honoring exactly those modes — dialect is received, never guessed.
//!
//! Pure on both sides; effects (wire sends) belong to callers.

pub mod decode;
pub mod encode;

/// Terminal modes the harness set on its output stream (DECSET/DECRST).
/// The neutral contract between the decode and encode halves: populated by
/// `Emu::feed`, honored by `Encoder`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modes {
    /// Application cursor keys (DECCKM, `?1`): arrows/Home/End send SS3.
    pub app_cursor: bool,
    /// Bracketed paste (`?2004`): pastes are wrapped in `ESC [ 200 ~ …`.
    pub bracketed_paste: bool,
}

pub use decode::Emu;
pub use encode::Encoder;
