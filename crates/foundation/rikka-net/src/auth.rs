use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use http::{HeaderName, HeaderValue, header};

use crate::error::{Error, Result};

#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Auth {
    Basic {
        username: String,
        password: Option<String>,
    },
    Bearer(String),
    ApiKey {
        header: HeaderName,
        key: String,
    },
}

impl Auth {
    #[must_use]
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Auth {
        Auth::Basic {
            username: username.into(),
            password: Some(password.into()),
        }
    }

    #[must_use]
    pub fn bearer(token: impl Into<String>) -> Auth {
        Auth::Bearer(token.into())
    }

    pub fn api_key(header: impl TryInto<HeaderName>, key: impl Into<String>) -> Result<Auth> {
        let header = header
            .try_into()
            .map_err(|_| Error::InvalidHeader("invalid header name".to_owned()))?;

        Ok(Auth::ApiKey {
            header,
            key: key.into(),
        })
    }

    pub(crate) fn header(&self) -> Result<(HeaderName, HeaderValue)> {
        let (name, credentials) = match self {
            Auth::Basic { username, password } => {
                let pair = format!("{username}:{}", password.as_deref().unwrap_or_default());
                (
                    header::AUTHORIZATION,
                    format!("Basic {}", STANDARD.encode(pair)),
                )
            }
            Auth::Bearer(token) => (header::AUTHORIZATION, format!("Bearer {token}")),
            Auth::ApiKey { header, key } => (header.clone(), key.clone()),
        };

        let mut value = HeaderValue::from_str(&credentials).map_err(|_| {
            Error::InvalidHeader("credentials contain characters invalid in a header".to_owned())
        })?;
        value.set_sensitive(true);

        Ok((name, value))
    }
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Auth::Basic { username, .. } => f
                .debug_struct("Auth::Basic")
                .field("username", username)
                .field("password", &"<redacted>")
                .finish(),
            Auth::Bearer(_) => f.write_str("Auth::Bearer(<redacted>)"),
            Auth::ApiKey { header, .. } => f
                .debug_struct("Auth::ApiKey")
                .field("header", header)
                .field("key", &"<redacted>")
                .finish(),
        }
    }
}
