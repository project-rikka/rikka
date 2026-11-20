use rikka_net::HeaderMap;
use rikka_oauth::{Client, Refresh, RefreshingToken, Token};
use serde::{Deserialize, Serialize};

use crate::vault::{CredentialEntry, Vault};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Registrant {
    client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    #[serde(default)]
    client_secret_basic: bool,
    token_endpoint: String,
    resource: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Credentials {
    #[serde(flatten)]
    registrant: Registrant,
    token: Token,
}

#[derive(Clone)]
pub(crate) struct Signed {
    credentials: Credentials,
    entry: CredentialEntry,
}

pub(crate) struct Signing {
    token: RefreshingToken<Signed>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SigningError {
    #[error("the stored sign-in is no longer accepted")]
    SignedOut,

    #[error("the stored sign-in could not be renewed: {0}")]
    Renewal(String),
}

impl Registrant {
    pub(crate) fn new(
        client_id: String,
        client_secret: Option<String>,
        client_secret_basic: bool,
        token_endpoint: String,
        resource: String,
    ) -> Registrant {
        Registrant {
            client_id,
            client_secret,
            client_secret_basic,
            token_endpoint,
            resource,
        }
    }

    pub(crate) fn client(&self) -> rikka_oauth::Result<Client> {
        let builder = Client::builder(self.client_id.clone(), self.token_endpoint.clone());
        match (&self.client_secret, self.client_secret_basic) {
            (Some(secret), true) => builder.secret(secret.clone()).basic().build(),
            (Some(secret), false) => builder.secret(secret.clone()).build(),
            (None, _) => builder.build(),
        }
    }

    pub(crate) fn resource(&self) -> &str {
        &self.resource
    }

    pub(crate) fn granted(self, token: Token) -> Credentials {
        Credentials {
            registrant: self,
            token,
        }
    }
}

impl Signing {
    pub(crate) async fn stored(vault: &Vault, url: &str) -> Option<Signing> {
        if !vault.contains(url) {
            return None;
        }
        let entry = vault.entry(url);
        match entry.load().await {
            Ok(Some(credentials)) => Some(Signing {
                token: RefreshingToken::new(Signed { credentials, entry }),
            }),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!("stored sign-in for {url} is unusable: {error}");
                None
            }
        }
    }

    pub(crate) async fn bearer(&self) -> Result<HeaderMap, SigningError> {
        let signed = self.token.current().await?;
        rikka_oauth::bearer(signed.credentials.token.access_token())
            .map_err(|error| SigningError::Renewal(error.to_string()))
    }
}

impl Refresh for Signed {
    type Error = SigningError;

    fn expires_at(&self) -> Option<u64> {
        self.credentials.token.expires_at()
    }

    async fn refresh(&self) -> Result<Signed, SigningError> {
        let held = &self.credentials.token;
        let (Some(_), Some(refresh_token)) = (held.expires_at(), held.refresh_token()) else {
            return Ok(self.clone());
        };

        let _lock = self.entry.lock_renewal().await.map_err(unrenewed)?;
        let stored = self
            .entry
            .load()
            .await
            .map_err(unrenewed)?
            .ok_or(SigningError::SignedOut)?;
        if stored.token.access_token() != held.access_token() {
            return Ok(self.holding(stored));
        }

        let registrant = &self.credentials.registrant;
        let renewed = registrant
            .client()
            .map_err(unrenewed)?
            .refresh(refresh_token)
            .field("resource", registrant.resource())
            .send()
            .await;
        let token = match renewed {
            Ok(token) => token.fill_missing_from(held),
            Err(rikka_oauth::Error::ServerResponse(refusal)) if refusal.is_expired() => {
                return Err(SigningError::SignedOut);
            }
            Err(error) => return Err(unrenewed(error)),
        };

        let credentials = registrant.clone().granted(token);
        self.entry.save(&credentials).await.map_err(unrenewed)?;
        Ok(self.holding(credentials))
    }
}

impl Signed {
    fn holding(&self, credentials: Credentials) -> Signed {
        Signed {
            credentials,
            entry: self.entry.clone(),
        }
    }
}

fn unrenewed(error: impl std::fmt::Display) -> SigningError {
    SigningError::Renewal(error.to_string())
}

impl std::fmt::Debug for Signing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Signing").finish_non_exhaustive()
    }
}
