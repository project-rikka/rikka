use std::path::Path;

use rikka_account_book::{FolderSecret, Record};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::accounts::SavedAccount;
use crate::error::Result;

pub async fn keep_secret<T: Serialize>(
    folder: &Path,
    kind: &str,
    label: &str,
    value: &T,
) -> Result<()> {
    Ok(secret(folder, kind).keep(label, value).await?)
}

pub async fn renew_secret<T: Serialize>(
    folder: &Path,
    kind: &str,
    label: &str,
    value: &T,
) -> Result<()> {
    Ok(secret(folder, kind).renew(label, value).await?)
}

pub async fn kept_secret<T: DeserializeOwned>(folder: &Path, kind: &str) -> Result<Option<T>> {
    Ok(secret(folder, kind).kept().await?)
}

fn secret(folder: &Path, kind: &str) -> FolderSecret {
    SavedAccount::VAULT.secret(folder).kind(kind)
}
