use std::time::Duration;

use music_core::{
    Adapter, Album, AlbumRef, Artist, Artwork, Card, Credit, Id, Picture, Playlist, Release, Track,
};
use serde_json::Value;

use crate::json::Walk;
use crate::provider::YouTubeMusic;

const EXPLICIT: &str = "MUSIC_EXPLICIT_BADGE";
const GREYED: &str = "MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT";
const ALBUM_PAGE: &str = "MUSIC_PAGE_TYPE_ALBUM";
const ARTIST_PAGE: &str = "MUSIC_PAGE_TYPE_ARTIST";
const PLAYLIST_PAGE: &str = "MUSIC_PAGE_TYPE_PLAYLIST";
const SEPARATORS: &[&str] = &["", " • ", "•", ", ", " & ", " and "];
const KINDS: &[&str] = &["Song", "Video", "Episode", "Podcast", "Playlist", "Artist"];
const SIZES: &[u32] = &[120, 226, 544, 1080];
pub(crate) const LIKED: &str = "LM";
const EPISODES: &str = "SE";

pub(crate) fn cover(renderer: &Value) -> Artwork {
    renderer
        .at(&["thumbnail"])
        .or_else(|| renderer.at(&["thumbnailRenderer"]))
        .map_or_else(Artwork::default, artwork)
}

pub(crate) fn artwork(node: &Value) -> Artwork {
    let Some(list) = node.first("thumbnails").and_then(Value::as_array) else {
        return Artwork::default();
    };
    let mut pictures = Vec::new();
    for thumbnail in list {
        let Some(url) = thumbnail.text_at(&["url"]) else {
            continue;
        };
        let width = thumbnail
            .at(&["width"])
            .and_then(Value::as_u64)
            .and_then(|width| u32::try_from(width).ok())
            .unwrap_or(0);
        let height = thumbnail
            .at(&["height"])
            .and_then(Value::as_u64)
            .and_then(|height| u32::try_from(height).ok())
            .unwrap_or(width);
        pictures.extend(resized(url, width, height));
        pictures.push(Picture {
            url: url.to_owned(),
            width,
        });
    }
    Artwork::new(pictures)
}

fn resized(url: &str, width: u32, height: u32) -> Vec<Picture> {
    let Some((base, settings)) = url.rsplit_once('=') else {
        return Vec::new();
    };
    let sized = settings
        .split('-')
        .any(|setting| setting.strip_prefix(['w', 's']).is_some_and(digits));
    if !sized || !base.starts_with("https://") || width == 0 {
        return Vec::new();
    }
    SIZES
        .iter()
        .filter(|size| **size > width)
        .map(|size| {
            let tall = (u64::from(height) * u64::from(*size) / u64::from(width)) as u32;
            let settings: Vec<String> = settings
                .split('-')
                .map(|setting| match setting.as_bytes().first() {
                    Some(b'w') if digits(&setting[1..]) => format!("w{size}"),
                    Some(b'h') if digits(&setting[1..]) => format!("h{tall}"),
                    Some(b's') if digits(&setting[1..]) => format!("s{size}"),
                    _ => setting.to_owned(),
                })
                .collect();
            Picture {
                url: format!("{base}={}", settings.join("-")),
                width: *size,
            }
        })
        .collect()
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn clock(text: &str) -> Option<Duration> {
    let mut seconds = 0u64;
    for part in text.trim().split(':') {
        seconds = seconds * 60 + part.parse::<u64>().ok()?;
    }
    Some(Duration::from_secs(seconds))
}

pub(crate) fn year(text: &str) -> Option<i32> {
    let year: i32 = text.trim().parse().ok()?;
    (1000..=9999).contains(&year).then_some(year)
}

pub(crate) fn release(text: &str) -> Option<Release> {
    match text.trim() {
        "Album" => Some(Release::Album),
        "Single" => Some(Release::Single),
        "EP" => Some(Release::Ep),
        "Compilation" => Some(Release::Compilation),
        _ => None,
    }
}

fn browse_id(node: &Value) -> Option<&str> {
    node.text_at(&["navigationEndpoint", "browseEndpoint", "browseId"])
}

fn page_type(node: &Value) -> Option<&str> {
    node.text_at(&[
        "navigationEndpoint",
        "browseEndpoint",
        "browseEndpointContextSupportedConfigs",
        "browseEndpointContextMusicConfig",
        "pageType",
    ])
}

fn is_clock(text: &str) -> bool {
    !text.is_empty() && text.contains(':') && text.bytes().all(|b| b.is_ascii_digit() || b == b':')
}

pub(crate) fn credits(runs: &[Value]) -> Vec<Credit> {
    let linked: Vec<Credit> = runs
        .iter()
        .filter_map(|run| {
            let id = browse_id(run).filter(|id| id.starts_with("UC"))?;
            Some(Credit {
                id: Some(Id::new(id)),
                name: run.text_at(&["text"])?.to_owned(),
            })
        })
        .collect();
    if !linked.is_empty() {
        return linked;
    }
    let mut plain = Vec::new();
    for run in runs {
        let text = run.text_at(&["text"]).unwrap_or_default();
        if is_clock(text) || browse_id(run).is_some_and(|id| id.starts_with("MPRE")) {
            break;
        }
        if noise(text) {
            continue;
        }
        plain.push(Credit {
            id: None,
            name: text.to_owned(),
        });
    }
    plain
}

fn album_ref(runs: &[Value]) -> Option<AlbumRef> {
    runs.iter().find_map(|run| {
        let id = browse_id(run).filter(|id| id.starts_with("MPRE"))?;
        Some(AlbumRef {
            id: Some(Id::new(id)),
            name: run.text_at(&["text"]).unwrap_or_default().to_owned(),
        })
    })
}

fn explicit(node: &Value) -> bool {
    node.every("musicInlineBadgeRenderer")
        .iter()
        .any(|badge| badge.text_at(&["icon", "iconType"]) == Some(EXPLICIT))
}

fn columns(renderer: &Value) -> Vec<&Value> {
    renderer
        .list(&["flexColumns"])
        .iter()
        .filter_map(|column| column.at(&["musicResponsiveListItemFlexColumnRenderer"]))
        .collect()
}

fn unwrap<'a>(item: &'a Value, key: &str) -> &'a Value {
    item.at(&[key]).unwrap_or(item)
}

