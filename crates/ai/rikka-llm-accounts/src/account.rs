use std::time::Duration;

use rikka_llm::{Meter, Model, ModelInfo};
use rikka_llm_providers::{Backend, Credential, Endpoint, EndpointCredentials, Renewal};

use crate::account_id::AccountId;
use crate::error::{Error, Result};

const LISTING_LIFETIME: Duration = Duration::from_secs(3 * 60 * 60);

#[derive(Clone, Debug)]
pub struct Account {
    origin: Origin,
    model: String,
    models: Vec<ModelInfo>,
    listed: Option<u64>,
}

#[derive(Clone, Debug)]
enum Origin {
    Backend {
        backend: Backend,
        credential: Option<Credential>,
    },
    Endpoint {
        id: AccountId,
        name: String,
        endpoint: Endpoint,
        credentials: EndpointCredentials,
    },
}

impl Account {
    pub(crate) fn new(backend: Backend) -> Account {
        Account::kept(backend, String::new(), Vec::new(), None)
    }

    pub(crate) fn kept(
        backend: Backend,
        model: String,
        mut models: Vec<ModelInfo>,
        listed: Option<u64>,
    ) -> Account {
        models.sort_by_key(ModelInfo::is_superseded);
        Account {
            origin: Origin::Backend {
                backend,
                credential: None,
            },
            model,
            models,
            listed: listed.filter(|_| backend.is_listed()),
        }
    }

