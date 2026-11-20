use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use jiff::Timestamp;
use rikka_net::Client;
use rikka_sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::backoff::Backoff;
use crate::builder::ApiBuilder;
use crate::envelope;
use crate::environment::Environment;
use crate::error::{Error, Result};
use crate::request::{RawResponse, RequestBuilder, decode_json};
use crate::routing::Routing;
use crate::session::Session;
use crate::session_cookie::{self, CookieUpdate};
use crate::store::SessionStore;
use crate::version::AppVersion;

pub(crate) const ACCEPT: &str = "application/vnd.protonmail.v1+json";
pub(crate) const REFRESH_PATH: &str = "auth/v4/refresh";
const PING_PATH: &str = "tests/ping";
const REDIRECT_URI: &str = "http://protonmail.ch";
const REFRESH_REJECTED: [StatusCode; 3] = [
    StatusCode::BAD_REQUEST,
    StatusCode::UNAUTHORIZED,
    StatusCode::UNPROCESSABLE_ENTITY,
];
const UID: HeaderName = HeaderName::from_static("x-pm-uid");

#[derive(Debug, Clone)]
pub struct Api {
    inner: Arc<Inner>,
    service: Option<Environment>,
}

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) http: Client,
    pub(crate) api_http: Client,
    pub(crate) environment: Environment,
    pub(crate) app_version: AppVersion,
    pub(crate) headers: HeaderMap,
    pub(crate) timeout: Option<Duration>,
    pub(crate) backoff: Backoff,
    pub(crate) routing: Option<Routing>,
}

#[derive(Debug)]
struct Inner {
    config: Arc<Config>,
    store: Option<Arc<dyn SessionStore>>,
    state: RwLock<SessionState>,
    server_time: RwLock<Option<Timestamp>>,
    session_cookie: RwLock<Option<HeaderValue>>,
    refreshing: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default)]
struct SessionState {
    session: Option<Session>,
    generation: u64,
}

impl Inner {
    fn new(
        config: Arc<Config>,
        store: Option<Arc<dyn SessionStore>>,
        session: Option<Session>,
        server_time: Option<Timestamp>,
    ) -> Inner {
        Inner {
            config,
            store,
            state: RwLock::new(SessionState {
                session,
                generation: 0,
            }),
            server_time: RwLock::new(server_time),
            session_cookie: RwLock::new(None),
            refreshing: tokio::sync::Mutex::new(()),
        }
    }
}

impl Api {
    pub fn builder() -> ApiBuilder {
        ApiBuilder::new()
    }

    pub(crate) fn new(
        config: Config,
        store: Option<Arc<dyn SessionStore>>,
        session: Option<Session>,
    ) -> Api {
        Api {
            inner: Arc::new(Inner::new(Arc::new(config), store, session, None)),
            service: None,
        }
    }

    pub fn get(&self, path: impl Into<String>) -> RequestBuilder {
        self.request(Method::GET, path)
    }

    pub fn post(&self, path: impl Into<String>) -> RequestBuilder {
        self.request(Method::POST, path)
    }

    pub fn put(&self, path: impl Into<String>) -> RequestBuilder {
        self.request(Method::PUT, path)
    }

    pub fn patch(&self, path: impl Into<String>) -> RequestBuilder {
        self.request(Method::PATCH, path)
    }

    pub fn delete(&self, path: impl Into<String>) -> RequestBuilder {
        self.request(Method::DELETE, path)
    }

    pub fn request(&self, method: Method, path: impl Into<String>) -> RequestBuilder {
        RequestBuilder::new(self.clone(), method, path.into())
    }

    #[must_use]
    pub fn environment(&self) -> &Environment {
        self.service
            .as_ref()
            .unwrap_or(&self.inner.config.environment)
    }

    #[must_use]
    pub fn with_environment(&self, environment: Environment) -> Api {
        Api {
            inner: self.inner.clone(),
            service: Some(environment),
        }
    }

    #[must_use]
    pub fn app_version(&self) -> &AppVersion {
        &self.inner.config.app_version
    }

    #[must_use]
    pub fn http(&self) -> &Client {
        &self.inner.config.http
    }

    #[must_use]
    pub fn server_time(&self) -> Option<Timestamp> {
        *self.inner.server_time.read()
    }

    #[must_use]
    pub fn session(&self) -> Option<Session> {
        self.inner.state.read().session.clone()
    }

    pub fn set_session(&self, session: Session) -> Result<()> {
        if let Some(store) = &self.inner.store {
            store.save(&session)?;
        }
        self.replace_session(Some(session));
        Ok(())
    }

    pub fn clear_session(&self) -> Result<()> {
        self.replace_session(None);
        match &self.inner.store {
            Some(store) => store.remove(),
            None => Ok(()),
        }
    }

    pub fn with_session_store(&self, store: impl SessionStore) -> Result<Api> {
        let store: Arc<dyn SessionStore> = Arc::new(store);
        let session = store.load()?;
        Ok(Api {
            inner: Arc::new(Inner::new(
                Arc::clone(&self.inner.config),
                Some(store),
                session,
                self.server_time(),
            )),
            service: self.service.clone(),
        })
    }

    pub async fn warm_up(&self) {
        let ping = self
            .get(PING_PATH)
            .anonymous()
            .without_renewal()
            .send_bytes()
            .await;
        if let Err(error) = ping {
            tracing::debug!(%error, "failed to warm up API connection");
        }
    }

