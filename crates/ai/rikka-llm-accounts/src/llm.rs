use std::sync::Arc;

use futures_util::{Stream, StreamExt};
use rikka_llm::{Effort, Meter, Model, ModelInfo};
use rikka_seam::Delivery;
use rikka_settings::seam::{Announcement, Settings, protocol};
use rikka_settings::{Key, Schema};

use crate::account::Account;
use crate::account_id::AccountId;
use crate::account_store::{self, AccountStore, BACKENDS, ENDPOINTS, Secret};
use crate::error::{Error, Result};
use crate::listing::{self, Listed};
use crate::roster::ModelRoster;
use crate::selection::Selection;
use rikka_llm_providers::{Backend, Credential, Endpoint, EndpointCredentials, Renewal};

const DOMAIN: &str = "rikka.llm";

const CHOSEN: &str = "backend";
const EFFORT: &str = "effort";
const LANGUAGE: &str = "language";

pub const ANY_LANGUAGE: &str = "the language the person writes in";

#[derive(Clone, Debug)]
pub struct AccountsChanged(Announced);

#[derive(Clone, Debug)]
enum Announced {
    Values(protocol::Changed),
    Lagged,
    Credentials,
    Listed(Listed),
}

impl AccountsChanged {
    #[must_use]
    pub fn from_delivery(delivery: &Delivery) -> Option<AccountsChanged> {
        match delivery {
            Delivery::Event(event) => protocol::changed(event, DOMAIN)
                .map(|changed| AccountsChanged(Announced::Values(changed))),
            Delivery::Lagged(_) => Some(AccountsChanged(Announced::Lagged)),
        }
    }
}
#[derive(Debug)]
pub struct Llm {
    store: Option<AccountStore>,
    unavailable: Option<String>,
    keyring_fault: Option<String>,
    accounts: Arc<Vec<Account>>,
    renewals: u64,
    revision: u64,
    chosen: String,
    effort: Effort,
    language: String,
}

impl Llm {
    #[must_use]
    pub fn load() -> Llm {
        let (mut settings, mut unavailable) = match Settings::open(schema()) {
            Ok(settings) => (Some(settings), None),
            Err(error) => (None, Some(error.to_string())),
        };

        if let Some(open) = settings.as_ref()
            && let Err(error) = account_store::readable(open)
        {
            tracing::warn!("failed to read {DOMAIN}/{error}");
            unavailable = Some(error);
            settings = None;
        }

        let mut llm = Llm {
            store: settings.map(AccountStore::new),
            unavailable,
            keyring_fault: None,
            accounts: Arc::new(account_store::filled()),
            renewals: 0,
            revision: 0,
            chosen: String::new(),
            effort: Effort::default(),
            language: String::new(),
        };
        llm.take_up();
        llm
    }

