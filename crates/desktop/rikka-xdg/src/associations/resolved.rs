use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rikka_mime::MimeType;

use crate::catalog::desktop_files;
use crate::entry::DesktopEntry;
use crate::keyfile::KeyFile;

use super::{ADDED, DEFAULTS, REMOVED, split_ids};

type Table = HashMap<String, Vec<String>>;

pub(crate) struct Level {
    pub(crate) desktop_lists: Vec<PathBuf>,
    pub(crate) list: PathBuf,
    pub(crate) applications: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Associations {
    levels: Vec<Read>,
}

#[derive(Debug, Default)]
struct Read {
    defaults: Vec<Table>,
    added: Table,
    removed: Table,
    folder: Option<Folder>,
}

#[derive(Debug, Default)]
struct Folder {
    present: HashSet<String>,
    entries: BTreeMap<String, DesktopEntry>,
}

impl Associations {
    pub(crate) fn read(levels: &[Level], locales: &[String]) -> Associations {
        Associations {
            levels: levels
                .iter()
                .map(|level| Read::load(level, locales))
                .collect(),
        }
    }

    #[must_use]
    pub fn application(&self, id: &str) -> Option<&DesktopEntry> {
        self.levels
            .iter()
            .filter_map(|level| level.folder.as_ref())
            .find(|folder| folder.present.contains(id))?
            .entries
            .get(id)
            .filter(|entry| entry.is_installed())
    }

    #[must_use]
    pub fn default_for(&self, mime: &MimeType) -> Option<&DesktopEntry> {
        let mime = mime.as_str();
        let associated = self.associated(mime);
        let chosen = self
            .levels
            .iter()
            .flat_map(|level| &level.defaults)
            .filter_map(|defaults| defaults.get(mime))
            .flatten()
            .find(|id| associated.contains(&id.as_str()))
            .map(String::as_str);
        self.application(chosen.or_else(|| associated.first().copied())?)
    }

    #[must_use]
    pub fn handlers(&self, mime: &MimeType) -> Vec<&DesktopEntry> {
        self.associated(mime.as_str())
            .into_iter()
            .filter_map(|id| self.application(id))
            .collect()
    }

    #[must_use]
    pub fn default_for_any<'a>(
        &self,
        types: impl IntoIterator<Item = &'a MimeType>,
    ) -> Option<&DesktopEntry> {
        types.into_iter().find_map(|mime| self.default_for(mime))
    }

    #[must_use]
    pub fn handlers_for_any<'a>(
        &self,
        types: impl IntoIterator<Item = &'a MimeType>,
    ) -> Vec<&DesktopEntry> {
        let mut seen = HashSet::new();
        types
            .into_iter()
            .flat_map(|mime| self.handlers(mime))
            .filter(|entry| seen.insert(entry.id.as_str()))
            .collect()
    }

    #[must_use]
    pub fn is_chosen(&self, mime: &MimeType) -> bool {
        self.levels.first().is_some_and(|user| {
            user.defaults.iter().any(|defaults| {
                defaults
                    .get(mime.as_str())
                    .is_some_and(|ids| !ids.is_empty())
            })
        })
    }

    #[must_use]
    pub fn types(&self) -> Vec<MimeType> {
        let mut types: Vec<MimeType> = self
            .levels
            .iter()
            .flat_map(|level| {
                let listed = level
                    .defaults
                    .iter()
                    .flat_map(|defaults| defaults.keys())
                    .chain(level.added.keys())
                    .map(String::as_str);
                let handled = level
                    .folder
                    .iter()
                    .flat_map(|folder| folder.entries.values())
                    .flat_map(|entry| entry.mime_types.iter().map(String::as_str));
                listed.chain(handled)
            })
            .filter_map(MimeType::parse)
            .collect();
        types.sort_unstable();
        types.dedup();
        types.retain(|mime| !self.associated(mime.as_str()).is_empty());
        types
    }

    fn associated(&self, mime: &str) -> Vec<&str> {
        let mut found: Vec<&str> = Vec::new();
        let mut refused: HashSet<&str> = HashSet::new();
        for level in &self.levels {
            for id in level.added.get(mime).into_iter().flatten() {
                if !refused.contains(id.as_str()) {
                    found.push(id);
                }
            }
            refused.extend(
                level
                    .removed
                    .get(mime)
                    .into_iter()
                    .flatten()
                    .map(String::as_str),
            );
            let Some(folder) = &level.folder else {
                continue;
            };
            for (id, entry) in &folder.entries {
                if entry.handles(mime) && !refused.contains(id.as_str()) {
                    found.push(id);
                }
            }
            refused.extend(folder.present.iter().map(String::as_str));
        }
        let mut seen = HashSet::new();
        found.retain(|id| seen.insert(*id) && self.application(id).is_some());
        found
    }
}

impl Read {
    fn load(level: &Level, locales: &[String]) -> Read {
        let mut defaults: Vec<Table> = level
            .desktop_lists
            .iter()
            .map(|list| {
                groups(list, &[DEFAULTS])
                    .remove(DEFAULTS)
                    .unwrap_or_default()
            })
            .collect();
        let mut tables = groups(&level.list, &[DEFAULTS, ADDED, REMOVED]);
        defaults.push(tables.remove(DEFAULTS).unwrap_or_default());
        Read {
            defaults,
            added: tables.remove(ADDED).unwrap_or_default(),
            removed: tables.remove(REMOVED).unwrap_or_default(),
            folder: level
                .applications
                .as_deref()
                .map(|folder| Folder::scan(folder, locales)),
        }
    }
}

impl Folder {
    fn scan(folder: &Path, locales: &[String]) -> Folder {
        let mut read = Folder::default();
        for (id, path) in desktop_files(folder) {
            if !read.present.insert(id.clone()) {
                continue;
            }
            let parsed = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| DesktopEntry::parse(&id, &path, &text, locales));
            if let Some(entry) = parsed {
                read.entries.insert(id, entry);
            }
        }
        read
    }
}

fn groups(list: &Path, names: &[&'static str]) -> HashMap<&'static str, Table> {
    let mut tables: HashMap<&'static str, Table> = HashMap::new();
    let Ok(text) = std::fs::read_to_string(list) else {
        return tables;
    };
    for group in KeyFile::parse(&text).groups() {
        let Some(name) = names.iter().find(|name| **name == group.name()) else {
            continue;
        };
        let table = tables.entry(name).or_default();
        for (mime, locale, value) in group.entries() {
            if locale.is_none() {
                table.insert(mime.to_owned(), split_ids(value));
            }
        }
    }
    tables
}
