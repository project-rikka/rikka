use std::time::Duration;

use music_core::{
    Adapter, Album, AlbumRef, Artist, Artwork, Card, Credit, Id, Isrc, Picture, Playlist, Release,
    Shelf, Track,
};
use serde_json::Value;

use crate::provider::SoundCloud;

const SIZES: [(&str, u32); 4] = [
    ("t67x67", 67),
    ("large", 100),
    ("t300x300", 300),
    ("t500x500", 500),
];
const LISTED: u32 = 100;
const WITHHELD: [&str; 2] = ["BLOCK", "SNIP"];

pub(crate) fn id(value: &Value) -> Option<Id> {
    match value {
        Value::Number(number) => number.as_u64().map(|number| Id::new(number.to_string())),
        Value::String(text) if !text.is_empty() => Some(Id::new(text.as_str())),
        _ => None,
    }
}

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

pub(crate) fn collection(value: &Value) -> &[Value] {
    value
        .get("collection")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub(crate) fn artwork(url: Option<&str>) -> Artwork {
    let Some(url) = url else {
        return Artwork::default();
    };
    let pictures = match url.rsplit_once("-large.") {
        Some((stem, extension)) => SIZES
            .iter()
            .map(|(size, width)| Picture {
                url: format!("{stem}-{size}.{extension}"),
                width: *width,
            })
            .collect(),
        None => vec![Picture {
            url: url.to_owned(),
            width: LISTED,
        }],
    };
    Artwork::new(pictures)
}

pub(crate) fn track(value: &Value) -> Option<Track> {
    let id = value.get("id").and_then(id)?;
    let name = text(value, "title")?.to_owned();
    let user = value.get("user");
    let publisher = value.get("publisher_metadata");
    let milliseconds = value
        .get("full_duration")
        .and_then(Value::as_u64)
        .or_else(|| value.get("duration").and_then(Value::as_u64))
        .unwrap_or(0);
    let streamable = value
        .get("streamable")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let withheld = text(value, "policy").is_some_and(|policy| WITHHELD.contains(&policy));
    Some(Track {
        service: SoundCloud::SERVICE,
        id,
        name,
        artists: user.and_then(credit).into_iter().collect(),
        album: AlbumRef {
            id: None,
            name: publisher
                .and_then(|publisher| text(publisher, "album_title"))
                .unwrap_or_default()
                .to_owned(),
        },
        artwork: artwork(text(value, "artwork_url").or_else(|| user.and_then(avatar))),
        duration: Duration::from_millis(milliseconds),
        explicit: publisher
            .and_then(|publisher| publisher.get("explicit"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        playable: streamable && !withheld,
        number: 0,
        fidelity: None,
        isrc: publisher
            .and_then(|publisher| text(publisher, "isrc"))
            .and_then(Isrc::parse),
    })
}

pub(crate) fn in_album(value: &Value, album: &Album, number: u32) -> Option<Track> {
    let mut track = track(value)?;
    track.album = AlbumRef {
        id: Some(album.id.clone()),
        name: album.name.clone(),
    };
    track.number = number;
    Some(track)
}

pub(crate) fn artist(value: &Value) -> Option<Artist> {
    Some(Artist {
        id: value.get("id").and_then(id)?,
        name: text(value, "username")?.to_owned(),
        artwork: artwork(avatar(value)),
    })
}

fn is_album(value: &Value) -> bool {
    value.get("is_album").and_then(Value::as_bool) == Some(true)
}

pub(crate) fn album(value: &Value) -> Option<Album> {
    let user = value.get("user");
    Some(Album {
        id: value.get("id").and_then(id)?,
        name: text(value, "title")?.to_owned(),
        artists: user.and_then(credit).into_iter().collect(),
        artwork: artwork(set_artwork(value)),
        release: match text(value, "set_type") {
            Some("single") => Release::Single,
            Some("ep") => Release::Ep,
            Some("compilation") => Release::Compilation,
            _ => Release::Album,
        },
        year: ["release_date", "published_at", "display_date", "created_at"]
            .iter()
            .find_map(|key| text(value, key))
            .and_then(|date| date.get(..4))
            .and_then(|year| year.parse().ok()),
        fidelity: None,
    })
}

pub(crate) fn playlist(value: &Value, me: &str) -> Option<Playlist> {
    let user = value.get("user");
    let owned = user
        .and_then(|user| user.get("id"))
        .and_then(id)
        .is_some_and(|owner| owner.as_str() == me);
    let listed = value
        .get("tracks")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    Some(Playlist {
        id: value.get("id").and_then(id)?,
        name: text(value, "title")?.to_owned(),
        owner: user
            .and_then(|user| text(user, "username"))
            .unwrap_or_default()
            .to_owned(),
        description: text(value, "description").unwrap_or_default().to_owned(),
        artwork: artwork(set_artwork(value)),
        length: value
            .get("track_count")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(listed),
        editable: owned && text(value, "kind") == Some("playlist"),
    })
}

pub(crate) fn card(value: &Value, me: &str) -> Option<Card> {
    match text(value, "kind")? {
        "track" => track(value).map(Card::Song),
        "user" => artist(value).map(Card::Artist),
        "playlist" if is_album(value) => album(value).map(Card::Album),
        "playlist" | "system-playlist" => playlist(value, me).map(Card::Playlist),
        _ => None,
    }
}

pub(crate) fn selections(answer: &Value, me: &str) -> Vec<Shelf> {
    collection(answer)
        .iter()
        .filter_map(|selection| {
            let title = text(selection, "title")?.to_owned();
            let cards = selection
                .get("items")
                .map(collection)
                .unwrap_or_default()
                .iter()
                .filter_map(|item| card(item, me))
                .collect();
            Some(Shelf::Titled { title, cards })
        })
        .filter(|shelf| !shelf.is_empty())
        .collect()
}

pub(crate) fn liked_set(item: &Value) -> Option<&Value> {
    item.get("playlist")
        .or_else(|| item.get("system_playlist"))
        .filter(|set| set.is_object())
}

fn credit(user: &Value) -> Option<Credit> {
    Some(Credit {
        id: user.get("id").and_then(id),
        name: text(user, "username")?.to_owned(),
    })
}

fn avatar(user: &Value) -> Option<&str> {
    text(user, "avatar_url")
}

fn set_artwork(value: &Value) -> Option<&str> {
    text(value, "artwork_url")
        .or_else(|| text(value, "calculated_artwork_url"))
        .or_else(|| {
            value
                .get("tracks")
                .and_then(Value::as_array)
                .and_then(|tracks| tracks.iter().find_map(|track| text(track, "artwork_url")))
        })
        .or_else(|| value.get("user").and_then(avatar))
}