    #[must_use]
    pub fn changes(&self) -> Option<impl Stream<Item = AccountsChanged> + Send + 'static> {
        let store = self.store.as_ref()?;
        let announcements = store.lock().changes()?;
        let settings = announcements.map(|announcement| match announcement {
            Announcement::Changed(changed) => AccountsChanged(Announced::Values(changed)),
            Announcement::Lagged => AccountsChanged(Announced::Lagged),
        });
        let listings =
            listing::follow(store.clone()).map(|listed| AccountsChanged(Announced::Listed(listed)));
        Some(futures_util::stream::select(
            settings,
            futures_util::stream::select(credential_changes(), listings),
        ))
    }

    pub fn apply_change(&mut self, announced: AccountsChanged) {
        match announced.0 {
            Announced::Values(changed) => {
                let Some(store) = self.store.as_ref() else {
                    return;
                };
                store.lock().set_user_values(changed.user_values);
                self.take_up();
            }
            Announced::Lagged => self.reload(),
            Announced::Credentials => self.take_up(),
            Announced::Listed(listed) => self.take_listing(listed),
        }
    }

    pub fn reload(&mut self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        if let Err(error) = store.lock().refresh() {
            tracing::warn!("failed to refresh `{DOMAIN}` from session: {error}");
            return;
        }
        self.take_up();
    }

    #[must_use]
    pub fn unavailable(&self) -> Option<&str> {
        self.unavailable
            .as_deref()
            .or(self.keyring_fault.as_deref())
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn accounts(&self) -> &[Account] {
        self.accounts.as_slice()
    }

    #[must_use]
    pub fn account(&self, id: AccountId) -> Option<&Account> {
        account_for(&self.accounts, id)
    }

    pub fn ready(&self) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(|account| account.is_ready())
    }

    #[must_use]
    pub fn selected_account(&self) -> Option<&Account> {
        AccountId::from_id(&self.chosen)
            .and_then(|id| self.account(id))
            .filter(|account| account.is_ready())
            .or_else(|| self.ready().next())
    }

    pub fn metered(&self) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(|account| account.is_metered())
    }

    pub fn meter(&self, id: AccountId) -> Result<Meter> {
        account_for(&self.current(), id)
            .ok_or(Error::Unconfigured)?
            .meter(&self.renewal())
    }

    #[must_use]
    pub fn roster(&self) -> ModelRoster {
        ModelRoster::new(
            self.current(),
            self.selected_account().map(Account::id),
            self.effort,
            self.renewal(),
        )
    }

    pub fn connect(&self) -> Result<Model> {
        let id = self.selected_account().ok_or(Error::Unconfigured)?.id();
        account_for(&self.current(), id)
            .ok_or(Error::Unconfigured)?
            .connect(&self.renewal())
    }

    #[must_use]
    pub fn selection(&self) -> Option<Selection> {
        let account = self.selected_account()?;
        Some(Selection::new(account.id(), account.model(), self.effort))
    }

    #[must_use]
    pub fn selection_for(&self, model: &str) -> Option<Selection> {
        let account = self
            .selected_account()
            .into_iter()
            .chain(self.ready())
            .chain(self.accounts())
            .find(|account| account.find_model(model).is_some())?;
        Some(Selection::new(account.id(), model, self.effort))
    }

    #[must_use]
    pub fn restore_selection(&self, model: &str) -> Option<Selection> {
        Some(model)
            .filter(|model| !model.is_empty())
            .and_then(|model| self.selection_for(model))
            .or_else(|| self.selection())
    }

    pub fn select(&mut self, selection: &Selection) {
        let id = selection.account();
        self.select_account(id);
        self.set_model(id, selection.model());
        self.set_effort(selection.effort());
    }

    pub(crate) fn connect_to(&self, id: AccountId, model: &str) -> Result<Model> {
        account_for(&self.current(), id)
            .ok_or(Error::Unconfigured)?
            .connect_to(model, &self.renewal())
    }

    pub fn set_credential(&mut self, backend: Backend, credential: Credential) {
        let adrift = self.selected_account().is_none();
        self.change_credential(backend, Some(credential));
        if adrift {
            self.select_account(backend.into());
        }
    }

    pub fn clear_credential(&mut self, backend: Backend) {
        if self
            .account(backend.into())
            .is_some_and(Account::has_credential)
        {
            self.change_credential(backend, None);
        }
    }

    pub fn set_api_key(&mut self, id: AccountId, key: &str) {
        let Some(account) = self.account(id) else {
            return;
        };
        if let Some(credentials) = account.endpoint_credentials() {
            if credentials.key() != key.trim() {
                let credentials = credentials.clone().with_key(key);
                self.change_endpoint_credentials(id, credentials);
            }
            return;
        }
        if let Some(backend) = id.backend()
            && account.stored_key() != key
        {
            let credential = (!key.is_empty()).then(|| Credential::keyed(key));
            self.change_credential(backend, credential);
        }
    }

    pub fn set_model(&mut self, id: AccountId, model: &str) {
        if self
            .account(id)
            .is_some_and(|account| account.model() != model)
        {
            self.change(id, |account| account.set_model(model));
        }
    }

    pub fn add_model(&mut self, id: AccountId, model: ModelInfo) {
        self.change(id, |account| account.add_model(model));
    }

    pub fn remove_model(&mut self, id: AccountId, model: &str) {
        self.change(id, |account| account.remove_model(model));
    }

    pub fn add_endpoint(
        &mut self,
        name: &str,
        endpoint: Endpoint,
        credentials: EndpointCredentials,
    ) -> AccountId {
        let id = AccountId::minted();
        let account = Account::from_endpoint(
            id,
            name.trim().to_owned(),
            endpoint,
            String::new(),
            Vec::new(),
        );
        self.refresh_accounts();
        Arc::make_mut(&mut self.accounts).push(account);
        self.revision += 1;
        self.save_accounts();
        self.change_endpoint_credentials(id, credentials);
        id
    }

    pub fn edit_endpoint(
        &mut self,
        id: AccountId,
        name: &str,
        endpoint: Endpoint,
        credentials: EndpointCredentials,
    ) {
        let unchanged = self.account(id).is_none_or(|account| {
            account.label() == name.trim() && account.endpoint() == Some(&endpoint)
        });
        if !unchanged {
            self.change(id, |account| {
                account.set_endpoint(name.trim().to_owned(), endpoint);
            });
        }
        if self
            .account(id)
            .and_then(Account::endpoint_credentials)
            .is_some_and(|held| *held != credentials)
        {
            self.change_endpoint_credentials(id, credentials);
        }
    }

    pub fn remove_endpoint(&mut self, id: AccountId) {
        let Some(account) = self
            .account(id)
            .filter(|account| account.endpoint().is_some())
            .cloned()
        else {
            return;
        };
        self.keep(&account, Secret::Endpoint(None));
        self.refresh_accounts();
        Arc::make_mut(&mut self.accounts).retain(|account| account.id() != id);
        self.revision += 1;
        self.save_accounts();
        if self.chosen == id.to_string() {
            self.chosen.clear();
            self.store(CHOSEN, &self.chosen);
        }
    }

    #[must_use]
    pub fn effort(&self) -> Effort {
        self.effort
    }

    pub fn set_effort(&mut self, effort: Effort) {
        if self.effort == effort {
            return;
        }
        self.effort = effort;
        self.revision += 1;
        self.store(EFFORT, &effort);
    }

    pub fn select_account(&mut self, id: AccountId) {
        let chosen = id.to_string();
        if self.chosen == chosen {
            return;
        }
        self.chosen = chosen;
        self.revision += 1;
        self.store(CHOSEN, &self.chosen);
    }

    #[must_use]
    pub fn language(&self) -> Option<&str> {
        Some(self.language.as_str()).filter(|code| !code.is_empty())
    }

    #[must_use]
    pub fn reply_language(&self) -> &str {
        self.language().unwrap_or(ANY_LANGUAGE)
    }

    pub fn set_language(&mut self, language: &str) {
        if self.language == language {
            return;
        }
        self.language = language.to_owned();
        self.revision += 1;
        self.store(LANGUAGE, &self.language);
    }

    fn renewal(&self) -> Renewal {
        self.store
            .as_ref()
            .map_or_else(Renewal::none, AccountStore::renewal)
    }

    fn current(&self) -> Arc<Vec<Account>> {
        match &self.store {
            Some(store) if store.renewals() != self.renewals => store.load().accounts,
            _ => Arc::clone(&self.accounts),
        }
    }

    fn take_up(&mut self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        self.revision += 1;
        self.renewals = store.renewals();
        let loaded = store.load();
        self.accounts = loaded.accounts;
        self.keyring_fault = loaded.fault;
        store.wake_listing();
        let settings = store.lock();
        self.chosen = settings.read(CHOSEN).unwrap_or_default();
        self.effort = settings.read(EFFORT).unwrap_or_default();
        self.language = settings.read(LANGUAGE).unwrap_or_default();
    }

    fn take_listing(&mut self, listed: Listed) {
        let id = AccountId::from(listed.backend);
        if self
            .account(id)
            .is_some_and(|account| account.is_ready() && account.identity() == listed.who)
        {
            self.change(id, |account| account.set_listing(listed.models, listed.at));
        }
    }

    fn change(&mut self, id: AccountId, edit: impl FnOnce(&mut Account)) {
        if self.edit_account(id, edit) {
            self.save_accounts();
        }
    }

    fn change_credential(&mut self, backend: Backend, credential: Option<Credential>) {
        let id = AccountId::from(backend);
        if let Some(account) = self.account(id).cloned() {
            self.keep(&account, Secret::Backend(credential.as_ref()));
        }
        let mut forgotten = false;
        self.edit_account(id, |account| {
            account.set_credential(credential);
            forgotten = account.forget_listing();
        });
        if forgotten {
            self.save_accounts();
        }
        if let Some(store) = self.store.as_ref() {
            store.wake_listing();
        }
    }

    fn change_endpoint_credentials(&mut self, id: AccountId, credentials: EndpointCredentials) {
        if let Some(account) = self.account(id).cloned() {
            self.keep(&account, Secret::Endpoint(Some(&credentials)));
        }
        self.edit_account(id, |account| account.set_endpoint_credentials(credentials));
    }

    fn keep(&mut self, account: &Account, secret: Secret<'_>) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        match store.keep(account, secret) {
            Ok(()) => self.keyring_fault = None,
            Err(error) => {
                tracing::warn!(
                    "failed to keep credentials for {}: {error}",
                    account.label()
                );
                self.keyring_fault = Some(error);
            }
        }
    }

    fn refresh_accounts(&mut self) {
        if let Some(store) = self.store.as_ref()
            && store.renewals() != self.renewals
        {
            self.renewals = store.renewals();
            self.accounts = store.load().accounts;
        }
    }

    fn edit_account(&mut self, id: AccountId, edit: impl FnOnce(&mut Account)) -> bool {
        self.refresh_accounts();
        let Some(account) = Arc::make_mut(&mut self.accounts)
            .iter_mut()
            .find(|account| account.id() == id)
        else {
            return false;
        };
        edit(account);
        self.revision += 1;
        true
    }

    fn save_accounts(&self) {
        if let Some(store) = self.store.as_ref() {
            store.save(&self.accounts);
        }
    }

    fn store(&self, key: &str, value: &impl serde::Serialize) {
        if let Some(settings_store) = self.store.as_ref() {
            settings_store.lock().set_or_warn(key, value);
        }
    }
}

fn account_for(accounts: &[Account], id: AccountId) -> Option<&Account> {
    accounts.iter().find(|account| account.id() == id)
}

fn credential_changes() -> impl Stream<Item = AccountsChanged> + Send + 'static {
    futures_util::stream::once(async {
        let secrets = rikka_secrets::Secrets::shared().await?;
        secrets.changes().await
    })
    .filter_map(|watched| async move {
        watched
            .inspect_err(|error| tracing::warn!("failed to follow model credentials: {error}"))
            .ok()
    })
    .flatten()
    .map(|_| AccountsChanged(Announced::Credentials))
}

fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("The models this desktop answers with")
        .key(Key::list(BACKENDS, []).summary("The model each backend answers with"))
        .key(
            Key::list(ENDPOINTS, [])
                .summary("The OpenAI-compatible endpoints the person added, and their models"),
        )
        .key(
            Key::string(CHOSEN, "").summary("Which of them answers when nothing else is asked for"),
        )
        .key(
            Key::string(EFFORT, Effort::default().id())
                .summary("How hard a model is asked to think before it answers"),
        )
        .key(
            Key::string(LANGUAGE, "")
                .summary("The language an agent answers in, or nothing for the person's own"),
        )
}
