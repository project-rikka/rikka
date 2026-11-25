use std::collections::BTreeMap;

use serde::Deserialize;

use crate::checked::{RelativePath, Sha1};
use crate::error::Result;

pub const RESOURCES: &str = "https://resources.download.minecraft.net/";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AssetObject {
    pub hash: Sha1,
    pub size: u64,
}

impl AssetObject {
    #[must_use]
    pub fn path(&self) -> RelativePath {
        let hash = self.hash.as_str();
        RelativePath::new(&format!("{}/{hash}", &hash[..2]))
            .expect("a SHA-1 digest makes a safe path")
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!("{RESOURCES}{}", self.path())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AssetIndex {
    pub objects: BTreeMap<String, AssetObject>,
}

impl AssetIndex {
    pub fn from_json(bytes: &[u8]) -> Result<AssetIndex> {
        Ok(serde_json::from_slice(bytes)?)
    }

    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.objects.values().map(|object| object.size).sum()
    }
}
