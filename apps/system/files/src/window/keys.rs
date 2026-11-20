use files_core::ViewMode;
use rikka_app::prelude::*;
use rikka_input::Chord;

use crate::tabs::Tabs;
use crate::window::WindowState;

pub(crate) const DELETE_FOREVER: (Key, Modifiers) = (Key::Delete, Modifiers::SHIFT);
pub(crate) const NEW_FOLDER: (Key, Modifiers) = (Key::Char('n'), Modifiers::CONTROL_SHIFT);
pub(crate) const HIDDEN: (Key, Modifiers) = (Key::Char('h'), Modifiers::CONTROL);
pub(crate) const LOCATION: [(Key, Modifiers); 2] = [
    (Key::Char('l'), Modifiers::CONTROL),
    (Key::Char('d'), Modifiers::ALT),
];
pub(crate) const BACK: (Key, Modifiers) = (Key::Backspace, Modifiers::NONE);

pub(crate) fn label((key, modifiers): (Key, Modifiers)) -> String {
    Chord::new(modifiers, key).to_string()
}

pub(crate) fn bind(window: Window<WindowState>) -> Window<WindowState> {
    let bound = window
        .on_command(Command::Back, WindowState::back)
        .on_command(Command::Forward, WindowState::forward)
        .on_command(Command::Up, WindowState::up)
        .on_command(Command::Home, WindowState::home)
        .on_command(Command::Refresh, WindowState::refresh_active)
        .on_command(Command::Copy, WindowState::copy)
        .on_command(Command::Cut, WindowState::cut)
        .on_command(Command::Paste, WindowState::paste)
        .on_command(Command::Undo, WindowState::undo)
        .on_command(Command::Redo, WindowState::redo)
        .on_command(Command::SelectAll, WindowState::select_all)
        .on_command(Command::SelectNone, WindowState::select_none)
        .on_command(Command::Delete, WindowState::discard)
        .on_command(Command::Rename, |tabs: &mut Tabs| {
            tabs.active_mut().begin_rename();
        })
        .on_command(Command::Properties, WindowState::show_properties)
        .on_command(Command::Find, WindowState::seek)
        .on_command(Command::New, WindowState::new_window)
        .on_command(Command::NewTab, WindowState::new_tab)
        .on_command(Command::Close, WindowState::close_active_tab)
        .on_command(Command::NextTab, WindowState::next_tab)
        .on_command(Command::PreviousTab, WindowState::previous_tab)
        .on_chord(
            DELETE_FOREVER.0,
            DELETE_FOREVER.1,
            WindowState::confirm_delete,
        )
        .on_chord(NEW_FOLDER.0, NEW_FOLDER.1, |window: &mut WindowState| {
            if window.tabs.active().folder_allows().create {
                window.create(true);
            }
        })
        .on_chord(HIDDEN.0, HIDDEN.1, WindowState::toggle_hidden)
        .on_chord(BACK.0, BACK.1, WindowState::back)
        .on_chord(LOCATION[0].0, LOCATION[0].1, edit_location)
        .on_chord(LOCATION[1].0, LOCATION[1].1, edit_location)
        .on_chord(
            Key::Char('1'),
            Modifiers::CONTROL,
            |window: &mut WindowState| {
                window.view_as(ViewMode::Details);
            },
        )
        .on_chord(
            Key::Char('2'),
            Modifiers::CONTROL,
            |window: &mut WindowState| {
                window.view_as(ViewMode::Icons);
            },
        );
    ('1'..='9')
        .enumerate()
        .fold(bound, |bound, (index, digit)| {
            bound.on_chord(
                Key::Char(digit),
                Modifiers::ALT,
                move |window: &mut WindowState| {
                    window.nth_tab(index);
                },
            )
        })
}

fn edit_location(tabs: &mut Tabs) {
    tabs.active_mut().edit_path(true);
}
