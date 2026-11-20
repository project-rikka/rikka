use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use rikka_bus::Properties;
use zbus::zvariant::{ObjectPath, Value};

use crate::track::{Art, Track};

pub(crate) const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
const TRACKS: &str = "/org/rikka/mpris/track";

pub(crate) fn decode(metadata: &Properties) -> Option<Track> {
    let text = |key: &str| metadata.string_ref(key).filter(|text| !text.is_empty());
    let id = text("mpris:trackid").unwrap_or_default().to_owned();
    let title = text("xesam:title").unwrap_or_default();
    let url = text("xesam:url").map(str::to_owned);
    if title.is_empty() && url.is_none() && (id.is_empty() || id == NO_TRACK) {
        return None;
    }
    Some(Track {
        title: match title.is_empty() {
            true => url.as_deref().map(title_of_url).unwrap_or_default(),
            false => title.to_owned(),
        },
        id,
        artists: metadata.strings("xesam:artist").unwrap_or_default(),
        album: text("xesam:album").map(str::to_owned),
        album_artists: metadata.strings("xesam:albumArtist").unwrap_or_default(),
        length: metadata
            .int64("mpris:length")
            .filter(|micros| *micros > 0)
            .map(|micros| Duration::from_micros(micros.unsigned_abs())),
        art: text("mpris:artUrl").and_then(art_of),
        url,
        number: metadata.uint32("xesam:trackNumber"),
    })
}

pub(crate) fn encode(track: Option<&Track>) -> HashMap<String, Value<'static>> {
    let mut metadata = HashMap::new();
    let Some(track) = track else {
        metadata.insert("mpris:trackid".into(), path(NO_TRACK.to_owned()));
        return metadata;
    };
    metadata.insert("mpris:trackid".into(), path(track_path(&track.id)));
    metadata.insert("xesam:title".into(), Value::from(track.title.clone()));
    if !track.artists.is_empty() {
        metadata.insert("xesam:artist".into(), Value::from(track.artists.clone()));
    }
    if let Some(album) = &track.album {
        metadata.insert("xesam:album".into(), Value::from(album.clone()));
    }
    if !track.album_artists.is_empty() {
        metadata.insert(
            "xesam:albumArtist".into(),
            Value::from(track.album_artists.clone()),
        );
    }
    if let Some(length) = track.length {
        metadata.insert(
            "mpris:length".into(),
            Value::from(i64::try_from(length.as_micros()).unwrap_or(i64::MAX)),
        );
    }
    if let Some(art) = &track.art {
        metadata.insert("mpris:artUrl".into(), Value::from(art_url(art)));
    }
    if let Some(url) = &track.url {
        metadata.insert("xesam:url".into(), Value::from(url.clone()));
    }
    if let Some(number) = track.number {
        metadata.insert(
            "xesam:trackNumber".into(),
            Value::from(i32::try_from(number).unwrap_or(i32::MAX)),
        );
    }
    metadata
}

pub(crate) fn track_path(id: &str) -> String {
    let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{TRACKS}/t{hash:016x}")
}

fn path(path: String) -> Value<'static> {
    match ObjectPath::try_from(path) {
        Ok(path) => Value::from(path),
        Err(_) => Value::from(ObjectPath::from_static_str_unchecked(NO_TRACK)),
    }
}

fn art_of(url: &str) -> Option<Art> {
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(Art::Web(url.to_owned()));
    }
    let parsed = url::Url::parse(url).ok()?;
    match parsed.scheme() {
        "file" => parsed.to_file_path().ok().map(Art::File),
        _ => None,
    }
}

fn art_url(art: &Art) -> String {
    match art {
        Art::Web(url) => url.clone(),
        Art::File(path) => url::Url::from_file_path(path)
            .map(String::from)
            .unwrap_or_else(|()| format!("file://{}", path.display())),
    }
}

fn title_of_url(url: &str) -> String {
    let path = url::Url::parse(url)
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .unwrap_or_else(|| PathBuf::from(url));
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}
