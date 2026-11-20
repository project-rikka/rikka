use std::sync::Arc;
use std::time::Duration;

use music_spotify_wire::{DeviceId, StoredCredentials};
use rikka_net::{Bytes, HeaderName, Method, header};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::client_token::ClientTokenExchange;
use crate::error::{Error, Result};
use crate::identity::USER_AGENT;
use crate::login5::Login5Exchange;
use crate::platform::Platform;
use crate::token::{Progress, Token};

const CLIENT_TOKEN_URL: &str = "https://clienttoken.spotify.com/v1/clienttoken";
const LOGIN5_URL: &str = "https://login5.spotify.com/v3/login";
const RESOLVER: &str = "https://apresolve.spotify.com/?type=spclient";
const FALLBACK: &str = "spclient.wg.spotify.com:443";
const CLIENT_TOKEN: HeaderName = HeaderName::from_static("client-token");
const PATIENCE: Duration = Duration::from_secs(20);

pub(crate) const PROTOBUF: &str = "application/x-protobuf";
pub(crate) const JSON: &str = "application/json";

#[derive(Clone)]
pub struct SpClient(Arc<Inner>);

struct Inner {
    http: rikka_net::Client,
    device: DeviceId,
    credentials: StoredCredentials,
    platform: Platform,
    base: Mutex<Option<String>>,
    client_token: Mutex<Option<Token>>,
    access_token: Mutex<Option<Token>>,
}

pub(crate) struct Call<'a> {
    pub(crate) method: Method,
    pub(crate) path: &'a str,
    pub(crate) accept: Option<&'static str>,
    pub(crate) body: Option<(&'static str, Vec<u8>)>,
}

#[derive(Deserialize)]
struct Resolved {
    #[serde(default)]
    spclient: Vec<String>,
}

trait TokenExchange {
    fn request(&self) -> Vec<u8>;
    fn answer(&mut self, response: &[u8]) -> Result<Progress>;
}

impl TokenExchange for ClientTokenExchange {
    fn request(&self) -> Vec<u8> {
        ClientTokenExchange::request(self)
    }

    fn answer(&mut self, response: &[u8]) -> Result<Progress> {
        ClientTokenExchange::answer(self, response)
    }
}

impl TokenExchange for Login5Exchange {
    fn request(&self) -> Vec<u8> {
        Login5Exchange::request(self)
    }

    fn answer(&mut self, response: &[u8]) -> Result<Progress> {
        Login5Exchange::answer(self, response)
    }
}

impl SpClient {
    #[must_use]
    pub fn new(device: DeviceId, credentials: StoredCredentials) -> SpClient {
        SpClient(Arc::new(Inner {
            http: rikka_net::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(PATIENCE)
                .build()
                .unwrap_or_default(),
            device,
            credentials,
            platform: Platform::current(),
            base: Mutex::new(None),
            client_token: Mutex::new(None),
            access_token: Mutex::new(None),
        }))
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.0.credentials.username
    }

    pub(crate) async fn call(&self, call: Call<'_>) -> Result<Bytes> {
        let base = self.base().await;
        let url = format!("{base}{}", call.path);
        let access_token = self.access_token().await?;
        let client_token = self.client_token().await?;
        let mut request = self
            .0
            .http
            .request(call.method, url.as_str())
            .bearer_auth(access_token)
            .header(CLIENT_TOKEN, client_token);
        if let Some(accept) = call.accept {
            request = request.header(header::ACCEPT, accept);
        }
        if let Some((content_type, body)) = call.body {
            request = request
                .header(header::CONTENT_TYPE, content_type)
                .body(body);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                if error.is_connect() {
                    *self.0.base.lock().await = None;
                }
                return Err(Error::Unreachable(error));
            }
        };
        response
            .error_for_status()
            .await
            .map_err(Error::from_net(call.path))?
            .bytes()
            .await
            .map_err(Error::from_net(call.path))
    }

    async fn base(&self) -> String {
        let mut base = self.0.base.lock().await;
        if let Some(known) = base.as_ref() {
            return known.clone();
        }
        let address = match self.0.http.get_json::<Resolved>(RESOLVER).await {
            Ok(resolved) => resolved.spclient.into_iter().next(),
            Err(error) => {
                tracing::warn!("failed to resolve a Spotify spclient host: {error}");
                None
            }
        }
        .unwrap_or_else(|| FALLBACK.to_owned());
        let resolved = format!("https://{address}");
        *base = Some(resolved.clone());
        resolved
    }

    async fn client_token(&self) -> Result<String> {
        let mut held = self.0.client_token.lock().await;
        if let Some(token) = held.as_ref().filter(|token| token.is_fresh()) {
            return Ok(token.value().to_owned());
        }
        let mut exchange = ClientTokenExchange::new(&self.0.device, &self.0.platform);
        let token = self.obtain(CLIENT_TOKEN_URL, &mut exchange, None).await?;
        let value = token.value().to_owned();
        *held = Some(token);
        Ok(value)
    }

    async fn access_token(&self) -> Result<String> {
        let mut held = self.0.access_token.lock().await;
        if let Some(token) = held.as_ref().filter(|token| token.is_fresh()) {
            return Ok(token.value().to_owned());
        }
        let client_token = self.client_token().await?;
        let mut exchange = Login5Exchange::new(&self.0.device, &self.0.credentials);
        let token = self
            .obtain(LOGIN5_URL, &mut exchange, Some(&client_token))
            .await?;
        let value = token.value().to_owned();
        *held = Some(token);
        Ok(value)
    }

    async fn obtain(
        &self,
        url: &str,
        exchange: &mut impl TokenExchange,
        client_token: Option<&str>,
    ) -> Result<Token> {
        loop {
            let mut request = self
                .0
                .http
                .post(url)
                .header(header::ACCEPT, PROTOBUF)
                .body(exchange.request());
            if let Some(client_token) = client_token {
                request = request.header(CLIENT_TOKEN, client_token);
            }
            let answer = request
                .send()
                .await
                .map_err(Error::Unreachable)?
                .error_for_status()
                .await
                .map_err(Error::from_net(url))?
                .bytes()
                .await
                .map_err(Error::from_net(url))?;
            match exchange.answer(&answer)? {
                Progress::Granted(token) => return Ok(token),
                Progress::Challenged => {}
            }
        }
    }
}

impl std::fmt::Debug for SpClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpClient")
            .field("username", &self.0.credentials.username)
            .finish_non_exhaustive()
    }
}
