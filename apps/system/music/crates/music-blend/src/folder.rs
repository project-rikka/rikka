use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use music_core::Service;

use crate::blend::{Blend, BlendId};
use crate::jspf;
use crate::ledger::{Placed, Placements};

const EXTENSION: &str = "jspf";
const LONGEST_STEM: usize = 120;
const UNNAMED: &str = "Playlist";

pub(crate) struct FoundBlend {
    pub(crate) blend: Blend,
    pub(crate) placed: Placed,
}

pub(crate) struct Listing {
    pub(crate) blends: Vec<FoundBlend>,
    pub(crate) present: HashSet<BlendId>,
    pub(crate) whole: bool,
}

pub(crate) fn list(folder: &Path, services: &[Service], known: &Placements) -> Option<Listing> {
    let listed = match fs::read_dir(folder) {
        Ok(listed) => listed,
        Err(error) => {
            if error.kind() != ErrorKind::NotFound {
                tracing::warn!("failed to list playlists in {}: {error}", folder.display());
            }
            return None;
        }
    };
    let settled: HashMap<&Placed, &BlendId> =
        known.iter().map(|(id, placed)| (placed, id)).collect();
    let mut unchanged = HashSet::new();
    let mut found_by_id: HashMap<BlendId, FoundBlend> = HashMap::new();
    let mut present = HashSet::new();
    let mut whole = true;
    for path in listed
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| is_playlist(path))
    {
        let Ok(bytes) = fs::read(&path) else {
            whole = false;
            continue;
        };
        let placed = Placed::new(path, &bytes);
        if let Some(id) = settled.get(&placed) {
            present.insert((*id).clone());
            unchanged.insert((*id).clone());
            continue;
        }
        let Some(id) = jspf::identify(&bytes) else {
            whole = false;
            continue;
        };
        present.insert(id);
        let blend = match jspf::read(&bytes, services) {
            Ok(blend) => blend,
            Err(reason) => {
                tracing::warn!("skipping playlist {}: {reason}", placed.path.display());
                continue;
            }
        };
        let candidate = FoundBlend { placed, blend };
        match found_by_id.get(&candidate.blend.id) {
            Some(existing) if existing.blend.edited >= candidate.blend.edited => {
                tracing::warn!(
                    "skipping {}: duplicate of playlist in {}",
                    candidate.placed.path.display(),
                    existing.placed.path.display()
                );
            }
            _ => {
                found_by_id.insert(candidate.blend.id.clone(), candidate);
            }
        }
    }
    found_by_id.retain(|id, candidate| {
        let fresh = !unchanged.contains(id);
        if !fresh {
            tracing::warn!(
                "skipping {}: duplicate of playlist in another file",
                candidate.placed.path.display()
            );
        }
        fresh
    });
    Some(Listing {
        blends: found_by_id.into_values().collect(),
        present,
        whole,
    })
}

pub(crate) fn file_for(folder: &Path, name: &str, own: Option<&Path>) -> PathBuf {
    let stem = stem_of(name);
    if let Some(own) = own
        && own.parent() == Some(folder)
        && own
            .file_stem()
            .and_then(|own_stem| own_stem.to_str())
            .is_some_and(|own_stem| numbered(own_stem, &stem))
    {
        return own.to_owned();
    }
    let file = rikka_format::unique_name(&format!("{stem}.{EXTENSION}"), |candidate| {
        let path = folder.join(candidate);
        Some(path.as_path()) != own && fs::symlink_metadata(&path).is_ok()
    });
    folder.join(file)
}

fn is_playlist(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == EXTENSION)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.starts_with('.'))
}

fn stem_of(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| match character {
            '/' | '\\' => '-',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    let mut end = trimmed.len().min(LONGEST_STEM);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    match trimmed[..end].trim_end() {
        "" => UNNAMED.to_owned(),
        stem => stem.to_owned(),
    }
}

fn numbered(own: &str, stem: &str) -> bool {
    match own.strip_prefix(stem) {
        Some("") => true,
        Some(rest) => rest
            .strip_prefix(" (")
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())),
        None => false,
    }
}
