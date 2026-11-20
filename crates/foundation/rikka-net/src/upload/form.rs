use std::path::Path;

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use rand::Rng;

use crate::body::Body;
use crate::error::Result;
use crate::upload::part::Part;

const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

const BOUNDARY_LEN: usize = 32;

#[derive(Debug)]
pub struct Form {
    boundary: String,
    parts: Vec<(String, Part)>,
}

impl Form {
    #[must_use]
    pub fn new() -> Form {
        Form {
            boundary: boundary(),
            parts: Vec::new(),
        }
    }

    #[must_use]
    pub fn text(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.part(name, Part::text(value))
    }

    pub async fn file(self, name: impl Into<String>, path: impl AsRef<Path>) -> Result<Self> {
        Ok(self.part(name, Part::file(path).await?))
    }

    #[must_use]
    pub fn part(mut self, name: impl Into<String>, part: Part) -> Self {
        self.parts.push((name.into(), part));
        self
    }

    #[must_use]
    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }

    pub(crate) fn into_body(self) -> Body {
        let closing = self.closing();
        let mut length = Some(closing.len() as u64);
        let mut parts = Vec::with_capacity(self.parts.len());

        for (name, part) in self.parts {
            let encoded = part.encode(&name, &self.boundary);
            length = length
                .zip(encoded.length())
                .map(|(total, part)| total + part);
            parts.push(encoded.into_stream());
        }

        let stream = stream::iter(parts)
            .flatten()
            .chain(stream::once(async move { Ok(closing) }));

        Body::from_stream(Box::pin(stream), length)
    }

    fn closing(&self) -> Bytes {
        Bytes::from(format!("--{}--\r\n", self.boundary))
    }
}

impl Default for Form {
    fn default() -> Form {
        Form::new()
    }
}

fn boundary() -> String {
    let mut rng = rand::rng();
    (0..BOUNDARY_LEN)
        .map(|_| char::from(ALPHABET[rng.random_range(0..ALPHABET.len())]))
        .collect()
}
