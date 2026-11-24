use rikka_types::{Key, Modifiers};
use rikka_widgets::Side;

use crate::command::Command;

const LAST_TAB_DIGIT: u32 = 9;

pub(crate) fn command(key: Key, modifiers: Modifiers) -> Option<Command> {
    if modifiers.logo {
        return None;
    }
    let Modifiers {
        control,
        shift,
        alt,
        ..
    } = modifiers;
    match key {
        Key::Char(character) => character_command(character.to_ascii_lowercase(), modifiers),
        Key::Tab if control && !alt => Some(match shift {
            true => Command::PreviousTab,
            false => Command::NextTab,
        }),
        Key::PageUp if control && !shift && !alt => Some(Command::PreviousTab),
        Key::PageDown if control && !shift && !alt => Some(Command::NextTab),
        Key::F(11) if modifiers == Modifiers::NONE => Some(Command::Fullscreen),
        Key::Enter if modifiers == Modifiers::ALT => Some(Command::Fullscreen),
        _ if alt && !control => {
            let side = side_of(key)?;
            Some(match shift {
                true => Command::ResizePane(side),
                false => Command::FocusPane(side),
            })
        }
        _ => None,
    }
}

fn character_command(character: char, modifiers: Modifiers) -> Option<Command> {
    let Modifiers {
        control,
        shift,
        alt,
        ..
    } = modifiers;
    match (control, alt, shift) {
        (_, true, false) => tab_command(character),
        (false, true, true) => match character {
            'd' => Some(Command::SplitAuto),
            '+' => Some(Command::Split(Side::Right)),
            '-' | '_' | '=' => Some(Command::Split(Side::Bottom)),
            _ => None,
        },
        (true, false, true) => Some(match character {
            'n' => Command::NewWindow,
            'q' => Command::CloseWindow,
            't' => Command::NewTab,
            'd' => Command::DuplicateTab,
            'w' => Command::CloseTab,
            'f' => Command::Find,
            '5' | '%' => Command::Split(Side::Right),
            '\'' | '"' => Command::Split(Side::Bottom),
            '+' | ';' | '=' => Command::Bigger,
            _ => return None,
        }),
        (true, false, false) => Some(match character {
            '+' | '=' => Command::Bigger,
            '-' => Command::Smaller,
            '0' => Command::ResetSize,
            _ => return None,
        }),
        _ => None,
    }
}

fn tab_command(character: char) -> Option<Command> {
    match character.to_digit(10)? {
        0 => None,
        LAST_TAB_DIGIT => Some(Command::LastTab),
        digit => Some(Command::Tab(digit as usize - 1)),
    }
}

fn side_of(key: Key) -> Option<Side> {
    Some(match key {
        Key::ArrowLeft => Side::Left,
        Key::ArrowRight => Side::Right,
        Key::ArrowUp => Side::Top,
        Key::ArrowDown => Side::Bottom,
        _ => return None,
    })
}
