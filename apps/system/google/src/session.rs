use std::path::{Path, PathBuf};
use std::sync::Arc;

use google_account::{Endpoint, Grants, Master, Profile, SavedAccount};
use google_drive::{Drive, Shelf};
use google_mail::Mail;
use google_passwords::{Seal, Server, Vault};
use rikka_net::Client;

const DRIVE: &str = "drive";
const MAIL: &str = "mail";

#[derive(Clone, Debug)]
pub(crate) struct Roots {
    pub(crate) state: PathBuf,
    pub(crate) cache: PathBuf,
}

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) key: String,
    pub(crate) profile: Profile,
    pub(crate) http: Client,
    pub(crate) mail: Mail,
    pub(crate) drive: Drive,
    pub(crate) shelf: Shelf,
    pub(crate) vault: Vault,
    pub(crate) mail_cache: PathBuf,
}

#[derive(Debug)]
pub(crate) enum Failure {
    SignedOut,
    Other(String),
}

impl Roots {
    pub(crate) fn cache_of(&self, key: &str) -> PathBuf {
        self.cache.join(key)
    }
}

impl Session {
    pub(crate) async fn resume(
        saved: SavedAccount,
        folder: PathBuf,
        cache: PathBuf,
    ) -> Result<Session, Failure> {
        let master = SavedAccount::kept_master(&folder)
            .await
            .map_err(Failure::other)?
            .ok_or(Failure::SignedOut)?;
        Session::open(saved, &folder, cache, master).await
    }

    pub(crate) async fn open(
        saved: SavedAccount,
        folder: &Path,
        cache: PathBuf,
        master: Master,
    ) -> Result<Session, Failure> {
        let http = Client::new();
        let grants = Arc::new(Grants::new(Endpoint::google(), master));
        let seal = Seal::kept(folder).await.map_err(Failure::other)?;
        let vault = Vault::open(
            folder,
            Arc::clone(&grants),
            Server::google(http.clone()),
            seal,
        )
        .map_err(Failure::other)?;
        Ok(Session {
            key: saved.key,
            profile: saved.profile,
            mail: Mail::google(http.clone(), Arc::clone(&grants)),
            drive: Drive::google(http.clone(), grants),
            shelf: Shelf::new(cache.join(DRIVE)),
            vault,
            mail_cache: cache.join(MAIL),
            http,
        })
    }
}

impl Failure {
    pub(crate) fn other(error: impl std::fmt::Display) -> Failure {
        Failure::Other(error.to_string())
    }
}

pub(crate) async fn identified(oauth_token: String) -> Result<(Master, Profile), Failure> {
    let endpoint = Endpoint::google();
    let master = Master::sign_in(&endpoint, &oauth_token)
        .await
        .map_err(Failure::from)?;
    let grants = Grants::new(endpoint, master.clone());
    let profile = Profile::fetch(&Client::new(), &grants)
        .await
        .map_err(Failure::from)?;
    Ok((master, profile))
}

impl From<google_account::Error> for Failure {
    fn from(error: google_account::Error) -> Failure {
        match error.is_signed_out() {
            true => Failure::SignedOut,
            false => Failure::Other(error.to_string()),
        }
    }
}
