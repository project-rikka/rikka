use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rikka_dirs::{data_dirs, home};
use rikka_xdg::{KeyFile, locales};

#[derive(Debug, Clone)]
pub(crate) struct CursorTheme {
    pub(crate) slug: String,
    pub(crate) name: String,
}

pub(crate) fn installed() -> Vec<CursorTheme> {
    let languages = locales();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for root in roots() {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.join("cursors").is_dir() {
                continue;
            }
            let Some(slug) = path.file_name().and_then(|slug| slug.to_str()) else {
                continue;
            };
            names
                .entry(slug.to_owned())
                .or_insert_with(|| readable(&path, &languages).unwrap_or_else(|| slug.to_owned()));
        }
    }

    let mut themes: Vec<CursorTheme> = names
        .into_iter()
        .map(|(slug, name)| CursorTheme { slug, name })
        .collect();
    themes.sort_by_key(|theme| theme.name.to_lowercase());
    themes
}

const PIXMAPS: &str = "/run/current-system/sw/share/pixmaps";

fn roots() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("XCURSOR_PATH") {
        return std::env::split_paths(&path).collect();
    }

    let mut roots: Vec<PathBuf> = home().map(|home| home.join(".icons")).into_iter().collect();
    roots.extend(data_dirs().into_iter().map(|data| data.join("icons")));
    roots.push(PathBuf::from(PIXMAPS));
    roots
}

fn readable(path: &Path, languages: &[String]) -> Option<String> {
    let index = std::fs::read_to_string(path.join("index.theme")).ok()?;
    KeyFile::parse(&index)
        .group("Icon Theme")?
        .localized("Name", languages)
        .filter(|name| !name.is_empty())
}
