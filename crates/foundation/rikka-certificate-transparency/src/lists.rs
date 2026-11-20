use rikka_sync::RwLock;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::try_join;
use futures_util::lock::Mutex;
use jiff::{SignedDuration, Timestamp};
use rikka_net::{Bytes, Client, RetryPolicy};

use crate::error::{Error, Result};
use crate::list::LogList;
use crate::source::LogListSource;
use crate::store::Store;

const REFRESH_AGE: SignedDuration = SignedDuration::from_hours(24);
const FIRST_RETRY: SignedDuration = SignedDuration::from_mins(15);
const STALE_RETRY_CEILING: SignedDuration = SignedDuration::from_hours(1);
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LIST_BYTES: usize = 4 * 1024 * 1024;
const MAX_SIGNATURE_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListUpdate {
    NotDue,
    Unchanged,
    Updated,
}

#[derive(Clone)]
pub struct LogLists {
    shared: Arc<Shared>,
}

struct Shared {
    source: LogListSource,
    store: Option<Store>,
    client: Client,
    current: RwLock<Option<Arc<LogList>>>,
    attempts: Mutex<Attempts>,
}

#[derive(Default)]
struct Attempts {
    next: Option<Timestamp>,
    failures: u32,
}

struct Fetched {
    list: LogList,
    bytes: Bytes,
    signature: Bytes,
}

#[must_use]
pub struct LogListsBuilder {
    source: LogListSource,
    cache_dir: Option<PathBuf>,
}

impl LogListsBuilder {
    pub fn source(mut self, source: LogListSource) -> Self {
        self.source = source;
        self
    }

    pub fn cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(dir.into());
        self
    }

    pub fn build(self) -> LogLists {
        let store = self.cache_dir.map(|dir| Store::new(&dir));
        let current = store
            .as_ref()
            .and_then(|store| store.newer_than(self.source.key(), None))
            .map(Arc::new);
        LogLists {
            shared: Arc::new(Shared {
                source: self.source,
                store,
                client: Client::new(),
                current: RwLock::new(current),
                attempts: Mutex::new(Attempts::default()),
            }),
        }
    }
}

impl LogLists {
    pub fn builder() -> LogListsBuilder {
        LogListsBuilder {
            source: LogListSource::chrome(),
            cache_dir: None,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<Arc<LogList>> {
        self.shared.current.read().clone()
    }

    pub async fn fresh(&self, now: Timestamp) -> Result<Arc<LogList>> {
        if let Some(current) = self.current().filter(|current| current.is_fresh(now)) {
            return Ok(current);
        }
        if let Err(error) = self.refresh_if_due(now).await {
            tracing::warn!(%error, "failed to refresh stale CT log list");
        }
        match self.current() {
            Some(current) if current.is_fresh(now) => Ok(current),
            Some(current) => Err(Error::Expired(current.expires_at())),
            None => Err(Error::NotFetched),
        }
    }

    pub async fn refresh_if_due(&self, now: Timestamp) -> Result<ListUpdate> {
        let mut attempts = self.shared.attempts.lock().await;
        if !attempts.allows(now) || !is_due(self.current().as_deref(), now) {
            return Ok(ListUpdate::NotDue);
        }
        let adopted = self.adopt_cached();
        if adopted && !is_due(self.current().as_deref(), now) {
            return Ok(ListUpdate::Updated);
        }

        let fetched = match self.fetch().await {
            Ok(fetched) => fetched,
            Err(error) => {
                let fresh = self.current().is_some_and(|current| current.is_fresh(now));
                attempts.failed(
                    now,
                    if fresh {
                        REFRESH_AGE
                    } else {
                        STALE_RETRY_CEILING
                    },
                );
                return Err(error);
            }
        };
        attempts.succeeded(now);
        if !is_newer(&fetched.list, self.current().as_deref()) {
            return Ok(if adopted {
                ListUpdate::Updated
            } else {
                ListUpdate::Unchanged
            });
        }
        if let Some(store) = &self.shared.store
            && let Err(error) = store.write(&fetched.bytes, &fetched.signature)
        {
            tracing::warn!(%error, "failed to cache CT log list");
        }
        self.replace(fetched.list);
        Ok(ListUpdate::Updated)
    }

    fn adopt_cached(&self) -> bool {
        let Some(store) = &self.shared.store else {
            return false;
        };
        match store.newer_than(self.shared.source.key(), self.current().as_deref()) {
            Some(cached) => {
                self.replace(cached);
                true
            }
            None => false,
        }
    }

    fn replace(&self, list: LogList) {
        *self.shared.current.write() = Some(Arc::new(list));
    }

    async fn fetch(&self) -> Result<Fetched> {
        let source = &self.shared.source;
        let (bytes, signature) = try_join(
            self.download(source.list_url(), MAX_LIST_BYTES),
            self.download(source.signature_url(), MAX_SIGNATURE_BYTES),
        )
        .await?;
        let list = LogList::from_signed(&bytes, &signature, source.key())?;
        Ok(Fetched {
            list,
            bytes,
            signature,
        })
    }

    async fn download(&self, url: &str, limit: usize) -> Result<Bytes> {
        let response = self
            .shared
            .client
            .get(url)
            .timeout(FETCH_TIMEOUT)
            .retry(RetryPolicy::never())
            .send()
            .await?
            .error_for_status()
            .await?;
        Ok(response.bytes_limited(limit).await?)
    }
}

impl fmt::Debug for LogLists {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let current = self.current();
        formatter
            .debug_struct("LogLists")
            .field("source", &self.shared.source.list_url())
            .field("version", &current.as_ref().map(|list| list.version()))
            .field("timestamp", &current.as_ref().map(|list| list.timestamp()))
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for LogListsBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LogListsBuilder")
            .field("source", &self.source.list_url())
            .field("cache_dir", &self.cache_dir)
            .finish()
    }
}

impl Attempts {
    fn allows(&self, now: Timestamp) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    fn succeeded(&mut self, now: Timestamp) {
        self.failures = 0;
        self.next = Some(later(now, REFRESH_AGE));
    }

    fn failed(&mut self, now: Timestamp, ceiling: SignedDuration) {
        self.next = Some(later(now, retry_delay(self.failures, ceiling)));
        self.failures = self.failures.saturating_add(1);
    }
}

fn is_due(current: Option<&LogList>, now: Timestamp) -> bool {
    current.is_none_or(|list| now > later(list.timestamp(), REFRESH_AGE))
}

pub(crate) fn is_newer(list: &LogList, current: Option<&LogList>) -> bool {
    current.is_none_or(|current| list.timestamp() > current.timestamp())
}

fn retry_delay(failures: u32, ceiling: SignedDuration) -> SignedDuration {
    2i32.checked_pow(failures)
        .and_then(|factor| FIRST_RETRY.checked_mul(factor))
        .map_or(ceiling, |delay| delay.min(ceiling))
}

fn later(at: Timestamp, by: SignedDuration) -> Timestamp {
    at.saturating_add(by)
        .expect("adding a signed duration to a timestamp cannot fail")
}
