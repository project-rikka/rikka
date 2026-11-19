use std::ops::Range;

use rikka_types::{CursorShape, ResizeEdge, SCROLL_LINE};
use rikka_ui::{
    Command, Event, Key, KeyCode, KeyLocation, Modifiers, Point, PointerButton, ScrollDelta,
    ScrollSource,
};

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

pub(crate) fn point(x: f32, y: f32) -> Point {
    if !(x.is_finite() && y.is_finite()) {
        violation(format!("({x}, {y}) is not a finite point"));
    }
    Point::new(x, y)
}

pub(crate) fn button(button: ffi::PointerButton) -> PointerButton {
    match button {
        ffi::PointerButton::Left => PointerButton::Primary,
        ffi::PointerButton::Right => PointerButton::Secondary,
        ffi::PointerButton::Middle => PointerButton::Middle,
        ffi::PointerButton::Back => PointerButton::Back,
        ffi::PointerButton::Forward => PointerButton::Forward,
    }
}

pub(crate) fn wheel(at: Point, steps_x: f32, steps_y: f32, modifiers: Modifiers) -> Event {
    if !(steps_x.is_finite() && steps_y.is_finite()) {
        violation(format!("({steps_x}, {steps_y}) are not finite wheel steps"));
    }
    Event::Scroll {
        position: at,
        delta: ScrollDelta::from_content(Point::new(-steps_x * SCROLL_LINE, steps_y * SCROLL_LINE)),
        modifiers,
        source: ScrollSource::Wheel,
    }
}

pub(crate) fn key_press(key: Key, modifiers: Modifiers) -> Event {
    Event::KeyPressed {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers,
    }
}

pub(crate) fn key_release(key: Key, modifiers: Modifiers) -> Event {
    Event::KeyReleased {
        key,
        code: KeyCode::UNKNOWN,
        location: KeyLocation::Standard,
        modifiers,
    }
}

pub(crate) fn typed(text: &str) -> impl Iterator<Item = Key> + '_ {
    text.chars()
        .filter(|character| !character.is_control())
        .map(|character| match character {
            ' ' => Key::Space,
            character => Key::Char(character),
        })
}

pub(crate) fn caret(text: &str, start: usize, end: usize) -> Option<Range<usize>> {
    if start == usize::MAX {
        return None;
    }
    let valid = start <= end && text.is_char_boundary(start) && text.is_char_boundary(end);
    if !valid {
        violation(format!(
            "{start}..{end} is not a caret on character boundaries of the composition"
        ));
    }
    Some(start..end)
}

pub(crate) fn cursor(shape: CursorShape) -> ffi::CursorShape {
    match shape {
        CursorShape::Hidden => ffi::CursorShape::Hidden,
        CursorShape::Pointer => ffi::CursorShape::Pointer,
        CursorShape::Text | CursorShape::VerticalText => ffi::CursorShape::Text,
        CursorShape::Wait => ffi::CursorShape::Wait,
        CursorShape::Progress => ffi::CursorShape::Progress,
        CursorShape::Cell | CursorShape::Crosshair => ffi::CursorShape::Crosshair,
        CursorShape::Move | CursorShape::AllScroll => ffi::CursorShape::Move,
        CursorShape::Grab => ffi::CursorShape::Grab,
        CursorShape::Grabbing => ffi::CursorShape::Grabbing,
        CursorShape::NoDrop | CursorShape::NotAllowed => ffi::CursorShape::NotAllowed,
        CursorShape::ResizeColumn | CursorShape::ResizeEastWest => {
            ffi::CursorShape::ResizeHorizontal
        }
        CursorShape::ResizeRow | CursorShape::ResizeNorthSouth => ffi::CursorShape::ResizeVertical,
        CursorShape::ResizeNorthWestSouthEast => ffi::CursorShape::ResizeDiagonalDown,
        CursorShape::ResizeNorthEastSouthWest => ffi::CursorShape::ResizeDiagonalUp,
        CursorShape::Resize(edge) => match edge {
            ResizeEdge::Left | ResizeEdge::Right => ffi::CursorShape::ResizeHorizontal,
            ResizeEdge::Top | ResizeEdge::Bottom => ffi::CursorShape::ResizeVertical,
            ResizeEdge::TopLeft | ResizeEdge::BottomRight => ffi::CursorShape::ResizeDiagonalDown,
            ResizeEdge::TopRight | ResizeEdge::BottomLeft => ffi::CursorShape::ResizeDiagonalUp,
        },
        CursorShape::ZoomIn => ffi::CursorShape::ZoomIn,
        CursorShape::ZoomOut => ffi::CursorShape::ZoomOut,
        CursorShape::Help => ffi::CursorShape::Help,
        CursorShape::Default
        | CursorShape::ContextMenu
        | CursorShape::Alias
        | CursorShape::Copy
        | CursorShape::Image(_) => ffi::CursorShape::Default,
    }
}
