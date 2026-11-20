use std::time::Duration;

use music_core::{
    Adapter, Album, AlbumRef, Artist, Artwork, Codec, Credit, Fidelity, Id, Isrc, Picture,
    Playlist, Release, Resolution, Track,
};
use serde_json::Value;

use crate::provider::Qobuz;

const COVERS: [(&str, u32); 5] = [
    ("thumbnail", 50),
    ("small", 230),
    ("large", 600),
    ("extralarge", 1200),
    ("mega", 1400),
];
const PORTRAITS: [(&str, u32); 5] = [
    ("small", 100),
    ("medium", 300),
    ("large", 600),
    ("extralarge", 1200),
    ("mega", 1400),
];
const PORTRAIT_HOME: &str = "https://static.qobuz.com/images/artists/covers";
const MASTER: u32 = 1400;
const HERTZ_PER_KILOHERTZ: f64 = 1000.0;
const MAIN_ARTIST: &str = "main-artist";

pub(crate) fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(|list| list.get("items").or(Some(list)))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub(crate) fn id(value: &Value) -> Option<Id> {
    match value {
        Value::Number(number) => Some(Id::new(number.to_string())),
        Value::String(text) if !text.is_empty() => Some(Id::new(text.as_str())),
        _ => None,
    }
}

pub(crate) fn track(value: &Value, on: Option<&Album>) -> Option<Track> {
    let album = value.get("album");
    let artists: Vec<Credit> = value
        .get("performer")
        .or_else(|| value.get("artist"))
        .and_then(credit)
        .into_iter()
        .collect();
    let artists = match (artists.is_empty(), on) {
        (true, Some(on)) => on.artists.clone(),
        (true, None) => album.map(credits).unwrap_or_default(),
        (false, _) => artists,
    };
    let artwork = album
        .map(|album| pictures(album.get("image"), &COVERS))
        .filter(|artwork| !artwork.is_empty())
        .or_else(|| on.map(|on| on.artwork.clone()))
        .unwrap_or_default();
    let album_ref = match (album, on) {
        (Some(album), _) => AlbumRef {
            id: album.get("id").and_then(id),
            name: title(album).unwrap_or_default(),
        },
        (None, Some(on)) => AlbumRef {
            id: Some(on.id.clone()),
            name: on.name.clone(),
        },
        (None, None) => AlbumRef::default(),
    };
    Some(Track {
        service: Qobuz::SERVICE,
        id: id(value.get("id")?)?,
        name: title(value)?,
        artists,
        album: album_ref,
        artwork,
        duration: Duration::from_secs(whole(value, "duration").unwrap_or(0)),
        explicit: flag(value, "parental_warning"),
        playable: streamable(value),
        number: whole(value, "track_number")
            .or_else(|| {
                value
                    .get("physical_support")
                    .and_then(|at| whole(at, "track_number"))
            })
            .and_then(|number| u32::try_from(number).ok())
            .unwrap_or(0),
        fidelity: fidelity(value).or_else(|| on.and_then(|on| on.fidelity)),
        isrc: value
            .get("isrc")
            .and_then(Value::as_str)
            .and_then(Isrc::parse),
    })
}

pub(crate) fn album(value: &Value) -> Option<Album> {
    Some(Album {
        id: id(value.get("id")?)?,
        name: title(value)?,
        artists: credits(value),
        artwork: pictures(value.get("image"), &COVERS),
        release: release(value),
        year: year(value),
        fidelity: fidelity(value),
    })
}

pub(crate) fn artist(value: &Value) -> Option<Artist> {
    Some(Artist {
        id: id(value.get("id")?)?,
        name: name(value)?,
        artwork: portrait(value),
    })
}

pub(crate) fn playlist(value: &Value, me: u64) -> Option<Playlist> {
    let owner = value.get("owner");
    Some(Playlist {
        id: id(value.get("id")?)?,
        name: text(value, "name")
            .or_else(|| text(value, "title"))?
            .to_owned(),
        owner: owner
            .and_then(|owner| text(owner, "name"))
            .unwrap_or_default()
            .to_owned(),
        description: text(value, "description").unwrap_or_default().to_owned(),
        artwork: playlist_artwork(value),
        length: whole(value, "tracks_count")
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(0),
        editable: owner
            .and_then(|owner| owner.get("id"))
            .and_then(Value::as_u64)
            == Some(me),
    })
}

pub(crate) fn fidelity(value: &Value) -> Option<Fidelity> {
    let info = value.get("audio_info");
    let bits = whole(value, "maximum_bit_depth")
        .or_else(|| info.and_then(|info| whole(info, "maximum_bit_depth")))?;
    let rate = value
        .get("maximum_sampling_rate")
        .or_else(|| info.and_then(|info| info.get("maximum_sampling_rate")))
        .and_then(Value::as_f64)?;
    let hertz = match rate < HERTZ_PER_KILOHERTZ {
        true => rate * HERTZ_PER_KILOHERTZ,
        false => rate,
    };
    let resolution = Resolution {
        bits: u32::try_from(bits).ok()?,
        rate: hertz.round() as u32,
    };
    let high_streams = value
        .get("hires_streamable")
        .or_else(|| {
            value
                .get("rights")
                .and_then(|rights| rights.get("hires_streamable"))
        })
        .and_then(Value::as_bool);
    Some(Fidelity::lossless(
        Codec::Flac,
        match high_streams {
            Some(false) if resolution.is_high() => Resolution::CD,
            _ => resolution,
        },
    ))
}

