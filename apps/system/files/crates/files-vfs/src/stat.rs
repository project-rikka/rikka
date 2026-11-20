use std::collections::HashSet;
use std::path::Path;

use files_core::{Entry, Location};
use rikka_file_operations::EntryKind;
use rikka_mime::{Database, MimeType};

use crate::backend::{Detail, ListOptions};
use crate::error::{Error, Result};

pub(crate) struct Reading<'a> {
    pub(crate) mime: &'a Database,
    pub(crate) scheme: &'a str,
    pub(crate) detail: Detail,
}

pub(crate) fn list(
    mime: &Database,
    scheme: &str,
    directory: &Path,
    at: &Location,
    options: ListOptions,
) -> Result<Vec<Entry>> {
    let listed = std::fs::read_dir(directory).map_err(|error| Error::from_io(&error, at))?;
    let hidden = match options.detail {
        Detail::Typed => hidden_list(directory),
        Detail::Bare => HashSet::new(),
    };
    let reading = Reading {
        mime,
        scheme,
        detail: options.detail,
    };
    let mut entries = Vec::new();
    for dir_entry in listed.flatten() {
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        let is_hidden = is_hidden(&name, &hidden);
        if is_hidden && !options.show_hidden {
            continue;
        }
        if let Ok(entry) = entry(&reading, &dir_entry.path(), name, is_hidden) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub(crate) fn entry(
    reading: &Reading<'_>,
    path: &Path,
    name: String,
    hidden: bool,
) -> Result<Entry> {
    let Reading {
        mime,
        scheme,
        detail,
    } = *reading;
    let at = Location::from_scheme_and_path(scheme, path);
    let local =
        rikka_file_operations::Entry::read(path).map_err(|error| error.map(|_| at.clone()))?;
    let broken = local.link.as_ref().is_some_and(|link| link.broken);
    Ok(Entry {
        name,
        kind: local.kind,
        mime: type_of(mime, detail, local.kind, path, broken),
        size: local.size,
        modified: local.modified,
        created: local.created,
        mode: local.mode,
        hidden,
        link: local.link,
        location: at,
    })
}

fn type_of(
    mime: &Database,
    detail: Detail,
    kind: EntryKind,
    path: &Path,
    broken: bool,
) -> MimeType {
    if kind.is_directory() {
        return MimeType::well_known(MimeType::DIRECTORY);
    }
    if broken {
        return MimeType::well_known(MimeType::SYMLINK);
    }
    if kind != EntryKind::File || detail == Detail::Bare {
        return MimeType::well_known(MimeType::UNKNOWN);
    }
    mime.for_file(path)
}

fn is_hidden(name: &str, listed: &HashSet<String>) -> bool {
    name.starts_with('.') || listed.contains(name)
}

fn hidden_list(directory: &Path) -> HashSet<String> {
    std::fs::read_to_string(directory.join(".hidden"))
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
