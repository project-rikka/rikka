use rikka_app::prelude::*;

use crate::command::Command;
use crate::keys;
use crate::shell::{Shell, WindowContent, window};
use browser_pages::{Internal, Section};

impl Shell {
    pub(crate) fn pressed(&mut self, key: Key, modifiers: Modifiers) -> bool {
        if self.peek_key(key, modifiers) {
            return true;
        }
        let Some(command) = keys::command(key, modifiers) else {
            skip_view();
            return false;
        };
        if command == Command::Stop && !self.loading() {
            skip_view();
            return false;
        }
        self.command(command);
        true
    }

    pub(crate) fn command(&mut self, command: Command) {
        if self.little_command(command) {
            return;
        }
        let id = self.id;
        match command {
            Command::NewTab => self.new_tab(),
            Command::FocusAddress => self.focus_address(),
            Command::CloseTab => self.close_active(),
            Command::ReopenTab => self.reopen(),
            Command::NextTab => self.cycle(1),
            Command::PreviousTab => self.cycle(-1),
            Command::Tab(nth) => {
                self.browser.edit().select_nth(id, nth);
                self.changed();
            }
            Command::LastTab => {
                self.browser.edit().select_last(id);
                self.changed();
            }
            Command::Reload => self.reload(false),
            Command::HardReload => self.reload(true),
            Command::Stop => self.stop(),
            Command::Back => self.back(),
            Command::Forward => self.forward(),
            Command::ZoomIn => self.zoom(1),
            Command::ZoomOut => self.zoom(-1),
            Command::ZoomReset => self.zoom(0),
            Command::TogglePin => {
                let active = self.model().window(id).and_then(|layout| layout.active());
                if let Some(tab) = active {
                    self.toggle_pin(tab);
                }
            }
            Command::SavePage => self.save_page(),
            Command::NewWindow => open_window(window(&self.browser, false, WindowContent::Blank)),
            Command::PrivateWindow => {
                open_window(window(&self.browser, true, WindowContent::Blank))
            }
            Command::CloseWindow => self.close_window(),
            Command::Settings => self.open_settings(),
            Command::History => self.open_internal(Internal::History),
            Command::ClearData => self.open_internal(Internal::Settings(Some(Section::ClearData))),
            Command::ToggleSidebar => self.toggle_sidebar(),
            Command::PreviousSpace => self.step_space(-1),
            Command::NextSpace => self.step_space(1),
            Command::Palette => self.toggle_command_bar(),
            Command::SearchAddress => self.search_address(),
            Command::ReaderMode => self.toggle_reader(),
            Command::ToggleSplit => self.toggle_split(),
            Command::NextPane => self.step_pane(1),
            Command::PreviousPane => self.step_pane(-1),
            Command::Find => self.find_command(None),
            Command::FindNext => self.find_command(Some(true)),
            Command::FindPrevious => self.find_command(Some(false)),
            Command::ViewSource => self.view_active_source(),
            Command::Translate => self.toggle_translation(),
            Command::Downloads => self.open_internal(Internal::Downloads),
            Command::Fullscreen => self.toggle_fullscreen(),
        }
    }

    pub(crate) fn focus_address(&mut self) {
        let address = self
            .model()
            .active(self.id)
            .map(|tab_entry| tab_entry.address())
            .unwrap_or_default();
        self.address.begin(address);
    }

    pub(crate) fn toggle_sidebar(&mut self) {
        let preferences = self.browser.preferences();
        with_animation(Motion::Emphasized, || {
            preferences.set_sidebar_expanded(!preferences.sidebar_expanded());
        });
    }

    pub(crate) fn step_space(&mut self, by: isize) {
        self.browser.edit().step_space(self.id, by);
        self.changed();
    }

    fn cycle(&mut self, by: isize) {
        self.browser.edit().cycle(self.id, by);
        self.changed();
    }

    fn loading(&self) -> bool {
        self.model()
            .active(self.id)
            .is_some_and(|tab_entry| *tab_entry.meta.loading)
    }
}