fn credits(value: &Value) -> Vec<Credit> {
    if let Some(main) = value.get("artist").and_then(credit) {
        return vec![main];
    }
    let listed = value
        .get("artists")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let main: Vec<Credit> = listed
        .iter()
        .filter(|artist| {
            artist
                .get("roles")
                .and_then(Value::as_array)
                .is_some_and(|roles| roles.iter().any(|role| role == MAIN_ARTIST))
        })
        .filter_map(credit)
        .collect();
    match main.is_empty() {
        true => listed.iter().filter_map(credit).collect(),
        false => main,
    }
}

fn credit(value: &Value) -> Option<Credit> {
    Some(Credit {
        id: value.get("id").and_then(id),
        name: name(value)?,
    })
}

fn name(value: &Value) -> Option<String> {
    let name = value.get("name")?;
    name.as_str()
        .or_else(|| name.get("display").and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn title(value: &Value) -> Option<String> {
    let title = text(value, "title")?.trim();
    Some(match text(value, "version").map(str::trim) {
        Some(version) if !version.is_empty() && !title.contains(version) => {
            format!("{title} ({version})")
        }
        _ => title.to_owned(),
    })
}

fn release(value: &Value) -> Release {
    let kind = text(value, "release_type").or_else(|| text(value, "product_type"));
    match kind {
        Some("single") => Release::Single,
        Some("ep" | "epSingle") => Release::Ep,
        Some("compilation") => Release::Compilation,
        _ => Release::Album,
    }
}

fn year(value: &Value) -> Option<i32> {
    text(value, "release_date_original")
        .or_else(|| value.get("dates").and_then(|dates| text(dates, "original")))
        .and_then(|date| date.get(..4))
        .and_then(|year| year.parse().ok())
}

fn streamable(value: &Value) -> bool {
    let message = value
        .get("streamable")
        .or_else(|| {
            value
                .get("rights")
                .and_then(|rights| rights.get("streamable"))
        })
        .and_then(Value::as_bool);
    message != Some(false)
}

fn portrait(value: &Value) -> Artwork {
    if let Some(portrait) = value
        .get("images")
        .and_then(|images| images.get("portrait"))
    {
        let (Some(hash), Some(format)) = (text(portrait, "hash"), text(portrait, "format")) else {
            return Artwork::default();
        };
        return Artwork::new(
            PORTRAITS[..3]
                .iter()
                .map(|(size, width)| Picture {
                    url: format!("{PORTRAIT_HOME}/{size}/{hash}.{format}"),
                    width: *width,
                })
                .collect(),
        );
    }
    pictures(
        value.get("image").or_else(|| value.get("picture")),
        &PORTRAITS,
    )
}

fn pictures(image: Option<&Value>, sizes: &[(&str, u32)]) -> Artwork {
    let Some(image) = image else {
        return Artwork::default();
    };
    if let Some(url) = image.as_str().filter(|url| !url.is_empty()) {
        return Artwork::new(vec![Picture {
            url: url.to_owned(),
            width: width_of(url).unwrap_or(MASTER),
        }]);
    }
    Artwork::new(
        sizes
            .iter()
            .filter_map(|(key, width)| {
                let url = text(image, key)?;
                Some(Picture {
                    url: url.to_owned(),
                    width: width_of(url).unwrap_or(*width),
                })
            })
            .collect(),
    )
}

fn playlist_artwork(value: &Value) -> Artwork {
    let first = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .and_then(|list| list.first())
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
    };
    let mut pictures: Vec<Picture> = [("images150", 150), ("images300", 300), ("images", 50)]
        .iter()
        .filter_map(|(key, width)| {
            first(key).map(|url| Picture {
                url: url.to_owned(),
                width: width_of(url).unwrap_or(*width),
            })
        })
        .collect();
    if pictures.is_empty() {
        let image = value.get("image");
        let covers = image
            .and_then(|image| image.get("covers"))
            .and_then(Value::as_array)
            .and_then(|covers| covers.first())
            .and_then(Value::as_str);
        let rectangle =
            first("image_rectangle").or_else(|| image.and_then(|image| text(image, "rectangle")));
        pictures.extend(covers.or(rectangle).map(|url| Picture {
            url: url.to_owned(),
            width: width_of(url).unwrap_or(MASTER),
        }));
    }
    Artwork::new(pictures)
}

fn width_of(url: &str) -> Option<u32> {
    let file = url.rsplit('/').next()?;
    let stem = file.split('.').next()?;
    let (_, size) = stem.rsplit_once('_')?;
    match size {
        "max" | "org" => Some(MASTER),
        size => size.parse().ok(),
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

fn whole(value: &Value, key: &str) -> Option<u64> {
    let field = value.get(key)?;
    field
        .as_u64()
        .or_else(|| field.as_f64().map(|number| number.max(0.0).round() as u64))
        .or_else(|| field.as_str().and_then(|text| text.parse().ok()))
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}
