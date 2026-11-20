use std::path::PathBuf;
use std::sync::Arc;

use files_archive::Level;
use files_core::Location;
use rikka_app::prelude::*;
use rikka_ui::spawn_blocking;

use crate::browser::Row;
use crate::devices::Command;
use crate::window::jobs::After;
use crate::window::{Asking, Compressing, Dialog, WindowState};
use crate::wording;

pub(crate) const LEVELS: [Level; 3] = [Level::Balanced, Level::Fastest, Level::Best];

impl WindowState {
    pub(super) fn browse(&self, archive: Location, password: Option<String>) {
        let vfs = Arc::clone(&self.services.vfs);
        let archive_location = archive.clone();
        spawn_blocking(
            move || vfs.browse(&archive_location, password.as_deref()),
            move |window: &mut WindowState, opened| window.browsing(archive.clone(), opened),
        );
    }

    fn browsing(&mut self, archive: Location, opened: Result<Location, files_vfs::Error>) {
        if self.tabs.is_empty() {
            return;
        }
        self.tabs.active_mut().stop_loading();
        match opened {
            Ok(inside) => self.navigate_active(inside),
            Err(files_vfs::Error::Locked { wrong, .. }) => {
                self.ask_password(Asking::Browse { archive }, wrong);
            }
            Err(error) => self.fail(wording::failure(&error)),
        }
    }

    pub(super) fn extract(&mut self) {
        let Some(archive) = self.archive_selected().map(|row| row.at.clone()) else {
            return;
        };
        if let Some(beside) = archive.parent() {
            self.unpack(archive, beside, None);
        }
    }

    fn unpack(&mut self, archive: Location, beside: Location, password: Option<String>) {
        let (Some(source), Some(into)) = (archive.to_path(), beside.to_path()) else {
            return;
        };
        let job = files_archive::extract(source, into, password);
        self.jobs
            .run(job, After::Retry(Asking::Extract { archive, beside }));
    }

    pub(super) fn ask_password(&mut self, asking: Asking, wrong: bool) {
        self.dialog.show(Dialog::Password {
            asking,
            typed: TextDocument::new(""),
            wrong,
        });
    }

    pub(super) fn unlock(&mut self) {
        let Some(Dialog::Password { asking, typed, .. }) = self.dialog.showing() else {
            return;
        };
        let password = typed.text();
        if password.is_empty() {
            return;
        }
        let asking = asking.clone();
        self.dismiss();
        match asking {
            Asking::Volume { id, .. } => self.devices.run(Command::Unlock {
                id,
                passphrase: password,
            }),
            Asking::Browse { archive } => self.browse(archive, Some(password)),
            Asking::Extract { archive, beside } => self.unpack(archive, beside, Some(password)),
        }
    }

    pub(super) fn archive_selected(&self) -> Option<&Row> {
        let row = self.tabs.active().only_selected()?;
        let unpackable = !row.is_directory() && self.services.vfs.browses(&row.at);
        unpackable.then_some(row)
    }

    pub(super) fn packable(&self) -> bool {
        let tab = self.tabs.active();
        tab.folder_allows().create && tab.chosen_count() > 0 && tab.at().is_local()
    }

    pub(super) fn show_compress(&mut self) {
        if let Some(sheet) = self.packing() {
            self.dialog.show(Dialog::Compress(sheet));
        }
    }

    fn packing(&self) -> Option<Compressing> {
        let tab = self.tabs.active();
        let sources = tab.selection();
        let into = tab.at().clone();
        if sources.is_empty() || !self.packable() {
            return None;
        }
        let proposed = match tab.only_selected() {
            Some(only) => rikka_format::name_stem(&only.name).to_owned(),
            None => into.name(),
        };
        Some(Compressing {
            sources,
            into,
            name: TextDocument::new(&proposed),
            format: 0,
            compression: 0,
            password: TextDocument::new(""),
        })
    }

    pub(crate) fn compress_format(&mut self, index: usize) {
        if let Some(Dialog::Compress(packing)) = self.dialog.showing_mut() {
            packing.format = index.min(files_archive::OFFERS.len() - 1);
        }
    }

    pub(crate) fn compress_level(&mut self, index: usize) {
        if let Some(Dialog::Compress(packing)) = self.dialog.showing_mut() {
            packing.compression = index.min(LEVELS.len() - 1);
        }
    }

    pub(super) fn pack(&mut self) {
        let Some(Dialog::Compress(sheet)) = self.dialog.showing() else {
            return;
        };
        let format = files_archive::OFFERS[sheet.format];
        let written = sheet.name.text();
        let typed = written.trim();
        if typed.is_empty() {
            return;
        }
        let archive = sheet.into.child(format!("{typed}.{}", format.extension()));
        let (Some(into), Some(sources)) = (archive.to_path(), local(&sheet.sources)) else {
            return;
        };
        let packing = files_archive::Packing {
            sources,
            into,
            format,
            level: LEVELS[sheet.compression],
            password: format
                .supports_encryption()
                .then(|| sheet.password.text())
                .filter(|typed| !typed.is_empty()),
        };
        self.dismiss();
        self.jobs
            .run(files_archive::compress(packing), After::Nothing);
    }
}

pub(super) fn local(sources: &[Location]) -> Option<Vec<PathBuf>> {
    sources.iter().map(Location::to_path).collect()
}
