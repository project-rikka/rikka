use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};
use crate::spawn::spawn_detached;

const OPENER: &str = "rikka-open";
const FALLBACK: &str = "xdg-open";

#[must_use]
pub fn opener(target: &str) -> Vec<String> {
    let opener = match crate::is_runnable(OPENER) {
        true => OPENER,
        false => FALLBACK,
    };
    vec![opener.to_owned(), target.to_owned()]
}

pub(super) fn open(target: &str) -> Result<()> {
    let started = match spawn_detached(Command::new(OPENER).arg(target)) {
        Err(Error::NotInstalled { .. }) => spawn_detached(Command::new(FALLBACK).arg(target)),
        answered => answered,
    };
    started.map(drop)
}

pub(super) fn reveal(path: &Path) -> Result<()> {
    let folder = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    open(&crate::path_to_uri(folder.unwrap_or(path)))
}
