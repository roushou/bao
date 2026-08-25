//! The keyboard side of the terminal model: crossterm events → the exact
//! bytes a legacy xterm would have sent. The mirror of [`super::decode`] (which
//! decodes PTY bytes → screen); the two meet over the *modes* the harness
//! itself set on its output stream — application cursor keys (DECCKM) and
//! bracketed paste — which we honor rather than guess.
//!
//! Shape: enumerable families live in const tables; the genuinely parametric
//! cases (C0 control bytes, ESC-prefix composition) are three named rules.
//! Bao-reserved keys never reach here — the keymap consumes them first.

use crossterm::event::{KeyCode, KeyEvent};

use super::Modes;

/// Legacy encodings for non-character keys, normal (cursor) mode. Pure data,
/// auditable against any xterm reference.
const NAMED: &[(KeyCode, &[u8])] = &[
    (KeyCode::Enter, b"\r"),
    (KeyCode::Backspace, b"\x7f"),
    (KeyCode::Tab, b"\t"),
    (KeyCode::Esc, b"\x1b"),
];

/// DECCKM variants: in application cursor-keys mode the arrows and Home/End
/// move from CSI (`ESC [ A`) to SS3 (`ESC O A`). Only the keys whose encoding
/// the mode actually changes; everything else falls through to [`NAMED`].
const NAMED_APPCURSOR: &[(KeyCode, &[u8])] = &[
    (KeyCode::Up, b"\x1bOA"),
    (KeyCode::Down, b"\x1bOB"),
    (KeyCode::Right, b"\x1bOC"),
    (KeyCode::Left, b"\x1bOD"),
    (KeyCode::Home, b"\x1bOH"),
    (KeyCode::End, b"\x1bOF"),
];

const NAMED_NORMAL_CURSORS: &[(KeyCode, &[u8])] = &[
    (KeyCode::Up, b"\x1b[A"),
    (KeyCode::Down, b"\x1b[B"),
    (KeyCode::Right, b"\x1b[C"),
    (KeyCode::Left, b"\x1b[D"),
    (KeyCode::Home, b"\x1b[H"),
    (KeyCode::End, b"\x1b[F"),
    (KeyCode::Delete, b"\x1b[3~"),
    (KeyCode::Insert, b"\x1b[2~"),
    (KeyCode::PageUp, b"\x1b[5~"),
    (KeyCode::PageDown, b"\x1b[6~"),
];

/// The keyboard encoder for one session, reading the modes its emulator
/// observed. Cheap to construct; holds nothing but the modes reference.
#[derive(Debug, Clone, Copy)]
pub struct Encoder {
    modes: Modes,
}

impl Encoder {
    pub fn new(modes: Modes) -> Self {
        Encoder { modes }
    }

    /// Encode a keystroke. `None` = not a key a terminal would send.
    pub fn key(&self, k: &KeyEvent) -> Option<Vec<u8>> {
        let ctrl = k
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(crossterm::event::KeyModifiers::ALT);
        match k.code {
            // Rule: Ctrl+letter → C0 control byte (Ctrl-A=1 .. Ctrl-Z=26).
            KeyCode::Char(c) if ctrl && !alt => control_byte(c),
            // Rule: Alt/Meta → ESC-prefix composition.
            KeyCode::Char(c) if alt => Some(meta(utf8(c))),
            // Trivial case: the character itself, UTF-8.
            KeyCode::Char(c) => Some(utf8(c)),
            // Data: named keys. DECCKM only changes arrows/Home/End — the
            // mode selects which dialect is consulted first.
            code => {
                let found = if self.modes.app_cursor {
                    look(code, NAMED_APPCURSOR)
                        .or_else(|| look(code, NAMED_NORMAL_CURSORS))
                        .or_else(|| look(code, NAMED))
                } else {
                    look(code, NAMED_NORMAL_CURSORS).or_else(|| look(code, NAMED))
                };
                found.map(|b| b.to_vec())
            }
        }
    }

