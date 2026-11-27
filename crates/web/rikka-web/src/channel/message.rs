use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const PROTOCOL: u32 = 1;

#[derive(Debug, Deserialize)]
pub(crate) struct Hello {
    pub(crate) protocol: u32,
    pub(crate) version: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Incoming {
    Hello {
        hello: Hello,
    },
    Reply {
        reply: u64,
        #[serde(default)]
        value: Value,
        #[serde(default)]
        error: Option<String>,
    },
    Event {
        event: String,
        #[serde(default)]
        args: Vec<Value>,
    },
    Ask {
        ask: u64,
        kind: String,
        #[serde(default)]
        args: Vec<Value>,
    },
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum Outgoing<'a> {
    Call {
        call: u64,
        api: &'a str,
        args: &'a [Value],
    },
    Answer {
        answer: u64,
        value: &'a Value,
    },
}
