use std::path::Path;

use rikka_account_book::{FolderSecret, Record, Vault};
use rikka_secrets::schema;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::master::Master;
use crate::profile::Profile;

const MASTER: &str = "master";

pub const SEAL: &str = "seal";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedAccount {
    pub key: String,
    pub profile: Profile,
}

impl Record for SavedAccount {
    const VAULT: Vault = Vault::new(schema::GOOGLE);

    fn key(&self) -> &str {
        &self.key
    }

    fn is_same_account(&self, other: &SavedAccount) -> bool {
        self.profile.id == other.profile.id
    }
}

impl SavedAccount {
    #[must_use]
    pub fn secret(folder: &Path, kind: &str) -> FolderSecret {
        SavedAccount::VAULT.secret(folder).kind(kind)
    }

    pub async fn keep_master(folder: &Path, master: &Master) -> Result<()> {
        let label = format!("Google ({})", master.email());
        SavedAccount::secret(folder, MASTER)
            .keep(&label, master)
            .await?;
        Ok(())
    }

    pub async fn kept_master(folder: &Path) -> Result<Option<Master>> {
        Ok(SavedAccount::secret(folder, MASTER).kept().await?)
    }
}
