use std::collections::HashSet;

use serde::Deserialize;

use crate::arguments::Arguments;
use crate::checked::Sha1;
use crate::error::{Error, Result};
use crate::library::{Artifact, Download, Library};
use crate::rules::Environment;

pub const VERSION_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: Sha1,
    pub release_time: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

impl VersionManifest {
    pub fn from_json(bytes: &[u8]) -> Result<VersionManifest> {
        Ok(serde_json::from_slice(bytes)?)
    }

    #[must_use]
    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|entry| entry.id == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexReference {
    pub id: String,
    pub sha1: Sha1,
    pub size: u64,
    #[serde(default)]
    pub total_size: u64,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
struct VersionDownloads {
    #[serde(default)]
    client: Option<Download>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    #[serde(default)]
    pub inherits_from: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    main_class: Option<String>,
    #[serde(default)]
    arguments: Arguments,
    #[serde(default)]
    asset_index: Option<AssetIndexReference>,
    #[serde(default)]
    downloads: Option<VersionDownloads>,
    #[serde(default)]
    java_version: Option<JavaVersion>,
    #[serde(default)]
    libraries: Vec<Library>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientVersion {
    pub id: String,
    pub kind: String,
    pub main_class: String,
    pub arguments: Arguments,
    pub asset_index: AssetIndexReference,
    pub client: Download,
    pub java: JavaVersion,
    pub libraries: Vec<Library>,
}

impl Version {
    pub fn from_json(bytes: &[u8]) -> Result<Version> {
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn inherit(self, parent: Version) -> Result<Version> {
        match &self.inherits_from {
            Some(expected) if *expected == parent.id => {}
            expected => {
                return Err(Error::Parent {
                    child: self.id,
                    expected: expected.clone().unwrap_or_default(),
                    found: parent.id,
                });
            }
        }
        let mut arguments = parent.arguments;
        arguments.extend(self.arguments);
        let mut libraries = self.libraries;
        libraries.extend(parent.libraries);
        Ok(Version {
            id: self.id,
            inherits_from: parent.inherits_from,
            kind: self.kind.or(parent.kind),
            main_class: self.main_class.or(parent.main_class),
            arguments,
            asset_index: self.asset_index.or(parent.asset_index),
            downloads: match self.downloads.and_then(|downloads| downloads.client) {
                Some(client) => Some(VersionDownloads {
                    client: Some(client),
                }),
                None => parent.downloads,
            },
            java_version: self.java_version.or(parent.java_version),
            libraries,
        })
    }

    pub fn into_client(self) -> Result<ClientVersion> {
        let missing = |field| Error::Missing {
            version: self.id.clone(),
            field,
        };
        if self.inherits_from.is_some() {
            return Err(missing("parent version"));
        }
        Ok(ClientVersion {
            kind: self.kind.clone().unwrap_or_else(|| "release".to_owned()),
            main_class: self
                .main_class
                .clone()
                .ok_or_else(|| missing("mainClass"))?,
            asset_index: self
                .asset_index
                .clone()
                .ok_or_else(|| missing("assetIndex"))?,
            client: self
                .downloads
                .clone()
                .and_then(|downloads| downloads.client)
                .ok_or_else(|| missing("client download"))?,
            java: self
                .java_version
                .clone()
                .ok_or_else(|| missing("javaVersion"))?,
            arguments: self.arguments,
            libraries: self.libraries,
            id: self.id,
        })
    }
}

impl ClientVersion {
    pub fn artifacts(&self, environment: &Environment) -> Result<Vec<Artifact>> {
        let mut seen = HashSet::new();
        let mut artifacts = Vec::new();
        for library in &self.libraries {
            if !library.applies(environment) {
                continue;
            }
            let identity = library.name.identity();
            if !seen.insert(identity) {
                continue;
            }
            if let Some(artifact) = library.artifact()? {
                artifacts.push(artifact);
            }
        }
        Ok(artifacts)
    }
}
