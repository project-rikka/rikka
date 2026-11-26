use serde::Deserialize;

use crate::checked::{RelativePath, Sha1};
use crate::coordinate::Coordinate;
use crate::error::{Error, Result};
use crate::rules::{Environment, Rule, allowed};

pub const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: Sha1,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct LibraryArtifact {
    #[serde(default)]
    path: Option<RelativePath>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    sha1: Option<Sha1>,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
struct LibraryDownloads {
    #[serde(default)]
    artifact: Option<LibraryArtifact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Library {
    pub name: Coordinate,
    #[serde(default)]
    downloads: Option<LibraryDownloads>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    sha1: Option<Sha1>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub name: Coordinate,
    pub path: RelativePath,
    pub url: String,
    pub sha1: Option<Sha1>,
    pub size: Option<u64>,
}

impl Library {
    #[must_use]
    pub fn applies(&self, environment: &Environment) -> bool {
        allowed(&self.rules, environment)
    }

    pub fn artifact(&self) -> Result<Option<Artifact>> {
        let coordinate_path = || RelativePath::new(&self.name.path());
        match &self.downloads {
            Some(downloads) => {
                let Some(artifact) = &downloads.artifact else {
                    return Ok(None);
                };
                if artifact.url.is_empty() {
                    return Err(Error::Undownloadable(self.name.to_string()));
                }
                Ok(Some(Artifact {
                    name: self.name.clone(),
                    path: match &artifact.path {
                        Some(path) => path.clone(),
                        None => coordinate_path()?,
                    },
                    url: artifact.url.clone(),
                    sha1: artifact.sha1.clone(),
                    size: artifact.size,
                }))
            }
            None => {
                let base = self.url.as_deref().unwrap_or(MOJANG_LIBRARIES);
                let path = coordinate_path()?;
                let url = match base.ends_with('/') {
                    true => format!("{base}{path}"),
                    false => format!("{base}/{path}"),
                };
                Ok(Some(Artifact {
                    name: self.name.clone(),
                    path,
                    url,
                    sha1: self.sha1.clone(),
                    size: self.size,
                }))
            }
        }
    }
}
