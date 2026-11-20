use std::sync::Arc;

use crate::error::Result;
use crate::item::{Album, Artist, Playlist, Track};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumPage {
    pub album: Album,
    pub tracks: Arc<[Track]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtistPage {
    pub artist: Artist,
    pub popular: Arc<[Track]>,
    pub albums: Arc<[Album]>,
    pub singles: Arc<[Album]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistPage {
    pub playlist: Playlist,
    pub tracks: Arc<[Track]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Card {
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
    Mix(Track),
    Song(Track),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shelf {
    QuickPicks(Arc<[Track]>),
    RecentlyPlayed(Arc<[Track]>),
    Following(Arc<[Track]>),
    Favourites(Arc<[Track]>),
    MixesFor(Arc<[Card]>),
    RecentAlbums(Arc<[Card]>),
    RecentlyAdded(Arc<[Card]>),
    YourPlaylists(Arc<[Card]>),
    Artists(Arc<[Card]>),
    NewReleases(Arc<[Card]>),
    Acclaimed(Arc<[Card]>),
    MostPlayed(Arc<[Card]>),
    Curated(Arc<[Card]>),
    Titled { title: String, cards: Arc<[Card]> },
    Songs { title: String, tracks: Arc<[Track]> },
}

impl Shelf {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Shelf::QuickPicks(tracks)
            | Shelf::RecentlyPlayed(tracks)
            | Shelf::Following(tracks)
            | Shelf::Favourites(tracks)
            | Shelf::Songs { tracks, .. } => tracks.is_empty(),
            Shelf::MixesFor(cards)
            | Shelf::RecentAlbums(cards)
            | Shelf::RecentlyAdded(cards)
            | Shelf::YourPlaylists(cards)
            | Shelf::Artists(cards)
            | Shelf::NewReleases(cards)
            | Shelf::Acclaimed(cards)
            | Shelf::MostPlayed(cards)
            | Shelf::Curated(cards)
            | Shelf::Titled { cards, .. } => cards.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Home {
    pub shelves: Vec<Shelf>,
}

impl Home {
    pub fn from_outcomes(outcomes: impl IntoIterator<Item = Result<Vec<Shelf>>>) -> Result<Home> {
        let mut shelves = Vec::new();
        let mut failure = None;
        for outcome in outcomes {
            match outcome {
                Ok(loaded) => shelves.extend(loaded),
                Err(error) => failure = Some(error),
            }
        }
        shelves.retain(|shelf| !shelf.is_empty());
        match (shelves.is_empty(), failure) {
            (true, Some(error)) => Err(error),
            _ => Ok(Home { shelves }),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub tracks: Arc<[Track]>,
    pub albums: Arc<[Album]>,
    pub artists: Arc<[Artist]>,
    pub playlists: Arc<[Playlist]>,
}

impl Found {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
            && self.albums.is_empty()
            && self.artists.is_empty()
            && self.playlists.is_empty()
    }
}
