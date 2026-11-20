use bytes::Bytes;
use futures_util::{SinkExt, StreamExt, stream};
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use url::Url;

use crate::cancel::{self, CancelToken};
use crate::error::{Error, Result};
use crate::ready_state::ReadyState;
use crate::reconnect::Reconnect;
use crate::retry::RetryPolicy;
use crate::websocket::close_frame::CloseFrame;
use crate::websocket::connector::{Connector, Socket};
use crate::websocket::heartbeat::Heartbeat;
use crate::websocket::message::Message;

#[derive(Debug)]
pub struct WebSocket {
    connector: Connector,
    socket: Option<Socket>,
    protocol: Option<String>,
    heartbeat: Option<Heartbeat>,
    reconnect: Reconnect,
    on_reconnect: Vec<Message>,
    cancel: Option<CancelToken>,
    closed: bool,
    graceful: bool,
}

enum Received {
    Message(Result<Message>),
    Ping,
    Cancelled,
}

impl WebSocket {
    pub(crate) fn new(
        connector: Connector,
        socket: Socket,
        protocol: Option<String>,
        heartbeat: Option<Heartbeat>,
        reconnect: Option<RetryPolicy>,
        on_reconnect: Vec<Message>,
        cancel: Option<CancelToken>,
    ) -> WebSocket {
        WebSocket {
            connector,
            socket: Some(socket),
            protocol,
            heartbeat,
            reconnect: Reconnect::new(reconnect),
            on_reconnect,
            cancel,
            closed: false,
            graceful: false,
        }
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.connector.url
    }

