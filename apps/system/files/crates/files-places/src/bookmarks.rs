use std::path::PathBuf;

use files_core::Location;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bookmark {
    pub(crate) at: Location,
    pub(crate) name: String,
}

fn path() -> Option<PathBuf> {
    Some(rikka_dirs::config_home()?.join("gtk-3.0").join("bookmarks"))
}

#[must_use]
pub(crate) fn load() -> Vec<Bookmark> {
    let Some(path) = path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse).collect()
}

pub fn add(at: &Location, name: Option<&str>) -> std::io::Result<()> {
    let mut existing = load();
    if existing.iter().any(|bookmark| bookmark.at == *at) {
        return Ok(());
    }
    existing.push(Bookmark {
        at: at.clone(),
        name: name.map(ToOwned::to_owned).unwrap_or_else(|| at.name()),
    });
    write(&existing)
}

pub fn remove(at: &Location) -> std::io::Result<()> {
    let mut existing = load();
    let before = existing.len();
    existing.retain(|bookmark| bookmark.at != *at);
    if existing.len() == before {
        return Ok(());
    }
    write(&existing)
}

pub fn reorder(moved: &Location, before: &Location) -> std::io::Result<()> {
    let mut existing = load();
    let (Some(from), Some(to)) = (pinned(&existing, moved), pinned(&existing, before)) else {
        return Ok(());
    };
    if from == to {
        return Ok(());
    }
    let pin = existing.remove(from);
    existing.insert(to, pin);
    write(&existing)
}

fn pinned(pins: &[Bookmark], at: &Location) -> Option<usize> {
    pins.iter().position(|pin| pin.at == *at)
}

fn write(bookmarks: &[Bookmark]) -> std::io::Result<()> {
    let Some(path) = path() else {
        return Ok(());
    };
    let text: String = bookmarks.iter().map(line).collect();
    rikka_fs::replace(path, text)
}

fn parse(line: &str) -> Option<Bookmark> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (uri, label) = line.split_once(' ').unwrap_or((line, ""));
    let at = Location::parse(uri)?;
    let name = if label.trim().is_empty() {
        at.name()
    } else {
        label.trim().to_owned()
    };
    Some(Bookmark { at, name })
}

fn line(bookmark: &Bookmark) -> String {
    if bookmark.name == bookmark.at.name() {
        format!("{}\n", bookmark.at.as_str())
    } else {
        format!("{} {}\n", bookmark.at.as_str(), bookmark.name)
    }
}
