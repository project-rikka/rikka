use std::path::PathBuf;
use std::time::Duration;

use rikka_certificate_transparency::LogLists;
use rikka_proton_api::{Api, ApiBuilder, Environment, Session, SessionStore};
use rikka_proton_pgp::{Clock, Pgp, SymmetricKey};

use crate::accounts::Accounts;
use crate::client::Proton;
use crate::error::Result;

const LOG_LISTS: &str = "certificate-transparency";

#[must_use]
#[derive(Debug)]
pub struct ProtonBuilder {
    api: ApiBuilder,
    clock: Option<Clock>,
    log_lists: Option<LogLists>,
}

impl ProtonBuilder {
    pub(crate) fn new() -> ProtonBuilder {
        ProtonBuilder {
            api: Api::builder(),
            clock: None,
            log_lists: None,
        }
    }

    pub fn app_version(mut self, version: impl AsRef<str>) -> Self {
        self.api = self.api.app_version(version);
        self
    }

    pub fn environment(mut self, environment: Environment) -> Self {
        self.api = self.api.environment(environment);
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.api = self.api.user_agent(user_agent);
        self
    }

    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.api = self.api.locale(locale);
        self
    }

    pub fn session(mut self, session: Session) -> Self {
        self.api = self.api.session(session);
        self
    }

    pub fn session_store(mut self, store: impl SessionStore) -> Self {
        self.api = self.api.session_store(store);
        self
    }

    pub fn alternative_routing(mut self, enabled: bool) -> Self {
        self.api = self.api.alternative_routing(enabled);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.api = self.api.timeout(timeout);
        self
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.api = self.api.retries(retries);
        self
    }

    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = Some(clock);
        self
    }

    pub fn log_lists(mut self, log_lists: LogLists) -> Self {
        self.log_lists = Some(log_lists);
        self
    }

    pub fn build(mut self) -> Result<Proton> {
        let log_lists = self
            .log_lists
            .take()
            .unwrap_or_else(|| LogLists::builder().build());
        self.build_with(log_lists)
    }

    pub async fn accounts(
        mut self,
        root: impl Into<PathBuf>,
        key: SymmetricKey,
    ) -> Result<Accounts> {
        let root = root.into();
        let log_lists = self
            .log_lists
            .take()
            .unwrap_or_else(|| LogLists::builder().cache_dir(root.join(LOG_LISTS)).build());
        Accounts::open(self.build_with(log_lists)?, root, key).await
    }

    fn build_with(self, log_lists: LogLists) -> Result<Proton> {
        let api = self.api.build()?;
        let pgp = self.clock.map_or_else(Pgp::new, Pgp::with_clock);
        Ok(Proton::assembled(api, pgp, log_lists))
    }
}
