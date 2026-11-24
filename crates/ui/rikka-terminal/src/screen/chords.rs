use alacritty_terminal::grid::Scroll;
use rikka_input::Chord as Binding;
use rikka_types::{Key, Modifiers};
use rikka_ui::Selection;

use crate::movement::Movement;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Chord {
    Copy(Selection),
    Paste(Selection),
    SelectAll,
    Mark,
    Scroll(Scroll),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selected {
    Copy,
    Dismiss,
    Move { movement: Movement, extend: bool },
}

pub(crate) fn chord(key: Key, modifiers: Modifiers) -> Option<Chord> {
    if modifiers.alt || modifiers.logo {
        return None;
    }
    if key == Key::Insert {
        return match (modifiers.control, modifiers.shift) {
            (true, false) => Some(Chord::Copy(Selection::Clipboard)),
            (false, true) => Some(Chord::Paste(Selection::Primary)),
            _ => None,
        };
    }
    if let Some(scroll) = scrolling(key, modifiers) {
        return Some(Chord::Scroll(scroll));
    }
    if !modifiers.control {
        return None;
    }
    match (key.to_lowercase(), modifiers.shift) {
        (Key::Char('c'), true) => Some(Chord::Copy(Selection::Clipboard)),
        (Key::Char('v'), _) => Some(Chord::Paste(Selection::Clipboard)),
        (Key::Char('a'), true) => Some(Chord::SelectAll),
        (Key::Char('m'), true) => Some(Chord::Mark),
        _ => None,
    }
}

pub(crate) fn selected(key: Key, modifiers: Modifiers, marking: bool) -> Option<Selected> {
    if modifiers.alt || modifiers.logo {
        return None;
    }
    let plain = modifiers == Modifiers::NONE;
    match key.to_lowercase() {
        Key::Char('c') if modifiers == Modifiers::CONTROL => return Some(Selected::Copy),
        Key::Enter if plain && marking => return Some(Selected::Copy),
        Key::Escape if plain => return Some(Selected::Dismiss),
        _ => {}
    }
    if !modifiers.shift && !marking {
        return None;
    }
    Movement::from_key(key, modifiers.control).map(|movement| Selected::Move {
        movement,
        extend: modifiers.shift,
    })
}

pub(crate) fn copying() -> Binding {
    Binding::new(Modifiers::CONTROL, Key::Char('c'))
}

pub(crate) fn pasting() -> Binding {
    Binding::new(Modifiers::CONTROL, Key::Char('v'))
}

pub(crate) fn selecting_all() -> Binding {
    Binding::new(Modifiers::CONTROL_SHIFT, Key::Char('a'))
}

fn scrolling(key: Key, modifiers: Modifiers) -> Option<Scroll> {
    if !modifiers.shift {
        return None;
    }
    Some(match (key, modifiers.control) {
        (Key::PageUp, _) => Scroll::PageUp,
        (Key::PageDown, _) => Scroll::PageDown,
        (Key::ArrowUp, true) => Scroll::Delta(1),
        (Key::ArrowDown, true) => Scroll::Delta(-1),
        (Key::Home, true) => Scroll::Top,
        (Key::End, true) => Scroll::Bottom,
        _ => return None,
    })
}
