use rikka_sync::Mutex;
use std::fmt;
use std::sync::Arc;

use futures_util::future::try_join_all;
use jiff::Timestamp;
use rikka_certificate_transparency::{ListUpdate, LogList, LogLists};
use rikka_proton_account::Account;
use rikka_proton_api::{Api, offload};
use rikka_proton_cache::{SINGLETON_ID, Table};
use rikka_proton_pgp::Pgp;

use crate::epoch::Epoch;
use crate::error::Result;
use crate::failure::Failure;
use crate::identifier::Identifier;
use crate::proof::Proof;
use crate::realm::Realm;
use crate::routes;
use crate::state::{AuditState, PendingKeyList};
use crate::timing::{LATEST_EPOCH_LIFETIME, is_past_certificate_lifetime, is_past_epoch_interval};

type Persist = Box<dyn Fn(&AuditState) + Send + Sync>;

const AUDIT_STATE: Table<AuditState> = Table::new("key-transparency.audit-state");

#[derive(Clone)]
pub struct KeyTransparency {
    shared: Arc<Shared>,
}

struct Shared {
    api: Api,
    pgp: Pgp,
    realm: Realm,
    log_lists: LogLists,
    state: Mutex<AuditState>,
    persist: Option<Persist>,
    latest: futures_util::lock::Mutex<Option<Latest>>,
}

#[derive(Clone)]
struct Latest {
    epoch: Epoch,
    fetched_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EpochCache {
    Reuse,
    Refresh,
}

#[must_use]
pub struct KeyTransparencyBuilder {
    api: Api,
    pgp: Pgp,
    realm: Realm,
    log_lists: Option<LogLists>,
    state: AuditState,
    persist: Option<Persist>,
}

impl KeyTransparencyBuilder {
    pub fn pgp(mut self, pgp: Pgp) -> Self {
        self.pgp = pgp;
        self
    }

    pub fn realm(mut self, realm: Realm) -> Self {
        self.realm = realm;
        self
    }

    pub fn log_lists(mut self, log_lists: LogLists) -> Self {
        self.log_lists = Some(log_lists);
        self
    }

    pub fn state(mut self, state: AuditState) -> Self {
        self.state = state;
        self
    }

    pub fn persist(mut self, persist: impl Fn(&AuditState) + Send + Sync + 'static) -> Self {
        self.persist = Some(Box::new(persist));
        self
    }

    pub fn build(self) -> KeyTransparency {
        KeyTransparency {
            shared: Arc::new(Shared {
                api: self.api,
                pgp: self.pgp,
                realm: self.realm,
                log_lists: self
                    .log_lists
                    .unwrap_or_else(|| LogLists::builder().build()),
                state: Mutex::new(self.state),
                persist: self.persist,
                latest: futures_util::lock::Mutex::new(None),
            }),
        }
    }
}

impl KeyTransparency {
    pub fn builder(api: Api) -> KeyTransparencyBuilder {
        KeyTransparencyBuilder {
            api,
            pgp: Pgp::new(),
            realm: Realm::production(),
            log_lists: None,
            state: AuditState::new(),
            persist: None,
        }
    }

    #[must_use]
    pub fn for_account(account: &Account, log_lists: LogLists) -> KeyTransparency {
        let builder = KeyTransparency::builder(account.api().clone())
            .pgp(*account.pgp())
            .log_lists(log_lists);
        let Some(cache) = account.cache().cloned() else {
            return builder.build();
        };
        let state = cache.get(&AUDIT_STATE, SINGLETON_ID).unwrap_or_default();
        builder
            .state(state)
            .persist(move |state| cache.put(&AUDIT_STATE, SINGLETON_ID, state.clone()))
            .build()
    }

    #[must_use]
    pub fn realm(&self) -> &Realm {
        &self.shared.realm
    }

    #[must_use]
    pub fn state(&self) -> AuditState {
        self.lock_state().clone()
    }

    #[must_use]
    pub fn log_lists(&self) -> &LogLists {
        &self.shared.log_lists
    }

    pub async fn is_available(&self) -> bool {
        self.log_list().await.is_ok()
    }

    pub async fn refresh_log_list(&self) -> Result<ListUpdate> {
        Ok(self
            .shared
            .log_lists
            .refresh_if_due(self.timestamp())
            .await?)
    }

    #[must_use]
    pub fn is_audit_due(&self) -> bool {
        self.lock_state().is_audit_due(self.now())
    }

    pub async fn latest_epoch(&self) -> Result<Epoch> {
        self.latest(EpochCache::Reuse).await
    }

    pub async fn epoch(&self, id: u32) -> Result<Epoch> {
        let log_list = self.log_list().await?;
        let epoch = routes::epoch(self.api(), id).await?;
        let claimed_time = epoch.claimed_time();
        if is_past_certificate_lifetime(claimed_time, self.now()) {
            return Err(Failure::ExpiredEpoch.into());
        }
        let (epoch, _) = self.verify_epoch(epoch, claimed_time, log_list).await?;
        Ok(epoch)
    }

