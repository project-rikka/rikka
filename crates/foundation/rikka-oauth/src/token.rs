use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::claims::Claims;
use crate::error::{Error, Result, text};

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "Stored", into = "Stored")]
pub struct Token {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_at: Option<u64>,
    identity: Claims,
    body: Value,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
    #[serde(default)]
    expires_at: Option<u64>,
}

impl Token {
    pub(crate) fn from_response(body: Value) -> Result<Token> {
        let lifetime = match body.get("expires_in") {
            Some(Value::Number(seconds)) => seconds.as_u64(),
            Some(Value::String(seconds)) => seconds.parse().ok(),
            _ => None,
        };

        let stored = Stored {
            access_token: text(&body, "access_token"),
            refresh_token: Some(text(&body, "refresh_token")),
            id_token: Some(text(&body, "id_token")),
            expires_at: lifetime
                .filter(|seconds| *seconds > 0)
                .map(|seconds| now() + seconds),
        };
        Token::new(stored, body)
    }

    fn new(stored: Stored, body: Value) -> Result<Token> {
        if stored.access_token.is_empty() {
            return Err(Error::MissingField("access_token"));
        }

        let id_token = stored.id_token.filter(|token| !token.is_empty());
        Ok(Token {
            access_token: stored.access_token,
            refresh_token: stored.refresh_token.filter(|token| !token.is_empty()),
            identity: id_token
                .as_deref()
                .and_then(Claims::decode)
                .unwrap_or_default(),
            id_token,
            expires_at: stored.expires_at,
            body,
        })
    }

    pub(crate) fn or_refresh_token(mut self, refresh_token: &str) -> Token {
        if self.refresh_token.is_none() {
            self.refresh_token = Some(refresh_token.to_owned());
        }
        self
    }

    #[must_use]
    pub fn fill_missing_from(mut self, previous: &Token) -> Token {
        if let Some(refresh_token) = &previous.refresh_token {
            self = self.or_refresh_token(refresh_token);
        }
        if self.id_token.is_none() {
            self.id_token.clone_from(&previous.id_token);
            self.identity = previous.identity.clone();
        }
        self
    }

    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    #[must_use]
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }

    #[must_use]
    pub fn identity(&self) -> &Claims {
        &self.identity
    }

    #[must_use]
    pub fn body(&self) -> &Value {
        &self.body
    }
}

impl TryFrom<Stored> for Token {
    type Error = Error;

    fn try_from(stored: Stored) -> Result<Token> {
        Token::new(stored, Value::Null)
    }
}

impl From<Token> for Stored {
    fn from(token: Token) -> Stored {
        Stored {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            id_token: token.id_token,
            expires_at: token.expires_at,
        }
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

impl std::fmt::Debug for Token {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Token")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}
