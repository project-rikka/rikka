use std::io::ErrorKind;
use std::path::Path;

use rikka_secrets::{Entry, Secrets, schema};
use serde::{Deserialize, Serialize};

use crate::authenticator::{Authenticator, Session};
use crate::error::{Error, Result};
use crate::secret::SecretText;

const KIND: &str = "kind";
const ACCOUNT: &str = "account";
const ID: &str = "id";
const RENEW_BEFORE: i64 = 5 * 60;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub xuid: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accounts {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    accounts: Vec<Account>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    access_token: SecretText,
    refresh_token: SecretText,
    expires_at: i64,
    name: String,
    xuid: String,
}

fn entry(id: &str) -> Entry {
    Entry::new(schema::MINECRAFT)
        .attribute(KIND, ACCOUNT)
        .attribute(ID, id)
}

impl Accounts {
    pub fn load(path: &Path) -> Result<Accounts> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| Error::Load(std::io::Error::new(ErrorKind::InvalidData, error))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Accounts::default()),
            Err(error) => Err(Error::Load(error)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| Error::Save(std::io::Error::other(error)))?;
        rikka_fs::replace(path, &bytes).map_err(Error::Save)
    }

    #[must_use]
    pub fn list(&self) -> &[Account] {
        &self.accounts
    }

    #[must_use]
    pub fn active(&self) -> Option<&Account> {
        let active = self.active.as_deref()?;
        self.accounts.iter().find(|account| account.id == active)
    }

    pub fn select(&mut self, id: &str) {
        if self.accounts.iter().any(|account| account.id == id) {
            self.active = Some(id.to_owned());
        }
    }

    pub fn remember(&mut self, session: &Session) {
        let account = Account {
            id: session.id.clone(),
            name: session.name.clone(),
            xuid: session.xuid.clone(),
        };
        match self
            .accounts
            .iter_mut()
            .find(|known| known.id == account.id)
        {
            Some(known) => *known = account,
            None => self.accounts.push(account),
        }
        self.active = Some(session.id.clone());
    }

    pub fn forget(&mut self, id: &str) -> Option<Account> {
        let index = self.accounts.iter().position(|account| account.id == id)?;
        let removed = self.accounts.remove(index);
        if self.active.as_deref() == Some(id) {
            self.active = self.accounts.first().map(|account| account.id.clone());
        }
        Some(removed)
    }
}

pub async fn keep(session: &Session) -> Result<()> {
    let stored = Stored {
        access_token: SecretText(session.access_token.clone()),
        refresh_token: SecretText(session.refresh_token.clone()),
        expires_at: session.expires_at,
        name: session.name.clone(),
        xuid: session.xuid.clone(),
    };
    let label = format!("Minecraft account {}", session.name);
    Secrets::unlocked()
        .await?
        .store_json(&entry(&session.id), &label, &stored)
        .await?;
    Ok(())
}

pub async fn forget(id: &str) -> Result<()> {
    Secrets::unlocked().await?.remove(&entry(id)).await?;
    Ok(())
}

pub async fn session(authenticator: &Authenticator, id: &str) -> Result<Session> {
    let stored: Stored = Secrets::unlocked()
        .await?
        .lookup_json(&entry(id))
        .await?
        .ok_or_else(|| Error::NotSignedIn(id.to_owned()))?;
    let session = Session {
        id: id.to_owned(),
        name: stored.name,
        xuid: stored.xuid,
        expires_at: stored.expires_at,
        access_token: stored.access_token.0,
        refresh_token: stored.refresh_token.0,
    };
    if !session.expires_within(RENEW_BEFORE) {
        return Ok(session);
    }
    let renewed = authenticator.renew(&session.refresh_token).await?;
    keep(&renewed).await?;
    Ok(renewed)
}
