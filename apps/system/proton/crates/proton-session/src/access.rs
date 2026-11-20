use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_proton_account::{AccountKeys, LoginOutcome};
use rikka_proton_api::{Environment, offload};
use rikka_proton_client::{Proton, Renewal, Session};

use crate::error::{Error, Result};
use crate::renewed::Renewed;
use crate::step::{MailboxKey, SecondFactor, Step};
use crate::store::{AccountStore, SavedAccount};
use crate::write_gate::{WriteGate, WriteTicket};

pub const APP_VERSION: &str = "cli-pass@2.4.1";

#[derive(Clone, Debug)]
pub struct Access {
    proton: Proton,
    store: AccountStore,
    stay_signed_in: Arc<AtomicBool>,
    writes: Arc<WriteGate>,
}

impl Access {
    pub fn open(store: AccountStore) -> Result<Access> {
        let proton = Proton::builder()
            .app_version(APP_VERSION)
            .environment(Environment::production())
            .session_store(store.sessions())
            .alternative_routing(true)
            .log_lists(store.log_lists())
            .build()?;

        Ok(Access::with_proton(proton, store))
    }

    #[must_use]
    pub fn with_proton(proton: Proton, store: AccountStore) -> Access {
        Access {
            proton,
            store,
            stay_signed_in: Arc::new(AtomicBool::new(true)),
            writes: Arc::default(),
        }
    }

    #[must_use]
    pub fn store(&self) -> &AccountStore {
        &self.store
    }

    pub fn set_stay_signed_in(&self, stay: bool) {
        self.stay_signed_in.store(stay, Ordering::Relaxed);
    }

    fn stays_signed_in(&self) -> bool {
        self.stay_signed_in.load(Ordering::Relaxed)
    }

    fn cached(&self) -> Proton {
        self.proton.clone().with_cache(self.store.cache())
    }

