use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::fidelity::Fidelity;
use crate::isrc::Isrc;
use crate::service::Service;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(Arc<str>);

impl Id {
    #[must_use]
    pub fn new(id: impl Into<Arc<str>>) -> Id {
        Id(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, formatter)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Picture {
    pub url: String,
    pub width: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Artwork(Vec<Picture>);

impl Artwork {
    #[must_use]
    pub fn new(mut pictures: Vec<Picture>) -> Artwork {
        pictures.sort_by_key(|picture| picture.width);
        pictures.dedup_by(|a, b| a.url == b.url);
        Artwork(pictures)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn pictures(&self) -> &[Picture] {
        &self.0
    }

    #[must_use]
    pub fn fitting(&self, width: u32) -> Option<&str> {
        self.0
            .iter()
            .find(|picture| picture.width >= width)
            .or_else(|| self.0.last())
            .map(|picture| picture.url.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Credit {
    pub id: Option<Id>,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AlbumRef {
    pub id: Option<Id>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Track {
    pub service: Service,
    pub id: Id,
    pub name: String,
    pub artists: Vec<Credit>,
    pub album: AlbumRef,
    pub artwork: Artwork,
    pub duration: Duration,
    pub explicit: bool,
    pub playable: bool,
    pub number: u32,
    pub fidelity: Option<Fidelity>,
    pub isrc: Option<Isrc>,
}

impl Track {
    #[must_use]
    pub fn byline(&self) -> String {
        byline(&self.artists)
    }

    #[must_use]
    pub fn is(&self, other: &Track) -> bool {
        self.service.slug == other.service.slug && self.id == other.id
    }

    #[must_use]
    pub fn uri(&self) -> String {
        format!("{}:{}", self.service.slug, self.id)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Release {
    #[default]
    Album,
    Single,
    Ep,
    Compilation,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Album {
    pub id: Id,
    pub name: String,
    pub artists: Vec<Credit>,
    pub artwork: Artwork,
    pub release: Release,
    pub year: Option<i32>,
    pub fidelity: Option<Fidelity>,
}

impl Album {
    #[must_use]
    pub fn byline(&self) -> String {
        byline(&self.artists)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Artist {
    pub id: Id,
    pub name: String,
    pub artwork: Artwork,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Playlist {
    pub id: Id,
    pub name: String,
    pub owner: String,
    pub description: String,
    pub artwork: Artwork,
    pub length: usize,
    pub editable: bool,
}

fn byline(artists: &[Credit]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
