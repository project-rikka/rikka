use rikka_llm::{Meter, Model, ModelInfo};
use rikka_llm_adapter::{Access, DynAdapter, SignIn};

use crate::adapters;
use crate::credential::Credential;
use crate::error::{Error, Result};
use crate::login::Login;
use crate::renewal::Renewal;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Backend(usize);

impl Backend {
    pub fn all() -> impl ExactSizeIterator<Item = Backend> + Clone {
        (0..adapters::ALL.len()).map(Backend)
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Backend> {
        Backend::all().find(|backend| backend.id() == id)
    }

    #[must_use]
    pub fn id(self) -> &'static str {
        self.provider().id()
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        self.provider().label()
    }

    #[must_use]
    pub fn sign_in(self) -> SignIn {
        self.provider().sign_in()
    }

    #[must_use]
    pub fn is_subscription(self) -> bool {
        self.sign_in().variable().is_none()
    }

    #[must_use]
    pub fn is_metered(self) -> bool {
        self.provider()
            .meter(&Access::new(String::new(), None, None))
            .is_some()
    }

    #[must_use]
    pub fn is_listed(self) -> bool {
        self.provider()
            .listing(&Access::new(String::new(), None, None))
            .is_some()
    }

    #[must_use]
    pub fn holds(self, credential: &Credential) -> bool {
        match credential.backend() {
            Some(backend) => backend == self,
            None => self.sign_in().variable().is_some(),
        }
    }

    #[must_use]
    pub fn key(self, credential: Option<&Credential>) -> String {
        let Some(variable) = self.sign_in().variable() else {
            return String::new();
        };
        match credential.map(Credential::key) {
            Some(key) if !key.is_empty() => key.to_owned(),
            _ => std::env::var(variable).unwrap_or_default(),
        }
    }

    pub fn connect(
        self,
        credential: Option<&Credential>,
        model: &ModelInfo,
        renewal: &Renewal,
    ) -> Result<Model> {
        self.provider()
            .connect(&self.access(credential, renewal), model)
            .map_err(|error| Error::from_adapter(self, error))
    }

    pub fn meter(self, credential: Option<&Credential>, renewal: &Renewal) -> Result<Meter> {
        self.provider()
            .meter(&self.access(credential, renewal))
            .ok_or(Error::Unmetered(self))?
            .map_err(|error| Error::from_adapter(self, error))
    }

    pub async fn listed(
        self,
        credential: Option<&Credential>,
        renewal: &Renewal,
    ) -> Result<Vec<ModelInfo>> {
        let asking = self
            .provider()
            .listing(&self.access(credential, renewal))
            .ok_or(Error::Unlisted(self))?
            .map_err(|error| Error::from_adapter(self, error))?;
        asking
            .await
            .map_err(|refused| Error::Refused(refused.to_string()))
    }

    pub async fn login(self) -> Result<Login> {
        let starting = self.provider().login().ok_or(Error::Unsignable(self))?;
        let started = starting.await.map_err(Error::SignIn)?;
        Ok(Login::new(self, started))
    }

    pub async fn offered(self) -> Result<Vec<ModelInfo>> {
        let Some(asking) = self.provider().catalogue() else {
            return Err(Error::Uncatalogued(self));
        };
        asking
            .await
            .map_err(|refused| Error::Refused(refused.to_string()))
    }

    pub(crate) fn index(self) -> usize {
        self.0
    }

    pub(crate) fn provider(self) -> &'static dyn DynAdapter {
        adapters::ALL[self.0]
    }

    fn access<'a>(self, credential: Option<&'a Credential>, renewal: &Renewal) -> Access<'a> {
        let credential = credential.filter(|credential| self.holds(credential));
        Access::new(
            self.key(credential),
            credential.and_then(Credential::tokens),
            renewal.shared(self),
        )
    }
}

impl std::fmt::Debug for Backend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Backend").field(&self.id()).finish()
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

impl serde::Serialize for Backend {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

impl<'de> serde::Deserialize<'de> for Backend {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Backend, D::Error> {
        let id = String::deserialize(deserializer)?;
        Backend::from_id(&id)
            .ok_or_else(|| serde::de::Error::custom(format!("no backend is called \"{id}\"")))
    }
}
