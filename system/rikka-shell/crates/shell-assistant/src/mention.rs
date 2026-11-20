use std::path::{Path, PathBuf};

use rikka_assist::App;
use rikka_chat::Offer;
use rikka_ui::MaterialSymbol;

const MOST: usize = 50;

#[must_use]
pub(crate) fn offers(home: Option<&Path>, running: &[App], typed: &str) -> Vec<Offer> {
    let mut offers = apps(running, typed);
    if let Some(home) = home {
        offers.extend(paths(home, typed));
    }
    offers.truncate(MOST);
    offers
}

#[must_use]
pub(crate) fn listed(folder: &Path) -> Vec<String> {
    entries(folder, "", |_| true)
}

fn apps(running: &[App], typed: &str) -> Vec<Offer> {
    if typed.contains('/') {
        return Vec::new();
    }
    let needle = typed.to_lowercase();
    running
        .iter()
        .filter(|app| {
            app.kit().to_lowercase().starts_with(&needle)
                || app.title().to_lowercase().starts_with(&needle)
        })
        .map(|app| Offer::mentioned(MaterialSymbol::WIDGETS, app.kit(), app.title(), app.about()))
        .collect()
}

fn paths(home: &Path, typed: &str) -> Vec<Offer> {
    let (folder, prefix) = split(typed);
    let under = match folder.as_os_str().is_empty() {
        true => String::new(),
        false => format!("{}/", folder.display()),
    };
    let needle = prefix.to_lowercase();
    let mut paths = entries(&home.join(&folder), &under, |name| {
        name.to_lowercase().starts_with(&needle) && !hidden(name, &prefix)
    });
    paths.truncate(MOST);
    paths.into_iter().map(Offer::path).collect()
}

fn entries(folder: &Path, under: &str, keep: impl Fn(&str) -> bool) -> Vec<String> {
    let Ok(listing) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths: Vec<(bool, String)> = listing
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !keep(&name) {
                return None;
            }
            let folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
            let path = match folder {
                true => format!("{under}{name}/"),
                false => format!("{under}{name}"),
            };
            Some((folder, path))
        })
        .collect();
    paths.sort_by(|one, two| two.0.cmp(&one.0).then_with(|| one.1.cmp(&two.1)));
    paths.into_iter().map(|(_, path)| path).collect()
}

fn hidden(name: &str, prefix: &str) -> bool {
    name.starts_with('.') && !prefix.starts_with('.')
}

fn split(typed: &str) -> (PathBuf, String) {
    match typed.rsplit_once('/') {
        Some((folder, prefix)) => (PathBuf::from(folder), prefix.to_owned()),
        None => (PathBuf::new(), typed.to_owned()),
    }
}