pub(crate) fn list_track(item: &Value, number: u32) -> Option<Track> {
    let renderer = unwrap(item, "musicResponsiveListItemRenderer");
    let columns = columns(renderer);
    let first = columns.first()?;
    let name = first.message(&["text"])?;
    let video = renderer
        .text_at(&["playlistItemData", "videoId"])
        .or_else(|| {
            first.text_at(&[
                "text",
                "runs",
                "0",
                "navigationEndpoint",
                "watchEndpoint",
                "videoId",
            ])
        })
        .or_else(|| {
            renderer.text_at(&[
                "overlay",
                "musicItemThumbnailOverlayRenderer",
                "content",
                "musicPlayButtonRenderer",
                "playNavigationEndpoint",
                "watchEndpoint",
                "videoId",
            ])
        });
    let rest = &columns[1..];
    let mut artists: Vec<Credit> = rest
        .iter()
        .flat_map(|column| credits(column.runs(&["text"])))
        .filter(|credit| credit.id.is_some())
        .collect();
    if artists.is_empty()
        && let Some(second) = rest.first()
    {
        artists = credits(second.runs(&["text"]));
    }
    let album = rest
        .iter()
        .find_map(|column| album_ref(column.runs(&["text"])))
        .unwrap_or_default();
    let duration = renderer
        .list(&["fixedColumns"])
        .iter()
        .find_map(|column| column.message(&["musicResponsiveListItemFixedColumnRenderer", "text"]))
        .or_else(|| {
            rest.iter().find_map(|column| {
                column
                    .runs(&["text"])
                    .iter()
                    .filter_map(|run| run.text_at(&["text"]))
                    .find(|text| is_clock(text))
                    .map(str::to_owned)
            })
        })
        .as_deref()
        .and_then(clock)
        .unwrap_or_default();
    let greyed = renderer.text_at(&["musicItemRendererDisplayPolicy"]) == Some(GREYED);
    Some(Track {
        service: YouTubeMusic::SERVICE,
        id: Id::new(video.unwrap_or_default()),
        name,
        artists,
        album,
        artwork: cover(renderer),
        duration,
        explicit: explicit(renderer),
        playable: video.is_some() && !greyed,
        number,
        fidelity: None,
        isrc: None,
    })
}

pub(crate) fn panel_track(item: &Value, number: u32) -> Option<Track> {
    let renderer = item.at(&["playlistPanelVideoRenderer"]).or_else(|| {
        item.at(&[
            "playlistPanelVideoWrapperRenderer",
            "primaryRenderer",
            "playlistPanelVideoRenderer",
        ])
    })?;
    let video = renderer.text_at(&["videoId"])?;
    let byline = renderer.runs(&["longBylineText"]);
    Some(Track {
        service: YouTubeMusic::SERVICE,
        id: Id::new(video),
        name: renderer.message(&["title"])?,
        artists: credits(byline),
        album: album_ref(byline).unwrap_or_default(),
        artwork: cover(renderer),
        duration: renderer
            .message(&["lengthText"])
            .as_deref()
            .and_then(clock)
            .unwrap_or_default(),
        explicit: explicit(renderer),
        playable: true,
        number,
        fidelity: None,
        isrc: None,
    })
}

