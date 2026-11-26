use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    Config,
    Data,
    State,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    base: Base,
    path: String,
    #[serde(with = "crate::bytes")]
    contents: Zeroizing<Vec<u8>>,
}

impl File {
    pub fn new(base: Base, path: impl Into<String>, contents: Zeroizing<Vec<u8>>) -> Result<File> {
        let file = File {
            base,
            path: path.into(),
            contents,
        };
        file.validate()?;
        Ok(file)
    }

    #[must_use]
    pub fn base(&self) -> Base {
        self.base
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    #[must_use]
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let path = Path::new(&self.path);
        let plain = !self.path.is_empty()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        match plain {
            true => Ok(()),
            false => Err(Error::Invalid(format!(
                "`{}` is not a plain relative path",
                self.path
            ))),
        }
    }
}

impl std::fmt::Debug for File {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("File")
            .field("base", &self.base)
            .field("path", &self.path)
            .field("len", &self.contents.len())
            .finish()
    }
}
