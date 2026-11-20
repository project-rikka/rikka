use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub(crate) enum Message {
    Start {
        id: u64,
        level: u8,
        #[serde(default)]
        text: String,
        #[serde(rename = "type")]
        kind: u64,
        #[serde(default)]
        fields: Vec<Field>,
    },
    Stop {
        id: u64,
    },
    Result {
        id: u64,
        #[serde(rename = "type")]
        kind: u64,
        #[serde(default)]
        fields: Vec<Field>,
    },
    Msg {
        level: u8,
        msg: String,
        #[serde(default)]
        raw_msg: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Field {
    Number(u64),
    Text(String),
    Other(IgnoredAny),
}

impl Field {
    pub(crate) fn number(&self) -> Option<u64> {
        match self {
            Self::Number(number) => Some(*number),
            Self::Text(_) | Self::Other(_) => None,
        }
    }

    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Number(_) | Self::Other(_) => None,
        }
    }
}
