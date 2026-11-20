use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::UserDir;
use crate::base;

const FILE: &str = "user-dirs.dirs";

pub(super) fn named() -> HashMap<UserDir, PathBuf> {
    base::config_home()
        .map(|config| config.join(FILE))
        .and_then(|file| std::fs::read_to_string(file).ok())
        .map(|text| parse(&text, base::home()))
        .unwrap_or_default()
}

fn parse(text: &str, home: Option<&Path>) -> HashMap<UserDir, PathBuf> {
    let mut named = HashMap::new();
    for (which, quoted) in text.lines().filter_map(declaration) {
        if let Some(path) = expand(quoted, home) {
            named.insert(which, path);
        }
    }
    named
}

fn declaration(line: &str) -> Option<(UserDir, &str)> {
    let line = line.trim_start_matches([' ', '\t']);
    if line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let key = key.trim_end_matches([' ', '\t']);
    let which = UserDir::ALL.into_iter().find(|dir| dir.key() == key)?;
    let quoted = value.trim_start_matches([' ', '\t']).strip_prefix('"')?;
    Some((which, quoted))
}

fn expand(quoted: &str, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(rest) = quoted.strip_prefix("$HOME") {
        if !(rest.is_empty() || rest.starts_with('/') || rest.starts_with('"')) {
            return None;
        }
        let home = home?;
        let relative = unescape(rest);
        let relative = relative.trim_matches('/');
        return Some(match relative.is_empty() {
            true => home.to_path_buf(),
            false => home.join(relative),
        });
    }
    if !quoted.starts_with('/') {
        return None;
    }
    let absolute = unescape(quoted);
    let trimmed = absolute.trim_end_matches('/');
    Some(PathBuf::from(match trimmed.is_empty() {
        true => "/",
        false => trimmed,
    }))
}

fn unescape(value: &str) -> String {
    let mut path = String::with_capacity(value.len());
    let mut rest = value.chars();
    while let Some(character) = rest.next() {
        match character {
            '"' => break,
            '\\' => path.push(rest.next().unwrap_or('\\')),
            _ => path.push(character),
        }
    }
    path
}
