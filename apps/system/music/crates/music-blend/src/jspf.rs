use std::collections::BTreeMap;
use std::time::Duration;

use music_core::{
    AlbumRef, Artwork, Codec, Credit, Fidelity, Id, Isrc, Picture, Resolution, Service, Track,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::blend::{Blend, BlendId, Entry, EntryKey, Match, Mirror};

const EXTENSION: &str = "urn:rikka:music:blend";
const UUID_URN: &str = "urn:uuid:";

#[derive(Deserialize)]
struct Identified {
    playlist: Identifier,
}

#[derive(Deserialize)]
struct Identifier {
    identifier: String,
}

#[derive(Serialize, Deserialize)]
struct Document {
    playlist: Body,
}

#[derive(Serialize, Deserialize)]
struct Body {
    title: String,
    identifier: String,
    #[serde(default)]
    track: Vec<Song>,
    #[serde(default)]
    extension: BTreeMap<String, Vec<Value>>,
}

#[derive(Serialize, Deserialize)]
struct Song {
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    album: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duration: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(default)]
    identifier: Vec<String>,
    #[serde(default)]
    extension: BTreeMap<String, Vec<Value>>,
}

#[derive(Serialize, Deserialize)]
struct BlendMeta {
    created: u64,
    edited: u64,
    synced: u64,
    #[serde(default)]
    mirrors: Vec<Mirrored>,
}

