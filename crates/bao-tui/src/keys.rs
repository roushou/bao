//! The keymap: one declarative table that is the single source of truth for
//! every binding in the overview. Keys are data — if it is not in the table,
//! it is not a binding; and because each row carries its human label, help
//! and footer hints render *from* the table and structurally cannot drift.
//!
//! What deliberately lives outside the table:
//! - **Text entry** (the rail's filter input, footer prompts): most keys
//!   insert characters; those modes keep their own tiny handlers.
//! - **Raw terminal passthrough**: the harness owns the keyboard there. The
//!   table holds the one documented exception (`⌃q` step-out), which the
//!   terminal consults before forwarding bytes.
//!
//! Guarantees, enforced by tests in this module: no two bindings in a scope
//! share a key; `parse`/`display` round-trip for every row; crossterm's
//! modifier quirks normalize in exactly one place.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;

/// A normalized key. Character case is preserved (`g` and `G` differ);
/// the only modifier modeled is CONTROL — SHIFT is already encoded in the
/// character crossterm reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub ctrl: bool,
}

impl Key {
    const fn new(code: KeyCode, ctrl: bool) -> Self {
        Key { code, ctrl }
    }

    /// Parse a spec like `"j"`, `"ctrl+q"`, `"enter"`, `"pageup"`.
    pub fn parse(spec: &str) -> Option<Key> {
        let mut ctrl = false;
        let mut rest = spec;
        loop {
            if let Some(r) = rest.strip_prefix("ctrl+") {
                ctrl = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("shift+") {
                rest = r;
            } else {
                break;
            }
        }
        let code = match rest {
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "backspace" => KeyCode::Backspace,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "space" => KeyCode::Char(' '),
            other if other.chars().count() == 1 => KeyCode::Char(other.chars().next()?),
            _ => return None,
        };
        Some(Key::new(code, ctrl))
    }

    /// The house-style display form: `"⌃q"`, `"Enter"`, `"G"`.
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push('⌃');
        }
        s.push_str(&match self.code {
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Backspace => "Bksp".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            _ => "?".to_string(),
        });
        s
    }

    /// Normalize a crossterm event. This is the only place modifiers are
    /// inspected: components compare plain [`Key`]s and never see raw events.
    pub fn from_event(k: &KeyEvent) -> Key {
        Key::new(
            k.code,
            k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char(_)),
        )
    }
}

/// Where the keyboard currently is — derived from focus and mode, never
/// stored. The scopes with command bindings; text-entry modes are not scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Browse focus: navigation + session verbs.
    Rail,
    /// Raw passthrough; the table holds only the step-out exception.
    Terminal,
}

/// Help section a binding renders under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Navigation,
    Sessions,
    View,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Navigation => "navigation",
            Group::Sessions => "sessions",
            Group::View => "view",
        }
    }

    pub const ALL: [Group; 3] = [Group::Navigation, Group::Sessions, Group::View];
}

/// One row of the table: key → action, plus everything display needs.
#[derive(Debug, Clone)]
pub struct Binding {
    pub key: Key,
    pub action: Action,
    /// The spec it was parsed from (`"ctrl+q"`) — kept for round-trip tests.
    #[allow(dead_code)]
    pub spec: &'static str,
    pub label: &'static str,
    pub group: Group,
}

fn bind(spec: &'static str, action: Action, label: &'static str, group: Group) -> Binding {
    // Specs in this file are literals we control; bad ones are compile-time
    // review failures and test failures (round-trip), never runtime panics.
    match Key::parse(spec) {
        Some(key) => Binding {
            key,
            action,
            spec,
            label,
            group,
        },
        None => panic!("invalid keybinding spec"),
    }
}

/// The default keymap. Built once, shared by routing (overview), the help
/// overlay, the footer hints, and the terminal's step-out check.
#[derive(Debug, Clone)]
pub struct Keymap {
    rail: Vec<Binding>,
    terminal: Vec<Binding>,
}

impl Keymap {
    fn build() -> Self {
        use Action::*;
        use Group::{Navigation as Nav, Sessions as Sess, View};
        Keymap {
            rail: vec![
                bind("up", MoveUp, "move", Nav),
                bind("k", MoveUp, "move", Nav),
                bind("down", MoveDown, "move", Nav),
                bind("j", MoveDown, "move", Nav),
                bind("pageup", PageUp, "page up", Nav),
                bind("pagedown", PageDown, "page down", Nav),
                bind("g", First, "first", Nav),
                bind("G", Last, "last", Nav),
                bind("tab", FocusTerminal, "type into session", Nav),
                bind("enter", Open, "attach fullscreen", Nav),
                bind("/", StartFilter, "filter sessions", Nav),
                bind("?", OpenHelp, "this help", View),
                bind("ctrl+p", OpenPalette, "jump — quick switch", View),
                bind("ctrl+q", Quit, "quit", View),
                bind("c", Create, "create session", Sess),
                bind("r", Resume, "resume interrupted", Sess),
                bind("s", Stop, "stop running", Sess),
                bind("n", Rename, "rename", Sess),
                bind("d", Remove, "remove (confirm)", Sess),
            ],
            terminal: vec![bind("ctrl+q", StepOut, "back to sidebar", Nav)],
        }
    }

