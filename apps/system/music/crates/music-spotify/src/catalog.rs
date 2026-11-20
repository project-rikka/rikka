use std::collections::HashSet;
use std::sync::Arc;

use music_core::{
    AlbumPage, ArtistPage, Card, Catalog, Error, Found, Home, Id, Isrc, Playlist, PlaylistPage,
    Result, Shared, Shelf, Track, async_trait, lookup_by_isrc,
};
use music_spotify_api::{Album as AlbumMessage, AlbumGroup, Artist as ArtistMessage, SpClient};
use music_spotify_session::Session;

use crate::collection::{self, SAVED};
use crate::failure;
use crate::followed::Followed;
use crate::uri::{self, ALBUM, ARTIST, PLAYLIST, TRACK};
use crate::{editing, metadata, playlists, worker};

const OPEN: &str = "https://open.spotify.com";

const SEARCHED: usize = 40;
const DERIVED: usize = 10;
const SHELF: usize = 12;
const MIXES: usize = 8;
const PICKS: usize = 16;
const DISCOGRAPHY: usize = 40;
const POPULAR: usize = 10;

pub(crate) struct SpotifyCatalog {
    session: Session,
    api: SpClient,
    followed: Followed,
}

impl SpotifyCatalog {
    pub(crate) fn new(session: Session, api: SpClient) -> SpotifyCatalog {
        SpotifyCatalog {
            session,
            api,
            followed: Followed::default(),
        }
    }
}

#[async_trait]
impl Catalog for SpotifyCatalog {
    async fn home(&self) -> Result<Home> {
        let api = self.api.clone();
        worker::run(async move { home(&api).await }).await
    }

    async fn search(&self, query: &str) -> Result<Found> {
        let api = self.api.clone();
        let query = query.trim().to_owned();
        worker::run(async move { search(&api, &query).await }).await
    }

    async fn saved_tracks(&self) -> Result<Arc<[Track]>> {
        let api = self.api.clone();
        worker::run(async move {
            let saved = collection::saved(&api, SAVED, TRACK).await?;
            let uris: Vec<String> = saved.into_iter().map(|item| item.uri).collect();
            metadata::tracks(&api, &uris).await.map(Into::into)
        })
        .await
    }

    async fn playlists(&self) -> Result<Arc<[Playlist]>> {
        let api = self.api.clone();
        worker::run(async move { playlists::owned(&api).await.map(Into::into) }).await
    }

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage> {
        let api = self.api.clone();
        let id = id.clone();
        worker::run(async move { playlists::page(&api, &id).await }).await
    }

    async fn album(&self, id: &Id) -> Result<AlbumPage> {
        let api = self.api.clone();
        let id = id.clone();
        worker::run(async move { album(&api, &id).await }).await
    }

    async fn artist(&self, id: &Id) -> Result<ArtistPage> {
        let api = self.api.clone();
        let country = self.session.account().country;
        let id = id.clone();
        worker::run(async move { artist(&api, country.as_deref(), &id).await }).await
    }

    async fn mix(&self, seed: &Track) -> Result<Arc<[Track]>> {
        let api = self.api.clone();
        let seed = seed.id.clone();
        worker::run(async move { mix(&api, &seed).await.map(Into::into) }).await
    }

    async fn set_saved(&self, track: &Id, saved: bool) -> Result<()> {
        let api = self.api.clone();
        let uri = uri::compose(TRACK, track);
        worker::run(async move { collection::set_saved(&api, SAVED, &uri, saved).await }).await
    }

