use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use music_core::fold;

use crate::hash::stable_hash;

pub(crate) const VARIOUS_ARTISTS: &str = "Various Artists";
const VARIOUS_NAMES: [&str; 4] = ["various artists", "various", "va", "v.a."];
const DISC_WORDS: [&str; 4] = ["cd", "disc", "disk", "dvd"];

#[derive(Clone, Debug, Default)]
pub(crate) struct Member {
    pub(crate) id: i64,
    pub(crate) folder: PathBuf,
    pub(crate) file_name: String,
    pub(crate) album: Option<String>,
    pub(crate) album_artists: Vec<String>,
    pub(crate) artists: Vec<String>,
    pub(crate) genres: Vec<String>,
    pub(crate) compilation: bool,
    pub(crate) mb_release: Option<String>,
    pub(crate) year: Option<i64>,
    pub(crate) track: Option<i64>,
    pub(crate) disc: Option<i64>,
    pub(crate) disc_total: Option<i64>,
    pub(crate) codec: String,
    pub(crate) lossless: bool,
    pub(crate) bit_depth: Option<i64>,
    pub(crate) sample_rate: Option<i64>,
    pub(crate) duration_ms: i64,
    pub(crate) added_at: i64,
    pub(crate) cover: Option<String>,
    pub(crate) sort_album: Option<String>,
    pub(crate) sort_album_artist: Option<String>,
    pub(crate) sort_artist: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AlbumRow {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) artists: Vec<i64>,
    pub(crate) year: Option<i64>,
    pub(crate) compilation: bool,
    pub(crate) tracks: Vec<i64>,
    pub(crate) discs: i64,
    pub(crate) duration_ms: i64,
    pub(crate) codec: String,
    pub(crate) lossless: bool,
    pub(crate) bit_depth: Option<i64>,
    pub(crate) sample_rate: Option<i64>,
    pub(crate) artwork: Option<String>,
    pub(crate) folders: Vec<PathBuf>,
    pub(crate) added_at: i64,
    pub(crate) sort_title: String,
    pub(crate) sort_artist: String,
    pub(crate) folded: String,
}

#[derive(Clone, Debug)]
pub(crate) struct NameRow {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) sort_name: String,
    pub(crate) folded: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Grouped {
    pub(crate) albums: Vec<AlbumRow>,
    pub(crate) artists: Vec<NameRow>,
    pub(crate) genres: Vec<NameRow>,
    pub(crate) track_artists: Vec<(i64, i64, i64)>,
    pub(crate) track_genres: Vec<(i64, i64)>,
}

pub(crate) fn is_various(name: &str) -> bool {
    VARIOUS_NAMES.contains(&fold(name).as_str())
}

pub(crate) fn group(members: Vec<Member>) -> Grouped {
    let mut grouped = Grouped::default();
    let mut artists: BTreeMap<i64, NameRow> = BTreeMap::new();
    let mut genres: BTreeMap<i64, NameRow> = BTreeMap::new();
    let mut albums: HashMap<String, Vec<Member>> = HashMap::new();
    for member in members {
        for (position, name) in member.artists.iter().enumerate() {
            if let Some(id) = remember(&mut artists, name, sort_of(&member, false)) {
                grouped.track_artists.push((member.id, position as i64, id));
            }
        }
        for name in &member.album_artists {
            remember(&mut artists, name, sort_of(&member, true));
        }
        for name in &member.genres {
            if let Some(id) = remember(&mut genres, name, None)
                && !grouped.track_genres.contains(&(member.id, id))
            {
                grouped.track_genres.push((member.id, id));
            }
        }
        albums.entry(key(&member)).or_default().push(member);
    }
    grouped.albums = albums
        .into_iter()
        .map(|(key, members)| album(&key, members, &mut artists))
        .collect();
    grouped.artists = artists.into_values().collect();
    grouped.genres = genres.into_values().collect();
    grouped
}

fn sort_of(member: &Member, album_artist: bool) -> Option<&str> {
    if album_artist {
        (member.album_artists.len() == 1)
            .then_some(member.sort_album_artist.as_deref())
            .flatten()
    } else {
        (member.artists.len() == 1)
            .then_some(member.sort_artist.as_deref())
            .flatten()
    }
}

fn remember(names: &mut BTreeMap<i64, NameRow>, name: &str, sort: Option<&str>) -> Option<i64> {
    let folded = fold(name);
    if folded.is_empty() || VARIOUS_NAMES.contains(&folded.as_str()) {
        return None;
    }
    let id = stable_hash([folded.as_str()]);
    let row = names.entry(id).or_insert_with(|| NameRow {
        id,
        name: name.to_owned(),
        sort_name: folded.clone(),
        folded,
    });
    if let Some(sort) = sort.map(fold).filter(|sort| !sort.is_empty()) {
        row.sort_name = sort;
    }
    Some(id)
}

