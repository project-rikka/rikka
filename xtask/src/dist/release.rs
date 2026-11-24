use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::host;
use crate::shipped::{Component, Offer};

pub(crate) const SYSTEM: &str = "x86_64-linux";

pub(crate) const RELEASE: &str = "release.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Source {
    Download { url: String, hash: String },
    Local { file: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Packed {
    pub(crate) file: String,
    pub(crate) hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) offer: Offer,
    pub(crate) version: String,
    pub(crate) revision: String,
    pub(crate) pam: Vec<String>,
    pub(crate) portal: bool,
    pub(crate) templates: Vec<String>,
    pub(crate) links: BTreeMap<String, String>,
    #[serde(flatten)]
    pub(crate) source: Source,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Fragment {
    pub(crate) id: String,
    pub(crate) packed: Packed,
    pub(crate) entry: Entry,
    pub(crate) assets: BTreeMap<String, Packed>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Release {
    pub(crate) system: String,
    pub(crate) assets: BTreeMap<String, Source>,
    pub(crate) components: BTreeMap<String, Entry>,
}

impl Release {
    pub(crate) fn read_or_new(path: &Path) -> Result<Release> {
        if !path.exists() {
            return Ok(Release::empty());
        }
        read_json(path)
    }

    pub(crate) fn write(&self, into: &Path) -> Result<()> {
        write_json(&into.join(RELEASE), self)
    }

    pub(crate) fn empty() -> Release {
        Release {
            system: SYSTEM.to_owned(),
            assets: BTreeMap::new(),
            components: BTreeMap::new(),
        }
    }

    pub(crate) fn version(&self) -> Option<&str> {
        self.components
            .get(Component::BASE)
            .map(|entry| entry.version.as_str())
    }

    pub(crate) fn downloads(&self) -> impl Iterator<Item = &str> {
        self.components
            .values()
            .map(|entry| &entry.source)
            .chain(self.assets.values())
            .filter_map(|source| match source {
                Source::Download { url, .. } => Some(url.as_str()),
                Source::Local { .. } => None,
            })
    }

    pub(crate) fn referenced_assets(&self) -> impl Iterator<Item = &str> {
        self.components
            .values()
            .flat_map(|entry| entry.links.values())
            .filter(|name| self.assets.contains_key(name.as_str()))
            .map(String::as_str)
    }
}

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    serde_json::from_str(&host::read(path)?)
        .with_context(|| format!("failed to parse {}", path.display()))
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)
        .with_context(|| format!("failed to encode {}", path.display()))?;
    text.push('\n');
    host::write(path, text)
}