    async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        let api = self.api.clone();
        let (name, description) = (name.to_owned(), description.to_owned());
        let created =
            worker::run(async move { editing::create(&api, &name, &description).await }).await;
        self.followed.forget().await;
        created
    }

    async fn rename_playlist(&self, playlist: &Id, name: &str) -> Result<()> {
        let api = self.api.clone();
        let (playlist, name) = (playlist.clone(), name.to_owned());
        worker::run(async move { editing::rename(&api, &playlist, &name).await }).await
    }

    async fn delete_playlist(&self, playlist: &Playlist) -> Result<()> {
        let api = self.api.clone();
        let playlist = playlist.id.clone();
        let forgotten = worker::run(async move { editing::forget(&api, &playlist).await }).await;
        self.followed.forget().await;
        forgotten
    }

    async fn add_to_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let api = self.api.clone();
        let (playlist, tracks) = (playlist.clone(), tracks.to_vec());
        worker::run(async move { editing::add(&api, &playlist, &tracks).await }).await
    }

    async fn remove_from_playlist(&self, playlist: &Id, track: &Id) -> Result<()> {
        let api = self.api.clone();
        let (playlist, track) = (playlist.clone(), track.clone());
        worker::run(async move { editing::remove(&api, &playlist, &track).await }).await
    }

    async fn arrange_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let api = self.api.clone();
        let (playlist, tracks) = (playlist.clone(), tracks.to_vec());
        worker::run(async move { editing::arrange(&api, &playlist, &tracks).await }).await
    }

    async fn keeps_playlist(&self, playlist: &Id) -> Result<bool> {
        self.followed.contains(&self.api, playlist).await
    }

    async fn lookup(&self, track: &Track) -> Result<Arc<[Track]>> {
        let api = self.api.clone();
        let query = track.clone();
        worker::run(async move { lookup(&api, &query).await }).await
    }

    fn link(&self, shared: Shared<'_>) -> String {
        match shared {
            Shared::Track(id) => format!("{OPEN}/track/{id}"),
            Shared::Playlist(id) => format!("{OPEN}/playlist/{id}"),
        }
    }
}

async fn home(api: &SpClient) -> Result<Home> {
    let saved = collection::saved(api, SAVED, "spotify:").await?;
    let track_uris: Vec<String> = saved
        .iter()
        .filter(|item| item.uri.starts_with(TRACK))
        .take(SHELF * 4)
        .map(|item| item.uri.clone())
        .collect();
    let album_uris: Vec<String> = saved
        .iter()
        .filter(|item| item.uri.starts_with(ALBUM))
        .take(SHELF)
        .map(|item| item.uri.clone())
        .collect();

    let (recent, albums, owned) = tokio::join!(
        metadata::tracks(api, &track_uris),
        metadata::albums(api, &album_uris),
        playlists::owned(api),
    );
    let recent = recent?;

    let artist_uris = metadata::uris_of(
        ARTIST,
        distinct(
            recent
                .iter()
                .flat_map(|track| &track.artists)
                .filter_map(|artist| artist.id.clone()),
        )
        .take(SHELF),
    );
    let seeded = async {
        match recent.first() {
            Some(seed) => mix(api, &seed.id).await.unwrap_or_default(),
            None => Vec::new(),
        }
    };
    let (picks, artists) = tokio::join!(seeded, metadata::artists(api, &artist_uris));

    let liked: HashSet<&Id> = recent.iter().map(|track| &track.id).collect();
    let mut picks: Vec<Track> = picks
        .into_iter()
        .filter(|track| !liked.contains(&track.id))
        .take(PICKS)
        .collect();
    if picks.is_empty() {
        picks = recent.iter().take(PICKS).cloned().collect();
    }

    let mut voices = HashSet::new();
    let mixes = recent
        .iter()
        .filter(|track| {
            track
                .artists
                .first()
                .is_some_and(|artist| voices.insert(artist.name.clone()))
        })
        .take(MIXES)
        .cloned()
        .map(Card::Mix)
        .collect();

    let shelves = vec![
        Shelf::QuickPicks(picks.into()),
        Shelf::MixesFor(mixes),
        Shelf::RecentAlbums(
            albums
                .unwrap_or_default()
                .into_iter()
                .map(Card::Album)
                .collect(),
        ),
        Shelf::YourPlaylists(
            owned
                .unwrap_or_default()
                .into_iter()
                .take(SHELF)
                .map(Card::Playlist)
                .collect(),
        ),
        Shelf::Artists(
            artists
                .unwrap_or_default()
                .into_iter()
                .map(Card::Artist)
                .collect(),
        ),
    ];
    Ok(Home {
        shelves: shelves
            .into_iter()
            .filter(|shelf| !shelf.is_empty())
            .collect(),
    })
}

async fn lookup(api: &SpClient, target: &Track) -> Result<Arc<[Track]>> {
    let asking = |isrc: &Isrc| format!("isrc:{isrc}");
    lookup_by_isrc(
        target,
        asking,
        |query| async move { search(api, &query).await },
    )
    .await
}

