use rikka_llm::wire::{self, Headers, Settings};
use rikka_llm::{Model, Result};
use rikka_net::{HeaderName, HeaderValue, Url};

pub trait Api: Default + std::fmt::Debug {
    const VARIABLE: &'static str;

    const ENDPOINT: &'static str;

    fn connect(self, connection: KeyedConnection) -> Result<Model>;
}

#[derive(Debug)]
pub struct KeyedConnection {
    pub model: String,
    pub key: String,
    pub endpoint: Url,
    pub headers: Headers,
}

#[must_use]
pub struct Keyed<A> {
    key: Option<String>,
    endpoint: Option<String>,
    settings: Settings,
    api: A,
}

impl<A: Api> Keyed<A> {
    pub fn new(key: impl Into<String>) -> Keyed<A> {
        Keyed {
            key: Some(key.into()),
            ..Keyed::from_env()
        }
    }

    pub fn from_env() -> Keyed<A> {
        Keyed {
            key: None,
            endpoint: None,
            settings: Settings::default(),
            api: A::default(),
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Keyed<A> {
        self.settings.model(model);
        self
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Keyed<A> {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn header<K, V>(mut self, name: K, value: V) -> Keyed<A>
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.settings.header(name, value);
        self
    }

    pub fn with(mut self, set: impl FnOnce(A) -> A) -> Keyed<A> {
        self.api = set(self.api);
        self
    }

    pub fn build(mut self) -> Result<Model> {
        let resolved = self.settings.settled()?;
        let key = wire::key(self.key.take(), A::VARIABLE)?;
        let (model, headers) = resolved.named()?;
        let endpoint = wire::endpoint(self.endpoint.as_deref().unwrap_or(A::ENDPOINT))?;
        self.api.connect(KeyedConnection {
            model,
            key,
            endpoint,
            headers: Headers::from(headers),
        })
    }
}

impl<A: Api> std::fmt::Debug for Keyed<A> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Keyed")
            .field("api", &self.api)
            .field("model", &self.settings.name())
            .field("endpoint", &self.endpoint.as_deref().unwrap_or(A::ENDPOINT))
            .finish_non_exhaustive()
    }
}
