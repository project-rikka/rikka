use std::sync::Arc;

use music_core::{
    AlbumPage, ArtistPage, Card, Catalog, Error, Found, Home, Id, Playlist, PlaylistPage, Release,
    Result, Shared, Shelf, Track, async_trait,
};
use music_library::Library;

use crate::items::{self, album_id, artist_id, track_id};
use crate::local::Local;
use crate::lookup;
use crate::off_thread::{failed, reading};
use crate::radio;

const SHELF: usize = 24;
const FOUND: usize = 50;
const POPULAR: usize = 50;
const PLAYLISTS: &str = "local music keeps its playlists as Rikka playlists";

#[derive(Debug)]
pub(crate) struct LocalCatalog {
    library: Library,
}

impl LocalCatalog {
    pub(crate) fn new(library: Library) -> LocalCatalog {
        LocalCatalog { library }
    }
}

#[async_trait]
impl Catalog for LocalCatalog {
    async fn home(&self) -> Result<Home> {
        reading(&self.library, |library| {
            let added = library.recently_added(SHELF).map_err(failed)?;
            let played = library.most_played(SHELF).map_err(failed)?;
            let favourites = library.favourites().map_err(failed)?;
            let mut shelves = vec![
                Shelf::RecentlyAdded(
                    added
                        .iter()
                        .map(|album| Card::Album(items::album(album)))
                        .collect(),
                ),
                Shelf::MostPlayed(
                    played
                        .iter()
                        .map(|track| Card::Song(items::track(track)))
                        .collect(),
                ),
                Shelf::Favourites(items::tracks(favourites.iter().take(SHELF)).into()),
            ];
            shelves.retain(|shelf| !shelf.is_empty());
            Ok(Home { shelves })
        })
        .await
    }

    async fn search(&self, query: &str) -> Result<Found> {
        let query = query.to_owned();
        reading(&self.library, move |library| {
            let results = library.search(&query, FOUND).map_err(failed)?;
            Ok(Found {
                tracks: items::tracks(&results.tracks).into(),
                albums: results.albums.iter().map(items::album).collect(),
                artists: results.artists.iter().map(items::artist).collect(),
                playlists: Arc::default(),
            })
        })
        .await
    }

    async fn saved_tracks(&self) -> Result<Arc<[Track]>> {
        reading(&self.library, |library| {
            let favourites = library.favourites().map_err(failed)?;
            Ok(items::tracks(&favourites).into())
        })
        .await
    }

    async fn playlists(&self) -> Result<Arc<[Playlist]>> {
        Ok(Arc::default())
    }

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage> {
        Err(Error::Missing(format!("playlist {id}")))
    }

    async fn album(&self, id: &Id) -> Result<AlbumPage> {
        let album = album_id(id)?;
        reading(&self.library, move |library| {
            let detail = library
                .album(album)
                .map_err(failed)?
                .ok_or_else(|| Error::Missing(format!("album {album}")))?;
            Ok(AlbumPage {
                album: items::album(&detail.album),
                tracks: items::tracks(&detail.tracks).into(),
            })
        })
        .await
    }

    async fn artist(&self, id: &Id) -> Result<ArtistPage> {
        let artist = artist_id(id)?;
        reading(&self.library, move |library| {
            let detail = library
                .artist(artist)
                .map_err(failed)?
                .ok_or_else(|| Error::Missing(format!("artist {artist}")))?;
            let mut popular = library.artist_tracks(artist).map_err(failed)?;
            popular.sort_by_key(|track| std::cmp::Reverse(track.listening.plays));
            popular.truncate(POPULAR);
            let (singles, albums): (Vec<_>, Vec<_>) = detail
                .albums
                .iter()
                .partition(|album| matches!(items::release(album), Release::Single | Release::Ep));
            Ok(ArtistPage {
                artist: items::artist(&detail.artist),
                popular: items::tracks(&popular).into(),
                albums: albums
                    .into_iter()
                    .chain(&detail.appearances)
                    .map(items::album)
                    .collect(),
                singles: singles.into_iter().map(items::album).collect(),
            })
        })
        .await
    }

    async fn mix(&self, seed: &Track) -> Result<Arc<[Track]>> {
        let seed_id = track_id(&seed.id)?;
        reading(&self.library, move |library| {
            let mixed = radio::mix(library, seed_id)?;
            Ok(items::tracks(&mixed).into())
        })
        .await
    }

    async fn set_saved(&self, track: &Id, saved: bool) -> Result<()> {
        let local_id = track_id(track)?;
        reading(&self.library, move |library| {
            library.set_favourite(local_id, saved).map_err(failed)
        })
        .await
    }

    async fn create_playlist(&self, _name: &str, _description: &str) -> Result<Playlist> {
        Err(unsupported())
    }

    async fn rename_playlist(&self, _playlist: &Id, _name: &str) -> Result<()> {
        Err(unsupported())
    }

    async fn delete_playlist(&self, _playlist: &Playlist) -> Result<()> {
        Err(unsupported())
    }

    async fn add_to_playlist(&self, _playlist: &Id, _tracks: &[Id]) -> Result<()> {
        Err(unsupported())
    }

    async fn remove_from_playlist(&self, _playlist: &Id, _track: &Id) -> Result<()> {
        Err(unsupported())
    }

    async fn arrange_playlist(&self, _playlist: &Id, _tracks: &[Id]) -> Result<()> {
        Err(unsupported())
    }

    async fn lookup(&self, track: &Track) -> Result<Arc<[Track]>> {
        let target = track.clone();
        reading(&self.library, move |library| {
            let candidates = lookup::candidates(library, &target)?;
            Ok(items::tracks(&candidates).into())
        })
        .await
    }

    fn link(&self, shared: Shared<'_>) -> String {
        let named = |id: &Id| format!("{}:{id}", Local::SERVICE.slug);
        match shared {
            Shared::Track(id) => track_id(id)
                .ok()
                .and_then(|local_id| self.library.track(local_id).ok().flatten())
                .map_or_else(|| named(id), |track| rikka_open::path_to_uri(&track.path)),
            Shared::Playlist(id) => named(id),
        }
    }
}

fn unsupported() -> Error {
    Error::Unsupported(PLAYLISTS.into())
}
