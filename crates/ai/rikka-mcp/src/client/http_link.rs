use rikka_sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use rikka_net::{
    CancelToken, Client, HeaderMap, Method, RequestBuilder, Response, StatusCode, Url, header,
};
use rikka_oauth::Challenge;
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::client::event_stream;
use crate::client::link::{Arrival, LinkError};
use crate::credentials::Signing;
use crate::protocol::{Message, method};

const SESSION_ID: &str = "mcp-session-id";
const PROTOCOL_VERSION: &str = "mcp-protocol-version";
const LAST_EVENT_ID: &str = "last-event-id";
const JSON: &str = "application/json";
const EVENT_STREAM: &str = "text/event-stream";
const ANSWERS: &str = "application/json, text/event-stream";

const CONNECTING: Duration = Duration::from_secs(30);
const RENEWING: Duration = Duration::from_secs(60);
const CLOSING: Duration = Duration::from_secs(5);

pub(crate) struct HttpLink {
    shared: Arc<Shared>,
    runtime: Handle,
}

pub(super) struct Shared {
    http: Client,
    url: Url,
    headers: HeaderMap,
    signing: Option<Signing>,
    arriving: mpsc::Sender<Arrival>,
    state: Mutex<State>,
    renewing: tokio::sync::Mutex<()>,
    cancel: CancelToken,
}

#[derive(Default)]
struct State {
    attached: Attached,
    greeting: Option<Message>,
    listeners: u64,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Attached {
    session: Option<String>,
    revision: Option<String>,
}

impl HttpLink {
    pub(crate) fn new(
        url: &str,
        headers: HeaderMap,
        signing: Option<Signing>,
        arriving: mpsc::Sender<Arrival>,
    ) -> Result<HttpLink, LinkError> {
        let url = Url::parse(url)
            .map_err(|error| rikka_net::Error::InvalidUrl(format!("\"{url}\": {error}")))?;
        let http = Client::builder()
            .connect_timeout(CONNECTING)
            .cookies(false)
            .build()?;

        Ok(HttpLink {
            shared: Arc::new(Shared {
                http,
                url,
                headers,
                signing,
                arriving,
                state: Mutex::default(),
                renewing: tokio::sync::Mutex::default(),
                cancel: CancelToken::new(),
            }),
            runtime: Handle::current(),
        })
    }

    pub(crate) async fn send(&self, message: &Message) -> Result<(), LinkError> {
        let greeting = message.is_request(method::INITIALIZE);
        if greeting {
            let mut state = self.shared.state.lock();
            state.attached = Attached::default();
            state.greeting = Some(message.clone());
        }

        let attached = self.shared.attached();
        match self.shared.deliver(message, &attached).await {
            Err(LinkError::SessionExpired) if !greeting => {
                let renewing = Arc::clone(&self.shared).renew(attached);
                self.runtime
                    .spawn(async move { tokio::time::timeout(RENEWING, renewing).await })
                    .await
                    .map_err(|_| LinkError::SessionExpired)?
                    .map_err(|_| LinkError::SessionExpired)??;
                let attached = self.shared.attached();
                self.shared.deliver(message, &attached).await
            }
            delivered => delivered,
        }
    }

    pub(crate) fn negotiated(&self, revision: &str) {
        self.shared.state.lock().attached.revision = Some(revision.to_owned());
    }

    pub(crate) fn listen(&self) {
        self.shared.listen(&self.runtime);
    }
}

impl Drop for HttpLink {
    fn drop(&mut self) {
        self.shared.cancel.cancel();
        let shared = Arc::clone(&self.shared);
        self.runtime.spawn(async move { shared.close().await });
    }
}

impl Shared {
    pub(super) async fn arrive(&self, arrival: Arrival) -> bool {
        self.arriving.send(arrival).await.is_ok()
    }

    pub(super) fn is_listening(&self, listener: u64) -> bool {
        !self.cancel.is_cancelled() && self.state.lock().listeners == listener
    }

    pub(super) async fn paused(&self, pause: Duration) -> bool {
        tokio::select! {
            () = self.cancel.cancelled() => false,
            () = tokio::time::sleep(pause) => true,
        }
    }

    pub(super) async fn stream(&self, last_event_id: Option<&str>) -> Result<Response, LinkError> {
        let attached = self.attached();
        let mut request = self
            .request(Method::GET, &attached)
            .await?
            .cancel_token(self.cancel.clone())
            .header(header::ACCEPT, EVENT_STREAM);
        if let Some(last_event_id) = last_event_id {
            request = request.header(LAST_EVENT_ID, last_event_id);
        }

        let response = self.checked(request.send().await?, &attached).await?;
        match Answer::of(&response) {
            Answer::Events => Ok(response),
            Answer::Nothing => Err(LinkError::ContentType(String::from("nothing"))),
            Answer::Json => Err(LinkError::ContentType(JSON.to_owned())),
            Answer::Other(content_type) => Err(LinkError::ContentType(content_type)),
        }
    }

    fn attached(&self) -> Attached {
        self.state.lock().attached.clone()
    }