    /// Encode a paste — wrapped iff the harness asked for bracketed paste,
    /// so multi-line paste arrives as one unit instead of premature submits.
    pub fn paste(&self, text: &str) -> Vec<u8> {
        if self.modes.bracketed_paste {
            let mut out = b"\x1b[200~".to_vec();
            out.extend_from_slice(text.as_bytes());
            out.extend_from_slice(b"\x1b[201~");
            out
        } else {
            text.as_bytes().to_vec()
        }
    }
}

/// Ctrl+letter → the C0 control byte. Non-letters have no legacy encoding.
fn control_byte(c: char) -> Option<Vec<u8>> {
    let lo = c.to_ascii_lowercase();
    lo.is_ascii_lowercase().then(|| vec![lo as u8 - b'a' + 1])
}

/// Meta composition: ESC prefixed to any encoding.
fn meta(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.insert(0, 0x1b);
    bytes
}

fn utf8(c: char) -> Vec<u8> {
    c.encode_utf8(&mut [0u8; 4]).as_bytes().to_vec()
}

fn look(code: KeyCode, table: &'static [(KeyCode, &'static [u8])]) -> Option<&'static [u8]> {
    table.iter().find(|(kc, _)| *kc == code).map(|(_, b)| *b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn enc(modes: Modes) -> Encoder {
        Encoder::new(modes)
    }

    #[test]
    fn characters_pass_through_as_utf8() {
        let m = Modes::default();
        assert_eq!(
            enc(m).key(&ev(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(b"a".to_vec())
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Char('é'), KeyModifiers::NONE)),
            Some("é".as_bytes().to_vec())
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Some(b"A".to_vec())
        );
    }

    #[test]
    fn ctrl_letters_map_to_c0_bytes() {
        let m = Modes::default();
        for (i, c) in ('a'..='z').enumerate() {
            assert_eq!(
                enc(m).key(&ev(KeyCode::Char(c), KeyModifiers::CONTROL)),
                Some(vec![i as u8 + 1]),
                "ctrl+{c}"
            );
        }
        // Uppercase normalizes; non-letters have no legacy encoding.
        assert_eq!(
            enc(m).key(&ev(KeyCode::Char('C'), KeyModifiers::CONTROL)),
            Some(vec![3])
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Char('2'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn meta_prefixes_esc() {
        let m = Modes::default();
        assert_eq!(
            enc(m).key(&ev(KeyCode::Char('x'), KeyModifiers::ALT)),
            Some(b"\x1bx".to_vec())
        );
    }

    #[test]
    fn named_keys_in_normal_mode() {
        let m = Modes::default();
        assert_eq!(
            enc(m).key(&ev(KeyCode::Enter, KeyModifiers::NONE)),
            Some(b"\r".to_vec())
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Up, KeyModifiers::NONE)),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Delete, KeyModifiers::NONE)),
            Some(b"\x1b[3~".to_vec())
        );
        // Not a key a terminal sends.
        assert_eq!(enc(m).key(&ev(KeyCode::F(5), KeyModifiers::NONE)), None);
    }

    #[test]
    fn app_cursor_mode_switches_arrows_to_ss3() {
        let m = Modes {
            app_cursor: true,
            bracketed_paste: false,
        };
        assert_eq!(
            enc(m).key(&ev(KeyCode::Up, KeyModifiers::NONE)),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Home, KeyModifiers::NONE)),
            Some(b"\x1bOH".to_vec())
        );
        // Keys the mode doesn't touch keep their encoding.
        assert_eq!(
            enc(m).key(&ev(KeyCode::Enter, KeyModifiers::NONE)),
            Some(b"\r".to_vec())
        );
        assert_eq!(
            enc(m).key(&ev(KeyCode::Delete, KeyModifiers::NONE)),
            Some(b"\x1b[3~".to_vec())
        );
    }

    #[test]
    fn paste_wraps_only_when_bracketed() {
        let off = Modes::default();
        assert_eq!(enc(off).paste("hi\nthere"), b"hi\nthere".to_vec());
        let on = Modes {
            app_cursor: false,
            bracketed_paste: true,
        };
        assert_eq!(enc(on).paste("hi"), b"\x1b[200~hi\x1b[201~".to_vec());
    }
}
