use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use music_core::{Error, Prompt, Result, SignInStep, keep_secret, kept_secret};
use music_spotify_api::CLIENT_ID;
use music_spotify_session::{Account, Session};
use music_spotify_wire::{AccountType, DeviceId, StoredCredentials};
use rikka_oauth::{Client, Loopback};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::failure;

const AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
const TOKEN: &str = "https://accounts.spotify.com/api/token";
const PORT: u16 = 8898;
const CALLBACK: &str = "/login";
const SCOPES: &[&str] = &[
    "streaming",
    "user-read-private",
    "user-read-email",
    "user-library-read",
    "user-library-modify",
    "user-follow-read",
    "user-top-read",
    "user-read-recently-played",
    "playlist-read-private",
    "playlist-read-collaborative",
];
const KIND: &str = "credentials";
const LABEL: &str = "Spotify sign-in";

#[derive(Clone, Debug)]
pub(crate) struct Keys {
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Kept {
    username: String,
    #[serde(serialize_with = "base64_text", deserialize_with = "base64_bytes")]
    auth_data: Vec<u8>,
}

impl Keys {
    pub(crate) fn new(dir: PathBuf) -> Keys {
        Keys { dir }
    }

    pub(crate) async fn resume(&self) -> Result<Option<Session>> {
        let Some(kept) = kept_secret::<Kept>(&self.dir, KIND).await? else {
            return Ok(None);
        };
        let credentials = StoredCredentials {
            username: kept.username,
            auth_data: kept.auth_data,
        };
        let session = Session::connect(credentials, device())
            .await
            .map_err(failure::session)?;
        self.settle(session).await.map(Some)
    }

    pub(crate) async fn sign_in(&self, prompt: Prompt) -> Result<Session> {
        let token = authorize(prompt).await?;
        let session = Session::sign_in(&token, device())
            .await
            .map_err(failure::session)?;
        self.settle(session).await
    }

    async fn settle(&self, session: Session) -> Result<Session> {
        entitled(&session.account())?;
        let credentials = session.credentials();
        let kept = Kept {
            username: credentials.username,
            auth_data: credentials.auth_data,
        };
        keep_secret(&self.dir, KIND, LABEL, &kept).await?;
        Ok(session)
    }
}

fn device() -> DeviceId {
    DeviceId::generate(&mut rand::rng())
}

fn entitled(account: &Account) -> Result<()> {
    match &account.product.account {
        Some(AccountType::Other(kind)) => Err(Error::NotEntitled(kind.clone())),
        Some(AccountType::Premium) | None => Ok(()),
    }
}

async fn authorize(prompt: Prompt) -> Result<String> {
    let client = Client::new(CLIENT_ID, TOKEN).map_err(failed)?;
    let redirect = Loopback::ip(PORT)
        .path(CALLBACK)
        .bind()
        .await
        .map_err(failed)?;
    let authorization = client
        .authorization(AUTHORIZE, redirect.uri())
        .scope(SCOPES.join(" "))
        .pkce()
        .build()
        .map_err(failed)?;
    prompt(SignInStep::Browse(authorization.url().to_owned()));

    let code = redirect.code(authorization.state()).await.map_err(failed)?;
    let token = client
        .exchange(&code, &authorization)
        .send()
        .await
        .map_err(failed)?;
    Ok(token.access_token().to_owned())
}

fn failed(error: rikka_oauth::Error) -> Error {
    match error {
        rikka_oauth::Error::AccessDenied | rikka_oauth::Error::TimedOut => Error::Cancelled,
        rikka_oauth::Error::Listen { .. } | rikka_oauth::Error::Http(_) => {
            Error::Unreachable(error.to_string())
        }
        error => Error::Refused(error.to_string()),
    }
}

fn base64_text<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&STANDARD.encode(bytes))
}

fn base64_bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let text = String::deserialize(deserializer)?;
    STANDARD.decode(text).map_err(serde::de::Error::custom)
}
