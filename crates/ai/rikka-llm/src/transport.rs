use std::fmt::Debug;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use futures_util::{Stream as FuturesStream, StreamExt};
use rikka_net::{
    Bytes, Client, EventDecoder, HeaderMap, HeaderValue, RequestBuilder, Response, RetryPolicy,
    Url, header,
};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::failure::{self, Failure};
use crate::provider::Call;
use crate::request::Request;
use crate::stop::Stop;

const READ_TIMEOUT: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

pub fn client(read: Duration, connect: Duration) -> Result<Client> {
    Client::builder()
        .read_timeout(read)
        .connect_timeout(connect)
        .cookies(false)
        .retry(RetryPolicy::new().non_idempotent(true))
        .build()
        .map_err(failure::translate)
}

fn shared_client() -> Result<Client> {
    static SHARED: OnceLock<Client> = OnceLock::new();
    if let Some(shared) = SHARED.get() {
        return Ok(shared.clone());
    }
    let built = client(READ_TIMEOUT, CONNECT_TIMEOUT)?;
    Ok(SHARED.get_or_init(|| built).clone())
}

pub(crate) type Events = Pin<Box<dyn FuturesStream<Item = Result<rikka_net::Event>> + Send>>;

pub trait Authorize: Debug + Send + Sync + 'static {
    fn headers(&self) -> Call<'_, HeaderMap>;
}

#[derive(Debug, Clone)]
pub struct Transport {
    client: Client,
    endpoint: Url,
    headers: HeaderMap,
    authorize: Option<Arc<dyn Authorize>>,
}

impl Transport {
    pub fn new(endpoint: Url, headers: HeaderMap) -> Result<Transport> {
        Transport::build(endpoint, headers, None)
    }

    pub fn authorized(
        endpoint: Url,
        headers: HeaderMap,
        authorize: Arc<dyn Authorize>,
    ) -> Result<Transport> {
        Transport::build(endpoint, headers, Some(authorize))
    }

    fn build(
        endpoint: Url,
        headers: HeaderMap,
        authorize: Option<Arc<dyn Authorize>>,
    ) -> Result<Transport> {
        Ok(Transport {
            client: shared_client()?,
            endpoint,
            headers,
            authorize,
        })
    }

    pub(crate) fn with_endpoint(&self, endpoint: Url) -> Transport {
        Transport {
            endpoint,
            ..self.clone()
        }
    }

    pub(crate) fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    async fn assembled(&self, extra: &HeaderMap) -> Result<HeaderMap> {
        let mut headers = self.headers.clone();

        if let Some(authorize) = &self.authorize {
            for (name, value) in authorize.headers().await? {
                if let Some(name) = name {
                    headers.insert(name, value);
                }
            }
        }
        for (name, value) in extra {
            headers.insert(name.clone(), value.clone());
        }

        Ok(headers)
    }

    pub(crate) async fn get(&self) -> Result<Value> {
        let call = self
            .client
            .get(self.endpoint.clone())
            .headers(self.assembled(&HeaderMap::new()).await?);
        Ok(read(call).await?)
    }

    pub(crate) async fn post(
        &self,
        body: &Value,
        request: &Request,
        extra: &HeaderMap,
    ) -> Result<Value> {
        Ok(self.try_post(body, request, extra).await?)
    }

    pub(crate) async fn try_post(
        &self,
        body: &Value,
        request: &Request,
        extra: &HeaderMap,
    ) -> Result<Value, Failure> {
        let mut call = self
            .client
            .post(self.endpoint.clone())
            .headers(self.assembled(extra).await?)
            .json(body);

        if let Some(token) = &request.cancel {
            call = call.cancel_token(token.clone());
        }
        if let Some(timeout) = request.timeout {
            call = call.timeout(timeout);
        }

        read(call).await
    }

    pub(crate) async fn events(
        &self,
        body: &Value,
        request: &Request,
        extra: &HeaderMap,
    ) -> Result<Events> {
        Ok(self.try_events(body, request, extra).await?)
    }

    pub(crate) async fn try_events(
        &self,
        body: &Value,
        request: &Request,
        extra: &HeaderMap,
    ) -> Result<Events, Failure> {
        let mut headers = self.assembled(extra).await?;
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("identity"),
        );

        let mut call = self
            .client
            .post(self.endpoint.clone())
            .headers(headers)
            .json(body);

        if let Some(token) = &request.cancel {
            call = call.cancel_token(token.clone());
        }
        if let Some(timeout) = request.timeout {
            call = call.timeout(timeout);
        }

        Ok(decode(sent(call).await?.bytes_stream()))
    }
}

async fn sent(call: RequestBuilder) -> Result<Response, Failure> {
    Ok(call.send().await?.error_for_status().await?)
}

async fn read(call: RequestBuilder) -> Result<Value, Failure> {
    Ok(sent(call).await?.json().await?)
}

struct Decoding<S> {
    bytes: Pin<Box<S>>,
    decoder: EventDecoder,
    drained: bool,
}

fn decode<S>(bytes: S) -> Events
where
    S: FuturesStream<Item = rikka_net::Result<Bytes>> + Send + 'static,
{
    let state = Decoding {
        bytes: Box::pin(bytes),
        decoder: EventDecoder::new(),
        drained: false,
    };

    Box::pin(futures_util::stream::unfold(
        state,
        |mut state| async move {
            loop {
                if let Some(event) = state.decoder.next_event() {
                    return Some((Ok(event), state));
                }
                if state.drained {
                    return None;
                }

                match state.bytes.next().await {
                    Some(Ok(chunk)) => state.decoder.push(&chunk),
                    Some(Err(error)) => {
                        state.drained = true;
                        return Some((Err(failure::translate(error)), state));
                    }
                    None => state.drained = true,
                }
            }
        },
    ))
}

pub fn text_of(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

pub fn count_of(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or_default()
}

pub(crate) fn unless_cut_short<T>(called: Result<T>, stop: &Stop) -> Result<Option<T>> {
    match called {
        Ok(call) => Ok(Some(call)),
        Err(_) if *stop == Stop::MaxTokens => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn arguments(text: &str) -> Result<Value> {
    if text.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }

    serde_json::from_str(text)
        .map_err(|error| Error::Malformed(format!("a tool call's arguments are not JSON: {error}")))
}
