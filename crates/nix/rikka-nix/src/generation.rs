use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::error::{Error, Result};

const PROFILES: &str = "/nix/var/nix/profiles";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generation {
    pub number: u32,
    pub build_time: i64,
    pub version: String,
    pub current: bool,
}

impl Generation {
    pub fn all() -> Result<Vec<Self>> {
        Self::list_in(Path::new(PROFILES))
    }

    #[must_use]
    pub fn link(number: u32) -> PathBuf {
        Path::new(PROFILES).join(format!("system-{number}-link"))
    }

    #[must_use]
    pub fn current() -> Option<u32> {
        number(&std::fs::read_link(Path::new(PROFILES).join("system")).ok()?)
    }

    fn list_in(profiles: &Path) -> Result<Vec<Self>> {
        if !profiles.is_dir() {
            return Err(Error::NotNixos(profiles.to_owned()));
        }
        let current = std::fs::read_link(profiles.join("system"))
            .ok()
            .and_then(|link| number(&link));

        let entries = std::fs::read_dir(profiles).map_err(|error| Error::Unreadable {
            path: profiles.to_owned(),
            reason: error.to_string(),
        })?;

        let mut found: Vec<Self> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let number = number(&path)?;
                Some(Self {
                    number,
                    build_time: build_time(&path),
                    version: version(&path),
                    current: current == Some(number),
                })
            })
            .collect();
        found.sort_unstable_by_key(|found| std::cmp::Reverse(found.number));
        Ok(found)
    }
}

fn number(path: &Path) -> Option<u32> {
    path.file_name()?
        .to_str()?
        .strip_prefix("system-")?
        .strip_suffix("-link")?
        .parse()
        .ok()
}

fn build_time(link: &Path) -> i64 {
    std::fs::symlink_metadata(link)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_secs()).ok())
        .unwrap_or_default()
}

fn version(link: &Path) -> String {
    std::fs::read_to_string(link.join("nixos-version"))
        .map(|version| version.trim().to_owned())
        .unwrap_or_default()
}
