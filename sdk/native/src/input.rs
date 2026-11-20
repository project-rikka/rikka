use rikka_ui::{Command, Key, Modifiers};

use crate::abi::violation;
use crate::ffi;

const FIRST_FUNCTION_KEY: u32 = ffi::KeyCode::F1.0;
const LAST_FUNCTION_KEY: u32 = ffi::KeyCode::F24.0;

pub(crate) fn key(code: ffi::KeyCode) -> Key {
    match code {
        ffi::KeyCode::ENTER => Key::Enter,
        ffi::KeyCode::ESCAPE => Key::Escape,
        ffi::KeyCode::TAB => Key::Tab,
        ffi::KeyCode::BACKSPACE => Key::Backspace,
        ffi::KeyCode::DELETE => Key::Delete,
        ffi::KeyCode::INSERT => Key::Insert,
        ffi::KeyCode::SPACE => Key::Space,
        ffi::KeyCode::ARROW_UP => Key::ArrowUp,
        ffi::KeyCode::ARROW_DOWN => Key::ArrowDown,
        ffi::KeyCode::ARROW_LEFT => Key::ArrowLeft,
        ffi::KeyCode::ARROW_RIGHT => Key::ArrowRight,
        ffi::KeyCode::HOME => Key::Home,
        ffi::KeyCode::END => Key::End,
        ffi::KeyCode::PAGE_UP => Key::PageUp,
        ffi::KeyCode::PAGE_DOWN => Key::PageDown,
        ffi::KeyCode::MENU => Key::Menu,
        ffi::KeyCode(raw @ FIRST_FUNCTION_KEY..=LAST_FUNCTION_KEY) => {
            Key::F((raw - FIRST_FUNCTION_KEY + 1) as u8)
        }
        ffi::KeyCode(raw) => match char::from_u32(raw) {
            Some(character) if !character.is_control() => Key::Char(character).to_lowercase(),
            _ => violation(format!("{raw:#x} is not a key")),
        },
    }
}

pub(crate) fn modifiers(modifiers: ffi::Modifiers) -> Modifiers {
    Modifiers {
        shift: modifiers.contains(ffi::Modifiers::SHIFT),
        control: modifiers.contains(ffi::Modifiers::CONTROL),
        alt: modifiers.contains(ffi::Modifiers::ALT),
        logo: modifiers.contains(ffi::Modifiers::LOGO),
    }
}

pub(crate) fn command(command: ffi::Command) -> Command {
    match command {
        ffi::Command::Back => Command::Back,
        ffi::Command::Forward => Command::Forward,
        ffi::Command::Up => Command::Up,
        ffi::Command::Home => Command::Home,
        ffi::Command::Refresh => Command::Refresh,
        ffi::Command::Undo => Command::Undo,
        ffi::Command::Redo => Command::Redo,
        ffi::Command::Cut => Command::Cut,
        ffi::Command::Copy => Command::Copy,
        ffi::Command::Paste => Command::Paste,
        ffi::Command::SelectAll => Command::SelectAll,
        ffi::Command::SelectNone => Command::SelectNone,
        ffi::Command::Delete => Command::Delete,
        ffi::Command::Find => Command::Find,
        ffi::Command::FindNext => Command::FindNext,
        ffi::Command::FindPrevious => Command::FindPrevious,
        ffi::Command::ZoomIn => Command::ZoomIn,
        ffi::Command::ZoomOut => Command::ZoomOut,
        ffi::Command::ZoomReset => Command::ZoomReset,
        ffi::Command::New => Command::New,
        ffi::Command::NewTab => Command::NewTab,
        ffi::Command::Open => Command::Open,
        ffi::Command::Save => Command::Save,
        ffi::Command::SaveAs => Command::SaveAs,
        ffi::Command::Close => Command::Close,
        ffi::Command::Quit => Command::Quit,
        ffi::Command::NextTab => Command::NextTab,
        ffi::Command::PreviousTab => Command::PreviousTab,
        ffi::Command::Rename => Command::Rename,
        ffi::Command::Properties => Command::Properties,
        ffi::Command::Fullscreen => Command::Fullscreen,
        ffi::Command::Preferences => Command::Preferences,
        ffi::Command::ContextMenu => Command::ContextMenu,
    }
}
