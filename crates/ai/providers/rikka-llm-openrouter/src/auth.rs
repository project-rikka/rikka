use std::time::Duration;

use rikka_net::{Url, header};
use rikka_oauth::{Loopback, Pkce, Redirect};
use serde::{Deserialize, Serialize};
use serde_json::json;

use rikka_llm::wire::{self, text_of};
use rikka_llm::{Error, Result};

const AUTHORIZE: &str = "https://openrouter.ai/auth";
const KEYS: &str = "https://openrouter.ai/api/v1/auth/keys";
const PATH: &str = "/openrouter";

const ANY_PORT: u16 = 0;
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct OpenRouterCredentials {
    key: String,
}

impl OpenRouterCredentials {
    #[must_use]
    pub(crate) fn key(&self) -> &str {
        &self.key
    }
}

#[must_use]
pub(crate) struct OpenRouterLogin {
    redirect: Redirect,
    pkce: Pkce,
    url: String,
}

impl OpenRouterLogin {
    pub(crate) async fn start() -> Result<OpenRouterLogin> {
        let redirect = Loopback::localhost(ANY_PORT).path(PATH).bind().await?;
        let pkce = Pkce::new();

        let url = Url::parse_with_params(
            AUTHORIZE,
            [
                ("callback_url", redirect.uri()),
                ("code_challenge", pkce.challenge()),
                ("code_challenge_method", pkce.method()),
            ],
        )
        .map_err(|error| Error::Login(error.to_string()))?;

        Ok(OpenRouterLogin {
            redirect,
            pkce,
            url: url.into(),
        })
    }

    #[must_use]
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) async fn finish(self) -> Result<OpenRouterCredentials> {
        let code = self.redirect.wait().await?.code()?;

        let body = rikka_oauth::send(
            wire::client(TIMEOUT, TIMEOUT)?
                .post(KEYS)
                .header(header::ACCEPT, "application/json")
                .json(&json!({
                    "code": code,
                    "code_verifier": self.pkce.verifier(),
                    "code_challenge_method": self.pkce.method(),
                })),
        )
        .await?;

        let key = text_of(&body, "key");
        match key.is_empty() {
            true => Err(Error::Login(
                "the sign-in was approved and handed back no key".to_owned(),
            )),
            false => Ok(OpenRouterCredentials { key }),
        }
    }
}

impl std::fmt::Debug for OpenRouterCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenRouterCredentials")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for OpenRouterLogin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenRouterLogin")
            .field("redirect", &self.redirect.uri())
            .finish_non_exhaustive()
    }
}