    pub(crate) fn from_endpoint(
        id: AccountId,
        name: String,
        endpoint: Endpoint,
        model: String,
        models: Vec<ModelInfo>,
    ) -> Account {
        Account {
            origin: Origin::Endpoint {
                id,
                name,
                endpoint,
                credentials: EndpointCredentials::default(),
            },
            model,
            models,
            listed: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> AccountId {
        match &self.origin {
            Origin::Backend { backend, .. } => AccountId::from(*backend),
            Origin::Endpoint { id, .. } => *id,
        }
    }

    #[must_use]
    pub fn label(&self) -> &str {
        match &self.origin {
            Origin::Backend { backend, .. } => backend.label(),
            Origin::Endpoint { name, .. } => name,
        }
    }

    #[must_use]
    pub fn backend(&self) -> Option<Backend> {
        match &self.origin {
            Origin::Backend { backend, .. } => Some(*backend),
            Origin::Endpoint { .. } => None,
        }
    }

    #[must_use]
    pub fn endpoint(&self) -> Option<&Endpoint> {
        match &self.origin {
            Origin::Endpoint { endpoint, .. } => Some(endpoint),
            Origin::Backend { .. } => None,
        }
    }

    #[must_use]
    pub fn endpoint_credentials(&self) -> Option<&EndpointCredentials> {
        match &self.origin {
            Origin::Endpoint { credentials, .. } => Some(credentials),
            Origin::Backend { .. } => None,
        }
    }

    #[must_use]
    pub fn credential(&self) -> Option<&Credential> {
        match &self.origin {
            Origin::Backend {
                backend,
                credential,
            } => credential
                .as_ref()
                .filter(|credential| backend.holds(credential)),
            Origin::Endpoint { .. } => None,
        }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        match &self.origin {
            Origin::Backend { backend, .. } => match backend.sign_in().variable() {
                Some(_) => !self.key().is_empty(),
                None => self.credential().is_some(),
            },
            Origin::Endpoint { .. } => !self.models.is_empty(),
        }
    }

    #[must_use]
    pub fn has_credential(&self) -> bool {
        match &self.origin {
            Origin::Backend { .. } => self.credential().is_some(),
            Origin::Endpoint { credentials, .. } => !credentials.is_empty(),
        }
    }

    #[must_use]
    pub fn is_metered(&self) -> bool {
        self.backend().is_some_and(Backend::is_metered) && self.has_credential()
    }

    #[must_use]
    pub fn identity(&self) -> String {
        self.credential().map(Credential::who).unwrap_or_default()
    }

    #[must_use]
    pub fn key(&self) -> String {
        match &self.origin {
            Origin::Backend { backend, .. } => backend.key(self.credential()),
            Origin::Endpoint { credentials, .. } => credentials.key().to_owned(),
        }
    }

    #[must_use]
    pub fn stored_key(&self) -> &str {
        match &self.origin {
            Origin::Backend { .. } => self.credential().map(Credential::key).unwrap_or_default(),
            Origin::Endpoint { credentials, .. } => credentials.key(),
        }
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub fn models(&self) -> &[ModelInfo] {
        let current = self.models.partition_point(|model| !model.is_superseded());
        &self.models[..current]
    }

    #[must_use]
    pub fn find_model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|model| model.id() == id)
    }

    #[must_use]
    pub fn model_info(&self) -> Option<&ModelInfo> {
        self.find_model(&self.model)
    }

    #[must_use]
    pub fn has_custom_models(&self) -> bool {
        match &self.origin {
            Origin::Backend { backend, .. } => !backend.is_listed(),
            Origin::Endpoint { .. } => true,
        }
    }

    pub(crate) fn kept_models(&self) -> &[ModelInfo] {
        &self.models
    }

    pub(crate) fn listed(&self) -> Option<u64> {
        self.listed
    }

    pub(crate) fn is_listing_due(&self, now: u64) -> bool {
        self.backend().is_some_and(Backend::is_listed)
            && self.is_ready()
            && self
                .listed
                .is_none_or(|at| now.saturating_sub(at) >= LISTING_LIFETIME.as_secs())
    }

    pub(crate) fn set_listing(&mut self, mut models: Vec<ModelInfo>, at: u64) {
        models.sort_by_key(ModelInfo::is_superseded);
        if self.model.is_empty()
            && let Some(newest) = models.first().filter(|model| !model.is_superseded())
        {
            self.model = newest.id().to_owned();
        }
        self.models = models;
        self.listed = Some(at);
    }

    pub(crate) fn forget_listing(&mut self) -> bool {
        if self.listed.is_none() {
            return false;
        }
        self.models.clear();
        self.listed = None;
        true
    }

    pub(crate) fn set_credential(&mut self, kept: Option<Credential>) {
        if let Origin::Backend { credential, .. } = &mut self.origin {
            *credential = kept;
        }
    }

    pub(crate) fn set_endpoint_credentials(&mut self, kept: EndpointCredentials) {
        if let Origin::Endpoint { credentials, .. } = &mut self.origin {
            *credentials = kept;
        }
    }

    pub(crate) fn set_endpoint(&mut self, named: String, address: Endpoint) {
        if let Origin::Endpoint { name, endpoint, .. } = &mut self.origin {
            *name = named;
            *endpoint = address;
        }
    }

    pub(crate) fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    pub(crate) fn add_model(&mut self, model: ModelInfo) {
        if !self.has_custom_models() {
            return;
        }
        match self
            .models
            .iter_mut()
            .find(|entry| entry.id() == model.id())
        {
            Some(entry) => *entry = model,
            None => self.models.push(model),
        }
    }

    pub(crate) fn remove_model(&mut self, id: &str) {
        self.models.retain(|entry| entry.id() != id);
        if self.model == id {
            self.model.clear();
        }
    }

    pub(crate) fn connect(&self, renewal: &Renewal) -> Result<Model> {
        self.connect_to(&self.model, renewal)
    }

    pub(crate) fn connect_to(&self, model: &str, renewal: &Renewal) -> Result<Model> {
        if model.is_empty() {
            return Err(Error::Modelless(self.label().to_owned()));
        }
        match &self.origin {
            Origin::Backend { backend, .. } => {
                let unlisted = || ModelInfo::new(model, model, 0, 0);
                let info = self.find_model(model).cloned().unwrap_or_else(unlisted);
                Ok(backend.connect(self.credential(), &info, renewal)?)
            }
            Origin::Endpoint {
                endpoint,
                credentials,
                ..
            } => endpoint
                .connect(credentials, model)
                .map_err(|refused| Error::Refused(refused.to_string())),
        }
    }

    pub(crate) fn meter(&self, renewal: &Renewal) -> Result<Meter> {
        match &self.origin {
            Origin::Backend { backend, .. } => Ok(backend.meter(self.credential(), renewal)?),
            Origin::Endpoint { name, .. } => Err(Error::Unmetered(name.clone())),
        }
    }
}
