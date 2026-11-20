use rikka_certificate_transparency::LogLists;
use rikka_proton_account::{Account, AccountKeys, LoginOutcome, Profile};
use rikka_proton_api::Api;
use rikka_proton_cache::Cache;
use rikka_proton_pgp::{Pgp, SecretBytes};

use crate::builder::ProtonBuilder;
use crate::error::Result;
use crate::session::Session;

#[derive(Debug, Clone)]
pub struct Proton {
    account: Account,
    log_lists: LogLists,
}

impl Proton {
    pub fn builder() -> ProtonBuilder {
        ProtonBuilder::new()
    }

    #[must_use]
    pub fn new(api: Api) -> Proton {
        Proton {
            account: Account::new(api),
            log_lists: LogLists::builder().build(),
        }
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp) -> Proton {
        Proton::assembled(api, pgp, LogLists::builder().build())
    }

    pub(crate) fn assembled(api: Api, pgp: Pgp, log_lists: LogLists) -> Proton {
        Proton {
            account: Account::with_pgp(api, pgp),
            log_lists,
        }
    }

    #[must_use]
    pub fn with_cache(self, cache: Cache) -> Proton {
        Proton {
            account: self.account.with_cache(cache),
            log_lists: self.log_lists,
        }
    }

    #[must_use]
    pub fn log_lists(&self) -> &LogLists {
        &self.log_lists
    }

    #[must_use]
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub async fn sign_in(
        &self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<LoginOutcome> {
        Ok(self
            .account
            .login()
            .username(username)
            .password(password)
            .execute()
            .await?)
    }

    pub async fn unlock(&self, password: impl AsRef<str>) -> Result<Session> {
        let keys = self.account.unlock(password).await?;
        Ok(Session::new(self.clone(), keys))
    }

    pub async fn unlock_with_passphrase(&self, passphrase: impl AsRef<[u8]>) -> Result<Session> {
        let keys = self.account.unlock_with_passphrase(passphrase).await?;
        Ok(Session::new(self.clone(), keys))
    }

    pub async fn resume(&self, profile: Profile, passphrase: impl AsRef<[u8]>) -> Result<Session> {
        let passphrase = SecretBytes::new(passphrase.as_ref());
        let keys = self
            .account
            .unlock_profile(profile, Some(passphrase))
            .await?;
        Ok(Session::new(self.clone(), keys))
    }

    pub async fn resume_cached(&self, passphrase: impl AsRef<[u8]>) -> Result<Session> {
        match self.account.cached_profile() {
            Some(profile) => self.resume(profile, passphrase).await,
            None => self.unlock_with_passphrase(passphrase).await,
        }
    }

    #[must_use]
    pub fn unlocked(&self, keys: AccountKeys) -> Session {
        if self.account.cache().is_some() {
            self.account.cache_profile(&keys.profile());
        }
        Session::new(self.clone(), keys)
    }

    pub async fn sign_out(&self) -> Result<()> {
        self.account.revoke_session().await?;
        Ok(())
    }
}
