use files_core::Location;
use files_drives::VolumeId;

use crate::devices::{Command, Outcome};
use crate::places::Intent;
use crate::window::{Asking, WindowState};
use crate::wording;

impl WindowState {
    pub(crate) fn open_place(&mut self, index: usize, tab: bool) {
        let Some(row) = self.services.row(index) else {
            return;
        };
        match row.intent() {
            Intent::Go(at) if tab => self.open_tab(at, false),
            Intent::Go(at) => self.navigate_active(at),
            Intent::Mount(id) => self.devices.run(Command::Mount { id, open: true }),
            Intent::Unlock(id) => self.ask_passphrase(&id, false),
            Intent::Nothing => {}
        }
    }

    pub(crate) fn detach(&mut self, index: usize) {
        let Some(row) = self.services.row(index) else {
            return;
        };
        let (Some(device), Some(detach)) = (row.device(), row.detach()) else {
            return;
        };
        self.devices.run(detach.command(device.id.clone()));
    }

    pub(crate) fn reorder(&mut self, moved: &Location, before: &Location) {
        let saved = files_places::reorder(moved, before);
        self.bookmarks_saved(saved);
    }

    pub(crate) fn pin(&mut self, at: &Location) {
        let saved = files_places::add(at, None);
        self.bookmarks_saved(saved);
    }

    pub(crate) fn unpin(&mut self, at: &Location) {
        let saved = files_places::remove(at);
        self.bookmarks_saved(saved);
    }

    fn bookmarks_saved(&mut self, saved: std::io::Result<()>) {
        if let Err(error) = saved {
            self.fail(wording::bookmarks(&error));
        }
        self.services.reload_places();
    }

    pub(super) fn pin_here(&mut self) {
        let at = self.tabs.active().at().clone();
        self.pin(&at);
    }

    pub(super) fn unpin_here(&mut self) {
        let at = self.tabs.active().at().clone();
        self.unpin(&at);
    }

    pub(super) fn pin_selection(&mut self) {
        for at in self.tabs.active().selected_folders() {
            self.pin(&at);
        }
    }

    pub(super) fn device_command_finished(&mut self, outcome: Outcome) {
        if self.tabs.is_empty() {
            return;
        }
        if let Some(at) = outcome.open {
            self.navigate_active(at);
        }
        match outcome.failure {
            Some(files_drives::Error::WrongPassphrase) => self.ask_passphrase(&outcome.id, true),
            Some(error) => self.fail(wording::device(&error)),
            None => {}
        }
    }

    pub(super) fn ask_passphrase(&mut self, id: &VolumeId, wrong: bool) {
        let Some(volume) = self.services.volume(id) else {
            return;
        };
        self.ask_password(
            Asking::Volume {
                id: id.clone(),
                name: wording::volume(&volume),
            },
            wrong,
        );
    }
}