    pub(crate) fn api(&self) -> &Api {
        &self.shared.api
    }

    pub(crate) fn pgp(&self) -> &Pgp {
        &self.shared.pgp
    }

    pub(crate) fn now(&self) -> u64 {
        self.shared.pgp.now()
    }

    pub(crate) async fn ensure_available(&self) -> Result<()> {
        self.log_list().await.map(|_| ())
    }

    pub(crate) async fn latest(&self, cache: EpochCache) -> Result<Epoch> {
        let log_list = self.log_list().await?;
        let mut latest_guard = self.shared.latest.lock().await;
        let now = self.now();
        if cache == EpochCache::Reuse
            && let Some(latest) = latest_guard.as_ref()
            && now < latest.fetched_at.saturating_add(LATEST_EPOCH_LIFETIME)
        {
            return Ok(latest.epoch.clone());
        }

        let epoch = routes::latest_epoch(self.api()).await?;
        let (epoch, certified_at) = self.verify_epoch(epoch, now, log_list).await?;
        if is_past_epoch_interval(certified_at, now) {
            return Err(Failure::StaleEpoch.into());
        }
        *latest_guard = Some(Latest {
            epoch: epoch.clone(),
            fetched_at: now,
        });
        Ok(epoch)
    }

    async fn log_list(&self) -> Result<Arc<LogList>> {
        Ok(self.shared.log_lists.fresh(self.timestamp()).await?)
    }

    fn timestamp(&self) -> Timestamp {
        i64::try_from(self.now())
            .ok()
            .and_then(|seconds| Timestamp::from_second(seconds).ok())
            .unwrap_or(Timestamp::MAX)
    }

    async fn verify_epoch(
        &self,
        epoch: Epoch,
        time: u64,
        log_list: Arc<LogList>,
    ) -> Result<(Epoch, u64)> {
        let (epoch, verified) = self.verify_with(epoch, time, log_list).await;
        match verified {
            Err(Failure::InsufficientTimestamps) if self.refreshed_log_list().await => {
                let (epoch, verified) = self.verify_with(epoch, time, self.log_list().await?).await;
                Ok((epoch, verified?))
            }
            verified => Ok((epoch, verified?)),
        }
    }

    async fn verify_with(
        &self,
        epoch: Epoch,
        time: u64,
        log_list: Arc<LogList>,
    ) -> (Epoch, Result<u64, Failure>) {
        let realm = self.shared.realm.clone();
        offload(move || {
            let verified = epoch.verify(&realm, &log_list, time);
            (epoch, verified)
        })
        .await
    }

    async fn refreshed_log_list(&self) -> bool {
        match self.refresh_log_list().await {
            Ok(update) => update == ListUpdate::Updated,
            Err(error) => {
                tracing::warn!(%error, "failed to refresh CT log list");
                false
            }
        }
    }

    pub(crate) async fn proofs(
        &self,
        epoch: &Epoch,
        identifier: &Identifier,
        revisions: impl IntoIterator<Item = u32>,
    ) -> Result<Vec<Proof>> {
        try_join_all(
            revisions
                .into_iter()
                .map(|revision| routes::proof(self.api(), epoch.id(), identifier, revision)),
        )
        .await
    }

    pub(crate) async fn proofs_in_latest(
        &self,
        identifier: &Identifier,
        revisions: &[u32],
    ) -> Result<(Epoch, Vec<Proof>)> {
        let epoch = self.latest(EpochCache::Reuse).await?;
        match self
            .proofs(&epoch, identifier, revisions.iter().copied())
            .await
        {
            Err(error) if error.is_unprocessable() => {
                let epoch = self.latest(EpochCache::Refresh).await?;
                let proofs = self
                    .proofs(&epoch, identifier, revisions.iter().copied())
                    .await?;
                Ok((epoch, proofs))
            }
            response => response.map(|proofs| (epoch, proofs)),
        }
    }

    pub(crate) fn add_pending(&self, pending: PendingKeyList) {
        let mut state = self.lock_state();
        if state.add_pending(pending) {
            self.persist(&state);
        }
    }

    pub(crate) fn update_state(&self, change: impl FnOnce(&mut AuditState)) {
        let mut state = self.lock_state();
        change(&mut state);
        self.persist(&state);
    }

    fn persist(&self, state: &AuditState) {
        if let Some(persist) = &self.shared.persist {
            persist(state);
        }
    }

    fn lock_state(&self) -> rikka_sync::MutexGuard<'_, AuditState> {
        self.shared.state.lock()
    }
}

impl fmt::Debug for KeyTransparency {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyTransparency")
            .field("realm", &self.shared.realm)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for KeyTransparencyBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyTransparencyBuilder")
            .field("realm", &self.realm)
            .finish_non_exhaustive()
    }
}
