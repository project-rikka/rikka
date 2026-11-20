use rikka_llm::wire::{self, Headers};
use rikka_llm::{Error, Result};
use rikka_net::{HeaderName, HeaderValue};

const HEADER: &str = "a header of the endpoint";

#[derive(Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EndpointCredentials {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    key: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    headers: Vec<Header>,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Header {
    name: String,
    value: String,
}

impl EndpointCredentials {
    #[must_use]
    pub fn new(key: impl Into<String>, headers: Vec<Header>) -> EndpointCredentials {
        EndpointCredentials {
            key: key.into().trim().to_owned(),
            headers,
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn headers(&self) -> &[Header] {
        &self.headers
    }

    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> EndpointCredentials {
        self.key = key.into().trim().to_owned();
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.key.is_empty() && self.headers.is_empty()
    }

    pub(crate) fn signed(&self) -> Headers {
        let mut headers = wire::headers();
        if !self.key.is_empty() {
            headers = headers.bearer(&self.key);
        }
        for header in &self.headers {
            headers = headers.secret(header.name.as_str(), &header.value, HEADER);
        }
        headers
    }
}

impl Header {
    pub fn new(name: &str, value: &str) -> Result<Header> {
        let name = name.trim();
        let value = value.trim();
        if HeaderName::from_bytes(name.as_bytes()).is_err() || HeaderValue::from_str(value).is_err()
        {
            return Err(Error::Rejected(format!(
                "the header \"{name}\" is not valid"
            )));
        }
        Ok(Header {
            name: name.to_owned(),
            value: value.to_owned(),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Debug for EndpointCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EndpointCredentials")
            .field("keyed", &!self.key.is_empty())
            .field("headers", &self.headers)
            .finish()
    }
}

impl std::fmt::Debug for Header {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Header")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
