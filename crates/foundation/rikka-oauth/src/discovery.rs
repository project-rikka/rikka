use rikka_net::Url;
use serde_json::Value;

use crate::client::endpoint;
use crate::error::{Error, Result, text, texts};
use crate::http;
use crate::well_known;

const AUTHORIZATION_SERVER: &str = "oauth-authorization-server";
const OPENID_CONFIGURATION: &str = "openid-configuration";
const S256: &str = "S256";
const SECRET_BASIC: &str = "client_secret_basic";

#[derive(Debug, Clone)]
pub struct Metadata {
    issuer: Option<String>,
    authorization_endpoint: Option<Url>,
    token_endpoint: Url,
    device_authorization_endpoint: Option<Url>,
    registration_endpoint: Option<Url>,
    scopes_supported: Vec<String>,
    code_challenge_methods_supported: Option<Vec<String>>,
    token_endpoint_auth_methods_supported: Option<Vec<String>>,
    names_issuer_in_response: bool,
}

impl Metadata {
    pub async fn discover(url: &str) -> Result<Metadata> {
        let request = http::document(&http::client()?, endpoint(url)?);
        Metadata::from_document(&http::send(request).await?)
    }

    pub async fn discover_issuer(issuer: &str) -> Result<Option<Metadata>> {
        let identifier = endpoint(issuer)?;
        let client = http::client()?;
        let locations = well_known::candidates([
            well_known::inserted(&identifier, AUTHORIZATION_SERVER),
            well_known::inserted(&identifier, OPENID_CONFIGURATION),
            well_known::appended(&identifier, OPENID_CONFIGURATION),
        ]);
        for location in locations {
            let Some(document) = http::found(http::document(&client, location)).await? else {
                continue;
            };
            let metadata = Metadata::from_document(&document)?;
            return match &metadata.issuer {
                Some(named) if !same_issuer(named, issuer) => Err(Error::IssuerMismatch {
                    expected: issuer.to_owned(),
                    found: named.clone(),
                }),
                _ => Ok(Some(metadata)),
            };
        }
        Ok(None)
    }

    fn from_document(body: &Value) -> Result<Metadata> {
        let named = |field| match text(body, field) {
            address if address.is_empty() => None,
            address => Some(endpoint(&address)),
        };

        Ok(Metadata {
            issuer: Some(text(body, "issuer")).filter(|issuer| !issuer.is_empty()),
            authorization_endpoint: named("authorization_endpoint").transpose()?,
            token_endpoint: named("token_endpoint")
                .ok_or(Error::MissingField("token_endpoint"))??,
            device_authorization_endpoint: named("device_authorization_endpoint").transpose()?,
            registration_endpoint: named("registration_endpoint").transpose()?,
            scopes_supported: texts(body, "scopes_supported").unwrap_or_default(),
            code_challenge_methods_supported: texts(body, "code_challenge_methods_supported"),
            token_endpoint_auth_methods_supported: texts(
                body,
                "token_endpoint_auth_methods_supported",
            ),
            names_issuer_in_response: body
                .get("authorization_response_iss_parameter_supported")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    #[must_use]
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_deref()
    }

    #[must_use]
    pub fn authorization_endpoint(&self) -> Option<&Url> {
        self.authorization_endpoint.as_ref()
    }

    #[must_use]
    pub fn token_endpoint(&self) -> &Url {
        &self.token_endpoint
    }

    #[must_use]
    pub fn device_authorization_endpoint(&self) -> Option<&Url> {
        self.device_authorization_endpoint.as_ref()
    }

    #[must_use]
    pub fn registration_endpoint(&self) -> Option<&Url> {
        self.registration_endpoint.as_ref()
    }

    #[must_use]
    pub fn scopes_supported(&self) -> &[String] {
        &self.scopes_supported
    }

    #[must_use]
    pub fn supports_s256(&self) -> bool {
        self.code_challenge_methods_supported
            .as_ref()
            .is_none_or(|methods| methods.iter().any(|method| method == S256))
    }

    #[must_use]
    pub fn supports_secret_basic(&self) -> bool {
        self.token_endpoint_auth_methods_supported
            .as_ref()
            .is_none_or(|methods| methods.iter().any(|method| method == SECRET_BASIC))
    }

    pub fn check_response_issuer(&self, received: Option<&str>) -> Result<()> {
        match (self.issuer.as_deref(), received) {
            (Some(expected), Some(received)) if expected != received => {
                Err(Error::IssuerMismatch {
                    expected: expected.to_owned(),
                    found: received.to_owned(),
                })
            }
            (_, None) if self.names_issuer_in_response => Err(Error::MissingIssuer),
            _ => Ok(()),
        }
    }
}

fn same_issuer(named: &str, requested: &str) -> bool {
    named.trim_end_matches('/') == requested.trim().trim_end_matches('/')
}
