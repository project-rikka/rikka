use rikka_net::{HeaderMap, HeaderName, HeaderValue, Url};

use crate::error::{Error, Result};
use crate::headers;

#[derive(Debug, Default)]
pub struct Settings {
    model: Option<String>,
    headers: HeaderMap,
    error: Option<Error>,
}

impl Settings {
    pub fn model(&mut self, model: impl Into<String>) {
        self.model = Some(model.into());
    }

    pub fn header<K, V>(&mut self, name: K, value: V)
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        if let Err(error) = headers::insert(&mut self.headers, name, value) {
            self.error.get_or_insert(error);
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn settled(&mut self) -> Result<ResolvedSettings> {
        match self.error.take() {
            Some(error) => Err(error),
            None => Ok(ResolvedSettings {
                model: self.model.take(),
                headers: std::mem::take(&mut self.headers),
            }),
        }
    }
}

#[derive(Debug)]
pub struct ResolvedSettings {
    model: Option<String>,
    headers: HeaderMap,
}

impl ResolvedSettings {
    pub fn headers(self) -> HeaderMap {
        self.headers
    }

    pub fn named(self) -> Result<(String, HeaderMap)> {
        let model = self
            .model
            .ok_or_else(|| Error::Rejected("no model was named".to_owned()))?;
        Ok((model, self.headers))
    }
}

pub fn key(explicit: Option<String>, variable: &'static str) -> Result<String> {
    explicit
        .or_else(|| std::env::var(variable).ok())
        .filter(|key| !key.is_empty())
        .ok_or(Error::MissingKey(variable))
}

pub fn endpoint(url: &str) -> Result<Url> {
    Url::parse(url).map_err(|_| Error::InvalidEndpoint(url.to_owned()))
}

pub fn endpoint_under(base: &str, path: &str) -> Result<Url> {
    endpoint(&format!("{}{path}", base.trim_end_matches('/')))
}
