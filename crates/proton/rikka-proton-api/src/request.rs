use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;
use url::form_urlencoded::Serializer;

use crate::api::Api;
use crate::backoff::Attempt;
use crate::code::ResponseCode;
use crate::envelope::{self, Envelope};
use crate::environment::Environment;
use crate::error::{Error, Result};
use crate::field;
use crate::multipart::Multipart;
use crate::paging::{Pages, Paging};
use crate::response::Response;
use crate::routing::{ALTERNATIVES_TIMEOUT, Routing};
use crate::session::Session;

const OCTET_STREAM: &str = "application/octet-stream";

#[derive(Debug)]
pub(crate) struct RawResponse {
    pub(crate) status: StatusCode,
    pub(crate) retry_after: Option<Duration>,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponseKind {
    Envelope,
    Raw,
}

#[derive(Debug, Clone)]
enum Payload {
    Json(Bytes),
    Multipart {
        content_type: HeaderValue,
        body: Bytes,
    },
    Raw(Bytes),
}

#[must_use]
pub struct RequestBuilder {
    api: Api,
    method: Method,
    path: String,
    query: Vec<String>,
    headers: HeaderMap,
    payload: Option<Payload>,
    timeout: Option<Duration>,
    anonymous: bool,
    renewable: bool,
    error: Option<Error>,
}

impl RequestBuilder {
    pub(crate) fn new(api: Api, method: Method, path: String) -> RequestBuilder {
        RequestBuilder {
            api,
            method,
            path,
            query: Vec::new(),
            headers: HeaderMap::new(),
            payload: None,
            timeout: None,
            anonymous: false,
            renewable: true,
            error: None,
        }
    }

    pub fn query<K: AsRef<str>, V: fmt::Display>(mut self, pairs: &[(K, V)]) -> Self {
        if pairs.is_empty() {
            return self;
        }
        let mut encoded = Serializer::new(String::new());
        for (name, value) in pairs {
            encoded.append_pair(name.as_ref(), &value.to_string());
        }
        self.query.push(encoded.finish());
        self
    }

    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        let (Ok(name), Ok(value)) = (name.try_into(), value.try_into()) else {
            return self.fail(Error::InvalidHeader);
        };

        self.headers.insert(name, value);
        self
    }

    pub fn json<T: Serialize + ?Sized>(mut self, body: &T) -> Self {
        match serde_json::to_vec(body) {
            Ok(encoded) => {
                self.payload = Some(Payload::Json(Bytes::from(encoded)));
                self
            }
            Err(error) => self.fail(Error::Encode(error)),
        }
    }

    pub fn bytes(mut self, body: impl Into<Bytes>) -> Self {
        self.payload = Some(Payload::Raw(body.into()));
        self
    }