    pub async fn refresh(&self) -> Result<Session> {
        let generation = self.inner.state.read().generation;
        self.refresh_from(generation).await
    }

    pub(crate) async fn refresh_from(&self, generation: u64) -> Result<Session> {
        let _refresh_guard = self.inner.refreshing.lock().await;

        {
            let state = self.inner.state.read();
            if state.generation != generation {
                return state.session.clone().ok_or(Error::SessionExpired);
            }
        }

        let session = self.session().ok_or(Error::MissingSession)?;
        if !session.is_valid() {
            self.expire_session();
            return Err(Error::SessionExpired);
        }

        match self.request_tokens(&session).await {
            Ok(renewed) => {
                tracing::debug!("refreshed session tokens");
                self.set_session(renewed.clone())?;
                Ok(renewed)
            }
            Err(Error::SessionExpired) => {
                tracing::info!("refresh token rejected, session cleared");
                self.expire_session();
                Err(Error::SessionExpired)
            }
            Err(error) => Err(error),
        }
    }

    fn request_tokens<'a>(
        &'a self,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<Session>> + Send + 'a>> {
        Box::pin(async move {
            let refresh_request = RefreshRequest {
                uid: session.uid(),
                refresh_token: session.refresh_token(),
                response_type: "token",
                grant_type: "refresh_token",
                redirect_uri: REDIRECT_URI,
            };

            let RawResponse {
                status,
                retry_after,
                body,
                ..
            } = self
                .post(REFRESH_PATH)
                .json(&refresh_request)
                .without_renewal()
                .send_raw()
                .await?;

            if REFRESH_REJECTED.contains(&status) {
                return Err(Error::SessionExpired);
            }
            if !status.is_success() {
                return Err(envelope::classify(status, retry_after, &body));
            }

            let response: RefreshResponse = decode_json(&format!("POST {REFRESH_PATH}"), &body)?;

            let uid = response.uid.unwrap_or_else(|| session.uid().to_owned());
            let mut renewed = Session::new(uid, response.access_token, response.refresh_token)
                .with_scopes(response.scopes);

            if let Some(user_id) = session.user_id() {
                renewed = renewed.with_user_id(user_id);
            }

            Ok(renewed)
        })
    }

    pub(crate) fn session_state(&self) -> (u64, Option<Session>) {
        let state = self.inner.state.read();
        (state.generation, state.session.clone())
    }

    pub(crate) fn config(&self) -> &Config {
        &self.inner.config
    }

    pub(crate) fn routing(&self) -> Option<&Routing> {
        match self.service {
            Some(_) => None,
            None => self.inner.config.routing.as_ref(),
        }
    }

    pub(crate) fn base(&self) -> Environment {
        if let Some(service) = self.service.as_ref() {
            return service.clone();
        }

        self.routing()
            .and_then(Routing::active)
            .unwrap_or_else(|| self.inner.config.environment.clone())
    }

    pub(crate) fn update_server_time(&self, headers: &HeaderMap) {
        let Some(date) = headers
            .get(header::DATE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| jiff::fmt::rfc2822::parse(value).ok())
        else {
            return;
        };

        *self.inner.server_time.write() = Some(date.timestamp());
    }

    pub(crate) fn update_session_cookie(&self, headers: &HeaderMap) {
        match session_cookie::latest_update(headers) {
            Some(CookieUpdate::Set(value)) => {
                *self.inner.session_cookie.write() = session_cookie::cookie_header(&value);
            }
            Some(CookieUpdate::Clear) => *self.inner.session_cookie.write() = None,
            None => {}
        }
    }

    pub(crate) fn session_cookie(&self) -> Option<HeaderValue> {
        self.inner.session_cookie.read().clone()
    }

    pub(crate) fn headers(&self, session: Option<&Session>) -> HeaderMap {
        let mut headers = self.inner.config.headers.clone();

        if let Some(session) = session {
            if !session.uid().is_empty() {
                insert_header(&mut headers, UID, session.uid());
            }
            if !session.access_token().is_empty() {
                insert_header(
                    &mut headers,
                    header::AUTHORIZATION,
                    &format!("Bearer {}", session.access_token()),
                );
            }
        }

        headers
    }

    fn replace_session(&self, session: Option<Session>) {
        let mut state = self.inner.state.write();
        state.session = session;
        state.generation = state.generation.wrapping_add(1);
    }

    fn expire_session(&self) {
        if let Err(error) = self.clear_session() {
            tracing::warn!(%error, "failed to remove stored session");
        }
    }
}

fn insert_header(headers: &mut HeaderMap, name: HeaderName, value: &str) {
    match HeaderValue::from_str(value) {
        Ok(mut value) => {
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Err(_) => tracing::warn!(header = %name, "dropping invalid header value"),
    }
}

#[derive(Debug, Serialize)]
struct RefreshRequest<'a> {
    #[serde(rename = "UID")]
    uid: &'a str,
    #[serde(rename = "RefreshToken")]
    refresh_token: &'a str,
    #[serde(rename = "ResponseType")]
    response_type: &'a str,
    #[serde(rename = "GrantType")]
    grant_type: &'a str,
    #[serde(rename = "RedirectURI")]
    redirect_uri: &'a str,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    #[serde(rename = "UID", default)]
    uid: Option<String>,
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "RefreshToken")]
    refresh_token: String,
    #[serde(
        rename = "Scopes",
        default,
        deserialize_with = "crate::scopes::deserialize"
    )]
    scopes: Vec<String>,
}
