use std::time::Duration;

use rikka_net::{Client, Message, WebSocket};

use crate::error::{Error, Result};
use crate::gateway::{self, Outbound, Received};

const CLOSE_WAIT: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(crate) enum Event {
    Received(Received),
    Closed { code: Option<u16>, reason: String },
}

#[derive(Debug)]
pub(crate) struct Link {
    socket: WebSocket,
}

impl Link {
    pub(crate) async fn connect(client: &Client, url: &str) -> Result<Link> {
        let socket = client
            .websocket(url)
            .no_heartbeat()
            .connect()
            .await
            .map_err(Error::unreachable)?;
        Ok(Link { socket })
    }

    pub(crate) async fn send(&mut self, outbound: Outbound) {
        let sent = match outbound {
            Outbound::Text(text) => self.socket.send_text(text).await,
            Outbound::Binary(bytes) => self.socket.send_binary(bytes).await,
        };
        if let Err(error) = sent {
            tracing::debug!(%error, "failed to send voice gateway message");
        }
    }

    pub(crate) async fn next(&mut self) -> Event {
        loop {
            let message = match self.socket.recv().await {
                None => return closed(None, String::new()),
                Some(Err(error)) => return closed(None, error.to_string()),
                Some(Ok(message)) => message,
            };
            let received = match message {
                Message::Text(text) => gateway::text(&text),
                Message::Binary(bytes) => gateway::binary(&bytes),
                Message::Close(frame) => {
                    return match frame {
                        Some(frame) => closed(Some(frame.code.get()), frame.reason),
                        None => closed(None, String::new()),
                    };
                }
                Message::Ping(_) | Message::Pong(_) => continue,
            };
            match received {
                Ok(received) => return Event::Received(received),
                Err(error) => tracing::warn!(%error, "skipped voice gateway message"),
            }
        }
    }

    pub(crate) async fn close(&mut self) {
        if tokio::time::timeout(CLOSE_WAIT, self.socket.close())
            .await
            .is_err()
        {
            tracing::debug!("voice gateway close timed out");
        }
    }
}

fn closed(code: Option<u16>, reason: String) -> Event {
    Event::Closed { code, reason }
}