fn key(member: &Member) -> String {
    if let Some(release) = &member.mb_release {
        return format!("mb\u{1f}{release}");
    }
    let root = album_root(&member.folder);
    let title = member
        .album
        .as_deref()
        .map(fold)
        .unwrap_or_else(|| format!("\u{1f}{}", root.display()));
    let artist = member.album_artists.join("\u{1f}");
    if member.album_artists.is_empty() || (member.compilation && is_various(&artist)) {
        format!("dir\u{1f}{title}\u{1f}{}", root.display())
    } else {
        format!(
            "aa\u{1f}{}\u{1f}{title}\u{1f}{}",
            fold(&artist),
            root.display()
        )
    }
}

pub(crate) fn album_root(folder: &Path) -> PathBuf {
    let is_disc = folder
        .file_name()
        .map(|name| fold(&name.to_string_lossy()))
        .is_some_and(|name| {
            DISC_WORDS.iter().any(|word| {
                name.strip_prefix(word).is_some_and(|rest| {
                    rest.trim_start_matches([' ', '_', '-', '.', '#'])
                        .starts_with(|character: char| character.is_ascii_digit())
                })
            })
        });
    match folder.parent() {
        Some(parent) if is_disc => parent.to_path_buf(),
        _ => folder.to_path_buf(),
    }
}

fn album(key: &str, mut members: Vec<Member>, artists: &mut BTreeMap<i64, NameRow>) -> AlbumRow {
    members.sort_by(|a, b| {
        (
            a.disc.unwrap_or(1),
            a.track.is_none(),
            a.track,
            &a.file_name,
        )
            .cmp(&(
                b.disc.unwrap_or(1),
                b.track.is_none(),
                b.track,
                &b.file_name,
            ))
    });
    let first = &members[0];
    let root = album_root(&first.folder);
    let title = members
        .iter()
        .find_map(|member| member.album.clone())
        .unwrap_or_else(|| {
            root.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    let named = members
        .iter()
        .find(|member| !member.album_artists.is_empty())
        .map(|member| member.album_artists.clone());
    let leads: Vec<&str> = members
        .iter()
        .filter_map(|member| member.artists.first().map(String::as_str))
        .collect();
    let shared_lead = leads
        .first()
        .filter(|lead| leads.iter().all(|other| fold(other) == fold(lead)))
        .map(|lead| (*lead).to_owned());
    let flagged = members.iter().any(|member| member.compilation);
    let (artist_names, compilation) = match named {
        Some(names) if !names.iter().any(|name| is_various(name)) => (names, flagged),
        Some(_) => (Vec::new(), true),
        None => match shared_lead {
            Some(lead) if !flagged => (vec![lead], false),
            _ => (Vec::new(), true),
        },
    };
    let artist = if artist_names.is_empty() {
        VARIOUS_ARTISTS.to_owned()
    } else {
        artist_names.join(", ")
    };
    let artist_ids = artist_names
        .iter()
        .filter_map(|name| remember(artists, name, None))
        .collect();
    let sort_artist = members
        .iter()
        .find_map(|member| sort_of(member, true).map(fold))
        .unwrap_or_else(|| fold(&artist));
    let sort_title = members
        .iter()
        .find_map(|member| member.sort_album.as_deref().map(fold))
        .unwrap_or_else(|| fold(&title));
    let mut folders: Vec<PathBuf> = vec![root];
    for member in &members {
        if !folders.contains(&member.folder) {
            folders.push(member.folder.clone());
        }
    }
    let tagged_discs = members
        .iter()
        .flat_map(|member| [member.disc, member.disc_total])
        .flatten()
        .max();
    let discs = tagged_discs.unwrap_or_else(|| {
        let mut disc_folders: Vec<&Path> = members
            .iter()
            .map(|member| member.folder.as_path())
            .filter(|folder| album_root(folder) != *folder)
            .collect();
        disc_folders.sort();
        disc_folders.dedup();
        i64::try_from(disc_folders.len()).unwrap_or(i64::MAX)
    });
    let codec = most_common(members.iter().map(|member| member.codec.clone())).unwrap_or_default();
    let same_codec = || members.iter().filter(|member| member.codec == codec);
    AlbumRow {
        id: stable_hash([key]),
        folded: format!("{} | {}", fold(&title), fold(&artist)),
        title,
        artist,
        artists: artist_ids,
        year: most_common(members.iter().filter_map(|member| member.year)),
        compilation,
        tracks: members.iter().map(|member| member.id).collect(),
        discs: discs.max(1),
        duration_ms: members.iter().map(|member| member.duration_ms).sum(),
        bit_depth: same_codec().filter_map(|member| member.bit_depth).max(),
        sample_rate: same_codec().filter_map(|member| member.sample_rate).max(),
        lossless: members.iter().all(|member| member.lossless),
        codec,
        artwork: members.iter().find_map(|member| member.cover.clone()),
        folders,
        added_at: members
            .iter()
            .map(|member| member.added_at)
            .min()
            .unwrap_or_default(),
        sort_title,
        sort_artist,
    }
}

fn most_common<T: Ord + Clone>(values: impl Iterator<Item = T>) -> Option<T> {
    let mut counts: BTreeMap<T, usize> = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    let most = counts.values().copied().max()?;
    counts
        .into_iter()
        .find(|(_, count)| *count == most)
        .map(|(value, _)| value)
}
