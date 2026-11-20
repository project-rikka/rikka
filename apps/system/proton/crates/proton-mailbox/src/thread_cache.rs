use rikka_sync::{Mutex, MutexGuard};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::{Arc, Weak};

use futures_util::FutureExt as _;
use futures_util::future::{BoxFuture, Shared};
use rikka_proton_mail::{Event, EventAction};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::opened::OpenedThread;
use crate::row::RowId;

const CAPACITY: usize = 48;

type PendingFetch = Shared<BoxFuture<'static, Result<Arc<OpenedThread>, Arc<Error>>>>;

#[derive(Clone, Default)]
pub struct ThreadCache {
    cache: Arc<Mutex<ReadingCache>>,
}

#[derive(Default)]
struct ReadingCache {
    entries: VecDeque<Arc<OpenedThread>>,
    pending: HashMap<RowId, (u64, PendingFetch)>,
    issued: u64,
}

impl ThreadCache {
    #[must_use]
    pub fn new() -> ThreadCache {
        ThreadCache::default()
    }

    #[must_use]
    pub fn get(&self, id: &RowId) -> Option<Arc<OpenedThread>> {
        self.cache().touch(id)
    }

    #[must_use]
    pub fn contains(&self, id: &RowId) -> bool {
        let cache = self.cache();
        cache.pending.contains_key(id) || cache.entries.iter().any(|reading| &reading.id == id)
    }

    pub async fn read<F>(&self, id: &RowId, fetch: F) -> Result<Arc<OpenedThread>>
    where
        F: Future<Output = Result<OpenedThread>> + Send + 'static,
    {
        let pending = {
            let mut cache = self.cache();
            if let Some(entries) = cache.touch(id) {
                return Ok(entries);
            }
            match cache.pending.get(id) {
                Some((_, pending)) => pending.clone(),
                None => self.start_fetch(&mut cache, id, fetch),
            }
        };

        pending.await.map_err(Error::Shared)
    }

    pub fn apply_event(&self, event: &Event) {
        if event.needs_refresh() {
            let mut cache = self.cache();
            cache.entries.clear();
            cache.pending.clear();
            return;
        }

        let mut cache = self.cache();
        let mut stale: Vec<RowId> = Vec::new();
        for change in event.messages() {
            if change.action() == EventAction::UpdateFlags {
                continue;
            }
            stale.push(RowId::Message(change.id().clone()));
            stale.extend(
                cache
                    .entries
                    .iter()
                    .filter(|reading| reading.message(change.id()).is_some())
                    .map(|reading| reading.id.clone()),
            );
            if let Some(conversation) = change.message().map(|message| message.conversation_id())
                && !conversation.as_str().is_empty()
            {
                stale.push(RowId::Conversation(conversation.clone()));
            }
        }
        for change in event.conversations() {
            let id = RowId::Conversation(change.id().clone());
            let unchanged = matches!(
                change.action(),
                EventAction::Update | EventAction::UpdateFlags
            ) && change.conversation().is_some_and(|conversation| {
                cache
                    .entries
                    .iter()
                    .find(|reading| reading.id == id)
                    .is_some_and(|reading| reading.thread == conversation.messages())
            });
            if !unchanged {
                stale.push(id);
            }
        }
        cache.forget(&stale);
    }

    pub fn forget(&self, ids: &[RowId]) {
        self.cache().forget(ids);
    }

    fn start_fetch<F>(&self, cache: &mut ReadingCache, id: &RowId, fetch: F) -> PendingFetch
    where
        F: Future<Output = Result<OpenedThread>> + Send + 'static,
    {
        cache.issued += 1;
        let issued = cache.issued;
        let home = Arc::downgrade(&self.cache);
        let key = id.clone();
        let work = async move {
            let answer = fetch.await.map(Arc::new).map_err(Arc::new);
            fetch_finished(&home, &key, issued, &answer);
            answer
        };
        let pending = match Handle::try_current() {
            Ok(runtime) => spawn_fetch(runtime.spawn(work)).boxed().shared(),
            Err(_) => work.boxed().shared(),
        };
        cache.pending.insert(id.clone(), (issued, pending.clone()));
        pending
    }

    fn cache(&self) -> MutexGuard<'_, ReadingCache> {
        self.cache.lock()
    }
}

async fn spawn_fetch(
    running: JoinHandle<Result<Arc<OpenedThread>, Arc<Error>>>,
) -> Result<Arc<OpenedThread>, Arc<Error>> {
    match running.await {
        Ok(answer) => answer,
        Err(failure) => match failure.try_into_panic() {
            Ok(panic) => std::panic::resume_unwind(panic),
            Err(_) => Err(Arc::new(Error::Abandoned)),
        },
    }
}

fn fetch_finished(
    home: &Weak<Mutex<ReadingCache>>,
    id: &RowId,
    serial: u64,
    answer: &Result<Arc<OpenedThread>, Arc<Error>>,
) {
    let Some(home) = home.upgrade() else {
        return;
    };
    let mut cache = home.lock();
    if cache
        .pending
        .get(id)
        .is_none_or(|(issued, _)| *issued != serial)
    {
        return;
    }
    cache.pending.remove(id);
    if let Ok(reading) = answer {
        cache.insert(Arc::clone(reading));
    }
}

impl ReadingCache {
    fn touch(&mut self, id: &RowId) -> Option<Arc<OpenedThread>> {
        let at = self.entries.iter().position(|reading| &reading.id == id)?;
        let reading = self.entries.remove(at)?;
        self.entries.push_back(Arc::clone(&reading));
        Some(reading)
    }

    fn insert(&mut self, reading: Arc<OpenedThread>) {
        self.entries.retain(|cached| cached.id != reading.id);
        self.entries.push_back(reading);
        while self.entries.len() > CAPACITY {
            self.entries.pop_front();
        }
    }

    fn forget(&mut self, ids: &[RowId]) {
        self.entries.retain(|reading| !ids.contains(&reading.id));
        for id in ids {
            self.pending.remove(id);
        }
    }
}

impl fmt::Debug for ThreadCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cache = self.cache();
        formatter
            .debug_struct("ThreadCache")
            .field("entries", &cache.entries.len())
            .field("arriving", &cache.pending.len())
            .finish()
    }
}
