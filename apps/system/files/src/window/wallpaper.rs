use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use files_core::Location;
use files_vfs::{Outcome, Task as Job};
use files_wallpaper::Library;
use rikka_app::prelude::*;
use rikka_file_operations::Resolution;

use crate::window::WindowState;
use crate::window::jobs::After;
use crate::wording;

#[derive(Debug)]
enum Found {
    Held(String),
    Missing(Library),
}

fn find(source: &Path) -> files_wallpaper::Result<Found> {
    let library = Library::session()?;
    Ok(match library.holding(source)? {
        Some(id) => Found::Held(id),
        None => Found::Missing(library),
    })
}

impl WindowState {
    pub(super) fn wallpaper_source(&self) -> Option<Location> {
        let tab = self.tabs.active();
        if !tab.at().is_local() || tab.chosen_count() != 1 || tab.any_folder_selected() {
            return None;
        }
        let source = tab.selection().into_iter().next()?;
        let path = source.to_path()?;
        let named = path.file_name().and_then(OsStr::to_str).is_some();
        (named && files_wallpaper::is_wallpaper(&path)).then_some(source)
    }

    pub(super) fn set_wallpaper(&mut self) {
        let Some(source) = self.wallpaper_source() else {
            return;
        };
        let Some(path) = source.to_path() else {
            return;
        };
        spawn_blocking(
            move || find(&path),
            move |window: &mut WindowState, found: files_wallpaper::Result<Found>| {
                window.wallpaper_found(source.clone(), found);
            },
        );
    }

    fn wallpaper_found(&mut self, source: Location, found: files_wallpaper::Result<Found>) {
        match found {
            Ok(Found::Held(id)) => show(id),
            Ok(Found::Missing(library)) => {
                let into = Location::from_path(library.folder());
                let operation = rikka_file_operations::run(
                    Arc::clone(&self.services.vfs),
                    Job::Copy {
                        sources: vec![source],
                        into,
                    },
                );
                operation.resolve(Resolution::Keep, true);
                self.jobs.run(operation, After::ShowWallpaper(library));
            }
            Err(error) => self.wallpaper_failed(&error),
        }
    }

    pub(super) fn wallpaper_copied(&mut self, library: &Library, outcome: &Outcome) {
        let id = outcome
            .placed
            .iter()
            .filter_map(|placed| placed.to.to_path())
            .find_map(|path| library.id_of(&path));
        if let Some(id) = id {
            show(id);
        }
    }

    fn wallpaper_failed(&mut self, error: &files_wallpaper::Error) {
        if self.tabs.is_empty() {
            return;
        }
        self.fail(wording::wallpaper(error));
    }
}

fn show(id: String) {
    spawn_blocking(
        move || files_wallpaper::show(&id),
        |window: &mut WindowState, shown: files_wallpaper::Result<()>| {
            if let Err(error) = shown {
                window.wallpaper_failed(&error);
            }
        },
    );
}
