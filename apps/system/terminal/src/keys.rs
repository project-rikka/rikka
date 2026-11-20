use rikka_types::{Key, Modifiers};
use rikka_widgets::Side;

use crate::command::Command;

pub(crate) fn command(key: Key, modifiers: Modifiers) -> Option<Command> {
    let control = modifiers.control;
    let shift = modifiers.shift;
    let Key::Char(character) = key else {
        return match key {
            Key::Tab if control => Some(if shift {
                Command::PreviousTab
            } else {
                Command::NextTab
            }),
            Key::PageUp if control && !shift => Some(Command::PreviousTab),
            Key::PageDown if control && !shift => Some(Command::NextTab),
            Key::F(11) if modifiers == Modifiers::NONE => Some(Command::Fullscreen),
            _ => focus_command(key, modifiers),
        };
    };
    let character = character.to_ascii_lowercase();

    if modifiers.alt && !control {
        return character
            .to_digit(10)
            .filter(|digit| *digit > 0)
            .map(|digit| Command::Tab(digit as usize - 1));
    }
    if !control {
        return None;
    }
    if shift {
        return Some(match character {
            'n' => Command::NewWindow,
            'q' => Command::CloseWindow,
            't' => Command::NewTab,
            'w' => Command::CloseTab,
            'f' => Command::Find,
            '5' | '%' => Command::Split(Side::Right),
            '\'' | '"' => Command::Split(Side::Bottom),
            '+' | ';' | '=' => Command::Bigger,
            _ => return None,
        });
    }
    Some(match character {
        '+' | '=' => Command::Bigger,
        '-' => Command::Smaller,
        '0' => Command::ResetSize,
        _ => return None,
    })
}

fn focus_command(key: Key, modifiers: Modifiers) -> Option<Command> {
    if !modifiers.alt || modifiers.control {
        return None;
    }
    let side = match key {
        Key::ArrowLeft => Side::Left,
        Key::ArrowRight => Side::Right,
        Key::ArrowUp => Side::Top,
        Key::ArrowDown => Side::Bottom,
        _ => return None,
    };
    Some(Command::FocusPane(side))
}
