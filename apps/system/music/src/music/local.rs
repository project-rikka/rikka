use std::path::PathBuf;

use music_core::{Connection, Track};
use music_library::{Change, Library, Progress, TrackId};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};

use crate::library::Library as Shelves;
use crate::music::{Music, Start, resume_key};
use crate::player::Player;
use crate::settings::Section;

const LOCAL_SCAN: &str = "music-local-scan";
const LOCAL_CHANGES: &str = "music-local-changes";

impl Music {
    pub(crate) fn local_key(&self) -> Option<String> {
        self.providers
            .services()
            .find(|service| service.is_local())
            .map(|service| service.slug.to_owned())
    }

    pub(crate) fn local(&self) -> Option<&Library> {
        self.local.as_ref()
    }

    pub(crate) fn browses_local(&self) -> bool {
        self.connection()
            .is_some_and(|connection| connection.service.is_local())
    }

    pub(crate) fn scanning(&self) -> Progress {
        *self.scanning
    }

    pub(crate) fn indexed(&self) -> u64 {
        *self.indexed
    }

    pub(crate) fn listened(&self) -> u64 {
        *self.listened
    }

    pub(super) fn watch_local(&mut self) {
        let Some(library) = &self.local else {
            return;
        };
        self.scanning.set(library.progress().get());
        stream_keyed(
            LOCAL_SCAN,
            library.progress().changes(),
            |music: &mut Music, progress: Progress| music.scanning.set(progress),
        );
        stream_keyed(
            LOCAL_CHANGES,
            library.subscribe(),
            |music: &mut Music, change: Change| match change {
                Change::Listening(_) => music.listened.set(*music.listened + 1),
                Change::Indexed | Change::Folders => music.indexed.set(*music.indexed + 1),
            },
        );
    }

    pub(crate) fn open_library_settings(&mut self) {
        self.open_settings(true);
        self.settings.show(Section::Library);
    }

    pub(crate) fn add_folders(&mut self) {
        let Some(library) = self.local.clone() else {
            return;
        };
        let title = tr!("music.library.choose").to_string();
        let parent = window_identifier();
        spawn_blocking(
            move || {
                let chosen = Chooser::open(title)
                    .parent(parent)
                    .directory(true)
                    .multiple(true)
                    .run()
                    .map_err(|error| error.to_string())?;
                let ChooserResponse::Files(folders) = chosen else {
                    return Ok(());
                };
                folders.into_iter().try_for_each(|folder| {
                    library
                        .add_folder(folder)
                        .map_err(|error| error.to_string())
                })
            },
            |shelves: &mut Shelves, added: Result<(), String>| match added {
                Ok(()) => skip_view(),
                Err(reason) => shelves.fail(&reason),
            },
        );
    }

    pub(crate) fn remove_folder(&mut self, folder: PathBuf) {
        let Some(library) = self.local.clone() else {
            return;
        };
        spawn_blocking(
            move || {
                library
                    .remove_folder(&folder)
                    .map_err(|error| error.to_string())
            },
            |shelves: &mut Shelves, removed: Result<bool, String>| match removed {
                Ok(_) => skip_view(),
                Err(reason) => shelves.fail(&reason),
            },
        );
    }

    pub(crate) fn rescan(&mut self) {
        if let Some(library) = &self.local {
            drop(library.rescan());
        }
    }

    pub(crate) fn reveal(&mut self, track: &Track) {
        let Some(library) = self.local.clone() else {
            return;
        };
        let Ok(id) = music_local::track_id(&track.id) else {
            return;
        };
        spawn_blocking(
            move || revealed(&library, id),
            |shelves: &mut Shelves, revealed: Result<(), String>| match revealed {
                Ok(()) => skip_view(),
                Err(reason) => shelves.fail(&reason),
            },
        );
    }

    pub(crate) fn play_folder(&mut self, folder: PathBuf, start: Start) {
        let Some(library) = self.local.clone() else {
            return;
        };
        spawn_blocking(
            move || {
                library
                    .tracks_under(&folder)
                    .map(|rows| music_local::tracks(&rows))
            },
            move |player: &mut Player, gathered: music_library::Result<Vec<Track>>| match gathered {
                Ok(tracks) => player.start(&tracks, start),
                Err(error) => {
                    tracing::warn!("failed to read folder: {error}");
                    skip_view();
                }
            },
        );
    }

    pub(super) fn link_local(&mut self) {
        let Some(key) = self.local_key() else {
            return;
        };
        let Some(provider) = self.providers.find(&key).cloned() else {
            return;
        };
        let folder = self.accounts.folder(&key);
        spawn_keyed(
            resume_key(&key),
            async move {
                provider
                    .resume(&folder)
                    .await
                    .map_err(|error| error.to_string())
            },
            move |music: &mut Music, resumed: Result<Option<Connection>, String>| {
                music.linked_locally(&key, resumed);
            },
        );
    }

    pub(super) fn browse_local(&mut self) -> bool {
        match self.local_key().filter(|key| self.is_linked(key)) {
            Some(key) => {
                self.choose(&key);
                true
            }
            None => false,
        }
    }

    fn linked_locally(&mut self, key: &str, resumed: Result<Option<Connection>, String>) {
        let is_pending = self.pending_key.as_deref() == Some(key);
        let reason = match resumed {
            Ok(Some(connection)) => {
                self.link(key, connection);
                if is_pending {
                    self.choose(key);
                }
                return;
            }
            Ok(None) => music_core::Error::Missing(key.to_owned()).to_string(),
            Err(reason) => reason,
        };
        tracing::warn!("failed to link local music: {reason}");
        if is_pending {
            self.pending_key = None;
            self.fall_back(reason);
        }
    }
}

fn revealed(library: &Library, id: TrackId) -> Result<(), String> {
    let track = library
        .track(id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| music_core::Error::Missing(id.to_string()).to_string())?;
    let folder = track.path.parent().unwrap_or(&track.path);
    rikka_open::open(&rikka_open::path_to_uri(folder)).map_err(|error| error.to_string())
}
