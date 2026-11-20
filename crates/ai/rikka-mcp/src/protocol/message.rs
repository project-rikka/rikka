use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

const JSONRPC: &str = "2.0";
const METHOD_NOT_FOUND: i64 = -32601;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum RequestId {
    Number(i64),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Frame", into = "Frame")]
pub(crate) enum Message {
    Request {
        id: RequestId,
        method: String,
        params: Option<Value>,
    },
    Notification {
        method: String,
        params: Option<Value>,
    },
    Response {
        id: RequestId,
        outcome: Result<Value, ErrorObject>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ErrorObject {
    pub(crate) code: i64,
    #[serde(default)]
    pub(crate) message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<Value>,
}

impl Message {
    pub(crate) fn request(id: RequestId, method: &str, params: Option<Value>) -> Message {
        Message::Request {
            id,
            method: method.to_owned(),
            params,
        }
    }

    pub(crate) fn notification(method: &str, params: Option<Value>) -> Message {
        Message::Notification {
            method: method.to_owned(),
            params,
        }
    }

    pub(crate) fn is_request(&self, named: &str) -> bool {
        matches!(self, Message::Request { method, .. } if method == named)
    }

    pub(crate) fn request_id(&self) -> Option<&RequestId> {
        match self {
            Message::Request { id, .. } => Some(id),
            _ => None,
        }
    }

    pub(crate) fn answers(&self, request: &RequestId) -> bool {
        matches!(self, Message::Response { id, .. } if id == request)
    }
}

impl ErrorObject {
    pub(crate) fn method_not_found(method: &str) -> ErrorObject {
        ErrorObject {
            code: METHOD_NOT_FOUND,
            message: format!("method not found: {method}"),
            data: None,
        }
    }

    pub(crate) fn is_method_not_found(&self) -> bool {
        self.code == METHOD_NOT_FOUND
    }
}

#[derive(Serialize, Deserialize)]
struct Frame {
    #[serde(default)]
    jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<RequestId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObject>,
}

fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

impl TryFrom<Frame> for Message {
    type Error = &'static str;

    fn try_from(frame: Frame) -> Result<Message, &'static str> {
        match (frame.method, frame.id, frame.result, frame.error) {
            (Some(method), Some(id), _, _) => Ok(Message::Request {
                id,
                method,
                params: frame.params,
            }),
            (Some(method), None, _, _) => Ok(Message::Notification {
                method,
                params: frame.params,
            }),
            (None, Some(id), _, Some(error)) => Ok(Message::Response {
                id,
                outcome: Err(error),
            }),
            (None, Some(id), Some(result), None) => Ok(Message::Response {
                id,
                outcome: Ok(result),
            }),
            _ => Err("neither a request, a notification nor an answer to a request"),
        }
    }
}

impl From<Message> for Frame {
    fn from(message: Message) -> Frame {
        let mut frame = Frame {
            jsonrpc: JSONRPC.to_owned(),
            id: None,
            method: None,
            params: None,
            result: None,
            error: None,
        };
        match message {
            Message::Request { id, method, params } => {
                frame.id = Some(id);
                frame.method = Some(method);
                frame.params = params;
            }
            Message::Notification { method, params } => {
                frame.method = Some(method);
                frame.params = params;
            }
            Message::Response { id, outcome } => {
                frame.id = Some(id);
                match outcome {
                    Ok(result) => frame.result = Some(result),
                    Err(error) => frame.error = Some(error),
                }
            }
        }
        frame
    }
}