    fn listen(self: &Arc<Self>, runtime: &Handle) {
        let listener = {
            let mut state = self.state.lock();
            state.listeners += 1;
            state.listeners
        };
        runtime.spawn(event_stream::listen(Arc::clone(self), listener));
    }

    async fn deliver(
        self: &Arc<Self>,
        message: &Message,
        attached: &Attached,
    ) -> Result<(), LinkError> {
        let response = self.post(message, attached).await?;
        if message.is_request(method::INITIALIZE) {
            self.state.lock().attached.session = text(&response, SESSION_ID);
        }
        let Some(awaited) = message.request_id() else {
            return Ok(());
        };

        match Answer::of(&response) {
            Answer::Nothing => Ok(()),
            Answer::Json => {
                for message in messages(response).await? {
                    self.arrive(Arrival::Message(message)).await;
                }
                Ok(())
            }
            Answer::Events => {
                tokio::spawn(event_stream::answer(
                    Arc::clone(self),
                    response,
                    awaited.clone(),
                ));
                Ok(())
            }
            Answer::Other(content_type) => Err(LinkError::ContentType(content_type)),
        }
    }

    async fn renew(self: Arc<Self>, expired: Attached) -> Result<(), LinkError> {
        let _turn = self.renewing.lock().await;
        if self.attached() != expired {
            return Ok(());
        }
        let greeting = self
            .state
            .lock()
            .greeting
            .clone()
            .ok_or(LinkError::SessionExpired)?;

        tracing::info!("starting a new session with {}", self.url);
        let greeted = self.post(&greeting, &Attached::default()).await?;
        let renewed = Attached {
            session: text(&greeted, SESSION_ID),
            revision: expired.revision,
        };
        event_stream::first_message(greeted).await?;

        let initialized = Message::notification(method::INITIALIZED, None);
        self.post(&initialized, &renewed).await?;
        self.state.lock().attached = renewed;
        self.listen(&Handle::current());
        Ok(())
    }

    async fn post(&self, message: &Message, attached: &Attached) -> Result<Response, LinkError> {
        let request = self
            .request(Method::POST, attached)
            .await?
            .cancel_token(self.cancel.clone())
            .header(header::ACCEPT, ANSWERS)
            .json(message);
        self.checked(request.send().await?, attached).await
    }

    async fn request(
        &self,
        method: Method,
        attached: &Attached,
    ) -> Result<RequestBuilder, LinkError> {
        let mut request = self
            .http
            .request(method, self.url.clone())
            .headers(self.headers.clone());
        if let Some(session) = &attached.session {
            request = request.header(SESSION_ID, session.as_str());
        }
        if let Some(revision) = &attached.revision {
            request = request.header(PROTOCOL_VERSION, revision.as_str());
        }
        if let Some(signing) = &self.signing {
            request = request.headers(signing.bearer().await?);
        }
        Ok(request)
    }

    async fn checked(
        &self,
        response: Response,
        attached: &Attached,
    ) -> Result<Response, LinkError> {
        let challenge = text(&response, header::WWW_AUTHENTICATE.as_str());
        match response.status() {
            StatusCode::UNAUTHORIZED => Err(LinkError::Unauthorized { challenge }),
            StatusCode::FORBIDDEN
                if challenge.as_deref().is_some_and(|challenge| {
                    Challenge::parse(challenge).is_insufficient_scope()
                }) =>
            {
                Err(LinkError::Unauthorized { challenge })
            }
            StatusCode::NOT_FOUND if attached.session.is_some() => Err(LinkError::SessionExpired),
            _ => Ok(response.error_for_status().await?),
        }
    }

    async fn close(&self) {
        let attached = self.attached();
        if attached.session.is_none() {
            return;
        }
        let closed = match self.request(Method::DELETE, &attached).await {
            Ok(request) => request
                .timeout(CLOSING)
                .send()
                .await
                .map(drop)
                .map_err(LinkError::from),
            Err(error) => Err(error),
        };
        if let Err(error) = closed {
            tracing::debug!("failed to end the session with {}: {error}", self.url);
        }
    }
}

enum Answer {
    Nothing,
    Json,
    Events,
    Other(String),
}

impl Answer {
    fn of(response: &Response) -> Answer {
        if matches!(
            response.status(),
            StatusCode::ACCEPTED | StatusCode::NO_CONTENT
        ) {
            return Answer::Nothing;
        }
        let content_type = response.content_type().unwrap_or_default();
        let kind = content_type
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match kind.as_str() {
            JSON => Answer::Json,
            EVENT_STREAM => Answer::Events,
            "" if response.content_length() == Some(0) => Answer::Nothing,
            _ => Answer::Other(content_type.to_owned()),
        }
    }
}

pub(super) fn is_event_stream(response: &Response) -> bool {
    matches!(Answer::of(response), Answer::Events)
}

async fn messages(response: Response) -> Result<Vec<Message>, LinkError> {
    let body: Value = response.json().await?;
    match body {
        Value::Array(batch) => batch
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>(),
        single => serde_json::from_value(single).map(|message| vec![message]),
    }
    .map_err(LinkError::Malformed)
}

fn text(response: &Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

impl std::fmt::Debug for HttpLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpLink")
            .field("url", &self.shared.url.as_str())
            .finish_non_exhaustive()
    }
}
