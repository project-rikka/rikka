use std::sync::Arc;

use music_core::{
    Album, AlbumPage, AlbumRef, Artist, ArtistPage, Card, Error, Id, Playlist, Result, Track,
};
use serde_json::Value;

use crate::json::Walk;
use crate::parse::{self, cover, credits, release, texts, year};

const ALBUMS: &[&str] = &["Albums"];
const SINGLES: &[&str] = &["Singles", "Singles & EPs", "Singles and EPs"];

#[derive(Debug)]
pub(crate) struct Listed {
    pub(crate) playlist: Playlist,
    pub(crate) tracks: Vec<Track>,
    pub(crate) slots: Vec<Slot>,
    pub(crate) more: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) video: String,
    pub(crate) set: String,
}

fn header(answer: &Value) -> Result<&Value> {
    answer
        .first("musicResponsiveHeaderRenderer")
        .or_else(|| answer.first("musicDetailHeaderRenderer"))
        .ok_or_else(|| Error::Unreadable("the page has no header".into()))
}

fn description(header: &Value) -> String {
    header
        .first("musicDescriptionShelfRenderer")
        .and_then(|shelf| shelf.message(&["description"]))
        .or_else(|| header.message(&["description"]))
        .unwrap_or_default()
}

fn listed_tracks(answer: &Value, from: u32) -> Vec<Track> {
    let mut shelves = answer.every("musicPlaylistShelfRenderer");
    if shelves.is_empty() {
        shelves = answer.every("musicShelfRenderer");
    }
    let mut items: Vec<&Value> = shelves
        .into_iter()
        .flat_map(|shelf| shelf.list(&["contents"]))
        .collect();
    if items.is_empty() {
        items.extend(
            answer
                .every("appendContinuationItemsAction")
                .into_iter()
                .flat_map(|action| action.list(&["continuationItems"])),
        );
        items.extend(
            answer
                .every("musicPlaylistShelfContinuation")
                .into_iter()
                .chain(answer.every("musicShelfContinuation"))
                .flat_map(|shelf| shelf.list(&["contents"])),
        );
    }
    items
        .into_iter()
        .filter_map(|item| {
            item.get("musicResponsiveListItemRenderer")
                .and_then(|_| parse::list_track(item, 0))
        })
        .zip(from..)
        .map(|(track, number)| Track { number, ..track })
        .collect()
}

fn more(answer: &Value) -> Option<String> {
    answer
        .every("musicPlaylistShelfRenderer")
        .into_iter()
        .chain(answer.every("musicShelfRenderer"))
        .chain(answer.every("musicPlaylistShelfContinuation"))
        .chain(answer.every("musicShelfContinuation"))
        .chain(answer.every("appendContinuationItemsAction"))
        .find_map(|tracks| {
            tracks
                .text_at(&["continuations", "0", "nextContinuationData", "continuation"])
                .or_else(|| {
                    tracks
                        .every("continuationItemRenderer")
                        .into_iter()
                        .find_map(|item| {
                            item.text_at(&["continuationEndpoint", "continuationCommand", "token"])
                        })
                })
        })
        .map(str::to_owned)
}

pub(crate) fn more_tracks(answer: &Value, from: u32) -> (Vec<Track>, Option<String>) {
    (listed_tracks(answer, from), more(answer))
}

pub(crate) fn slots(answer: &Value) -> Vec<Slot> {
    answer
        .every("playlistItemData")
        .into_iter()
        .filter_map(|data| {
            Some(Slot {
                video: data.text_at(&["videoId"])?.to_owned(),
                set: data.text_at(&["playlistSetVideoId"])?.to_owned(),
            })
        })
        .collect()
}

pub(crate) fn album(id: &Id, answer: &Value) -> Result<AlbumPage> {
    let header = header(answer)?;
    let subtitle = header.runs(&["subtitle"]);
    let mut artists = credits(header.runs(&["straplineTextOne"]));
    if artists.is_empty() {
        artists = credits(subtitle);
    }
    let album = Album {
        id: id.clone(),
        name: header.message(&["title"]).unwrap_or_default(),
        artists,
        artwork: cover(header),
        release: texts(subtitle).find_map(release).unwrap_or_default(),
        year: texts(subtitle)
            .chain(texts(header.runs(&["secondSubtitle"])))
            .find_map(year),
        fidelity: None,
    };
    let tracks = listed_tracks(answer, 1)
        .into_iter()
        .map(|track| Track {
            album: match track.album.id {
                Some(_) => track.album,
                None => AlbumRef {
                    id: Some(album.id.clone()),
                    name: album.name.clone(),
                },
            },
            artists: match track.artists.is_empty() {
                true => album.artists.clone(),
                false => track.artists,
            },
            artwork: match track.artwork.is_empty() {
                true => album.artwork.clone(),
                false => track.artwork,
            },
            ..track
        })
        .collect();
    Ok(AlbumPage { album, tracks })
}

pub(crate) fn artist(id: &Id, answer: &Value) -> Result<ArtistPage> {
    let header = answer
        .first("musicImmersiveHeaderRenderer")
        .or_else(|| answer.first("musicVisualHeaderRenderer"))
        .or_else(|| answer.first("musicHeaderRenderer"))
        .ok_or_else(|| Error::Unreadable("the artist page has no header".into()))?;
    let popular = answer
        .first("musicShelfRenderer")
        .map(|shelf| {
            shelf
                .list(&["contents"])
                .iter()
                .zip(1..)
                .filter_map(|(item, number)| parse::list_track(item, number))
                .collect::<Arc<[Track]>>()
        })
        .unwrap_or_default();
    let mut albums = Vec::new();
    let mut singles = Vec::new();
    for carousel in answer.every("musicCarouselShelfRenderer") {
        let title = carousel
            .message(&["header", "musicCarouselShelfBasicHeaderRenderer", "title"])
            .unwrap_or_default();
        let into = match title.as_str() {
            title if ALBUMS.contains(&title) => &mut albums,
            title if SINGLES.contains(&title) => &mut singles,
            _ => continue,
        };
        into.extend(
            carousel
                .list(&["contents"])
                .iter()
                .filter_map(parse::card)
                .filter_map(|card| match card {
                    Card::Album(album) => Some(album),
                    _ => None,
                }),
        );
    }
    Ok(ArtistPage {
        artist: Artist {
            id: id.clone(),
            name: header.message(&["title"]).unwrap_or_default(),
            artwork: cover(header),
        },
        popular,
        albums: albums.into(),
        singles: singles.into(),
    })
}

pub(crate) fn playlist(id: &Id, answer: &Value, account: &str) -> Result<Listed> {
    let header = header(answer)?;
    let owned = answer
        .first("musicEditablePlaylistDetailHeaderRenderer")
        .is_some();
    let tracks = listed_tracks(answer, 1);
    let owner = match owned {
        true => account.to_owned(),
        false => header
            .text_at(&["facepile", "avatarStackViewModel", "text", "content"])
            .map(str::to_owned)
            .or_else(|| header.message(&["straplineTextOne"]))
            .unwrap_or_else(|| parse::owner(header.runs(&["subtitle"]))),
    };
    let length = texts(header.runs(&["secondSubtitle"]))
        .find_map(parse::counted)
        .unwrap_or(tracks.len());
    Ok(Listed {
        playlist: Playlist {
            id: id.clone(),
            name: header.message(&["title"]).unwrap_or_default(),
            owner,
            description: description(header),
            artwork: cover(header),
            length,
            editable: owned,
        },
        slots: slots(answer),
        more: more(answer),
        tracks,
    })
}
