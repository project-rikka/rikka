use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{Stream, future};
use rikka_llm::ModelInfo;
use rikka_llm_providers::Backend;

use crate::account::Account;
use crate::account_store::AccountStore;

const CHECK_EVERY: Duration = Duration::from_secs(15 * 60);
const RETRY_AFTER: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug)]
pub(crate) struct Listed {
    pub(crate) backend: Backend,
    pub(crate) who: String,
    pub(crate) models: Vec<ModelInfo>,
    pub(crate) at: u64,
}

struct Follower {
    store: AccountStore,
    failed: HashMap<Backend, Instant>,
    ready: VecDeque<Listed>,
    checked: bool,
}

pub(crate) fn follow(store: AccountStore) -> impl Stream<Item = Listed> + Send + 'static {
    let follower = Follower {
        store,
        failed: HashMap::new(),
        ready: VecDeque::new(),
        checked: false,
    };
    futures_util::stream::unfold(follower, |mut follower| async move {
        loop {
            if let Some(listed) = follower.ready.pop_front() {
                return Some((listed, follower));
            }
            if follower.checked {
                follower.store.listing_woken(CHECK_EVERY).await;
            }
            follower.checked = true;
            follower.check().await;
        }
    })
}

impl Follower {
    async fn check(&mut self) {
        let store = self.store.clone();
        let loaded = match tokio::task::spawn_blocking(move || store.load()).await {
            Ok(loaded) => loaded,
            Err(error) => {
                tracing::warn!("failed to read model accounts for their listings: {error}");
                return;
            }
        };
        let now = unix_now();
        let due: Vec<(Backend, &Account)> = loaded
            .accounts
            .iter()
            .filter(|account| account.is_listing_due(now))
            .filter_map(|account| Some((account.backend()?, account)))
            .filter(|(backend, _)| self.may_ask(*backend))
            .collect();
        let renewal = self.store.renewal();
        let answers = future::join_all(
            due.iter()
                .map(|(backend, account)| backend.listed(account.credential(), &renewal)),
        )
        .await;

        for ((backend, account), answer) in due.into_iter().zip(answers) {
            match answer {
                Ok(models) => {
                    self.failed.remove(&backend);
                    self.ready.push_back(Listed {
                        backend,
                        who: account.identity(),
                        models,
                        at: now,
                    });
                }
                Err(error) => {
                    tracing::warn!("failed to list the models of {}: {error}", backend.label());
                    self.failed.insert(backend, Instant::now());
                }
            }
        }
    }

    fn may_ask(&self, backend: Backend) -> bool {
        self.failed
            .get(&backend)
            .is_none_or(|failed| failed.elapsed() >= RETRY_AFTER)
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