    pub fn multipart(mut self, form: Multipart) -> Self {
        match form.encode() {
            Ok((content_type, body)) => {
                self.payload = Some(Payload::Multipart { content_type, body });
                self
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn anonymous(mut self) -> Self {
        self.anonymous = true;
        self
    }

    pub(crate) fn without_renewal(mut self) -> Self {
        self.renewable = false;
        self
    }

    pub fn pages(self, paging: Paging) -> Pages {
        Pages::new(self, paging)
    }

    pub(crate) fn into_validated(mut self) -> Result<RequestBuilder> {
        match self.error.take() {
            Some(error) => Err(error),
            None => Ok(self),
        }
    }

    pub(crate) fn replicate(&self) -> RequestBuilder {
        RequestBuilder {
            api: self.api.clone(),
            method: self.method.clone(),
            path: self.path.clone(),
            query: self.query.clone(),
            headers: self.headers.clone(),
            payload: self.payload.clone(),
            timeout: self.timeout,
            anonymous: self.anonymous,
            renewable: self.renewable,
            error: None,
        }
    }

    pub async fn send<T: DeserializeOwned>(self) -> Result<T> {
        let request_line = self.request_line();
        let raw = self.send_with(ResponseKind::Envelope).await?;

        decode_json(&request_line, &raw.body)
    }

    pub async fn send_field<T: DeserializeOwned>(self, name: &str) -> Result<T> {
        self.send_field_at(&[name]).await
    }

    pub async fn send_field_at<T: DeserializeOwned>(self, path: &[&str]) -> Result<T> {
        let request = self.request_line();
        let raw = self.send_with(ResponseKind::Envelope).await?;

        field::decode(path, &raw.body).map_err(|source| Error::Decode { request, source })
    }

    pub async fn send_field_or_default<T: DeserializeOwned + Default>(
        self,
        name: &str,
    ) -> Result<T> {
        let request = self.request_line();
        let raw = self.send_with(ResponseKind::Envelope).await?;

        field::decode_or_default(&[name], &raw.body)
            .map_err(|source| Error::Decode { request, source })
    }

    pub async fn send_bytes(self) -> Result<Bytes> {
        self.send_with(ResponseKind::Raw).await.map(|raw| raw.body)
    }

    pub async fn send_unit(self) -> Result<()> {
        self.send_with(ResponseKind::Envelope).await.map(|_| ())
    }

    pub async fn receive<T: DeserializeOwned>(self) -> Result<Response<T>> {
        let request_line = self.request_line();
        let raw = self.send_with(ResponseKind::Envelope).await?;
        let value = decode_json(&request_line, &raw.body)?;

        Ok(Response::new(value, raw.headers))
    }

    pub(crate) async fn send_raw(self) -> Result<RawResponse> {
        self.run().await
    }

    async fn send_with(self, kind: ResponseKind) -> Result<RawResponse> {
        check_envelope(self.run().await?, kind)
    }

    fn request_line(&self) -> String {
        format!("{} {}", self.method, self.path)
    }

    fn fail(mut self, error: Error) -> Self {
        if self.error.is_none() {
            self.error = Some(error);
        }
        self
    }

    async fn run(self) -> Result<RawResponse> {
        if let Some(error) = self.error {
            return Err(error);
        }

        let api = self.api.clone();
        let backoff = api.config().backoff;
        let started = Instant::now();
        let absolute = is_absolute(&self.path);

        let mut base = api.base();
        let mut attempt = 0u32;
        let mut renewed = false;
        let mut alternatives: Option<VecDeque<Environment>> = None;

        loop {
            let (generation, session) = api.session_state();
            let session = session.filter(|_| !self.anonymous && !absolute);
            let url = request_url(&base, &self.path, &self.query)?;
            let route = route(&api, &base, absolute);

            let result = self.dispatch(&api, url, session.as_ref(), &route).await;

            let failure = match result {
                Ok(response) => {
                    let status = response.status();
                    let retry_after = parse_retry_after(response.headers());
                    let headers = response.headers().clone();
                    record_response(&api, &headers, status, absolute);
                    if let Route::Alternative(routing) = route {
                        routing.set_active(base.clone());
                    }

                    let body = response.bytes().await?;

                    if status == StatusCode::UNAUTHORIZED
                        && self.renewable
                        && !renewed
                        && session.is_some()
                    {
                        renewed = true;
                        api.refresh_from(generation).await?;
                        continue;
                    }

                    if backoff.should_retry(attempt, Attempt::Status(status), retry_after) {
                        let delay = backoff.delay(attempt, retry_after);
                        tracing::debug!(%status, attempt, ?delay, "retrying API request");
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    return Ok(RawResponse {
                        status,
                        retry_after,
                        headers,
                        body,
                    });
                }
                Err(failure) if failure.is_transient() => failure,
                Err(failure) => return Err(Error::Network(failure)),
            };

            if !absolute
                && let Some(next) = next_alternative(&api, &base, started, &mut alternatives).await
            {
                tracing::debug!(route = %next, "trying alternative route");
                base = next;
                continue;
            }

            if backoff.should_retry(attempt, Attempt::ConnectionFailed, None) {
                let delay = backoff.delay(attempt, None);
                tracing::debug!(%failure, attempt, ?delay, "API unreachable, retrying");
                tokio::time::sleep(delay).await;
                attempt += 1;
                base = api.base();
                continue;
            }

            return Err(match alternatives.is_some() {
                true => Error::NoRoute,
                false => Error::Network(failure),
            });
        }
    }

    async fn dispatch(
        &self,
        api: &Api,
        url: Url,
        session: Option<&Session>,
        route: &Route<'_>,
    ) -> rikka_net::Result<rikka_net::Response> {
        let client = match route {
            Route::Api => &api.config().api_http,
            Route::Alternative(routing) => routing.client(),
            Route::External => api.http(),
        };

        let mut request_headers = self.headers.clone();
        if matches!(
            self.payload,
            Some(Payload::Json(_) | Payload::Multipart { .. })
        ) {
            request_headers.remove(header::CONTENT_TYPE);
        }

        let mut request = client
            .request(self.method.clone(), url)
            .headers(api.headers(session))
            .headers(request_headers);

        if let Route::Alternative(_) = route
            && let Some(cookie) = api.session_cookie()
        {
            request = request.header(header::COOKIE, cookie);
        }

        if let Some(timeout) = self.timeout.or(api.config().timeout) {
            request = request.timeout(timeout);
        }

        request = match &self.payload {
            Some(Payload::Json(body)) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(body.clone()),
            Some(Payload::Multipart { content_type, body }) => request
                .header(header::CONTENT_TYPE, content_type.clone())
                .body(body.clone()),
            Some(Payload::Raw(body)) => match self.headers.contains_key(header::CONTENT_TYPE) {
                true => request.body(body.clone()),
                false => request
                    .header(header::CONTENT_TYPE, OCTET_STREAM)
                    .body(body.clone()),
            },
            None => request,
        };

        request.send().await
    }
}

enum Route<'a> {
    Api,
    Alternative(&'a Routing),
    External,
}

fn route<'a>(api: &'a Api, base: &Environment, absolute: bool) -> Route<'a> {
    if absolute {
        return Route::External;
    }
    match api.routing() {
        Some(routing) if *base != api.config().environment => Route::Alternative(routing),
        _ => Route::Api,
    }
}

fn is_absolute(path: &str) -> bool {
    Url::parse(path).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

impl fmt::Debug for RequestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestBuilder")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("anonymous", &self.anonymous)
            .finish_non_exhaustive()
    }
}

pub(crate) fn decode_json<T: DeserializeOwned>(request: &str, body: &[u8]) -> Result<T> {
    serde_json::from_slice(body).map_err(|source| Error::Decode {
        request: request.to_owned(),
        source,
    })
}

fn check_envelope(raw: RawResponse, kind: ResponseKind) -> Result<RawResponse> {
    let RawResponse {
        status,
        retry_after,
        body,
        ..
    } = &raw;

    if !status.is_success() {
        return Err(envelope::classify(*status, *retry_after, body));
    }

    if kind == ResponseKind::Envelope
        && Envelope::read(body)
            .is_some_and(|envelope| !envelope.code().is_some_and(ResponseCode::is_ok))
    {
        return Err(envelope::classify(*status, *retry_after, body));
    }

    Ok(raw)
}

fn request_url(base: &Environment, path: &str, query: &[String]) -> Result<Url> {
    let mut url = base.join(path)?;

    let current_query = url.query().map(str::to_owned);
    let merged: Vec<&str> = current_query
        .iter()
        .map(String::as_str)
        .chain(query.iter().map(String::as_str))
        .filter(|fragment| !fragment.is_empty())
        .collect();

    match merged.is_empty() {
        true => url.set_query(None),
        false => url.set_query(Some(&merged.join("&"))),
    }

    Ok(url)
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?.trim();

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let deadline = jiff::fmt::rfc2822::parse(value).ok()?.timestamp();
    let milliseconds = deadline.as_millisecond() - jiff::Timestamp::now().as_millisecond();
    Some(Duration::from_millis(milliseconds.max(0).unsigned_abs()))
}

fn record_response(api: &Api, headers: &HeaderMap, status: StatusCode, absolute: bool) {
    if !absolute {
        api.update_session_cookie(headers);
    }
    if status.is_success() {
        api.update_server_time(headers);
    }
}

async fn next_alternative(
    api: &Api,
    failed: &Environment,
    started: Instant,
    alternatives: &mut Option<VecDeque<Environment>>,
) -> Option<Environment> {
    let routing = api.routing()?;
    routing.invalidate(failed);
    if started.elapsed() >= ALTERNATIVES_TIMEOUT {
        return None;
    }
    if alternatives.is_none() {
        let uid = api.session().map(|session| session.uid().to_owned());
        *alternatives = Some(
            routing
                .alternatives(&api.config().environment, uid.as_deref())
                .await
                .into(),
        );
    }
    alternatives.as_mut().and_then(VecDeque::pop_front)
}
