use std::path::Path;

use rikka_secrets::{Entry, Secrets};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::Result;

pub const FOLDER_ATTRIBUTE: &str = "folder";
const KIND: &str = "kind";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vault {
    schema: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderSecret {
    entry: Entry,
}

impl Vault {
    #[must_use]
    pub const fn new(schema: &'static str) -> Vault {
        Vault { schema }
    }

    #[must_use]
    pub fn secret(self, folder: &Path) -> FolderSecret {
        FolderSecret {
            entry: self.folder(folder),
        }
    }

    pub(crate) fn forget(self, folder: &Path) -> Result<()> {
        rikka_secrets::blocking::Secrets::shared()?.remove(&self.folder(folder))?;
        Ok(())
    }

    fn folder(self, folder: &Path) -> Entry {
        Entry::new(self.schema).attribute(FOLDER_ATTRIBUTE, folder.to_string_lossy())
    }
}

impl FolderSecret {
    #[must_use]
    pub fn kind(self, kind: &str) -> FolderSecret {
        FolderSecret {
            entry: self.entry.attribute(KIND, kind),
        }
    }

    #[must_use]
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    pub async fn keep<T: Serialize>(&self, label: &str, value: &T) -> Result<()> {
        Secrets::unlocked()
            .await?
            .store_json(&self.entry, label, value)
            .await?;
        Ok(())
    }

    pub async fn renew<T: Serialize>(&self, label: &str, value: &T) -> Result<()> {
        Secrets::shared()
            .await?
            .store_json(&self.entry, label, value)
            .await?;
        Ok(())
    }

    pub async fn kept<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        Ok(Secrets::unlocked().await?.lookup_json(&self.entry).await?)
    }
}
