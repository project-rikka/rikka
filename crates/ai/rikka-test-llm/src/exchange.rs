use serde_json::Value;

use crate::frames;

#[derive(Debug, Clone, PartialEq)]
pub struct Exchange {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Value>,
}

impl Exchange {
    pub(crate) fn new(
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Option<Value>,
    ) -> Exchange {
        Exchange {
            method,
            path,
            headers,
            body,
        }
    }

    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(held, _)| held.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[must_use]
    pub fn body(&self) -> Option<&Value> {
        self.body.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct Reply(Vec<u8>);

impl Reply {
    #[must_use]
    pub fn json(status: u16, body: &Value) -> Reply {
        Reply(frames::json(status, body))
    }

    #[must_use]
    pub fn events(frames: &[&str]) -> Reply {
        Reply(frames::events(
            frames.iter().map(|frame| (*frame).to_owned()),
        ))
    }

    #[must_use]
    pub fn text(status: u16, body: &str) -> Reply {
        Reply(frames::text(status, body))
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}
