mod failure;
mod upload;

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use discord_rate_limit::{Headers, Limiter, Route, Throttle};
use rikka_net::{Method, header};
use rikka_sync::Mutex;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::identity::{Identity, REFERER};
pub(crate) use failure::failure;
pub(crate) use upload::upload;

pub(crate) const API: &str = "https://discord.com/api/v9";
const LONGEST_WAIT: Duration = Duration::from_secs(10);
const TRIES: u32 = 3;
const BUSY_PAUSE: Duration = Duration::from_secs(1);

pub(crate) struct Token(String);

impl Token {
    pub(crate) fn new(token: impl Into<String>) -> Token {
        Token(token.into())
    }

    pub(crate) fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Token(..)")
    }
}

#[derive(Debug)]
pub(crate) struct Rest {
    http: rikka_net::Client,
    token: Arc<Token>,
    identity: Arc<Identity>,
    limiter: Mutex<Limiter>,
}

pub(crate) struct Request<'a> {
    rest: &'a Rest,
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    body: Option<Value>,
    context: Option<String>,
}

impl Rest {
    pub(crate) fn new(token: Arc<Token>, identity: Arc<Identity>) -> Result<Rest> {
        let http = rikka_net::Client::builder()
            .user_agent(crate::identity::USER_AGENT)
            .retry(rikka_net::RetryPolicy::never())
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(Error::unreachable)?;
        Ok(Rest {
            http,
            token,
            identity,
            limiter: Mutex::new(Limiter::default()),
        })
    }

    pub(crate) fn token(&self) -> &Arc<Token> {
        &self.token
    }

    pub(crate) fn identity(&self) -> &Arc<Identity> {
        &self.identity
    }

    pub(crate) fn http(&self) -> &rikka_net::Client {
        &self.http
    }

    pub(crate) fn get(&self, path: impl Into<String>) -> Request<'_> {
        self.request(Method::GET, path)
    }

    pub(crate) fn post(&self, path: impl Into<String>) -> Request<'_> {
        self.request(Method::POST, path)
    }

    pub(crate) fn put(&self, path: impl Into<String>) -> Request<'_> {
        self.request(Method::PUT, path)
    }

    pub(crate) fn patch(&self, path: impl Into<String>) -> Request<'_> {
        self.request(Method::PATCH, path)
    }

    pub(crate) fn delete(&self, path: impl Into<String>) -> Request<'_> {
        self.request(Method::DELETE, path)
    }

    fn request(&self, method: Method, path: impl Into<String>) -> Request<'_> {
        Request {
            rest: self,
            method,
            path: path.into(),
            query: Vec::new(),
            body: None,
            context: None,
        }
    }

    async fn wait_for_rate_limit(&self, route: &Route) {
        loop {
            let wait = self.limiter.lock().take(route, Instant::now());
            match wait {
                Some(wait) => tokio::time::sleep(wait).await,
                None => return,
            }
        }
    }
}

impl Request<'_> {
    #[must_use]
    pub(crate) fn query(mut self, name: &str, value: impl ToString) -> Self {
        self.query.push((name.to_owned(), value.to_string()));
        self
    }

    #[must_use]
    pub(crate) fn json(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    #[must_use]
    pub(crate) fn context(mut self, context: &Value) -> Self {
        self.context = Some(STANDARD.encode(context.to_string()));
        self
    }

    pub(crate) async fn parse<T: DeserializeOwned>(self) -> Result<T> {
        let bytes = self.send().await?;
        serde_json::from_slice(&bytes).map_err(Error::unreadable)
    }

    pub(crate) async fn done(self) -> Result<()> {
        self.send().await.map(|_| ())
    }

    pub(crate) async fn send(self) -> Result<rikka_net::Bytes> {
        self.respond().await.map(|(_, bytes)| bytes)
    }

    pub(crate) async fn respond(self) -> Result<(u16, rikka_net::Bytes)> {
        let rest = self.rest;
        let route = Route::new(self.method.as_str(), &self.path);
        let url = format!("{API}{}", self.path);
        let mut attempt = 0;
        loop {
            attempt += 1;
            rest.wait_for_rate_limit(&route).await;
            let mut builder = rest
                .http
                .request(self.method.clone(), url.as_str())
                .headers(rest.identity.headers(REFERER));
            if !rest.token.reveal().is_empty() {
                builder = builder.header(header::AUTHORIZATION, rest.token.reveal());
            }
            if !self.query.is_empty() {
                builder = builder.query(&self.query);
            }
            if let Some(context) = &self.context {
                builder = builder.header("x-context-properties", context.as_str());
            }
            if let Some(body) = &self.body {
                builder = builder.json(body);
            }
            let response = builder.send().await.map_err(Error::unreachable)?;
            let status = response.status().as_u16();
            let limits = Headers::read(|name| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
            });
            rest.limiter.lock().record(&route, &limits, Instant::now());
            let bytes = response.bytes().await.map_err(Error::unreachable)?;
            match status {
                200..=299 => return Ok((status, bytes)),
                429 => {
                    let throttle = Throttle::read(&limits, &bytes);
                    rest.limiter.lock().throttle(
                        &route,
                        throttle.retry_after,
                        throttle.scope,
                        Instant::now(),
                    );
                    if throttle.retry_after > LONGEST_WAIT || attempt >= TRIES {
                        return Err(Error::Throttled(throttle.retry_after));
                    }
                }
                502..=504 if attempt < TRIES && self.method == Method::GET => {
                    tokio::time::sleep(BUSY_PAUSE).await;
                }
                _ => return Err(failure(status, &bytes)),
            }
        }
    }
}
