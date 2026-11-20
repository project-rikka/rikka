use std::borrow::Cow;
use std::fmt;
use std::path::Path;

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use http::{HeaderName, HeaderValue};

use crate::body::Body;
use crate::error::Result;
use crate::headers;

const TRAILER: &[u8] = b"\r\n";

pub struct Part {
    body: Body,
    file_name: Option<String>,
    content_type: Option<String>,
    headers: Vec<(HeaderName, HeaderValue)>,
}

impl Part {
    #[must_use]
    pub fn text(value: impl Into<String>) -> Part {
        Part::raw(Body::text(value))
    }

    #[must_use]
    pub fn bytes(value: impl Into<Bytes>) -> Part {
        Part::raw(Body::bytes(value))
    }

    pub async fn file(path: impl AsRef<Path>) -> Result<Part> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let content_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Ok(Part {
            body: Body::file(path).await?,
            file_name,
            content_type: Some(content_type),
            headers: Vec::new(),
        })
    }

    #[must_use]
    pub fn body(body: Body) -> Part {
        Part::raw(body)
    }

    #[must_use]
    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn header<K, V>(mut self, name: K, value: V) -> Result<Self>
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.headers.push(headers::pair(name, value)?);
        Ok(self)
    }

    pub(crate) fn into_body_and_type(self) -> (Body, String) {
        let content_type = self
            .content_type
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        (self.body, content_type)
    }

    fn raw(body: Body) -> Part {
        Part {
            body,
            file_name: None,
            content_type: None,
            headers: Vec::new(),
        }
    }

    fn preamble(&self, name: &str, boundary: &str) -> Bytes {
        let file_name = self
            .file_name
            .as_ref()
            .map(|file_name| format!("; filename=\"{}\"", escape(file_name)))
            .unwrap_or_default();
        let mut head = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"{file_name}\r\n",
            escape(name)
        )
        .into_bytes();
        if let Some(content_type) = &self.content_type {
            head.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        for (name, value) in &self.headers {
            head.extend_from_slice(name.as_str().as_bytes());
            head.extend_from_slice(b": ");
            head.extend_from_slice(value.as_bytes());
            head.extend_from_slice(b"\r\n");
        }
        head.extend_from_slice(b"\r\n");
        Bytes::from(head)
    }

    pub(crate) fn encode(self, name: &str, boundary: &str) -> Body {
        let preamble = self.preamble(name, boundary);
        let length = self
            .body
            .length()
            .map(|body| preamble.len() as u64 + body + TRAILER.len() as u64);

        let stream = stream::once(async move { Ok(preamble) })
            .chain(self.body.into_stream())
            .chain(stream::once(async { Ok(Bytes::from_static(TRAILER)) }));

        Body::from_stream(Box::pin(stream), length)
    }
}

fn escape(value: &str) -> Cow<'_, str> {
    if !value.contains(['\\', '"', '\r', '\n']) {
        return Cow::Borrowed(value);
    }

    let escaped = value.chars().fold(String::new(), |mut text, character| {
        match character {
            '\\' => text.push_str("%5C"),
            '"' => text.push_str("%22"),
            '\r' => text.push_str("%0D"),
            '\n' => text.push_str("%0A"),
            other => text.push(other),
        }
        text
    });
    Cow::Owned(escaped)
}

impl fmt::Debug for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Part")
            .field("file_name", &self.file_name)
            .field("content_type", &self.content_type)
            .field("body", &self.body)
            .finish()
    }
}
