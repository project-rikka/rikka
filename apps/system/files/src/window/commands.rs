use std::path::Path;
use std::sync::Arc;

use files_core::Location;
use files_undo::Step;
use files_vfs::Task as Job;
use rikka_app::prelude::*;

use crate::rename::RenameOutcome;
use crate::window::jobs::{After, Finished};
use crate::window::undo::Journal;
use crate::window::{Dialog, WindowState, clipboard, properties};
use crate::wording;

impl WindowState {
    pub(super) fn open_selected(&mut self) {
        if let Some(index) = self.tabs.active().first_chosen() {
            self.open(index);
        }
    }

    pub(super) fn open_selected_tabs(&mut self) {
        for at in self.tabs.active().selected_folders() {
            self.open_tab(at, false);
        }
    }

    pub(crate) fn open_with(&mut self, id: &str) {
        let targets = self.tabs.active().selection();
        let applications = self.services.applications();
        let Some(application) = applications.get(id) else {
            return;
        };
        if let Err(error) = files_open::launch(application, &targets) {
            self.fail(wording::launch(&error));
        }
    }

    pub(crate) fn open_with_desktop(&mut self) {
        if let Err(error) = files_open::open_with_desktop(&self.tabs.active().selection()) {
            self.fail(wording::launch(&error));
        }
    }

    pub(super) fn open_terminal_here(&mut self) {
        if let Some(folder) = self.tabs.active().at().to_path() {
            self.open_terminal(&folder);
        }
    }

    pub(super) fn open_terminal_in_selection(&mut self) {
        if let Some(folder) = self
            .tabs
            .active()
            .selected_folders()
            .first()
            .and_then(Location::to_path)
        {
            self.open_terminal(&folder);
        }
    }

    fn open_terminal(&mut self, folder: &Path) {
        let folder = folder.to_path_buf();
        spawn_blocking(
            move || files_open::open_terminal(&folder),
            |window: &mut WindowState, opened: rikka_open::Result<()>| {
                if let Err(error) = opened {
                    window.fail(wording::launch(&error));
                }
            },
        );
    }

    pub(super) fn copy(&mut self) {
        clipboard::publish(&self.tabs.active().selection(), false);
    }

    pub(super) fn cut(&mut self) {
        if self.tabs.active().selection_allows().rename {
            clipboard::publish(&self.tabs.active().selection(), true);
        }
    }

    pub(super) fn paste(&mut self) {
        if !self.tabs.active().folder_allows().write {
            return;
        }
        let into = self.tabs.active().at().clone();
        if let Some(job) = clipboard::paste(into) {
            self.run(job);
        }
    }

    pub(super) fn trash(&mut self) {
        let tab = self.tabs.active();
        if tab.chosen_count() == 0 || !tab.selection_allows().trash {
            return;
        }
        let items = tab.selection();
        self.run(Job::Trash { items });
    }

    pub(super) fn discard(&mut self) {
        match self.tabs.active().selection_allows().trash {
            true => self.trash(),
            false => self.confirm_delete(),
        }
    }

    pub(super) fn restore(&mut self) {
        let items = self.tabs.active().selection();
        self.run(Job::Restore { items });
    }

    pub(super) fn confirm_delete(&mut self) {
        let tab = self.tabs.active();
        if tab.chosen_count() == 0 || !tab.selection_allows().delete {
            return;
        }
        let items = tab.selection();
        self.dialog.show(Dialog::ConfirmDelete { items });
    }

    pub(super) fn empty_trash(&mut self) {
        let items = self.tabs.active().everything();
        self.dialog.show(Dialog::ConfirmDelete { items });
    }

    pub(super) fn show_properties(&mut self) {
        if let Some(properties) = properties::dialog(&self.services, self.tabs.active()) {
            self.dialog.show(properties);
        }
    }

    pub(super) fn create(&mut self, folder: bool) {
        match self.tabs.active_mut().make(&self.services.vfs, folder) {
            Ok(at) => {
                self.record(match folder {
                    true => Step::new_folder(at.clone()),
                    false => Step::new_file(at.clone()),
                });
                self.refresh_and_select(at);
            }
            Err(error) => self.fail(wording::failure(&error)),
        }
    }

    pub(crate) fn rename_to(&mut self, typed: &str) {
        match self
            .tabs
            .active_mut()
            .commit_rename(&self.services.vfs, typed)
        {
            RenameOutcome::Renamed { from, to } => {
                self.record(Step::renamed(from, to.clone()));
                self.refresh_and_select(to);
            }
            RenameOutcome::Refused(error) => self.fail(wording::failure(&error)),
            RenameOutcome::Nothing => {}
        }
    }

    fn refresh_and_select(&mut self, at: Location) {
        self.tabs.active_mut().select_on_arrival(vec![at]);
        self.refresh_active();
    }

    pub(crate) fn accept(&mut self) {
        match self.dialog.showing() {
            Some(Dialog::ConfirmDelete { items }) => {
                let job = Job::Delete {
                    items: items.clone(),
                };
                self.dismiss();
                self.run(job);
            }
            Some(Dialog::ConfirmRun { program }) => {
                let started = program.to_path().map(|path| files_open::run(&path));
                self.dismiss();
                if let Some(Err(error)) = started {
                    self.fail(wording::launch(&error));
                }
            }
            Some(Dialog::Compress(_)) => self.pack(),
            Some(Dialog::Convert(_)) => self.convert_chosen(),
            Some(Dialog::Password { .. }) => self.unlock(),
            _ => {}
        }
    }

    pub(crate) fn dismiss(&mut self) {
        self.dialog.dismiss();
    }

    pub(super) fn run(&mut self, job: Job) {
        let operation = rikka_file_operations::run(Arc::clone(&self.services.vfs), job);
        let journal = Journal::Record(operation.kind());
        self.jobs.run(operation, After::Journal(journal));
    }

    pub(super) fn job_finished(&mut self, finished: Finished) {
        let Finished { outcome, after } = finished;
        let retry = match after {
            After::Nothing => None,
            After::Retry(asking) => Some(asking),
            After::Journal(journal) => {
                self.write_journal(journal, &outcome);
                None
            }
            After::ShowWallpaper(library) => {
                self.wallpaper_copied(&library, &outcome);
                None
            }
        };
        if self.tabs.is_empty() {
            return;
        }
        if let Some(failure) = outcome.failures.first() {
            if let (files_vfs::Error::Locked { wrong, .. }, Some(asking)) = (&failure.error, retry)
            {
                self.ask_password(asking, *wrong);
                self.refresh_active();
                return;
            }
            let first = wording::failure(&failure.error);
            self.fail(match outcome.failures.len() {
                1 => first,
                count => tr!("files.error.and-more", first = &first, count = count - 1).to_string(),
            });
        }
        self.refresh_active();
    }
}