    /// The shared default keymap (built once).
    pub fn defaults() -> &'static Keymap {
        static DEFAULTS: std::sync::OnceLock<Keymap> = std::sync::OnceLock::new();
        DEFAULTS.get_or_init(Keymap::build)
    }

    /// Resolve a key event in `scope`. `None` = not a binding here (for the
    /// terminal that means: forward the raw bytes).
    pub fn resolve(&self, scope: Scope, ev: &KeyEvent) -> Option<Action> {
        let key = Key::from_event(ev);
        self.bindings(scope)
            .iter()
            .find(|b| b.key == key)
            .map(|b| b.action.clone())
    }

    /// All bindings of a scope, table order.
    pub fn bindings(&self, scope: Scope) -> &[Binding] {
        match scope {
            Scope::Rail => &self.rail,
            Scope::Terminal => &self.terminal,
        }
    }

    /// The display form of whichever key performs `action` in `scope`.
    pub fn display_of(&self, scope: Scope, action: &Action) -> Option<String> {
        self.bindings(scope)
            .iter()
            .find(|b| std::mem::discriminant(&b.action) == std::mem::discriminant(action))
            .map(|b| b.key.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key_event(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn no_conflicts_within_a_scope() {
        let km = Keymap::defaults();
        for scope in [Scope::Rail, Scope::Terminal] {
            let keys: Vec<_> = km.bindings(scope).iter().map(|b| b.key).collect();
            let unique = keys.iter().collect::<std::collections::HashSet<_>>();
            assert_eq!(keys.len(), unique.len(), "{scope:?}: duplicate key");
        }
    }

    #[test]
    fn specs_round_trip_through_display() {
        let km = Keymap::defaults();
        for b in km
            .bindings(Scope::Rail)
            .iter()
            .chain(km.bindings(Scope::Terminal))
        {
            let parsed = Key::parse(b.spec).unwrap_or_else(|| panic!("parse {}", b.spec));
            assert_eq!(parsed, b.key, "{} re-parses differently", b.spec);
            assert_eq!(parsed.display(), b.key.display());
            assert!(!b.key.display().is_empty());
        }
    }

    #[test]
    fn resolves_rail_bindings() {
        let km = Keymap::defaults();
        assert!(matches!(
            km.resolve(
                Scope::Rail,
                &key_event(KeyCode::Char('j'), KeyModifiers::NONE)
            ),
            Some(Action::MoveDown)
        ));
        assert!(matches!(
            km.resolve(Scope::Rail, &key_event(KeyCode::Up, KeyModifiers::NONE)),
            Some(Action::MoveUp)
        ));
        assert!(matches!(
            km.resolve(
                Scope::Rail,
                &key_event(KeyCode::Char('p'), KeyModifiers::CONTROL)
            ),
            Some(Action::OpenPalette)
        ));
        assert!(matches!(
            km.resolve(
                Scope::Rail,
                &key_event(KeyCode::Char('G'), KeyModifiers::NONE)
            ),
            Some(Action::Last)
        ));
        // Not a binding.
        assert!(
            km.resolve(
                Scope::Rail,
                &key_event(KeyCode::Char('z'), KeyModifiers::NONE)
            )
            .is_none()
        );
    }

    #[test]
    fn normalizes_modifier_quirks_once() {
        // Ctrl+j arrives as Char('j') + CONTROL; case is preserved so `G`
        // stays distinct from `g`.
        let k = Key::from_event(&key_event(KeyCode::Char('j'), KeyModifiers::CONTROL));
        assert!(k.ctrl);
        assert_eq!(k.code, KeyCode::Char('j'));
        let g = Key::from_event(&key_event(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert!(!g.ctrl);
        assert_eq!(g.code, KeyCode::Char('G'));
        // Non-char keys don't claim ctrl from unrelated modifiers.
        let e = Key::from_event(&key_event(KeyCode::Enter, KeyModifiers::SHIFT));
        assert!(!e.ctrl);
    }

    #[test]
    fn terminal_scope_is_only_step_out() {
        let km = Keymap::defaults();
        assert_eq!(km.bindings(Scope::Terminal).len(), 1);
        assert!(matches!(
            km.resolve(
                Scope::Terminal,
                &key_event(KeyCode::Char('q'), KeyModifiers::CONTROL)
            ),
            Some(Action::StepOut)
        ));
    }

    #[test]
    fn help_covers_every_binding_exactly_once() {
        let km = Keymap::defaults();
        let rail = km.bindings(Scope::Rail);
        let rendered: Vec<&Binding> = Group::ALL
            .iter()
            .flat_map(|g| rail.iter().filter(|b| b.group == *g))
            .collect();
        assert_eq!(rendered.len(), rail.len(), "every rail binding has a group");
    }

    #[test]
    fn display_forms_are_distinct_per_scope() {
        let km = Keymap::defaults();
        let labels: std::collections::HashSet<String> = km
            .bindings(Scope::Rail)
            .iter()
            .map(|b| b.key.display())
            .collect();
        assert_eq!(km.bindings(Scope::Rail).len(), labels.len());
    }
}
