use rikka_account_book::{Record, Vault};
use rikka_secrets::schema;
use serde::{Deserialize, Serialize};

use crate::provider::Account;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedAccount {
    pub key: String,
    pub provider: String,
    pub account: Account,
}

impl Record for SavedAccount {
    const VAULT: Vault = Vault::new(schema::MUSIC);

    fn key(&self) -> &str {
        &self.key
    }

    fn is_same_account(&self, other: &SavedAccount) -> bool {
        self.provider == other.provider && self.account.id == other.account.id
    }
}
