use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use music_blend::Places;
use music_core::{
    Account, AlbumPage, AlbumRef, ArtistPage, Artwork, Catalog, Connection, Credit, Error, Events,
    Found, Home, Id, Kind, Mark, Playback, PlaybackEvent, Playlist, PlaylistPage, Quality, Result,
    Service, Shared, Track, async_trait,
};
use rikka_worker::Broadcast;

pub(crate) const SERVICES: [Service; 5] = [
    named_service("spotify", Kind::Streaming),
    named_service("qobuz", Kind::Streaming),
    named_service("youtube-music", Kind::Streaming),
    named_service("soundcloud", Kind::Streaming),
    named_service("local", Kind::Local),
];

const fn named_service(slug: &'static str, kind: Kind) -> Service {
    Service {
        slug,
        name: slug,
        mark: Mark::Symbol('\u{e030}'),
        kind,
    }
}

pub(crate) fn service(slug: &str) -> Service {
    SERVICES
        .into_iter()
        .find(|service| service.slug == slug)
        .unwrap_or_else(|| panic!("no test service is called {slug}"))
}

pub(crate) fn track(slug: &str, id: &str) -> Track {
    Track {
        service: service(slug),
        id: Id::new(id),
        name: id.to_uppercase(),
        artists: vec![Credit {
            id: None,
            name: "Singer".into(),
        }],
        album: AlbumRef::default(),
        artwork: Artwork::default(),
        duration: Duration::from_secs(180),
        explicit: false,
        playable: true,
        number: 0,
        fidelity: None,
        isrc: None,
    }
}

pub(crate) trait Rename {
    fn named(self, name: &str) -> Track;
}

impl Rename for Track {
    fn named(mut self, name: &str) -> Track {
        name.clone_into(&mut self.name);
        self
    }
}

pub(crate) fn places(root: &Path) -> Places {
    Places {
        playlists: root.join("Playlists"),
        state: root.join("state"),
    }
}

pub(crate) fn settled<T>(future: impl Future<Output = T>) -> T {
    let mut context = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the fake services never wait"),
    }
}

#[derive(Default)]
struct CatalogState {
    playlists: Vec<(Playlist, Vec<Track>)>,
    stock: Vec<Track>,
    hidden: HashSet<String>,
    offline: bool,
    made: usize,
}

pub(crate) struct Shelf {
    service: Service,
    account: Account,
    state: Mutex<CatalogState>,
}

impl Shelf {
    pub(crate) fn new(slug: &str) -> Arc<Shelf> {
        Arc::new(Shelf {
            service: service(slug),
            account: Account {
                id: "me".into(),
                name: "ME".into(),
            },
            state: Mutex::default(),
        })
    }

    pub(crate) fn service(&self) -> Service {
        self.service
    }

    pub(crate) fn stock(&self, track: Track) {
        self.state().stock.push(track);
    }

    pub(crate) fn hide_in_pages(&self, id: &str) {
        self.state().hidden.insert(id.into());
    }

    pub(crate) fn go_offline(&self) {
        self.state().offline = true;
    }

    pub(crate) fn content_of(&self, playlist: &Id) -> Vec<String> {
        self.state()
            .playlists
            .iter()
            .find(|(listed, _)| &listed.id == playlist)
            .map(|(_, tracks)| tracks.iter().map(|track| track.id.to_string()).collect())
            .unwrap_or_default()
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.state()
            .playlists
            .iter()
            .map(|(playlist, _)| playlist.name.clone())
            .collect()
    }

    pub(crate) fn description_of(&self, playlist: &Id) -> String {
        self.state()
            .playlists
            .iter()
            .find(|(listed, _)| &listed.id == playlist)
            .map(|(listed, _)| listed.description.clone())
            .unwrap_or_default()
    }

    pub(crate) fn create_unmarked_playlist(
        &self,
        name: &str,
        description: &str,
        tracks: Vec<Track>,
    ) -> Id {
        let mut state = self.state();
        state.made += 1;
        let id = Id::new(format!("{}-{}", self.service.slug, state.made));
        state.playlists.push((
            Playlist {
                id: id.clone(),
                name: name.into(),
                owner: self.account.name.clone(),
                description: description.into(),
                artwork: Artwork::default(),
                length: tracks.len(),
                editable: true,
            },
            tracks,
        ));
        id
    }

    pub(crate) fn arrange_by_hand(&self, playlist: &Id, ids: &[&str], extra: Track) {
        self.stock(extra);
        let ids: Vec<Id> = ids.iter().map(|id| Id::new(*id)).collect();
        let mut state = self.state();
        let tracks = ids
            .iter()
            .map(|id| resolved(&state, self.service, id))
            .collect();
        if let Some((_, content)) = state
            .playlists
            .iter_mut()
            .find(|(listed, _)| &listed.id == playlist)
        {
            *content = tracks;
        }
    }

    pub(crate) fn rename_by_hand(&self, playlist: &Id, name: &str) {
        if let Some((state, _)) = self
            .state()
            .playlists
            .iter_mut()
            .find(|(listed, _)| &listed.id == playlist)
        {
            name.clone_into(&mut state.name);
        }
    }

