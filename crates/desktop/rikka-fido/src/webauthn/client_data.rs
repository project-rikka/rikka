use std::fmt::Write as _;

use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine as _};

use crate::crypto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientDataType {
    Get,
    Create,
}

impl ClientDataType {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ClientDataType::Get => "webauthn.get",
            ClientDataType::Create => "webauthn.create",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientData {
    kind: ClientDataType,
    challenge: Vec<u8>,
    origin: String,
    top_origin: Option<String>,
}

impl ClientData {
    #[must_use]
    pub fn new(
        kind: ClientDataType,
        challenge: impl Into<Vec<u8>>,
        origin: impl Into<String>,
    ) -> ClientData {
        ClientData {
            kind,
            challenge: challenge.into(),
            origin: origin.into(),
            top_origin: None,
        }
    }

    #[must_use]
    pub fn cross_origin(mut self, top_origin: impl Into<String>) -> ClientData {
        self.top_origin = Some(top_origin.into());
        self
    }

    #[must_use]
    pub fn kind(&self) -> ClientDataType {
        self.kind
    }

    #[must_use]
    pub fn challenge(&self) -> &[u8] {
        &self.challenge
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    #[must_use]
    pub fn json(&self) -> String {
        let mut json = String::from("{\"type\":");
        quote(self.kind.name(), &mut json);
        json.push_str(",\"challenge\":");
        quote(&BASE64_URL_SAFE_NO_PAD.encode(&self.challenge), &mut json);
        json.push_str(",\"origin\":");
        quote(&self.origin, &mut json);
        match &self.top_origin {
            Some(top_origin) => {
                json.push_str(",\"crossOrigin\":true,\"topOrigin\":");
                quote(top_origin, &mut json);
            }
            None => json.push_str(",\"crossOrigin\":false"),
        }
        json.push('}');
        json
    }

    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        crypto::sha256(self.json())
    }
}

fn quote(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{20}'..='\u{10ffff}' => out.push(character),
            control => {
                write!(out, "\\u{:04x}", u32::from(control)).expect("writing to a string succeeds");
            }
        }
    }
    out.push('"');
}
