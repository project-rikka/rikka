use rikka_net::{HeaderMap, HeaderName, HeaderValue, header};

use crate::error::{Error, Result};

pub(crate) fn insert<K, V>(headers: &mut HeaderMap, name: K, value: V) -> Result<()>
where
    K: TryInto<HeaderName>,
    V: TryInto<HeaderValue>,
{
    let (Ok(name), Ok(value)) = (name.try_into(), value.try_into()) else {
        return Err(Error::Rejected("a header is not valid".to_owned()));
    };

    headers.insert(name, value);
    Ok(())
}

pub(crate) fn value(text: &str, what: &'static str) -> Result<HeaderValue> {
    HeaderValue::from_str(text)
        .map_err(|_| Error::Rejected(format!("{what} carries bytes a header cannot")))
}

pub(crate) fn beta(flags: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    if !flags.is_empty() {
        headers.insert(
            HeaderName::from_static("anthropic-beta"),
            value(flags, "the beta flags")?,
        );
    }
    Ok(headers)
}

pub(crate) fn secret(text: &str, what: &'static str) -> Result<HeaderValue> {
    let mut value = value(text, what)?;
    value.set_sensitive(true);
    Ok(value)
}

pub fn headers() -> Headers {
    Headers::default()
}

#[must_use]
#[derive(Debug, Default)]
pub struct Headers {
    map: HeaderMap,
    error: Option<Error>,
}

impl Headers {
    pub fn set<K, V>(mut self, name: K, value: V) -> Headers
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        if let Err(error) = insert(&mut self.map, name, value) {
            self.error.get_or_insert(error);
        }
        self
    }

    pub fn text<K: TryInto<HeaderName>>(self, name: K, text: &str, what: &'static str) -> Headers {
        self.checked(name, value(text, what))
    }

    pub fn secret<K: TryInto<HeaderName>>(
        self,
        name: K,
        text: &str,
        what: &'static str,
    ) -> Headers {
        self.checked(name, secret(text, what))
    }

    pub fn bearer(self, key: &str) -> Headers {
        self.secret(
            header::AUTHORIZATION,
            &format!("Bearer {key}"),
            "the API key",
        )
    }

    pub fn user_agent(self, agent: &str) -> Headers {
        self.text(header::USER_AGENT, agent, "the user agent")
    }

    pub fn accept(self, accepted: &str) -> Headers {
        self.text(header::ACCEPT, accepted, "what is accepted")
    }

    pub fn json(self) -> Headers {
        self.set(header::CONTENT_TYPE, "application/json")
    }

    pub fn keep_alive(self) -> Headers {
        self.set(header::CONNECTION, "Keep-Alive")
    }

    pub fn extend(mut self, more: HeaderMap) -> Headers {
        self.map.extend(more);
        self
    }

    pub fn build(self) -> Result<HeaderMap> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.map),
        }
    }

    fn checked<K: TryInto<HeaderName>>(mut self, name: K, value: Result<HeaderValue>) -> Headers {
        let checked = value.and_then(|value| insert(&mut self.map, name, value));
        if let Err(error) = checked {
            self.error.get_or_insert(error);
        }
        self
    }
}

impl From<HeaderMap> for Headers {
    fn from(map: HeaderMap) -> Headers {
        Headers { map, error: None }
    }
}
