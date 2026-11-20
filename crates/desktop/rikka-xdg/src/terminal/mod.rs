mod launch;
mod list;

pub use launch::{TerminalCommand, TerminalLaunch};

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::catalog::desktop_files;
use crate::entry::DesktopEntry;
use crate::error::{Error, Result};
use crate::keyfile::KeyFile;
use crate::locale::locales;
use crate::roots::Roots;
use list::{Line, TerminalList};

const LIST: &str = "xdg-terminals.list";
const FALLBACKS: &str = "xdg-terminal-exec";
const APPLICATIONS: &str = "applications";
const CATEGORY: &str = "TerminalEmulator";
const ACTION_GROUP: &str = "Desktop Action";

#[derive(Clone, Debug)]
pub struct Terminals {
    roots: Roots,
}

#[derive(Clone, Debug)]
pub struct Terminal {
    entry: DesktopEntry,
    exec: String,
}

impl Terminals {
    #[must_use]
    pub fn session() -> Option<Terminals> {
        Roots::session().map(|roots| Terminals { roots })
    }

    #[must_use]
    pub fn new(config_home: impl Into<PathBuf>, data_home: impl Into<PathBuf>) -> Terminals {
        Terminals {
            roots: Roots::new(config_home.into(), data_home.into()),
        }
    }

    #[must_use]
    pub fn config_dirs(mut self, dirs: impl IntoIterator<Item = impl Into<PathBuf>>) -> Terminals {
        self.roots.config_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn data_dirs(mut self, dirs: impl IntoIterator<Item = impl Into<PathBuf>>) -> Terminals {
        self.roots.data_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn desktops(mut self, names: impl IntoIterator<Item = impl AsRef<str>>) -> Terminals {
        self.roots.set_desktops(names);
        self
    }

    #[must_use]
    pub fn preferred(&self) -> Option<Terminal> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut excluded: HashSet<&str> = HashSet::new();
        let lists: Vec<TerminalList> = self
            .lists()
            .iter()
            .filter_map(|path| TerminalList::read(path).ok().flatten())
            .collect();
        let locales = locales();
        for line in lists.iter().flat_map(TerminalList::lines) {
            if !seen.insert(line.id()) {
                continue;
            }
            match line {
                Line::Chosen { id, action } => {
                    let chosen = self
                        .find(id, &locales)
                        .and_then(|entry| Terminal::opened(entry, action));
                    if chosen.is_some() {
                        return chosen;
                    }
                    excluded.insert(id);
                }
                Line::Excluded(id) => {
                    excluded.insert(id);
                }
                Line::Protected(_) => {}
            }
        }
        self.installed()
            .into_iter()
            .filter(|entry| !excluded.contains(entry.id.as_str()))
            .find(|entry| shows_here(entry, &self.roots.desktops))
            .and_then(|entry| Terminal::opened(entry, None))
    }

    #[must_use]
    pub fn installed(&self) -> Vec<DesktopEntry> {
        let locales = locales();
        let mut seen: HashSet<String> = HashSet::new();
        let mut found = Vec::new();
        for dir in self.application_dirs() {
            let mut files = desktop_files(&dir);
            files.sort();
            for (id, path) in files {
                if !seen.insert(id.clone()) {
                    continue;
                }
                if let Some(entry) = parse(&id, &path, &locales).filter(is_terminal) {
                    found.push(entry);
                }
            }
        }
        found
    }

    fn find(&self, id: &str, locales: &[String]) -> Option<DesktopEntry> {
        let spellings: Vec<String> = std::iter::once(id.to_owned())
            .chain((1..=id.matches('-').count()).map(|nested| id.replacen('-', "/", nested)))
            .collect();
        self.application_dirs()
            .find_map(|dir| {
                spellings.iter().find_map(|spelled| {
                    let path = dir.join(spelled);
                    let text = std::fs::read_to_string(&path).ok()?;
                    Some((path, text))
                })
            })
            .and_then(|(path, text)| DesktopEntry::parse(id, &path, &text, locales))
            .filter(is_terminal)
    }

    fn application_dirs(&self) -> impl Iterator<Item = PathBuf> + '_ {
        std::iter::once(&self.roots.data_home)
            .chain(&self.roots.data_dirs)
            .map(|dir| dir.join(APPLICATIONS))
    }

