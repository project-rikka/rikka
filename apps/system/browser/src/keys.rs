use rikka_input::Chord;
use rikka_types::{Key, Modifiers};

use rikka_app::Window;

use crate::command::Command;
use crate::shell::Shell;

const CONTROL: Modifiers = Modifiers::CONTROL;
const ALT: Modifiers = Modifiers::ALT;
const SHIFT: Modifiers = Modifiers::SHIFT;
const NONE: Modifiers = Modifiers::NONE;
const CONTROL_SHIFT: Modifiers = Modifiers {
    shift: true,
    control: true,
    alt: false,
    logo: false,
};
const CONTROL_ALT: Modifiers = Modifiers {
    shift: false,
    control: true,
    alt: true,
    logo: false,
};

const TABLE: [(Modifiers, Key, Command); 59] = [
    (CONTROL, Key::Char('t'), Command::NewTab),
    (CONTROL, Key::Char('l'), Command::FocusAddress),
    (ALT, Key::Char('d'), Command::FocusAddress),
    (NONE, Key::F(6), Command::FocusAddress),
    (CONTROL, Key::Char('w'), Command::CloseTab),
    (CONTROL, Key::F(4), Command::CloseTab),
    (CONTROL_SHIFT, Key::Char('t'), Command::ReopenTab),
    (CONTROL, Key::Tab, Command::NextTab),
    (CONTROL_SHIFT, Key::Tab, Command::PreviousTab),
    (CONTROL, Key::PageDown, Command::NextTab),
    (CONTROL, Key::PageUp, Command::PreviousTab),
    (CONTROL, Key::Char('1'), Command::Tab(0)),
    (CONTROL, Key::Char('2'), Command::Tab(1)),
    (CONTROL, Key::Char('3'), Command::Tab(2)),
    (CONTROL, Key::Char('4'), Command::Tab(3)),
    (CONTROL, Key::Char('5'), Command::Tab(4)),
    (CONTROL, Key::Char('6'), Command::Tab(5)),
    (CONTROL, Key::Char('7'), Command::Tab(6)),
    (CONTROL, Key::Char('8'), Command::Tab(7)),
    (CONTROL, Key::Char('9'), Command::LastTab),
    (CONTROL, Key::Char('r'), Command::Reload),
    (NONE, Key::F(5), Command::Reload),
    (CONTROL_SHIFT, Key::Char('r'), Command::HardReload),
    (SHIFT, Key::F(5), Command::HardReload),
    (NONE, Key::Escape, Command::Stop),
    (ALT, Key::ArrowLeft, Command::Back),
    (ALT, Key::ArrowRight, Command::Forward),
    (NONE, Key::BrowserBack, Command::Back),
    (NONE, Key::BrowserForward, Command::Forward),
    (NONE, Key::BrowserRefresh, Command::Reload),
    (CONTROL, Key::F(5), Command::HardReload),
    (CONTROL, Key::Char('+'), Command::ZoomIn),
    (CONTROL, Key::Char('='), Command::ZoomIn),
    (CONTROL_SHIFT, Key::Char('+'), Command::ZoomIn),
    (CONTROL_SHIFT, Key::Char('='), Command::ZoomIn),
    (CONTROL, Key::Char('-'), Command::ZoomOut),
    (CONTROL_SHIFT, Key::Char('_'), Command::ZoomOut),
    (CONTROL, Key::Char('0'), Command::ZoomReset),
    (CONTROL, Key::Char('d'), Command::TogglePin),
    (CONTROL, Key::Char('s'), Command::Print),
    (CONTROL, Key::Char('u'), Command::ViewSource),
    (CONTROL, Key::Char('j'), Command::Downloads),
    (NONE, Key::F(11), Command::Fullscreen),
    (CONTROL, Key::Char('n'), Command::NewWindow),
    (CONTROL_SHIFT, Key::Char('n'), Command::PrivateWindow),
    (CONTROL_SHIFT, Key::Char('w'), Command::CloseWindow),
    (CONTROL, Key::Char(','), Command::Settings),
    (CONTROL, Key::Char('h'), Command::History),
    (CONTROL_SHIFT, Key::Delete, Command::ClearData),
    (CONTROL_SHIFT, Key::Char('b'), Command::ToggleSidebar),
    (CONTROL_ALT, Key::ArrowLeft, Command::PreviousSpace),
    (CONTROL_ALT, Key::ArrowRight, Command::NextSpace),
    (CONTROL, Key::Char('p'), Command::Palette),
    (CONTROL, Key::Char('k'), Command::SearchAddress),
    (CONTROL, Key::Char('e'), Command::SearchAddress),
    (NONE, Key::F(9), Command::ReaderMode),
    (CONTROL_ALT, Key::Char('s'), Command::ToggleSplit),
    (CONTROL_ALT, Key::Char(']'), Command::NextPane),
    (CONTROL_ALT, Key::Char('['), Command::PreviousPane),
];

pub(crate) fn bind(window: Window<Shell>) -> Window<Shell> {
    TABLE.iter().fold(window, |window, &(modifiers, key, _)| {
        window.on_chord(key, modifiers, move |shell: &mut Shell| {
            shell.pressed(key, modifiers);
        })
    })
}

pub(crate) fn command(key: Key, modifiers: Modifiers) -> Option<Command> {
    TABLE
        .iter()
        .find(|(candidate, chord, _)| Chord::new(*candidate, *chord).matches(modifiers, key))
        .map(|(_, _, command)| *command)
}

pub(crate) fn shortcut(command: Command) -> String {
    TABLE
        .iter()
        .find(|(_, _, bound)| *bound == command)
        .map(|(candidate, key, _)| Chord::new(*candidate, *key).to_string())
        .expect("every command a menu names has a chord")
}

pub(crate) fn reserved(loading: bool) -> impl Iterator<Item = Chord> {
    TABLE
        .iter()
        .filter(move |(_, key, command)| {
            *command != Command::Stop || (loading && *key == Key::Escape)
        })
        .map(|(candidate, key, _)| Chord::new(*candidate, *key))
}
