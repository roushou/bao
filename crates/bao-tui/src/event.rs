//! Typed input events (crossterm), as handed to components.

use crossterm::event::KeyEvent;

#[derive(Debug, Clone)]
pub enum Event {
    Key(KeyEvent),
}
