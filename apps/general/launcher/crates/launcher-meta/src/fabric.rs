use serde::Deserialize;

use crate::error::Result;

pub const FABRIC_META: &str = "https://meta.fabricmc.net/v2";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LoaderRelease {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct LoaderEntry {
    loader: LoaderRelease,
}

pub fn loaders_url(game: &str) -> String {
    format!("{FABRIC_META}/versions/loader/{game}")
}

pub fn profile_url(game: &str, loader: &str) -> String {
    format!("{FABRIC_META}/versions/loader/{game}/{loader}/profile/json")
}

pub fn loaders_from_json(bytes: &[u8]) -> Result<Vec<LoaderRelease>> {
    let entries: Vec<LoaderEntry> = serde_json::from_slice(bytes)?;
    Ok(entries.into_iter().map(|entry| entry.loader).collect())
}