pub(crate) fn card(item: &Value) -> Option<Card> {
    two_row(item.at(&["musicTwoRowItemRenderer"])?)
}

pub(crate) fn two_row(renderer: &Value) -> Option<Card> {
    let name = renderer.message(&["title"])?;
    let subtitle = renderer.runs(&["subtitle"]);
    if let Some(video) = renderer.text_at(&["navigationEndpoint", "watchEndpoint", "videoId"]) {
        return Some(Card::Song(Track {
            service: YouTubeMusic::SERVICE,
            id: Id::new(video),
            name,
            artists: credits(subtitle),
            album: album_ref(subtitle).unwrap_or_default(),
            artwork: cover(renderer),
            duration: Duration::ZERO,
            explicit: explicit(renderer),
            playable: true,
            number: 1,
            fidelity: None,
            isrc: None,
        }));
    }
    let id = browse_id(renderer)?;
    if id.starts_with("MPRE") {
        return Some(Card::Album(Album {
            id: Id::new(id),
            name,
            artists: credits(subtitle),
            artwork: cover(renderer),
            release: texts(subtitle).find_map(release).unwrap_or_default(),
            year: texts(subtitle).find_map(year),
            fidelity: None,
        }));
    }
    if id.starts_with("UC") {
        return Some(Card::Artist(Artist {
            id: Id::new(id),
            name,
            artwork: cover(renderer),
        }));
    }
    let playlist = id.strip_prefix("VL")?;
    if playlist == LIKED || playlist == EPISODES {
        return None;
    }
    Some(Card::Playlist(Playlist {
        id: Id::new(playlist),
        name,
        owner: owner(subtitle),
        description: String::new(),
        artwork: cover(renderer),
        length: texts(subtitle).find_map(counted).unwrap_or(0),
        editable: false,
    }))
}

pub(crate) fn list_album(item: &Value) -> Option<Album> {
    let renderer = unwrap(item, "musicResponsiveListItemRenderer");
    if page_type(renderer) != Some(ALBUM_PAGE) {
        return None;
    }
    let columns = columns(renderer);
    let subtitle = columns
        .get(1)
        .map_or(&[][..], |column| column.runs(&["text"]));
    Some(Album {
        id: Id::new(browse_id(renderer)?),
        name: columns.first()?.message(&["text"])?,
        artists: credits(subtitle),
        artwork: cover(renderer),
        release: texts(subtitle).find_map(release).unwrap_or_default(),
        year: texts(subtitle).find_map(year),
        fidelity: None,
    })
}

pub(crate) fn list_artist(item: &Value) -> Option<Artist> {
    let renderer = unwrap(item, "musicResponsiveListItemRenderer");
    if page_type(renderer) != Some(ARTIST_PAGE) {
        return None;
    }
    Some(Artist {
        id: Id::new(browse_id(renderer)?),
        name: columns(renderer).first()?.message(&["text"])?,
        artwork: cover(renderer),
    })
}

pub(crate) fn list_playlist(item: &Value) -> Option<Playlist> {
    let renderer = unwrap(item, "musicResponsiveListItemRenderer");
    if page_type(renderer) != Some(PLAYLIST_PAGE) {
        return None;
    }
    let columns = columns(renderer);
    let subtitle = columns
        .get(1)
        .map_or(&[][..], |column| column.runs(&["text"]));
    Some(Playlist {
        id: Id::new(browse_id(renderer)?.trim_start_matches("VL")),
        name: columns.first()?.message(&["text"])?,
        owner: owner(subtitle),
        description: String::new(),
        artwork: cover(renderer),
        length: texts(subtitle).find_map(counted).unwrap_or(0),
        editable: false,
    })
}

pub(crate) fn texts(runs: &[Value]) -> impl Iterator<Item = &str> {
    runs.iter().filter_map(|run| run.text_at(&["text"]))
}

fn noise(text: &str) -> bool {
    SEPARATORS.contains(&text)
        || KINDS.contains(&text)
        || release(text).is_some()
        || year(text).is_some()
        || counted(text).is_some()
        || text.contains(" views")
        || text.contains(" plays")
}

pub(crate) fn owner(runs: &[Value]) -> String {
    texts(runs)
        .find(|text| !noise(text))
        .unwrap_or_default()
        .to_owned()
}

pub(crate) fn counted(text: &str) -> Option<usize> {
    let mut words = text.split_whitespace();
    let count = words.next()?.replace(',', "").parse().ok()?;
    let unit = words.next()?;
    (unit.starts_with("song") || unit.starts_with("track")).then_some(count)
}
