use std::collections::BTreeMap;

use serde::Deserialize;

use crate::checked::RelativePath;
use crate::error::Result;
use crate::library::Download;

pub const RUNTIME_INDEX: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RuntimeName {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RuntimeRelease {
    pub manifest: Download,
    pub version: RuntimeName,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct RuntimeIndex(BTreeMap<String, BTreeMap<String, Vec<RuntimeRelease>>>);

impl RuntimeIndex {
    pub fn from_json(bytes: &[u8]) -> Result<RuntimeIndex> {
        Ok(serde_json::from_slice(bytes)?)
    }

    #[must_use]
    pub fn release(&self, platform: &str, component: &str) -> Option<&RuntimeRelease> {
        self.0.get(platform)?.get(component)?.first()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RuntimeDownloads {
    pub raw: Download,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeFile {
    Directory,
    File {
        downloads: RuntimeDownloads,
        #[serde(default)]
        executable: bool,
    },
    Link {
        target: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RuntimeManifest {
    pub files: BTreeMap<RelativePath, RuntimeFile>,
}

impl RuntimeManifest {
    pub fn from_json(bytes: &[u8]) -> Result<RuntimeManifest> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