    #[must_use]
    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_deref()
    }

    #[must_use]
    pub fn state(&self) -> ReadyState {
        ReadyState::new(self.closed, self.socket.is_some())
    }

    pub async fn send(&mut self, message: impl Into<Message>) -> Result<()> {
        self.write(message.into().into_tungstenite()).await
    }

    pub async fn send_text(&mut self, text: impl Into<String>) -> Result<()> {
        self.send(Message::text(text)).await
    }

    pub async fn send_binary(&mut self, bytes: impl Into<Bytes>) -> Result<()> {
        self.send(Message::binary(bytes)).await
    }

    pub async fn send_json<T: serde::Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.send(Message::json(value)?).await
    }

    pub async fn ping(&mut self, payload: impl Into<Bytes>) -> Result<()> {
        self.send(Message::Ping(payload.into())).await
    }

    pub async fn recv(&mut self) -> Option<Result<Message>> {
        loop {
            if self.closed {
                return None;
            }

            if self.socket.is_none() {
                match self.reopen().await {
                    Ok(true) => continue,
                    Ok(false) => {
                        self.closed = true;
                        return None;
                    }
                    Err(error) => {
                        self.closed = true;
                        return Some(Err(error));
                    }
                }
            }

            match self.receive().await {
                Received::Message(Ok(message)) => {
                    self.record_incoming(&message);
                    return Some(Ok(message));
                }
                Received::Message(Err(Error::ConnectionClosed)) => {
                    if !self.dropped() {
                        return None;
                    }
                    tracing::debug!("connection closed, reconnecting");
                }
                Received::Message(Err(error)) => {
                    if !self.dropped() {
                        return Some(Err(error));
                    }
                    tracing::debug!(%error, "connection failed, reconnecting");
                }
                Received::Ping => {
                    if let Err(error) = self.send_heartbeat().await {
                        tracing::debug!(%error, "failed to send heartbeat");
                        self.socket = None;
                    }
                }
                Received::Cancelled => {
                    self.socket = None;
                    self.closed = true;
                    return Some(Err(Error::Cancelled));
                }
            }
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        self.close_with(CloseFrame::normal()).await
    }

    pub async fn close_with(&mut self, frame: CloseFrame) -> Result<()> {
        self.closed = true;
        self.graceful = true;

        let Some(mut socket) = self.socket.take() else {
            return Ok(());
        };
        socket.close(Some(frame.into_tungstenite())).await?;
        Ok(())
    }

    pub fn into_stream(self) -> impl stream::Stream<Item = Result<Message>> + Send + Unpin {
        Box::pin(stream::unfold(self, |mut socket| async move {
            let message = socket.recv().await?;
            Some((message, socket))
        }))
    }

    async fn receive(&mut self) -> Received {
        let WebSocket {
            socket,
            heartbeat,
            cancel,
            ..
        } = self;

        let Some(socket) = socket.as_mut() else {
            return Received::Message(Err(Error::ConnectionClosed));
        };
        let cancel = cancel.as_ref();

        match heartbeat {
            Some(send_heartbeat) => tokio::select! {
                biased;
                () = cancel::pending_until_cancelled(cancel) => Received::Cancelled,
                message = socket.next() => received(message),
                () = send_heartbeat.tick() => match send_heartbeat.is_stale() {
                    true => Received::Message(Err(Error::Timeout)),
                    false => Received::Ping,
                },
            },
            None => tokio::select! {
                biased;
                () = cancel::pending_until_cancelled(cancel) => Received::Cancelled,
                message = socket.next() => received(message),
            },
        }
    }

    fn dropped(&mut self) -> bool {
        self.socket = None;

        if self.graceful || !self.reconnect.is_enabled() {
            self.closed = true;
            return false;
        }
        true
    }

    fn record_incoming(&mut self, message: &Message) {
        match message {
            Message::Pong(_) => {
                if let Some(send_heartbeat) = self.heartbeat.as_mut() {
                    send_heartbeat.record_pong();
                }
            }
            Message::Close(_) => self.graceful = true,
            _ => {}
        }
    }

    async fn send_heartbeat(&mut self) -> Result<()> {
        self.write(TungsteniteMessage::Ping(Bytes::new())).await?;
        if let Some(send_heartbeat) = self.heartbeat.as_mut() {
            send_heartbeat.sent();
        }
        Ok(())
    }

    async fn write(&mut self, message: TungsteniteMessage) -> Result<()> {
        let cancel = self.cancel.clone();
        let Some(socket) = self.socket.as_mut() else {
            return Err(Error::ConnectionClosed);
        };

        match cancel::until_cancelled(socket.send(message), cancel.as_ref()).await? {
            Ok(()) => Ok(()),
            Err(error) => {
                self.socket = None;
                Err(Error::from(error))
            }
        }
    }

    async fn reopen(&mut self) -> Result<bool> {
        if self.graceful || !self.reconnect.wait(self.cancel.as_ref()).await? {
            return Ok(false);
        }

        match self.connector.connect(self.cancel.as_ref()).await {
            Ok((socket, protocol)) => {
                self.socket = Some(socket);
                self.protocol = protocol;
                self.reconnect.succeeded();
                self.restore().await?;
                Ok(true)
            }
            Err(error) => {
                if !self.reconnect.failed(&error) {
                    return Err(error);
                }
                tracing::debug!(%error, attempts = self.reconnect.attempts(), "reconnect failed");
                Ok(true)
            }
        }
    }

    async fn restore(&mut self) -> Result<()> {
        let messages = std::mem::take(&mut self.on_reconnect);
        let mut result = Ok(());

        for message in &messages {
            result = self.send(message.clone()).await;
            if result.is_err() {
                break;
            }
        }

        self.on_reconnect = messages;
        result
    }
}

fn received(
    message: Option<std::result::Result<TungsteniteMessage, tokio_tungstenite::tungstenite::Error>>,
) -> Received {
    match message {
        None => Received::Message(Err(Error::ConnectionClosed)),
        Some(Ok(message)) => Received::Message(Ok(Message::from_tungstenite(message))),
        Some(Err(error)) => Received::Message(Err(Error::from(error))),
    }
}
