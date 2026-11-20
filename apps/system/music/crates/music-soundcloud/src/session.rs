use std::path::{Path, PathBuf};

use music_core::{Cookie, Error, Result, keep_secret, kept_secret, renew_secret};
use rikka_oauth::{Claims, Client};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub(crate) const PROOF: &[&str] = &[ACCESS];
const ACCESS: &str = "oauth_token";
const REFRESH: &str = "oauth_refresh_token";
const KIND: &str = "session";
const LABEL: &str = "SoundCloud sign-in";
const TOKENS: &str = "https://secure.soundcloud.com/oauth/token";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Tokens {
    access: String,
    #[serde(default)]
    refresh: Option<String>,
}

pub(crate) struct Session {
    folder: PathBuf,
    tokens: Mutex<Tokens>,
}

impl Session {
    pub(crate) async fn fill(folder: &Path, cookies: &[Cookie]) -> Result<Session> {
        let named = |name: &str| {
            cookies
                .iter()
                .find(|cookie| cookie.name == name && !cookie.value.is_empty())
                .map(|cookie| cookie.value.clone())
        };
        let access = named(ACCESS)
            .ok_or_else(|| Error::Refused("SoundCloud returned no sign-in cookie".into()))?;
        let tokens = Tokens {
            access,
            refresh: named(REFRESH),
        };
        keep_secret(folder, KIND, LABEL, &tokens).await?;
        Ok(Session {
            folder: folder.to_owned(),
            tokens: Mutex::new(tokens),
        })
    }

    pub(crate) async fn open(folder: &Path) -> Result<Option<Session>> {
        Ok(kept_secret(folder, KIND).await?.map(|tokens| Session {
            folder: folder.to_owned(),
            tokens: Mutex::new(tokens),
        }))
    }

    pub(crate) async fn token(&self) -> String {
        self.tokens.lock().await.access.clone()
    }

    pub(crate) async fn refresh(&self, client: &str, stale: &str) -> Result<bool> {
        let mut tokens = self.tokens.lock().await;
        if tokens.access != stale {
            return Ok(true);
        }
        let Some(refresh) = tokens.refresh.clone() else {
            return Ok(false);
        };
        let issued_to = issued_to(&tokens.access).unwrap_or_else(|| client.to_owned());
        let renewal = Client::new(issued_to, TOKENS)
            .map_err(Error::unreachable)?
            .refresh(&refresh)
            .send()
            .await;
        let token = match renewal {
            Ok(token) => token,
            Err(rikka_oauth::Error::ServerResponse(refusal)) => {
                tracing::warn!("SoundCloud token refresh refused: {refusal}");
                return Ok(false);
            }
            Err(rikka_oauth::Error::MissingField(missing)) => {
                return Err(Error::Unreadable(format!(
                    "the renewed sign-in is missing {missing}"
                )));
            }
            Err(error) => return Err(Error::unreachable(error)),
        };
        let renewed = Tokens {
            access: token.access_token().to_owned(),
            refresh: token.refresh_token().map(str::to_owned),
        };
        renew_secret(&self.folder, KIND, LABEL, &renewed).await?;
        *tokens = renewed;
        Ok(true)
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("folder", &self.folder)
            .finish_non_exhaustive()
    }
}

fn issued_to(token: &str) -> Option<String> {
    Claims::decode(token)
        .map(|claims| claims.text(&["client_id"]).to_owned())
        .filter(|client| !client.is_empty())
}
