use std::time::Duration;

use bytes::Bytes;
use http::{HeaderName, HeaderValue};
use serde::Serialize;
use url::Url;

use crate::auth::Auth;
use crate::cancel::CancelToken;
use crate::error::Result;
use crate::ready_state::ReadyState;
use crate::retry::RetryPolicy;
use crate::runtime;
use crate::websocket::{CloseFrame, Message, WebSocket, WebSocketBuilder};

#[derive(Debug)]
pub struct BlockingWebSocketBuilder {
    inner: WebSocketBuilder,
}

impl BlockingWebSocketBuilder {
    pub(crate) fn new(inner: WebSocketBuilder) -> BlockingWebSocketBuilder {
        BlockingWebSocketBuilder { inner }
    }

    #[must_use]
    pub fn header(self, name: HeaderName, value: HeaderValue) -> Self {
        self.map(|inner| inner.header(name, value))
    }

    #[must_use]
    pub fn auth(self, auth: Auth) -> Self {
        self.map(|inner| inner.auth(auth))
    }

    #[must_use]
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.map(|inner| inner.bearer_auth(token))
    }

    #[must_use]
    pub fn protocols<I, S>(self, protocols: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.map(|inner| inner.protocols(protocols))
    }

    #[must_use]
    pub fn max_message_size(self, bytes: Option<usize>) -> Self {
        self.map(|inner| inner.max_message_size(bytes))
    }

    #[must_use]
    pub fn max_frame_size(self, bytes: Option<usize>) -> Self {
        self.map(|inner| inner.max_frame_size(bytes))
    }

    #[must_use]
    pub fn heartbeat(self, every: Duration, timeout: Duration) -> Self {
        self.map(|inner| inner.heartbeat(every, timeout))
    }

    #[must_use]
    pub fn no_heartbeat(self) -> Self {
        self.map(WebSocketBuilder::no_heartbeat)
    }

    #[must_use]
    pub fn reconnect(self, policy: RetryPolicy) -> Self {
        self.map(|inner| inner.reconnect(policy))
    }

    #[must_use]
    pub fn send_on_reconnect<I, M>(self, messages: I) -> Self
    where
        I: IntoIterator<Item = M>,
        M: Into<Message>,
    {
        self.map(|inner| inner.send_on_reconnect(messages))
    }

    #[must_use]
    pub fn cancel_token(self, token: CancelToken) -> Self {
        self.map(|inner| inner.cancel_token(token))
    }

    pub fn connect(self) -> Result<BlockingWebSocket> {
        runtime::block_on_local(self.inner.connect()).map(BlockingWebSocket::new)
    }

    fn map(mut self, with: impl FnOnce(WebSocketBuilder) -> WebSocketBuilder) -> Self {
        self.inner = with(self.inner);
        self
    }
}

#[derive(Debug)]
pub struct BlockingWebSocket {
    inner: WebSocket,
}

impl BlockingWebSocket {
    pub(crate) fn new(inner: WebSocket) -> BlockingWebSocket {
        BlockingWebSocket { inner }
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        self.inner.url()
    }

    #[must_use]
    pub fn protocol(&self) -> Option<&str> {
        self.inner.protocol()
    }

    #[must_use]
    pub fn state(&self) -> ReadyState {
        self.inner.state()
    }

    pub fn send(&mut self, message: impl Into<Message>) -> Result<()> {
        runtime::block_on_local(self.inner.send(message))
    }

    pub fn send_text(&mut self, text: impl Into<String>) -> Result<()> {
        runtime::block_on_local(self.inner.send_text(text))
    }

    pub fn send_binary(&mut self, bytes: impl Into<Bytes>) -> Result<()> {
        runtime::block_on_local(self.inner.send_binary(bytes))
    }

    pub fn send_json<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        runtime::block_on_local(self.inner.send_json(value))
    }

    pub fn recv(&mut self) -> Option<Result<Message>> {
        runtime::block_on_local(self.inner.recv())
    }

    pub fn close(&mut self) -> Result<()> {
        runtime::block_on_local(self.inner.close())
    }

    pub fn close_with(&mut self, frame: CloseFrame) -> Result<()> {
        runtime::block_on_local(self.inner.close_with(frame))
    }
}

impl Iterator for BlockingWebSocket {
    type Item = Result<Message>;

    fn next(&mut self) -> Option<Result<Message>> {
        self.recv()
    }
}