async fn search(api: &SpClient, query: &str) -> Result<Found> {
    if query.is_empty() {
        return Ok(Found::default());
    }
    let context = api.search(query).await.map_err(failure::api)?;
    let uris: Vec<String> = context
        .pages
        .into_iter()
        .flat_map(|page| page.tracks)
        .map(|track| track.uri)
        .filter(|uri| uri.starts_with(TRACK))
        .take(SEARCHED)
        .collect();
    let tracks = metadata::tracks(api, &uris).await?;

    let album_uris = metadata::uris_of(
        ALBUM,
        distinct(tracks.iter().filter_map(|track| track.album.id.clone())).take(DERIVED),
    );
    let artist_uris = metadata::uris_of(
        ARTIST,
        distinct(
            tracks
                .iter()
                .flat_map(|track| &track.artists)
                .filter_map(|artist| artist.id.clone()),
        )
        .take(DERIVED),
    );
    let (albums, artists) = tokio::join!(
        metadata::albums(api, &album_uris),
        metadata::artists(api, &artist_uris),
    );
    Ok(Found {
        tracks: tracks.into(),
        albums: albums?.into(),
        artists: artists?.into(),
        playlists: Arc::default(),
    })
}

async fn album(api: &SpClient, id: &Id) -> Result<AlbumPage> {
    let album_uri = uri::compose(ALBUM, id);
    let message = metadata::message::<AlbumMessage>(api, &album_uri)
        .await?
        .ok_or_else(|| Error::Missing(album_uri.clone()))?;
    let album =
        metadata::album(&album_uri, &message).ok_or_else(|| Error::Missing(album_uri.clone()))?;
    let tracks = metadata::tracks(api, &metadata::album_track_uris(&message))
        .await?
        .into_iter()
        .map(|mut track| {
            if track.artwork.is_empty() {
                track.artwork = album.artwork.clone();
            }
            track
        })
        .collect::<Vec<_>>()
        .into();
    Ok(AlbumPage { album, tracks })
}

async fn artist(api: &SpClient, country: Option<&str>, id: &Id) -> Result<ArtistPage> {
    let artist_uri = uri::compose(ARTIST, id);
    let message = metadata::message::<ArtistMessage>(api, &artist_uri)
        .await?
        .ok_or_else(|| Error::Missing(artist_uri.clone()))?;
    let artist = metadata::artist(&artist_uri, &message)
        .ok_or_else(|| Error::Missing(artist_uri.clone()))?;

    let popular_uris = popular(&message, country);
    let album_uris = firsts(&message.album_group);
    let single_uris = firsts(&message.single_group);
    let (popular, albums, singles) = tokio::join!(
        metadata::tracks(api, &popular_uris),
        metadata::albums(api, &album_uris),
        metadata::albums(api, &single_uris),
    );
    Ok(ArtistPage {
        artist,
        popular: popular?.into(),
        albums: newest_first(albums?).into(),
        singles: newest_first(singles?).into(),
    })
}

async fn mix(api: &SpClient, seed: &Id) -> Result<Vec<Track>> {
    let media = api
        .radio(uri::spotify_id(seed)?)
        .await
        .map_err(failure::api)?;
    let playlist = media
        .iter()
        .find_map(|media| uri::id_in(media, PLAYLIST))
        .ok_or_else(|| Error::Missing(format!("a mix for {seed}")))?;
    playlists::tracks(api, &playlist).await
}

fn popular(message: &ArtistMessage, country: Option<&str>) -> Vec<String> {
    let top = message
        .top_track
        .iter()
        .find(|top| Some(top.country.as_str()) == country)
        .or_else(|| message.top_track.first());
    top.map(|top| {
        top.track
            .iter()
            .filter_map(|track| uri::gid_uri(TRACK, &track.gid))
            .take(POPULAR)
            .collect()
    })
    .unwrap_or_default()
}

fn firsts(groups: &[AlbumGroup]) -> Vec<String> {
    groups
        .iter()
        .filter_map(|group| group.album.first())
        .filter_map(|album| uri::gid_uri(ALBUM, &album.gid))
        .take(DISCOGRAPHY)
        .collect()
}

fn newest_first(mut albums: Vec<music_core::Album>) -> Vec<music_core::Album> {
    albums.sort_by_key(|album| std::cmp::Reverse(album.year));
    albums
}

fn distinct(ids: impl Iterator<Item = Id>) -> impl Iterator<Item = Id> {
    let mut seen = HashSet::new();
    ids.filter(move |id| seen.insert(id.clone()))
}