    pub(crate) fn delete_by_hand(&self, playlist: &Id) {
        self.state()
            .playlists
            .retain(|(listed, _)| &listed.id != playlist);
    }

    fn state(&self) -> MutexGuard<'_, CatalogState> {
        self.state.lock()
    }

    fn reachable(&self) -> Result<MutexGuard<'_, CatalogState>> {
        let state = self.state();
        match state.offline {
            true => Err(Error::Unreachable("offline".into())),
            false => Ok(state),
        }
    }

    fn change(
        &self,
        playlist: &Id,
        change: impl FnOnce(&mut Vec<Track>, &CatalogState),
    ) -> Result<()> {
        let mut state = self.reachable()?;
        let Some(at) = state
            .playlists
            .iter()
            .position(|(listed, _)| &listed.id == playlist)
        else {
            return Err(Error::Missing(playlist.to_string()));
        };
        let mut content = std::mem::take(&mut state.playlists[at].1);
        change(&mut content, &state);
        state.playlists[at].1 = content;
        Ok(())
    }
}

fn resolved(state: &CatalogState, service: Service, id: &Id) -> Track {
    state
        .stock
        .iter()
        .find(|track| &track.id == id)
        .cloned()
        .unwrap_or_else(|| track(service.slug, id.as_str()))
}

pub(crate) fn link(shelf: &Arc<Shelf>) -> Connection {
    Connection {
        service: shelf.service,
        account: shelf.account.clone(),
        catalog: Arc::clone(shelf) as Arc<dyn Catalog>,
        playback: Arc::new(Silent::default()),
    }
}

#[derive(Default)]
struct Silent {
    listeners: Broadcast<PlaybackEvent>,
}

impl Playback for Silent {
    fn load(&self, _track: &Id, _at: Duration, _play: bool) {}

    fn prepare(&self, _next: Option<&Id>) {}

    fn play(&self) {}

    fn pause(&self) {}

    fn seek(&self, _at: Duration) {}

    fn set_quality(&self, _quality: Quality) {}

    fn events(&self) -> Events {
        self.listeners.subscribe()
    }
}

#[async_trait]
impl Catalog for Shelf {
    async fn home(&self) -> Result<Home> {
        Ok(Home::default())
    }

    async fn search(&self, _query: &str) -> Result<Found> {
        let state = self.reachable()?;
        Ok(Found {
            tracks: state.stock.iter().cloned().collect(),
            ..Found::default()
        })
    }

    async fn saved_tracks(&self) -> Result<Arc<[Track]>> {
        Ok(Arc::default())
    }

    async fn playlists(&self) -> Result<Arc<[Playlist]>> {
        let state = self.reachable()?;
        Ok(state
            .playlists
            .iter()
            .map(|(playlist, tracks)| Playlist {
                length: tracks.len(),
                ..playlist.clone()
            })
            .collect())
    }

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage> {
        let state = self.reachable()?;
        state
            .playlists
            .iter()
            .find(|(playlist, _)| &playlist.id == id)
            .map(|(playlist, tracks)| PlaylistPage {
                playlist: Playlist {
                    length: tracks.len(),
                    ..playlist.clone()
                },
                tracks: tracks
                    .iter()
                    .filter(|track| !state.hidden.contains(track.id.as_str()))
                    .cloned()
                    .collect(),
            })
            .ok_or_else(|| Error::Missing(id.to_string()))
    }

    async fn album(&self, id: &Id) -> Result<AlbumPage> {
        Err(Error::Missing(id.to_string()))
    }

    async fn artist(&self, id: &Id) -> Result<ArtistPage> {
        Err(Error::Missing(id.to_string()))
    }

    async fn mix(&self, _seed: &Track) -> Result<Arc<[Track]>> {
        Ok(Arc::default())
    }

    async fn set_saved(&self, _track: &Id, _saved: bool) -> Result<()> {
        Ok(())
    }

    async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        drop(self.reachable()?);
        let id = self.create_unmarked_playlist(name, description, Vec::new());
        self.playlist(&id).await.map(|page| page.playlist)
    }

    async fn rename_playlist(&self, playlist: &Id, name: &str) -> Result<()> {
        drop(self.reachable()?);
        self.rename_by_hand(playlist, name);
        Ok(())
    }

    async fn delete_playlist(&self, playlist: &Playlist) -> Result<()> {
        drop(self.reachable()?);
        self.delete_by_hand(&playlist.id);
        Ok(())
    }

    async fn add_to_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let service = self.service;
        self.change(playlist, |content, state| {
            content.extend(tracks.iter().map(|id| resolved(state, service, id)));
        })
    }

    async fn remove_from_playlist(&self, playlist: &Id, track: &Id) -> Result<()> {
        self.change(playlist, |content, _| {
            content.retain(|state| &state.id != track)
        })
    }

    async fn arrange_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let service = self.service;
        self.change(playlist, |content, state| {
            *content = tracks
                .iter()
                .map(|id| resolved(state, service, id))
                .collect();
        })
    }

    fn link(&self, shared: Shared<'_>) -> String {
        match shared {
            Shared::Track(id) | Shared::Playlist(id) => format!("{}:{id}", self.service.slug),
        }
    }
}
