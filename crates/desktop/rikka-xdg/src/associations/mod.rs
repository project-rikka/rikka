mod list;
mod resolved;

pub use resolved::Associations;

use std::path::{Path, PathBuf};

use rikka_mime::MimeType;

use crate::error::{Error, Result};
use crate::locale::locales;
use crate::roots::Roots;
use crate::watch::Watch;
use list::ListText;
use resolved::Level;

const LIST: &str = "mimeapps.list";
const APPLICATIONS: &str = "applications";
const DEFAULTS: &str = "Default Applications";
const ADDED: &str = "Added Associations";
const REMOVED: &str = "Removed Associations";

#[derive(Clone, Debug)]
pub struct MimeApps {
    roots: Roots,
}

impl MimeApps {
    #[must_use]
    pub fn session() -> Option<MimeApps> {
        Roots::session().map(|roots| MimeApps { roots })
    }

    #[must_use]
    pub fn new(config_home: impl Into<PathBuf>, data_home: impl Into<PathBuf>) -> MimeApps {
        MimeApps {
            roots: Roots::new(config_home.into(), data_home.into()),
        }
    }

    #[must_use]
    pub fn config_dirs(mut self, dirs: impl IntoIterator<Item = impl Into<PathBuf>>) -> MimeApps {
        self.roots.config_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn data_dirs(mut self, dirs: impl IntoIterator<Item = impl Into<PathBuf>>) -> MimeApps {
        self.roots.data_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn desktops(mut self, names: impl IntoIterator<Item = impl AsRef<str>>) -> MimeApps {
        self.roots.set_desktops(names);
        self
    }

    #[must_use]
    pub fn read(&self) -> Associations {
        Associations::read(&self.levels(), &locales())
    }

    pub fn set_default(&self, application: &str, types: &[MimeType]) -> Result<()> {
        if !is_desktop_id(application) {
            return Err(Error::InvalidAssociation(application.to_owned()));
        }
        let user = self.roots.config_home.join(LIST);
        let mut list = ListText::read(&user)?.unwrap_or_else(|| ListText::fresh(&user));
        for mime in types.iter().map(MimeType::as_str) {
            let defaults = first(application, list.ids(DEFAULTS, mime));
            list.put(DEFAULTS, mime, &defaults);
            let added = first(application, list.ids(ADDED, mime));
            list.put(ADDED, mime, &added);
            let removed = without(application, list.ids(REMOVED, mime));
            list.put(REMOVED, mime, &removed);
        }
        list.write()?;
        for path in self.desktop_lists(&self.roots.config_home) {
            let Some(mut list) = ListText::read(&path)? else {
                continue;
            };
            for mime in types.iter().map(MimeType::as_str) {
                let ids = list.ids(DEFAULTS, mime);
                if !ids.is_empty() {
                    list.put(DEFAULTS, mime, &first(application, ids));
                }
            }
            list.write()?;
        }
        Ok(())
    }

    pub fn forget_default(&self, types: &[MimeType]) -> Result<()> {
        let home = &self.roots.config_home;
        let user_lists = std::iter::once(home.join(LIST)).chain(self.desktop_lists(home));
        for path in user_lists {
            let Some(mut list) = ListText::read(&path)? else {
                continue;
            };
            for mime in types.iter().map(MimeType::as_str) {
                list.put(DEFAULTS, mime, &[]);
            }
            list.write()?;
        }
        Ok(())
    }

    pub fn watch(&self, changed: impl Fn() + Send + 'static) -> Result<Watch> {
        let roots = self
            .levels()
            .into_iter()
            .flat_map(|level| {
                let Level {
                    desktop_lists,
                    list,
                    applications,
                } = level;
                desktop_lists.into_iter().chain([list]).chain(applications)
            })
            .collect();
        Watch::spawn(roots, changed)
    }

    fn levels(&self) -> Vec<Level> {
        let roots = &self.roots;
        let configs = std::iter::once(&roots.config_home)
            .chain(&roots.config_dirs)
            .map(|dir| (dir.clone(), false));
        let datas = std::iter::once(&roots.data_home)
            .chain(&roots.data_dirs)
            .map(|dir| (dir.join(APPLICATIONS), true));
        let mut levels: Vec<Level> = Vec::new();
        for (dir, holds_applications) in configs.chain(datas) {
            let list = dir.join(LIST);
            if levels.iter().any(|level| level.list == list) {
                continue;
            }
            levels.push(Level {
                desktop_lists: self.desktop_lists(&dir),
                list,
                applications: holds_applications.then_some(dir),
            });
        }
        levels
    }

    fn desktop_lists(&self, dir: &Path) -> Vec<PathBuf> {
        self.roots.desktop_lists(dir, LIST)
    }
}

pub(crate) fn split_ids(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect()
}

fn first(application: &str, ids: Vec<String>) -> Vec<String> {
    std::iter::once(application.to_owned())
        .chain(without(application, ids))
        .collect()
}

fn without(application: &str, mut ids: Vec<String>) -> Vec<String> {
    ids.retain(|id| id != application);
    ids
}

pub(crate) fn is_desktop_id(id: &str) -> bool {
    id.strip_suffix(".desktop")
        .is_some_and(|stem| !stem.is_empty())
        && id.trim() == id
        && !id
            .chars()
            .any(|ch| ch == ';' || ch == '/' || ch.is_control())
}