    pub fn prefer(&self, id: &str) -> Result<()> {
        let marked = id.starts_with(['-', '+', '/', '#'])
            || id.contains(|ch: char| ch == ':' || ch.is_whitespace());
        if marked || !crate::associations::is_desktop_id(id) {
            return Err(Error::InvalidTerminal(id.to_owned()));
        }
        let home = &self.roots.config_home;
        let user = home.join(LIST);
        let mut list = TerminalList::read(&user)?.unwrap_or_else(|| TerminalList::fresh(&user));
        list.choose_first(id);
        list.write()?;
        for path in self.desktop_lists(home) {
            let Some(mut list) = TerminalList::read(&path)? else {
                continue;
            };
            if list.chooses_any() {
                list.choose_first(id);
                list.write()?;
            }
        }
        Ok(())
    }

    pub fn forget_choice(&self) -> Result<()> {
        for path in self.user_lists() {
            let Some(mut list) = TerminalList::read(&path)? else {
                continue;
            };
            list.drop_chosen();
            list.write()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn is_chosen(&self) -> bool {
        self.user_lists()
            .filter_map(|path| TerminalList::read(&path).ok().flatten())
            .any(|list| list.chooses_any())
    }

    #[must_use]
    pub fn lists(&self) -> Vec<PathBuf> {
        let roots = &self.roots;
        let configs = std::iter::once(&roots.config_home).chain(&roots.config_dirs);
        let fallbacks = roots.data_dirs.iter().map(|dir| dir.join(FALLBACKS));
        let mut lists: Vec<PathBuf> = Vec::new();
        for dir in configs.cloned().chain(fallbacks) {
            for path in self.desktop_lists(&dir).into_iter().chain([dir.join(LIST)]) {
                if !lists.contains(&path) {
                    lists.push(path);
                }
            }
        }
        lists
    }

    fn user_lists(&self) -> impl Iterator<Item = PathBuf> {
        let home = &self.roots.config_home;
        std::iter::once(home.join(LIST)).chain(self.desktop_lists(home))
    }

    fn desktop_lists(&self, dir: &Path) -> Vec<PathBuf> {
        self.roots.desktop_lists(dir, LIST)
    }
}

impl Terminal {
    fn opened(entry: DesktopEntry, action: Option<&str>) -> Option<Terminal> {
        let exec = match action {
            None => entry.exec.clone(),
            Some(action) => {
                let text = std::fs::read_to_string(&entry.path).ok()?;
                KeyFile::parse(&text)
                    .group(&format!("{ACTION_GROUP} {action}"))?
                    .string("Exec")
                    .filter(|exec| !exec.is_empty())?
            }
        };
        Some(Terminal { entry, exec })
    }

    #[must_use]
    pub fn entry(&self) -> &DesktopEntry {
        &self.entry
    }

    #[must_use]
    pub fn launch(&self) -> TerminalLaunch<'_> {
        TerminalLaunch::new(self)
    }
}

fn is_terminal(entry: &DesktopEntry) -> bool {
    entry.categories.iter().any(|category| category == CATEGORY) && entry.is_installed()
}

fn shows_here(entry: &DesktopEntry, desktops: &[String]) -> bool {
    let names_one = |listed: &[String]| {
        listed.iter().any(|desktop| {
            desktops
                .iter()
                .any(|here| here.eq_ignore_ascii_case(desktop))
        })
    };
    (entry.only_show_in.is_empty() || names_one(&entry.only_show_in))
        && !names_one(&entry.not_show_in)
}

fn parse(id: &str, path: &Path, locales: &[String]) -> Option<DesktopEntry> {
    let text = std::fs::read_to_string(path).ok()?;
    DesktopEntry::parse(id, path, &text, locales)
}
