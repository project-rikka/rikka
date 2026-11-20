use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rikka_proton_cache::{Cache, Table};
use rikka_proton_pgp::PublicKey;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::error::Result;

pub(crate) type Verifiers = Arc<[PublicKey]>;

const MAX_ENTRIES: usize = 512;
const TTL: Duration = Duration::from_secs(15 * 60);
const KEPT_VERIFIERS: usize = 4_096;
const VERIFIERS: Table<CachedVerifiers> = Table::bounded("contacts.verifiers", KEPT_VERIFIERS);

type VerifierCell = OnceCell<(Instant, Verifiers)>;

#[derive(Default)]
pub(crate) struct KnownVerifiers {
    memo: Mutex<VerifierMemo>,
}

#[derive(Default)]
struct VerifierMemo {
    entries: HashMap<String, MemoEntry>,
    use_counter: u64,
}

struct MemoEntry {
    cell: Arc<VerifierCell>,
    last_used: u64,
}

impl KnownVerifiers {
    pub(crate) async fn get_or_fetch<Fut>(
        &self,
        email: &str,
        fetch: impl FnOnce() -> Fut,
    ) -> Result<Verifiers>
    where
        Fut: Future<Output = Result<Verifiers>>,
    {
        let cell = self.memo().cell_for(email);
        let (_, verifiers) = cell
            .get_or_try_init(|| async { fetch().await.map(|keys| (Instant::now(), keys)) })
            .await?;

        Ok(Arc::clone(verifiers))
    }

    fn memo(&self) -> MutexGuard<'_, VerifierMemo> {
        self.memo.lock()
    }
}

impl VerifierMemo {
    fn cell_for(&mut self, email: &str) -> Arc<VerifierCell> {
        self.use_counter += 1;
        let last_used = self.use_counter;
        let entry = self
            .entries
            .entry(email.to_owned())
            .or_insert_with(|| MemoEntry {
                cell: Arc::default(),
                last_used,
            });
        entry.last_used = last_used;
        if entry
            .cell
            .get()
            .is_some_and(|(fetched_at, _)| fetched_at.elapsed() >= TTL)
        {
            entry.cell = Arc::default();
        }
        let cell = Arc::clone(&entry.cell);
        self.evict_least_recently_used();
        cell
    }

    fn evict_least_recently_used(&mut self) {
        while self.entries.len() > MAX_ENTRIES {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(email, _)| email.clone())
            else {
                return;
            };
            self.entries.remove(&oldest);
        }
    }
}

impl fmt::Debug for KnownVerifiers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KnownVerifiers")
            .field("emails", &self.memo().entries.len())
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedVerifiers {
    fetched_at: u64,
    keys: Vec<String>,
}

pub(crate) fn cached_verifiers(cache: &Cache, email: &str) -> Option<Verifiers> {
    let cached = cache.get(&VERIFIERS, email)?;
    if unix_now().saturating_sub(cached.fetched_at) >= TTL.as_secs() {
        cache.remove(&VERIFIERS, email);
        return None;
    }
    cached
        .keys
        .iter()
        .map(|key| {
            let bytes = rikka_proton_api::base64::decode(key).ok()?;
            PublicKey::import(bytes).ok()
        })
        .collect::<Option<Vec<PublicKey>>>()
        .map(Verifiers::from)
}

pub(crate) fn store_verifiers(cache: &Cache, email: &str, keys: &[PublicKey]) {
    let exported = match keys
        .iter()
        .map(|key| key.export().map(rikka_proton_api::base64::encode))
        .collect::<rikka_proton_pgp::Result<Vec<String>>>()
    {
        Ok(exported) => exported,
        Err(error) => {
            tracing::warn!(%email, %error, "failed to export verification keys for cache");
            return;
        }
    };
    cache.put(
        &VERIFIERS,
        email,
        CachedVerifiers {
            fetched_at: unix_now(),
            keys: exported,
        },
    );
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}
