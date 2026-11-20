use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rikka_net::Client;
use rikka_sync::RwLock;

use crate::doh::Resolver;
use crate::environment::Environment;

pub(crate) const VALIDITY: Duration = Duration::from_secs(90 * 60);
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);
pub(crate) const ALTERNATIVES_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) struct Routing {
    resolver: Resolver,
    client: Client,
    validity: Duration,
    refreshing: tokio::sync::Mutex<()>,
    state: RwLock<State>,
}

#[derive(Debug, Default)]
struct State {
    active: Option<Active>,
    alternatives: Vec<Environment>,
    refreshed: Option<Instant>,
}

#[derive(Debug)]
struct Active {
    base: Environment,
    since: Instant,
}

impl Routing {
    pub(crate) fn new(resolver: Resolver, client: Client, validity: Duration) -> Routing {
        Routing {
            resolver,
            client,
            validity,
            refreshing: tokio::sync::Mutex::new(()),
            state: RwLock::new(State::default()),
        }
    }

    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) fn active(&self) -> Option<Environment> {
        {
            let state = self.state.read();
            let active = state.active.as_ref()?;
            if active.since.elapsed() < self.validity {
                return Some(active.base.clone());
            }
        }

        let mut state = self.state.write();
        if state
            .active
            .as_ref()
            .is_some_and(|active| active.since.elapsed() >= self.validity)
        {
            state.active = None;
        }
        None
    }

    pub(crate) fn set_active(&self, base: Environment) {
        self.state.write().active = Some(Active {
            base,
            since: Instant::now(),
        });
    }

    pub(crate) fn invalidate(&self, base: &Environment) {
        let mut state = self.state.write();
        if state
            .active
            .as_ref()
            .is_some_and(|active| &active.base == base)
        {
            state.active = None;
        }
    }

    pub(crate) async fn alternatives(
        &self,
        primary: &Environment,
        session: Option<&str>,
    ) -> Vec<Environment> {
        let _refresh_guard = self.refreshing.lock().await;

        if self.is_stale() {
            let hosts = self.resolver.resolve(primary.host(), session).await;
            let routes: Vec<_> = hosts
                .iter()
                .filter_map(|host| primary.with_host(host).ok())
                .collect();

            let mut state = self.state.write();
            state.refreshed = Some(Instant::now());
            if !routes.is_empty() {
                tracing::info!(routes = routes.len(), "resolved alternative routes");
                state.alternatives = routes;
            }
        }

        let mut alternatives = self.state.read().alternatives.clone();
        alternatives.shuffle(&mut rand::rng());
        alternatives
    }

    fn is_stale(&self) -> bool {
        let state = self.state.read();
        state.alternatives.is_empty()
            || state
                .refreshed
                .is_none_or(|refreshed| refreshed.elapsed() >= MIN_REFRESH_INTERVAL)
    }
}