    pub fn keep_key(&self, session: &Session) -> impl Future<Output = ()> + Send + 'static {
        self.persist(self.writes.generation(), session, true)
    }

    pub fn drop_key(&self, session: &Session) -> impl Future<Output = ()> + Send + 'static {
        self.persist(self.writes.generation(), session, false)
    }

    pub async fn warm_up(&self) {
        self.proton.account().api().warm_up().await;
    }

    pub async fn resume(&self) -> Result<Resumed> {
        let generation = self.writes.generation();
        let store = self.store.clone();
        let proton = self.cached();
        let recalling = proton.clone();
        let (saved_account, profile) =
            offload(move || (store.saved_account(), recalling.account().cached_profile())).await;
        let Some(saved_account) = saved_account else {
            return Ok(Resumed::SignedOut);
        };
        if self.proton.account().api().session().is_none() {
            return Ok(Resumed::SignedOut);
        }
        let Some(passphrase) = saved_account.passphrase else {
            return Ok(Resumed::Locked(saved_account.username));
        };

        if let Some(profile) = profile {
            match proton.resume(profile, passphrase.as_bytes()).await {
                Ok(session) => return Ok(Resumed::Cached(Box::new(session))),
                Err(error) => {
                    tracing::warn!(%error, "failed to open cached session")
                }
            }
        }

        match proton.unlock_with_passphrase(passphrase.as_bytes()).await {
            Ok(session) => {
                self.persist(generation, &session, self.stays_signed_in())
                    .await;
                Ok(Resumed::SignedIn(Box::new(session)))
            }
            Err(error) => self.signed_out(Error::from(error)).await,
        }
    }

    pub fn renew(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<Renewed>> + Send + 'static {
        let generation = self.writes.generation();
        let access = self.clone();
        let session = session.clone();
        async move { access.renewed(generation, &session).await }
    }

    async fn renewed(&self, generation: u64, session: &Session) -> Result<Renewed> {
        let renewal = match session.renew().await {
            Ok(renewal) => renewal,
            Err(error) => {
                let error = Error::from(error);
                if !error.is_signed_out() {
                    return Err(error);
                }
                self.forget().await;
                return Ok(Renewed::SignedOut);
            }
        };

        Ok(self.adopted(generation, renewal).await)
    }

    pub fn adopt(&self, renewal: Renewal) -> impl Future<Output = Renewed> + Send + 'static {
        let generation = self.writes.generation();
        let access = self.clone();
        async move { access.adopted(generation, renewal).await }
    }

    async fn adopted(&self, generation: u64, renewal: Renewal) -> Renewed {
        match renewal {
            Renewal::Unchanged => Renewed::Current,
            Renewal::Updated(next) => {
                self.persist(generation, &next, self.stays_signed_in())
                    .await;
                Renewed::ProfileChanged(next)
            }
            Renewal::Rekeyed(next) => {
                self.persist(generation, &next, self.stays_signed_in())
                    .await;
                Renewed::KeysChanged(next)
            }
        }
    }

    pub async fn unlock(&self, password: impl AsRef<str>) -> Result<Session> {
        let generation = self.writes.generation();
        let session = self.cached().unlock(password).await?;
        self.persist(generation, &session, self.stays_signed_in())
            .await;
        Ok(session)
    }

    pub async fn sign_in(
        &self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Step> {
        let outcome = self.proton.sign_in(username, password).await?;
        self.advance(outcome).await
    }

    pub async fn sign_out(&self, session: &Session) -> Result<()> {
        let outcome = session.sign_out().await;
        self.forget().await;
        Ok(outcome?)
    }

    pub fn forget(&self) -> impl Future<Output = ()> + Send + 'static {
        self.writes.invalidate();
        let store = self.store.clone();
        let writes = Arc::clone(&self.writes);
        async move { offload(move || writes.exclusive(|| store.forget())).await }
    }

    pub(crate) async fn advance(&self, outcome: LoginOutcome) -> Result<Step> {
        match outcome {
            LoginOutcome::Unlocked(unlocked) => Ok(Step::Signed(Box::new(
                self.finish_sign_in(unlocked.into_keys()).await,
            ))),
            LoginOutcome::TwoFactorRequired(pending) => Ok(Step::SecondFactor(Box::new(
                SecondFactor::new(self.clone(), *pending),
            ))),
            LoginOutcome::MailboxPasswordRequired(pending) => Ok(Step::MailboxPassword(Box::new(
                MailboxKey::new(self.clone(), *pending),
            ))),
            LoginOutcome::Sso(sso) => Err(Error::IdentityProvider(sso.username().to_owned())),
            _ => Err(Error::UnsupportedSignIn),
        }
    }

    pub(crate) async fn finish_sign_in(&self, keys: AccountKeys) -> Session {
        let session = self.cached().unlocked(keys);
        self.persist(self.writes.generation(), &session, self.stays_signed_in())
            .await;
        session
    }

    fn persist(
        &self,
        generation: u64,
        session: &Session,
        keep_passphrase: bool,
    ) -> impl Future<Output = ()> + Send + 'static {
        let ticket = self.writes.ticket(generation);
        let keys = session.keys();
        let saved_account = SavedAccount {
            username: account_name(keys),
            passphrase: keep_passphrase
                .then(|| keys.fork_payload())
                .flatten()
                .and_then(|payload| payload.key_password().map(str::to_owned)),
        };
        let store = self.store.clone();
        let writes = Arc::clone(&self.writes);

        async move {
            let saved =
                offload(move || save_if_current(&writes, ticket, &store, &saved_account)).await;
            if let Err(error) = saved {
                tracing::warn!(%error, "failed to save account");
            }
        }
    }

    async fn signed_out(&self, error: Error) -> Result<Resumed> {
        if !error.is_signed_out() {
            return Err(error);
        }
        self.forget().await;
        Ok(Resumed::SignedOut)
    }
}

#[derive(Debug)]
pub enum Resumed {
    Cached(Box<Session>),
    SignedIn(Box<Session>),
    Locked(String),
    SignedOut,
}

fn account_name(keys: &AccountKeys) -> String {
    let user = keys.user();
    user.name()
        .or_else(|| user.email())
        .unwrap_or_default()
        .to_owned()
}

fn save_if_current(
    writes: &WriteGate,
    ticket: WriteTicket,
    store: &AccountStore,
    saved_account: &SavedAccount,
) -> Result<()> {
    let saved = writes.write_if_current(ticket, || {
        store.save_account(saved_account)?;
        Ok(store.cache().flush()?)
    });
    saved.unwrap_or(Ok(()))
}
