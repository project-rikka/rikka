use std::os::unix::fs::MetadataExt as _;

use files_core::Location;
use files_vfs::Task as Job;
use rikka_ui::{DragAction, DragOffer, spawn_blocking};

use crate::window::WindowState;

#[derive(Debug)]
struct DroppedFiles {
    sources: Vec<Location>,
    into: Location,
    moving: bool,
}

impl WindowState {
    pub(crate) fn drop_into(
        &mut self,
        folder: Option<usize>,
        offer: DragOffer,
        action: DragAction,
    ) {
        let tab = self.tabs.active();
        let into = folder
            .and_then(|index| tab.rows().get(index))
            .filter(|row| row.is_directory())
            .map_or_else(|| tab.at().clone(), |row| row.at.clone());
        spawn_blocking(
            move || dropped_files(&offer.uris(), into, action == DragAction::Move),
            WindowState::drop_files,
        );
    }

    fn drop_files(&mut self, dropped_files: DroppedFiles) {
        let DroppedFiles {
            sources,
            into,
            moving,
        } = dropped_files;
        if sources.is_empty() {
            return;
        }
        self.run(match moving {
            true => Job::Move { sources, into },
            false => Job::Copy { sources, into },
        });
    }
}

fn dropped_files(uris: &[String], into: Location, moving: bool) -> DroppedFiles {
    let sources: Vec<Location> = uris
        .iter()
        .filter_map(|uri| Location::parse(uri))
        .filter(|source| can_drop_into(source, &into))
        .collect();
    let moving = moving && sources.iter().all(|source| same_device(source, &into));
    DroppedFiles {
        sources,
        into,
        moving,
    }
}

fn can_drop_into(source: &Location, into: &Location) -> bool {
    source != into && !source.contains(into) && source.parent().as_ref() != Some(into)
}

fn same_device(source: &Location, into: &Location) -> bool {
    let (Some(source), Some(into)) = (source.to_path(), into.to_path()) else {
        return false;
    };
    match (std::fs::symlink_metadata(source), std::fs::metadata(into)) {
        (Ok(source), Ok(into)) => source.dev() == into.dev(),
        _ => false,
    }
}
