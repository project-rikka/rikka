use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use music_core::{Catalog, Id, Playlist, PlaylistPage, Track};
use rikka_app::prelude::*;

const NOTE: &str = "music-note";
const NOTED: Duration = Duration::from_secs(4);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Load<T> {
    #[default]
    Loading,
    Ready(T),
    Failed(String),
}

pub(crate) type Page = Load<Arc<PlaylistPage>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    pub(crate) message: String,
    pub(crate) failed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Loaded {
    pub(crate) era: u64,
    pub(crate) id: Id,
    pub(crate) page: music_core::Result<PlaylistPage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum OnSuccess {
    Nothing,
    Notify(String),
    Reload(String),
}

#[derive(State)]
pub(crate) struct Library {
    #[untracked]
    catalog: Option<Arc<dyn Catalog>>,
    #[untracked]
    era: u64,
    songs: Tracked<Load<Arc<[Track]>>>,
    saved: Tracked<Arc<HashSet<Id>>>,
    playlists: Tracked<Load<Arc<[Playlist]>>>,
    pages: Tracked<HashMap<Id, Page>>,
    note: Tracked<Option<Note>>,
}

impl Library {
    pub(crate) fn new() -> Library {
        Library {
            catalog: None,
            era: 0,
            songs: Tracked::default(),
            saved: Tracked::default(),
            playlists: Tracked::default(),
            pages: Tracked::default(),
            note: Tracked::new(None),
        }
    }

    pub(crate) fn attach(&mut self, catalog: Arc<dyn Catalog>) {
        self.era += 1;
        self.catalog = Some(catalog);
        self.refresh();
    }

    pub(crate) fn detach(&mut self) {
        cancel("music-songs");
        cancel("music-playlists");
        cancel(NOTE);
        self.era += 1;
        self.catalog = None;
        self.songs.set(Load::Loading);
        self.playlists.set(Load::Loading);
        self.saved.set(Arc::default());
        self.pages.set(HashMap::new());
        self.note.set(None);
    }

    pub(crate) fn refresh(&mut self) {
        self.refresh_songs();
        self.refresh_playlists();
    }

    fn refresh_songs(&mut self) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        spawn_keyed(
            "music-songs",
            async move { catalog.saved_tracks().await },
            Library::songs_found,
        );
    }

    fn refresh_playlists(&mut self) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        spawn_keyed(
            "music-playlists",
            async move { catalog.playlists().await },
            Library::playlists_found,
        );
    }

    pub(crate) fn songs_found(&mut self, result: music_core::Result<Arc<[Track]>>) {
        match result {
            Ok(tracks) => {
                self.saved.set(Arc::new(
                    tracks.iter().map(|track| track.id.clone()).collect(),
                ));
                self.songs.set(Load::Ready(tracks));
            }
            Err(error) => self.songs.set(Load::Failed(error.to_string())),
        }
    }

    pub(crate) fn playlists_found(&mut self, result: music_core::Result<Arc<[Playlist]>>) {
        self.playlists.set(match result {
            Ok(playlists) => Load::Ready(playlists),
            Err(error) => Load::Failed(error.to_string()),
        });
    }

    pub(crate) fn era(&self) -> u64 {
        self.era
    }

    pub(crate) fn songs(&self) -> &Load<Arc<[Track]>> {
        &self.songs
    }

    pub(crate) fn playlists(&self) -> &Load<Arc<[Playlist]>> {
        &self.playlists
    }

    pub(crate) fn playlist(&self, id: &Id) -> Option<&Playlist> {
        match &*self.playlists {
            Load::Ready(listed) => listed.iter().find(|playlist| &playlist.id == id),
            _ => None,
        }
    }

    pub(crate) fn can_edit(&self, playlist: &Playlist) -> bool {
        playlist.editable
            || self
                .playlist(&playlist.id)
                .is_some_and(|listed| listed.editable)
    }

    pub(crate) fn editable(&self) -> Vec<Playlist> {
        match &*self.playlists {
            Load::Ready(listed) => listed
                .iter()
                .filter(|playlist| playlist.editable)
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn page(&self, id: &Id) -> Page {
        self.pages.get(id).cloned().unwrap_or_default()
    }

    pub(crate) fn note(&self) -> Option<&Note> {
        self.note.as_ref()
    }

    pub(crate) fn is_saved(&self, track: &Id) -> bool {
        self.saved.contains(track)
    }

    pub(crate) fn saved(&self) -> Arc<HashSet<Id>> {
        Arc::clone(&self.saved)
    }

    pub(crate) fn set_saved(&mut self, track: &Track, saved: bool) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        if self.saved.contains(&track.id) == saved {
            return;
        }
        let mut ids = (**self.saved).clone();
        let mut songs = match &*self.songs {
            Load::Ready(songs) => songs.to_vec(),
            _ => Vec::new(),
        };
        match saved {
            true => {
                ids.insert(track.id.clone());
                if !songs.iter().any(|song| song.id == track.id) {
                    songs.insert(0, track.clone());
                }
            }
            false => {
                ids.remove(&track.id);
                songs.retain(|song| song.id != track.id);
            }
        }
        self.saved.set(Arc::new(ids));
        if matches!(*self.songs, Load::Ready(_)) {
            self.songs.set(Load::Ready(songs.into()));
        }
        let (era, id) = (self.era, track.id.clone());
        spawn(
            async move { catalog.set_saved(&id, saved).await },
            move |library: &mut Library, saved: music_core::Result<()>| {
                if era != library.era {
                    return;
                }
                if let Err(error) = saved {
                    library.failed(&error);
                    library.refresh_songs();
                }
            },
        );
    }

    pub(crate) fn loading(
        &self,
        id: &Id,
    ) -> Option<impl Future<Output = Loaded> + Send + 'static + use<>> {
        let catalog = self.catalog.clone()?;
        let (era, id) = (self.era, id.clone());
        Some(async move {
            let page = catalog.playlist(&id).await;
            Loaded { era, id, page }
        })
    }

    pub(crate) fn visit(&mut self, id: &Id) {
        let Some(loading) = self.loading(id) else {
            return;
        };
        if !self.pages.contains_key(id) {
            self.pages.insert(id.clone(), Load::Loading);
        }
        spawn_keyed(page_key(id), loading, Library::page_found);
    }

    pub(crate) fn page_found(&mut self, loaded: Loaded) {
        self.apply_page(loaded);
    }

    pub(crate) fn apply_page(
        &mut self,
        loaded: Loaded,
    ) -> Option<music_core::Result<Arc<PlaylistPage>>> {
        if loaded.era != self.era {
            return None;
        }
        match loaded.page {
            Ok(page) => {
                let page = Arc::new(page);
                self.pages.insert(loaded.id, Load::Ready(Arc::clone(&page)));
                Some(Ok(page))
            }
            Err(error) => {
                if matches!(self.pages.get(&loaded.id), Some(Load::Loading)) {
                    self.pages
                        .insert(loaded.id, Load::Failed(error.to_string()));
                }
                Some(Err(error))
            }
        }
    }

    pub(crate) fn creating(
        &self,
        name: &str,
    ) -> Option<impl Future<Output = music_core::Result<Playlist>> + Send + 'static + use<>> {
        let catalog = self.catalog.clone()?;
        let name = name.to_owned();
        Some(async move { catalog.create_playlist(&name, "").await })
    }

    pub(crate) fn made(
        &mut self,
        era: u64,
        made: music_core::Result<Playlist>,
    ) -> Option<Playlist> {
        if era != self.era {
            return None;
        }
        match made {
            Ok(playlist) => {
                self.listed(|listed| listed.insert(0, playlist.clone()));
                let page = PlaylistPage {
                    playlist: playlist.clone(),
                    tracks: Arc::default(),
                };
                self.pages
                    .insert(playlist.id.clone(), Load::Ready(Arc::new(page)));
                Some(playlist)
            }
            Err(error) => {
                self.failed(&error);
                None
            }
        }
    }

    pub(crate) fn rename(&mut self, playlist: &Id, name: &str) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        self.listed(|listed| {
            listed
                .iter_mut()
                .filter(|listed| &listed.id == playlist)
                .for_each(|listed| listed.name = name.to_owned());
        });
        self.paged(playlist, |page| page.playlist.name = name.to_owned());
        let (id, name) = (playlist.clone(), name.to_owned());
        self.apply_edit(
            playlist,
            async move { catalog.rename_playlist(&id, &name).await },
            OnSuccess::Nothing,
        );
    }

    pub(crate) fn delete(&mut self, playlist: &Playlist) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        self.listed(|listed| listed.retain(|listed| listed.id != playlist.id));
        self.pages.remove(&playlist.id);
        let removed = playlist.clone();
        self.apply_edit(
            &playlist.id,
            async move { catalog.delete_playlist(&removed).await },
            OnSuccess::Nothing,
        );
    }

    pub(crate) fn add(&mut self, playlist: &Playlist, tracks: &[Track]) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        self.paged(&playlist.id, |page| {
            let fresh = tracks
                .iter()
                .filter(|track| !page.tracks.iter().any(|listed| listed.id == track.id))
                .cloned();
            page.tracks = page.tracks.iter().cloned().chain(fresh).collect();
            page.playlist.length = page.tracks.len();
        });
        let message = tr!("music.edit.added", name = playlist.name.as_str()).to_string();
        let id = playlist.id.clone();
        let ids: Vec<Id> = tracks.iter().map(|track| track.id.clone()).collect();
        self.apply_edit(
            &playlist.id,
            async move { catalog.add_to_playlist(&id, &ids).await },
            OnSuccess::Reload(message),
        );
    }

    pub(crate) fn remove(&mut self, playlist: &Id, track: &Id) {
        let Some(catalog) = self.catalog.clone() else {
            return;
        };
        self.paged(playlist, |page| {
            page.tracks = page
                .tracks
                .iter()
                .filter(|listed| &listed.id != track)
                .cloned()
                .collect();
            page.playlist.length = page.tracks.len();
        });
        let name = self.name_of(playlist);
        let message = tr!("music.edit.removed", name = name.as_str()).to_string();
        let (id, track_id) = (playlist.clone(), track.clone());
        self.apply_edit(
            playlist,
            async move { catalog.remove_from_playlist(&id, &track_id).await },
            OnSuccess::Notify(message),
        );
    }

    pub(crate) fn show_note(&mut self, message: String) {
        self.note.set(Some(Note {
            message,
            failed: false,
        }));
        after_keyed(NOTE, NOTED, |library: &mut Library| library.note.set(None));
    }

    pub(crate) fn dismiss(&mut self) {
        cancel(NOTE);
        self.note.set(None);
    }

    pub(crate) fn failed(&mut self, error: &music_core::Error) {
        self.fail(&error.to_string());
    }

    pub(crate) fn fail(&mut self, reason: &str) {
        cancel(NOTE);
        self.note.set(Some(Note {
            message: tr!("music.edit.failed", reason = reason).to_string(),
            failed: true,
        }));
    }

    pub(crate) fn retain_pages(&mut self, keep: impl Fn(&Id) -> bool) {
        if self.pages.keys().all(&keep) {
            return;
        }
        self.pages.retain(|id, _| keep(id));
    }

    fn apply_edit(
        &mut self,
        playlist: &Id,
        work: impl Future<Output = music_core::Result<()>> + Send + 'static,
        on_success: OnSuccess,
    ) {
        cancel(page_key(playlist));
        let (era, id) = (self.era, playlist.clone());
        spawn(
            work,
            move |library: &mut Library, done: music_core::Result<()>| {
                library.edit_finished(era, &id, done, on_success.clone());
            },
        );
    }

    fn edit_finished(
        &mut self,
        era: u64,
        playlist: &Id,
        done: music_core::Result<()>,
        on_success: OnSuccess,
    ) {
        if era != self.era {
            return;
        }
        let reload = match (done, on_success) {
            (Ok(()), OnSuccess::Nothing) => false,
            (Ok(()), OnSuccess::Notify(message)) => {
                self.show_note(message);
                false
            }
            (Ok(()), OnSuccess::Reload(message)) => {
                self.show_note(message);
                true
            }
            (Err(error), _) => {
                self.failed(&error);
                self.refresh_playlists();
                true
            }
        };
        if reload && self.pages.contains_key(playlist) {
            self.visit(playlist);
        }
    }

    fn name_of(&self, playlist: &Id) -> String {
        if let Some(listed) = self.playlist(playlist) {
            return listed.name.clone();
        }
        match self.pages.get(playlist) {
            Some(Load::Ready(page)) => page.playlist.name.clone(),
            _ => String::new(),
        }
    }

    fn listed(&mut self, change: impl FnOnce(&mut Vec<Playlist>)) {
        let Load::Ready(listed) = &*self.playlists else {
            return;
        };
        let mut listed = listed.to_vec();
        change(&mut listed);
        self.playlists.set(Load::Ready(listed.into()));
    }

    fn paged(&mut self, playlist: &Id, change: impl FnOnce(&mut PlaylistPage)) {
        let Some(Load::Ready(page)) = self.pages.get(playlist) else {
            return;
        };
        let mut page = (**page).clone();
        change(&mut page);
        self.pages
            .insert(playlist.clone(), Load::Ready(Arc::new(page)));
    }
}

fn page_key(playlist: &Id) -> (&'static str, Id) {
    ("music-playlist", playlist.clone())
}

impl std::fmt::Debug for Library {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Library")
            .field("attached", &self.catalog.is_some())
            .finish_non_exhaustive()
    }
}
