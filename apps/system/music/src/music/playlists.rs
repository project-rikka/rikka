use std::sync::Arc;

use music_core::{Id, Playlist, PlaylistPage, Shared, Track};
use rikka_app::prelude::*;
use rikka_ui::{Selection, set_text_of};

use crate::dialogs::{Dialog, Named, Target};
use crate::library::{Load, Loaded};
use crate::music::Music;
use crate::nav::Place;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Start {
    Play,
    Shuffle,
    Queue,
}

impl Music {
    pub(crate) fn new_playlist(&mut self, adding: Arc<[Track]>) {
        let count = match self.library.playlists() {
            Load::Ready(listed) => listed.len(),
            _ => 0,
        };
        let suggested = tr!("music.dialog.suggested", count = &(count + 1).to_string());
        self.dialogs.ask(Dialog::Create {
            target: Target::Playlist,
            adding,
            suggested: suggested.to_string(),
        });
    }

    pub(crate) fn accept_dialog(&mut self) {
        let Some((dialog, name)) = self.dialogs.accept() else {
            return;
        };
        match dialog {
            Dialog::Create {
                target: Target::Playlist,
                adding,
                ..
            } => self.create_playlist(&name, adding),
            Dialog::Create {
                target: Target::Blend,
                adding,
                ..
            } => self.create_blend(&name, &adding),
            Dialog::Rename(named) if named.name() == name => {}
            Dialog::Rename(Named::Playlist(playlist)) => self.library.rename(&playlist.id, &name),
            Dialog::Rename(Named::Blend { id, .. }) => self.rename_blend(&id, &name),
            Dialog::Delete(Named::Playlist(playlist)) => self.delete_playlist(&playlist),
            Dialog::Delete(Named::Blend { id, .. }) => self.delete_blend(&id),
        }
    }

    pub(crate) fn create_playlist(&mut self, name: &str, adding: Arc<[Track]>) {
        let Some(creating) = self.library.creating(name) else {
            return;
        };
        let era = self.library.era();
        spawn(
            creating,
            move |music: &mut Music, made: music_core::Result<Playlist>| {
                music.playlist_made(era, made, &adding);
            },
        );
    }

    pub(crate) fn playlist_made(
        &mut self,
        era: u64,
        made: music_core::Result<Playlist>,
        adding: &[Track],
    ) {
        let Some(playlist) = self.library.made(era, made) else {
            return;
        };
        match adding.is_empty() {
            true => self.go(Place::Playlist(playlist.id)),
            false => self.library.add(&playlist, adding),
        }
    }

    pub(crate) fn delete_playlist(&mut self, playlist: &Playlist) {
        self.nav.forget(&Place::Playlist(playlist.id.clone()));
        self.library.delete(playlist);
        self.arrived();
    }

    pub(crate) fn start_playlist(&mut self, playlist: &Id, start: Start) {
        if let Load::Ready(page) = self.library.page(playlist) {
            self.play_page(&page, start);
            return;
        }
        let Some(loading) = self.library.loading(playlist) else {
            return;
        };
        spawn(loading, move |music: &mut Music, loaded: Loaded| {
            music.started(loaded, start);
        });
    }

    pub(crate) fn started(&mut self, loaded: Loaded, start: Start) {
        match self.library.apply_page(loaded) {
            Some(Ok(page)) => self.play_page(&page, start),
            Some(Err(error)) => self.library.failed(&error),
            None => {}
        }
    }

    pub(crate) fn copy_link(&mut self, shared: Shared<'_>) {
        let Some(catalog) = self.catalog() else {
            return;
        };
        self.copied(&catalog.link(shared));
    }

    pub(crate) fn copy_track_link(&mut self, track: &Track) {
        let Some(connection) = self.player.serving(track.service.slug) else {
            return;
        };
        let catalog = Arc::clone(&connection.catalog);
        let id = track.id.clone();
        spawn_blocking(
            move || catalog.link(Shared::Track(&id)),
            |music: &mut Music, link: String| music.copied(&link),
        );
    }

    fn copied(&mut self, link: &str) {
        set_text_of(Selection::Clipboard, link);
        self.library.show_note(tr!("music.edit.copied").to_string());
    }

    fn play_page(&mut self, page: &PlaylistPage, start: Start) {
        self.player.start(&page.tracks, start);
    }
}
