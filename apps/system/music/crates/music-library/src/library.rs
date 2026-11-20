use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use music_core::Isrc;
use rikka_worker::{Broadcast, Observable, Subscription};

use crate::artwork::ArtworkCache;
use crate::error::{Error, Result};
use crate::folders;
use crate::ids::{AlbumId, ArtistId, TrackId};
use crate::listening;
use crate::model::{
    Album, AlbumDetail, Artist, ArtistDetail, Change, Counts, Folder, Found, Genre, Listing,
    Progress, Summary, Track,
};
use crate::query::{Albums, Songs};
use crate::read;
use crate::scan::{Scanner, Shared};
use crate::setup::{self, Setup};
use crate::store::Store;
use crate::time;
use crate::watch::Live;

#[derive(Clone, Debug)]
pub struct Library {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    shared: Arc<Shared>,
    scanner: Arc<Scanner>,
    live: Option<Live>,
}

#[derive(Debug)]
#[must_use]
pub struct Scanning {
    answer: Receiver<Result<Summary>>,
}

impl Scanning {
    pub fn wait(self) -> Result<Summary> {
        self.answer.recv().unwrap_or(Err(Error::Stopped))
    }
}

impl Library {
    pub(crate) fn open(setup: Setup) -> Result<Library> {
        let music = setup.music()?;
        let store = Arc::new(Store::open(&setup.database)?);
        let shared = Arc::new(Shared {
            store,
            artwork: ArtworkCache::new(setup.artwork),
            progress: Observable::new(Progress::default()),
            listeners: Broadcast::new(),
            music,
            excluded: setup.excluded,
            epoch: AtomicU64::new(0),
        });
        let scanner = Arc::new(Scanner::start(Arc::clone(&shared))?);
        scanner.scan(None, false, None);
        let live = Some(Live::start(Arc::clone(&shared), Arc::clone(&scanner))?);
        Ok(Library {
            inner: Arc::new(Inner {
                shared,
                scanner,
                live,
            }),
        })
    }

    pub fn folders(&self) -> Result<Vec<Folder>> {
        folders::listed(self.store(), &self.inner.shared.music)
    }

    pub fn add_folder(&self, folder: impl Into<PathBuf>) -> Result<()> {
        let folder = folder.into();
        setup::absolute(&folder)?;
        let folder = std::fs::canonicalize(&folder).unwrap_or(folder);
        let path = folder.clone();
        let now = time::now();
        let added = self
            .store()
            .write(move |transaction| folders::add(transaction, &path, now))?;
        if added {
            self.folders_changed();
            self.inner.scanner.scan(Some(vec![folder]), false, None);
        }
        Ok(())
    }

    pub fn remove_folder(&self, folder: &Path) -> Result<bool> {
        let path = folder.to_path_buf();
        let removed = self
            .store()
            .write(move |transaction| folders::remove(transaction, &path))?;
        if removed {
            self.inner.scanner.forget(folder.to_path_buf(), None);
            self.folders_changed();
        }
        Ok(removed)
    }

    pub fn scan(&self) -> Scanning {
        self.request(false)
    }

    pub fn rescan(&self) -> Scanning {
        self.request(true)
    }

    pub fn cancel_scan(&self) {
        self.inner.scanner.cancel();
    }

    #[must_use]
    pub fn progress(&self) -> Observable<Progress> {
        self.inner.shared.progress.clone()
    }

    #[must_use]
    pub fn subscribe(&self) -> Subscription<Change> {
        self.inner.shared.listeners.subscribe()
    }

    pub fn albums(&self, query: Albums) -> Result<Vec<Album>> {
        self.read(|transaction, artwork| read::albums(transaction, artwork, query))
    }

    pub fn album(&self, id: AlbumId) -> Result<Option<AlbumDetail>> {
        self.read(|transaction, artwork| read::album(transaction, artwork, id))
    }

    pub fn recently_added(&self, limit: usize) -> Result<Vec<Album>> {
        self.read(|transaction, artwork| read::recently_added(transaction, artwork, limit))
    }

    pub fn artists(&self) -> Result<Vec<Artist>> {
        self.read(read::artists)
    }

    pub fn artist(&self, id: ArtistId) -> Result<Option<ArtistDetail>> {
        self.read(|transaction, artwork| read::artist(transaction, artwork, id))
    }

    pub fn songs(&self, songs: Songs) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| read::songs(transaction, artwork, songs))
    }

    pub fn genres(&self) -> Result<Vec<Genre>> {
        self.read(|transaction, _| read::genres(transaction))
    }

    pub fn folder(&self, folder: &Path) -> Result<Listing> {
        self.read(|transaction, artwork| read::listing(transaction, artwork, folder))
    }

    pub fn tracks_under(&self, folder: &Path) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| read::under(transaction, artwork, folder))
    }

    pub fn count_under(&self, folder: &Path) -> Result<u32> {
        self.read(|transaction, _| read::count_under(transaction, folder))
    }

    pub fn counts(&self) -> Result<Counts> {
        self.read(|transaction, _| read::counts(transaction))
    }

    pub fn artist_tracks(&self, id: ArtistId) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| read::by_artist(transaction, artwork, id))
    }

    pub fn tracks_of_albums(&self, albums: &[AlbumId]) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| read::of_albums(transaction, artwork, albums))
    }

    pub fn favourites(&self) -> Result<Vec<Track>> {
        self.read(read::favourites)
    }

    pub fn most_played(&self, limit: usize) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| read::most_played(transaction, artwork, limit))
    }

    pub fn search(&self, text: &str, limit: usize) -> Result<Found> {
        self.read(|transaction, artwork| read::search(transaction, artwork, text, limit))
    }

    pub fn by_isrc(&self, isrc: &Isrc) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| read::by_isrc(transaction, artwork, isrc))
    }

    pub fn matching(
        &self,
        title: &str,
        artist: &str,
        duration: Duration,
        window: Duration,
    ) -> Result<Vec<Track>> {
        self.read(|transaction, artwork| {
            read::matching(transaction, artwork, title, artist, duration, window)
        })
    }

    pub fn track(&self, id: TrackId) -> Result<Option<Track>> {
        self.read(|transaction, artwork| read::track(transaction, artwork, id))
    }

    pub fn set_favourite(&self, id: TrackId, favourite: bool) -> Result<()> {
        let now = time::now();
        self.store()
            .write(move |transaction| listening::favour(transaction, id, favourite, now))?;
        self.inner.shared.listeners.send(&Change::Listening(id));
        Ok(())
    }

    pub fn played(&self, id: TrackId) -> Result<()> {
        let now = time::now();
        self.store()
            .write(move |transaction| listening::played(transaction, id, now))?;
        self.inner.shared.listeners.send(&Change::Listening(id));
        Ok(())
    }

    fn request(&self, force: bool) -> Scanning {
        let (reply, answer) = mpsc::sync_channel(1);
        self.inner.scanner.scan(None, force, Some(reply));
        Scanning { answer }
    }

    fn folders_changed(&self) {
        self.inner.shared.listeners.send(&Change::Folders);
        if let Some(live) = &self.inner.live {
            live.roots_changed();
        }
    }

    fn store(&self) -> &Store {
        &self.inner.shared.store
    }

    fn read<T>(
        &self,
        query: impl FnOnce(&rusqlite::Transaction<'_>, &ArtworkCache) -> Result<T>,
    ) -> Result<T> {
        let artwork = &self.inner.shared.artwork;
        self.store().read(|transaction| query(transaction, artwork))
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.live.take();
        self.scanner.stop();
        self.shared.store.stop();
    }
}