#[derive(Serialize, Deserialize)]
struct Mirrored {
    service: String,
    account: String,
    playlist: String,
    name: String,
    #[serde(default)]
    tracks: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct TrackMeta {
    key: u64,
    service: String,
    id: String,
    #[serde(default)]
    artists: Vec<Named>,
    #[serde(default)]
    album: Named,
    #[serde(default)]
    artwork: Vec<ImageRef>,
    #[serde(default)]
    explicit: bool,
    #[serde(default = "playable")]
    playable: bool,
    #[serde(default)]
    number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    isrc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fidelity: Option<Sound>,
    #[serde(default)]
    matches: BTreeMap<String, Matched>,
}

#[derive(Default, Serialize, Deserialize)]
struct Named {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct ImageRef {
    url: String,
    width: u32,
}

#[derive(Serialize, Deserialize)]
struct Sound {
    codec: String,
    lossless: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bits: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kbps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dsd: Option<u32>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Matched {
    Found(String),
    Missing(u64),
}

fn playable() -> bool {
    true
}

pub(crate) fn write(blend: &Blend) -> Vec<u8> {
    let meta = BlendMeta {
        created: blend.created,
        edited: blend.edited,
        synced: blend.synced,
        mirrors: blend.mirrors.iter().map(copied).collect(),
    };
    let document = Document {
        playlist: Body {
            title: blend.name.clone(),
            identifier: format!("{UUID_URN}{}", blend.id),
            track: blend.entries.iter().map(song).collect(),
            extension: extended(&meta),
        },
    };
    serde_json::to_vec_pretty(&document).unwrap_or_default()
}

pub(crate) fn identify(bytes: &[u8]) -> Option<BlendId> {
    let document: Identified = serde_json::from_slice(bytes).ok()?;
    document
        .playlist
        .identifier
        .strip_prefix(UUID_URN)
        .and_then(BlendId::parse)
}

pub(crate) fn read(bytes: &[u8], services: &[Service]) -> Result<Blend, String> {
    let document: Document = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let body = document.playlist;
    let id = body
        .identifier
        .strip_prefix(UUID_URN)
        .and_then(BlendId::parse)
        .ok_or_else(|| format!("{} names no blend", body.identifier))?;
    let meta: BlendMeta = ours(&body.extension)?;
    let service_of = |slug: &str| {
        services
            .iter()
            .find(|service| service.slug == slug)
            .copied()
            .ok_or_else(|| format!("no service is called {slug}"))
    };
    let entries = body
        .track
        .into_iter()
        .map(|song| {
            let track_meta: TrackMeta = ours(&song.extension)?;
            entry(song, track_meta, &service_of)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mirrors = meta
        .mirrors
        .into_iter()
        .map(|mirrored| {
            Ok(Mirror {
                service: service_of(&mirrored.service)?,
                account: mirrored.account,
                playlist: Id::new(mirrored.playlist),
                name: mirrored.name,
                tracks: mirrored.tracks.into_iter().map(Id::new).collect(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Blend {
        id,
        name: body.title,
        created: meta.created,
        entries,
        mirrors,
        edited: meta.edited,
        synced: meta.synced,
    })
}

fn ours<T: for<'de> Deserialize<'de>>(
    extension: &BTreeMap<String, Vec<Value>>,
) -> Result<T, String> {
    let value = extension
        .get(EXTENSION)
        .and_then(|values| values.first())
        .ok_or_else(|| format!("the {EXTENSION} extension is missing"))?;
    T::deserialize(value).map_err(|error| error.to_string())
}

fn extended<T: Serialize>(ours: &T) -> BTreeMap<String, Vec<Value>> {
    let value = serde_json::to_value(ours).unwrap_or(Value::Null);
    BTreeMap::from([(EXTENSION.to_owned(), vec![value])])
}

fn copied(mirror: &Mirror) -> Mirrored {
    Mirrored {
        service: mirror.service.slug.to_owned(),
        account: mirror.account.clone(),
        playlist: mirror.playlist.to_string(),
        name: mirror.name.clone(),
        tracks: mirror.tracks.iter().map(Id::to_string).collect(),
    }
}

fn song(entry: &Entry) -> Song {
    let track = &entry.track;
    let track_meta = TrackMeta {
        key: entry.key.0,
        service: track.service.slug.to_owned(),
        id: track.id.to_string(),
        artists: track
            .artists
            .iter()
            .map(|artist| Named {
                id: artist.id.as_ref().map(Id::to_string),
                name: artist.name.clone(),
            })
            .collect(),
        album: Named {
            id: track.album.id.as_ref().map(Id::to_string),
            name: track.album.name.clone(),
        },
        artwork: track
            .artwork
            .pictures()
            .iter()
            .map(|picture| ImageRef {
                url: picture.url.clone(),
                width: picture.width,
            })
            .collect(),
        explicit: track.explicit,
        playable: track.playable,
        number: track.number,
        isrc: track.isrc.as_ref().map(Isrc::to_string),
        fidelity: track.fidelity.map(|fidelity| Sound {
            codec: fidelity.codec.as_str().to_owned(),
            lossless: fidelity.lossless,
            bits: fidelity.resolution.map(|resolution| resolution.bits),
            rate: fidelity.resolution.map(|resolution| resolution.rate),
            kbps: fidelity.kbps,
            dsd: fidelity.dsd,
        }),
        matches: entry
            .matches
            .iter()
            .map(|(service, status)| {
                let matched = match status {
                    Match::Found(id) => Matched::Found(id.to_string()),
                    Match::Missing { checked } => Matched::Missing(*checked),
                };
                ((*service).to_owned(), matched)
            })
            .collect(),
    };
    let mut identifier = vec![track.uri()];
    identifier.extend(
        entry
            .matches
            .iter()
            .filter_map(|(service, status)| match status {
                Match::Found(id) => Some(format!("{service}:{id}")),
                Match::Missing { .. } => None,
            }),
    );
    Song {
        title: track.name.clone(),
        creator: Some(track.byline()).filter(|byline| !byline.is_empty()),
        album: Some(track.album.name.clone()).filter(|album| !album.is_empty()),
        duration: u64::try_from(track.duration.as_millis()).ok(),
        image: track
            .artwork
            .pictures()
            .last()
            .map(|picture| picture.url.clone()),
        identifier,
        extension: extended(&track_meta),
    }
}

fn entry(
    song: Song,
    track_meta: TrackMeta,
    service_of: &impl Fn(&str) -> Result<Service, String>,
) -> Result<Entry, String> {
    let track = Track {
        service: service_of(&track_meta.service)?,
        id: Id::new(track_meta.id),
        name: song.title,
        artists: track_meta
            .artists
            .into_iter()
            .map(|artist| Credit {
                id: artist.id.map(Id::new),
                name: artist.name,
            })
            .collect(),
        album: AlbumRef {
            id: track_meta.album.id.map(Id::new),
            name: track_meta.album.name,
        },
        artwork: Artwork::new(
            track_meta
                .artwork
                .into_iter()
                .map(|picture| Picture {
                    url: picture.url,
                    width: picture.width,
                })
                .collect(),
        ),
        duration: Duration::from_millis(song.duration.unwrap_or(0)),
        explicit: track_meta.explicit,
        playable: track_meta.playable,
        number: track_meta.number,
        fidelity: track_meta.fidelity.map(|sound| Fidelity {
            codec: Codec::from_slug(&sound.codec),
            lossless: sound.lossless,
            resolution: sound
                .bits
                .zip(sound.rate)
                .map(|(bits, rate)| Resolution { bits, rate }),
            kbps: sound.kbps,
            dsd: sound.dsd,
        }),
        isrc: track_meta.isrc.as_deref().and_then(Isrc::parse),
    };
    let matches = track_meta
        .matches
        .into_iter()
        .filter_map(|(slug, status)| {
            let service = service_of(&slug).ok()?;
            let matched = match status {
                Matched::Found(id) => Match::Found(Id::new(id)),
                Matched::Missing(checked) => Match::Missing { checked },
            };
            Some((service.slug, matched))
        })
        .collect();
    Ok(Entry {
        key: EntryKey(track_meta.key),
        track,
        matches,
    })
}
