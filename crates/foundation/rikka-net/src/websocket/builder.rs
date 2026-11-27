use std::time::Duration;

use http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use url::Url;

use crate::auth::Auth;
use crate::cancel::CancelToken;
use crate::client::Client;
use crate::error::{Error, Result};
use crate::request_options::RequestOptions;
use crate::retry::RetryPolicy;
use crate::websocket::connector::Connector;
use crate::websocket::handle::SocketHandle;
use crate::websocket::heartbeat::{self, Heartbeat};
use crate::websocket::message::Message;
use crate::websocket::socket::WebSocket;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct WebSocketBuilder {
    client: Client,
    url: Result<Url>,
    options: RequestOptions,
    protocols: Vec<String>,
    max_message_size: Option<usize>,
    max_frame_size: Option<usize>,
    heartbeat: Option<(Duration, Duration)>,
    reconnect: Option<RetryPolicy>,
    on_reconnect: Vec<Message>,
}

impl WebSocketBuilder {
    pub(crate) fn new(client: Client, url: Result<Url>) -> WebSocketBuilder {
        WebSocketBuilder {
            client,
            url: url.and_then(websocket_url),
            options: RequestOptions::default(),
            protocols: Vec::new(),
            max_message_size: None,
            max_frame_size: None,
            heartbeat: Some((HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT)),
            reconnect: None,
            on_reconnect: Vec::new(),
        }
    }

    #[must_use]
    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.options.header(name, value);
        self
    }

    #[must_use]
    pub fn auth(mut self, auth: Auth) -> Self {
        self.options.auth(auth);
        self
    }

    #[must_use]
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.auth(Auth::bearer(token))
    }

    #[must_use]
    pub fn protocols<I, S>(mut self, protocols: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.protocols = protocols.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn max_message_size(mut self, bytes: Option<usize>) -> Self {
        self.max_message_size = bytes;
        self
    }

    #[must_use]
    pub fn max_frame_size(mut self, bytes: Option<usize>) -> Self {
        self.max_frame_size = bytes;
        self
    }

    #[must_use]
    pub fn heartbeat(mut self, every: Duration, timeout: Duration) -> Self {
        self.heartbeat = Some((every, timeout));
        self
    }

    #[must_use]
    pub fn heartbeat_every(self, every: Duration) -> Self {
        let timeout = heartbeat::response_window(every);
        self.heartbeat(every, timeout)
    }

    #[must_use]
    pub fn no_heartbeat(mut self) -> Self {
        self.heartbeat = None;
        self
    }

    #[must_use]
    pub fn reconnect(mut self, policy: RetryPolicy) -> Self {
        self.reconnect = Some(policy);
        self
    }

    #[must_use]
    pub fn send_on_reconnect<I, M>(mut self, messages: I) -> Self
    where
        I: IntoIterator<Item = M>,
        M: Into<Message>,
    {
        self.on_reconnect = messages.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.options.set_cancel(token);
        self
    }

    pub async fn connect(mut self) -> Result<WebSocket> {
        self.options.check()?;
        let url = self.url?;
        let shared = self.client.shared();

        let mut headers = shared.headers.clone();
        self.options.apply_to_headers(&mut headers)?;

        let config = WebSocketConfig::default()
            .max_message_size(self.max_message_size)
            .max_frame_size(self.max_frame_size);

        let connector = Connector {
            url,
            headers,
            protocols: self.protocols,
            config,
            tls: crate::tls::client_config()?,
            connect_timeout: shared.connect_timeout,
        };

        let cancel = self.options.cancel().cloned();
        let (socket, protocol) = connector.connect(cancel.as_ref()).await?;
        let heartbeat = self
            .heartbeat
            .map(|(every, timeout)| Heartbeat::new(every, timeout));

        Ok(WebSocket::new(
            connector,
            socket,
            protocol,
            heartbeat,
            self.reconnect,
            self.on_reconnect,
            cancel,
        ))
    }

    #[must_use]
    pub fn spawn(self) -> SocketHandle {
        SocketHandle::spawn(self)
    }
}

fn websocket_url(mut url: Url) -> Result<Url> {
    let scheme = match url.scheme() {
        "ws" | "wss" => return Ok(url),
        "http" => "ws",
        "https" => "wss",
        other => {
            return Err(Error::InvalidUrl(format!(
                "unsupported WebSocket scheme \"{other}\""
            )));
        }
    };

    url.set_scheme(scheme)
        .map_err(|()| Error::InvalidUrl("cannot set a WebSocket scheme".to_owned()))?;
    Ok(url)
}
