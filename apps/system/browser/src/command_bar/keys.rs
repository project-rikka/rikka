use rikka_app::prelude::*;
use rikka_omnibox::Surface;

use crate::omnibox::Opening;
use crate::shell::Shell;

const SURFACE: Surface = Surface::CommandBar;

impl Shell {
    pub(crate) fn toggle_command_bar(&mut self) {
        if self.command_bar.is_open() {
            self.close_command_bar();
            return;
        }
        self.address_mut().omnibox.picker.reset();
        self.command_bar.open();
        self.pick(SURFACE);
    }

    pub(crate) fn close_command_bar(&mut self) {
        self.forget_picking(SURFACE);
        self.command_bar.close();
    }

    pub(crate) fn command_bar_edited(&mut self) {
        skip_view();
        self.command_bar.picker.arrowed = false;
        self.pick(SURFACE);
    }

    pub(crate) fn command_bar_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        match key {
            Key::Enter => {
                let opening = match (modifiers.control || modifiers.alt, modifiers.shift) {
                    (true, _) => Opening::NewTab,
                    (false, true) => Opening::NewWindow,
                    (false, false) => Opening::Here,
                };
                self.run_command_bar(opening);
                true
            }
            Key::Escape => {
                self.close_command_bar();
                true
            }
            Key::ArrowDown => self.step_command_bar(1),
            Key::ArrowUp => self.step_command_bar(-1),
            Key::Tab => self.step_command_bar(if modifiers.shift { -1 } else { 1 }),
            _ => false,
        }
    }

    pub(crate) fn run_command_bar(&mut self, opening: Opening) {
        let Some(index) = self.command_bar.picker.selected() else {
            return;
        };
        self.choose(SURFACE, index, opening);
    }

    fn step_command_bar(&mut self, by: isize) -> bool {
        let picker = &mut self.command_bar.picker;
        let Some(listing) = picker.listing_mut() else {
            return false;
        };
        let stepped = listing.step(by).is_some();
        picker.arrowed = stepped;
        skip_view();
        stepped
    }
}
