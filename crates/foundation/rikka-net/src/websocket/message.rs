use bytes::Bytes;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

use crate::error::{Error, Result};
use crate::websocket::close_frame::CloseFrame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Bytes),
    Ping(Bytes),
    Pong(Bytes),
    Close(Option<CloseFrame>),
}

impl Message {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Message {
        Message::Text(text.into())
    }

    #[must_use]
    pub fn binary(bytes: impl Into<Bytes>) -> Message {
        Message::Binary(bytes.into())
    }

    pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<Message> {
        let text = serde_json::to_string(value).map_err(|error| Error::Encode {
            format: "JSON",
            source: Box::new(error),
        })?;
        Ok(Message::Text(text))
    }

    pub fn parse_json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(self.as_bytes()).map_err(|error| Error::Decode {
            format: "JSON",
            source: Box::new(error),
        })
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Message::Text(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Message::Text(text) => text.as_bytes(),
            Message::Binary(bytes) | Message::Ping(bytes) | Message::Pong(bytes) => bytes,
            Message::Close(_) => &[],
        }
    }

    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Message::Text(_))
    }

    #[must_use]
    pub fn is_binary(&self) -> bool {
        matches!(self, Message::Binary(_))
    }

    #[must_use]
    pub fn is_close(&self) -> bool {
        matches!(self, Message::Close(_))
    }

    #[must_use]
    pub fn is_control(&self) -> bool {
        matches!(
            self,
            Message::Ping(_) | Message::Pong(_) | Message::Close(_)
        )
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<&str> for Message {
    fn from(text: &str) -> Message {
        Message::Text(text.to_owned())
    }
}

impl From<String> for Message {
    fn from(text: String) -> Message {
        Message::Text(text)
    }
}

impl From<Bytes> for Message {
    fn from(bytes: Bytes) -> Message {
        Message::Binary(bytes)
    }
}

impl From<Vec<u8>> for Message {
    fn from(bytes: Vec<u8>) -> Message {
        Message::Binary(Bytes::from(bytes))
    }
}

impl Message {
    pub(crate) fn from_tungstenite(message: TungsteniteMessage) -> Message {
        match message {
            TungsteniteMessage::Text(text) => Message::Text(text.as_str().to_owned()),
            TungsteniteMessage::Binary(bytes) => Message::Binary(bytes),
            TungsteniteMessage::Ping(bytes) => Message::Ping(bytes),
            TungsteniteMessage::Pong(bytes) => Message::Pong(bytes),
            TungsteniteMessage::Close(frame) => {
                Message::Close(frame.map(CloseFrame::from_tungstenite))
            }
            frame @ TungsteniteMessage::Frame(_) => Message::Binary(frame.into_data()),
        }
    }

    pub(crate) fn into_tungstenite(self) -> TungsteniteMessage {
        match self {
            Message::Text(text) => TungsteniteMessage::Text(text.into()),
            Message::Binary(bytes) => TungsteniteMessage::Binary(bytes),
            Message::Ping(bytes) => TungsteniteMessage::Ping(bytes),
            Message::Pong(bytes) => TungsteniteMessage::Pong(bytes),
            Message::Close(frame) => {
                TungsteniteMessage::Close(frame.map(CloseFrame::into_tungstenite))
            }
        }
    }
}
