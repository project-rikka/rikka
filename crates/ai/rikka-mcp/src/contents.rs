use crate::protocol::ResourceContents;

use crate::encoded::decoded;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contents {
    uri: String,
    mime: Option<String>,
    body: Body,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Text(String),
    Bytes(Vec<u8>),
}

impl Contents {
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    #[must_use]
    pub fn mime(&self) -> Option<&str> {
        self.mime.as_deref()
    }

    #[must_use]
    pub fn body(&self) -> &Body {
        &self.body
    }

    #[must_use]
    pub fn into_body(self) -> Body {
        self.body
    }

    pub(crate) fn with_text(mut self, text: String) -> Contents {
        self.body = Body::Text(text);
        self
    }

    pub(crate) fn from_resource(contents: ResourceContents) -> Result<Contents, String> {
        let ResourceContents {
            uri,
            mime_type,
            text,
            blob,
        } = contents;
        let body = match (text, blob) {
            (Some(text), _) => Body::Text(text),
            (None, Some(blob)) => match decoded(&blob) {
                Some(bytes) => Body::Bytes(bytes),
                None => return Err(format!("{uri} came as bytes that are not base64")),
            },
            (None, None) => return Err(String::from("a resource this client cannot read")),
        };
        Ok(Contents {
            uri,
            mime: mime_type,
            body,
        })
    }
}
