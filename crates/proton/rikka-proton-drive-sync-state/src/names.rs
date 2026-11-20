use jiff::civil::Date;
use rikka_proton_names::{fitted_name, split_extension};
use rikka_text_unicode::nfc;
use serde::{Deserialize, Serialize};

pub const TEMPORARY_PREFIX: &str = ".~rikka-sync-";

const IGNORED_PREFIXES: [&str; 5] = [".~", "~$", ".#", ".goutputstream-", ".Trash"];
const IGNORED_NAMES: [&str; 5] = [".", "..", ".DS_Store", "Thumbs.db", "desktop.ini"];
const IGNORED_EXTENSIONS: [&str; 9] = [
    "tmp",
    "swp",
    "swo",
    "swx",
    "part",
    "partial",
    "crdownload",
    "download",
    "kate-swp",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    Edit,
    Clash,
}

#[must_use]
pub fn is_ignored(name: &str) -> bool {
    if name.is_empty() || IGNORED_NAMES.contains(&name) {
        return true;
    }
    if IGNORED_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return true;
    }
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => IGNORED_EXTENSIONS
            .iter()
            .any(|ignored| extension.eq_ignore_ascii_case(ignored)),
        _ => false,
    }
}

#[must_use]
pub fn same_name(one: &str, other: &str) -> bool {
    one == other || nfc(one) == nfc(other)
}

#[must_use]
pub fn local_name(remote: &str) -> String {
    let replaced: String = remote
        .chars()
        .map(|ch| match ch {
            '/' | '\0' => '_',
            other => other,
        })
        .collect();
    let sanitized = match replaced.as_str() {
        "" => "_".to_owned(),
        "." | ".." => replaced.replace('.', "_"),
        _ => replaced,
    };
    fitted_name(&sanitized)
}

#[must_use]
pub fn conflict_name(name: &str, conflict: Conflict, device: &str, date: Date) -> String {
    let (stem, extension) = split_extension(name);
    let label = match conflict {
        Conflict::Edit => "Edit conflict",
        Conflict::Clash => "Name clash",
    };
    let device = local_name(device.trim());
    let labeled = format!("{stem} (# {label} {date} {device} #)");
    fitted_name(&match extension {
        "" => labeled,
        extension => format!("{labeled}.{extension}"),
    })
}
