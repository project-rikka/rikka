use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use music_core::{Fidelity, Isrc};
use music_tags::ReplayGain;

use crate::ids::{AlbumId, ArtistId, GenreId, TrackId};

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub available: bool,
    pub title: String,
    pub artist: String,
    pub artists: Vec<ArtistRef>,
    pub album: AlbumRef,
    pub album_artist: String,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub disc_subtitle: Option<String>,
    pub year: Option<u16>,
    pub genres: Vec<String>,
    pub composer: Option<String>,
    pub isrc: Option<Isrc>,
    pub musicbrainz_recording: Option<String>,
    pub duration: Duration,
    pub fidelity: Fidelity,
    pub channels: Option<u8>,
    pub gain: ReplayGain,
    pub artwork: Option<Artwork>,
    pub listening: Listening,
    pub added: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArtistRef {
    pub id: ArtistId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AlbumRef {
    pub id: AlbumId,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Listening {
    pub favourite: bool,
    pub plays: u32,
    pub last_played: Option<SystemTime>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub artist: String,
    pub artists: Vec<ArtistRef>,
    pub year: Option<u16>,
    pub compilation: bool,
    pub tracks: u32,
    pub discs: u32,
    pub duration: Duration,
    pub fidelity: Fidelity,
    pub artwork: Option<Artwork>,
    pub added: SystemTime,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub albums: u32,
    pub tracks: u32,
    pub artwork: Option<Artwork>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArtistDetail {
    pub artist: Artist,
    pub albums: Vec<Album>,
    pub appearances: Vec<Album>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Genre {
    pub id: GenreId,
    pub name: String,
    pub albums: u32,
    pub tracks: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FolderKind {
    Music,
    Added,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub path: PathBuf,
    pub kind: FolderKind,
    pub online: bool,
    pub remote: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    pub path: PathBuf,
    pub folders: Vec<Subfolder>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subfolder {
    pub path: PathBuf,
    pub name: String,
    pub tracks: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Artwork {
    pub files: Vec<ArtworkFile>,
}

impl Artwork {
    #[must_use]
    pub fn fitting(&self, width: u32) -> Option<&Path> {
        self.files
            .iter()
            .find(|file| file.width >= width)
            .or_else(|| self.files.last())
            .map(|file| file.path.as_path())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArtworkFile {
    pub path: PathBuf,
    pub width: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub scanning: bool,
    pub found: u64,
    pub read: u64,
    pub to_read: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub songs: u32,
    pub albums: u32,
    pub artists: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Indexed,
    Listening(TrackId),
    Folders,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub added: u64,
    pub updated: u64,
    pub moved: u64,
    pub removed: u64,
    pub unchanged: u64,
}
