use rikka_net::Url;

use crate::client::{Client, endpoint};
use crate::error::Result;
use crate::pkce::Pkce;
use crate::random;

pub struct Authorization {
    url: String,
    state: String,
    redirect_uri: String,
    pkce: Option<Pkce>,
}

#[must_use]
pub struct AuthorizationBuilder<'a> {
    client: &'a Client,
    endpoint: String,
    redirect_uri: String,
    scope: Option<String>,
    pkce: Option<Pkce>,
    params: Vec<(&'static str, String)>,
}

impl Authorization {
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }

    pub(crate) fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub(crate) fn verifier(&self) -> Option<&str> {
        self.pkce.as_ref().map(Pkce::verifier)
    }
}

impl<'a> AuthorizationBuilder<'a> {
    pub(crate) fn new(
        client: &'a Client,
        endpoint: String,
        redirect_uri: String,
    ) -> AuthorizationBuilder<'a> {
        AuthorizationBuilder {
            client,
            endpoint,
            redirect_uri,
            scope: None,
            pkce: None,
            params: Vec::new(),
        }
    }

    pub fn scope(mut self, scope: impl Into<String>) -> AuthorizationBuilder<'a> {
        self.scope = Some(scope.into());
        self
    }

    pub fn pkce(mut self) -> AuthorizationBuilder<'a> {
        self.pkce = Some(Pkce::new());
        self
    }

    pub fn param(
        mut self,
        name: &'static str,
        value: impl Into<String>,
    ) -> AuthorizationBuilder<'a> {
        self.params.push((name, value.into()));
        self
    }

    pub fn build(self) -> Result<Authorization> {
        let mut url: Url = endpoint(&self.endpoint)?;
        let state = random::random_state();

        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("client_id", self.client.id())
                .append_pair("response_type", "code")
                .append_pair("redirect_uri", &self.redirect_uri);
            if let Some(scope) = &self.scope {
                query.append_pair("scope", scope);
            }
            query.append_pair("state", &state);
            if let Some(pkce) = &self.pkce {
                query
                    .append_pair("code_challenge", pkce.challenge())
                    .append_pair("code_challenge_method", pkce.method());
            }
            for (name, value) in &self.params {
                query.append_pair(name, value);
            }
        }

        Ok(Authorization {
            url: url.into(),
            state,
            redirect_uri: self.redirect_uri,
            pkce: self.pkce,
        })
    }
}

impl std::fmt::Debug for Authorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Authorization")
            .field("redirect_uri", &self.redirect_uri)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AuthorizationBuilder<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthorizationBuilder")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}
