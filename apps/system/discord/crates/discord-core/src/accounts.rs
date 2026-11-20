use std::path::Path;

use rikka_account_book::{FolderSecret, Record, Vault};
use rikka_secrets::schema;
use serde::{Deserialize, Serialize};

use crate::id::UserId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: UserId,
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedAccount {
    pub key: String,
    pub account: Account,
}

impl Record for SavedAccount {
    const VAULT: Vault = Vault::new(schema::DISCORD);

    fn key(&self) -> &str {
        &self.key
    }

    fn is_same_account(&self, other: &SavedAccount) -> bool {
        self.account.id == other.account.id
    }
}

#[must_use]
pub fn account_secret(folder: &Path) -> FolderSecret {
    SavedAccount::VAULT.secret(folder)
}
