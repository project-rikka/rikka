use rikka_net::header;
use serde_json::{Value, json};

use crate::client::endpoint;
use crate::error::{Error, Result, text};
use crate::http;

#[must_use]
#[derive(Debug, Clone)]
pub struct Registration {
    client_name: String,
    redirect_uris: Vec<String>,
    scope: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Registered {
    client_id: String,
    client_secret: Option<String>,
}

impl Registration {
    pub fn new(client_name: impl Into<String>) -> Registration {
        Registration {
            client_name: client_name.into(),
            redirect_uris: Vec::new(),
            scope: None,
        }
    }

    pub fn redirect_uri(mut self, redirect_uri: impl Into<String>) -> Registration {
        self.redirect_uris.push(redirect_uri.into());
        self
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Registration {
        self.scope = Some(scope.into());
        self
    }

    pub async fn send(self, registration_endpoint: &str) -> Result<Registered> {
        let mut metadata = json!({
            "client_name": self.client_name,
            "redirect_uris": self.redirect_uris,
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
            "application_type": "native",
        });
        if let Some(scope) = self.scope {
            metadata["scope"] = Value::String(scope);
        }

        let request = http::client()?
            .post(endpoint(registration_endpoint)?)
            .header(header::ACCEPT, "application/json")
            .json(&metadata);
        Registered::from_response(&http::send(request).await?)
    }
}

impl Registered {
    fn from_response(body: &Value) -> Result<Registered> {
        let client_id = match text(body, "client_id") {
            id if id.is_empty() => return Err(Error::MissingField("client_id")),
            id => id,
        };
        Ok(Registered {
            client_id,
            client_secret: Some(text(body, "client_secret")).filter(|secret| !secret.is_empty()),
        })
    }

    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    #[must_use]
    pub fn client_secret(&self) -> Option<&str> {
        self.client_secret.as_deref()
    }
}

impl std::fmt::Debug for Registered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Registered")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}
