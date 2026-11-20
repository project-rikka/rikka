use std::sync::LazyLock;

use rikka_types::{Key, Modifiers, PointerButton};

use crate::action::{ActionMap, Chord};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Back,
    Forward,
    Up,
    Home,
    Refresh,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    SelectNone,
    Delete,
    Find,
    FindNext,
    FindPrevious,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    New,
    NewTab,
    Open,
    Save,
    SaveAs,
    Close,
    Quit,
    NextTab,
    PreviousTab,
    Rename,
    Properties,
    Fullscreen,
    Preferences,
    ContextMenu,
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::SHIFT;
const CONTROL: Modifiers = Modifiers::CONTROL;
const ALT: Modifiers = Modifiers::ALT;
const CONTROL_SHIFT: Modifiers = Modifiers::CONTROL_SHIFT;

const STANDARD: [(Modifiers, Key, Command); 58] = [
    (ALT, Key::ArrowLeft, Command::Back),
    (NONE, Key::BrowserBack, Command::Back),
    (ALT, Key::ArrowRight, Command::Forward),
    (NONE, Key::BrowserForward, Command::Forward),
    (ALT, Key::ArrowUp, Command::Up),
    (ALT, Key::Home, Command::Home),
    (NONE, Key::BrowserHome, Command::Home),
    (NONE, Key::F(5), Command::Refresh),
    (CONTROL, Key::Char('r'), Command::Refresh),
    (NONE, Key::BrowserRefresh, Command::Refresh),
    (CONTROL, Key::Char('z'), Command::Undo),
    (NONE, Key::Undo, Command::Undo),
    (CONTROL_SHIFT, Key::Char('z'), Command::Redo),
    (CONTROL, Key::Char('y'), Command::Redo),
    (NONE, Key::Redo, Command::Redo),
    (CONTROL, Key::Char('x'), Command::Cut),
    (SHIFT, Key::Delete, Command::Cut),
    (NONE, Key::Cut, Command::Cut),
    (CONTROL, Key::Char('c'), Command::Copy),
    (CONTROL, Key::Insert, Command::Copy),
    (NONE, Key::Copy, Command::Copy),
    (CONTROL, Key::Char('v'), Command::Paste),
    (SHIFT, Key::Insert, Command::Paste),
    (NONE, Key::Paste, Command::Paste),
    (CONTROL, Key::Char('a'), Command::SelectAll),
    (CONTROL_SHIFT, Key::Char('a'), Command::SelectNone),
    (NONE, Key::Delete, Command::Delete),
    (CONTROL, Key::Char('f'), Command::Find),
    (NONE, Key::Find, Command::Find),
    (CONTROL, Key::Char('g'), Command::FindNext),
    (NONE, Key::F(3), Command::FindNext),
    (CONTROL_SHIFT, Key::Char('g'), Command::FindPrevious),
    (SHIFT, Key::F(3), Command::FindPrevious),
    (CONTROL, Key::Char('+'), Command::ZoomIn),
    (CONTROL, Key::Char('='), Command::ZoomIn),
    (CONTROL_SHIFT, Key::Char('+'), Command::ZoomIn),
    (CONTROL_SHIFT, Key::Char('='), Command::ZoomIn),
    (CONTROL, Key::Char('-'), Command::ZoomOut),
    (CONTROL_SHIFT, Key::Char('_'), Command::ZoomOut),
    (CONTROL, Key::Char('0'), Command::ZoomReset),
    (CONTROL, Key::Char('n'), Command::New),
    (CONTROL, Key::Char('t'), Command::NewTab),
    (CONTROL, Key::Char('o'), Command::Open),
    (CONTROL, Key::Char('s'), Command::Save),
    (CONTROL_SHIFT, Key::Char('s'), Command::SaveAs),
    (CONTROL, Key::Char('w'), Command::Close),
    (CONTROL, Key::F(4), Command::Close),
    (CONTROL, Key::Char('q'), Command::Quit),
    (CONTROL, Key::Tab, Command::NextTab),
    (CONTROL, Key::PageDown, Command::NextTab),
    (CONTROL_SHIFT, Key::Tab, Command::PreviousTab),
    (CONTROL, Key::PageUp, Command::PreviousTab),
    (NONE, Key::F(2), Command::Rename),
    (ALT, Key::Enter, Command::Properties),
    (NONE, Key::F(11), Command::Fullscreen),
    (CONTROL, Key::Char(','), Command::Preferences),
    (NONE, Key::Menu, Command::ContextMenu),
    (SHIFT, Key::F(10), Command::ContextMenu),
];

static KEYMAP: LazyLock<ActionMap<Command>> = LazyLock::new(|| {
    STANDARD
        .into_iter()
        .fold(ActionMap::new(), |map, (modifiers, key, command)| {
            map.bind(Chord::new(modifiers, key), command)
        })
});

impl Command {
    #[must_use]
    pub fn for_key(key: Key, modifiers: Modifiers) -> Option<Command> {
        KEYMAP.resolve(modifiers, key).copied()
    }

    #[must_use]
    pub fn for_button(button: PointerButton) -> Option<Command> {
        match button {
            PointerButton::Back => Some(Command::Back),
            PointerButton::Forward => Some(Command::Forward),
            PointerButton::Primary
            | PointerButton::Secondary
            | PointerButton::Middle
            | PointerButton::Other(_) => None,
        }
    }

    #[must_use]
    pub fn chord(self) -> Chord {
        KEYMAP
            .chord_for(&self)
            .expect("every standard command has a chord")
    }
}
