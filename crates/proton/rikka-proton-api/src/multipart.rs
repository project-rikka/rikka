use std::fmt;

use bytes::{BufMut, Bytes, BytesMut};
use http::HeaderValue;

use crate::error::{Error, Result};

const OCTET_STREAM: &str = "application/octet-stream";
const BOUNDARY_PREFIX: &str = "rikka-";
const BOUNDARY_ENTROPY: usize = 16;
const PART_HEAD_ROOM: usize = 256;

#[must_use]
#[derive(Clone)]
pub struct Multipart {
    boundary: String,
    fields: Vec<(String, Part)>,
}

#[must_use]
#[derive(Clone)]
pub struct Part {
    body: Bytes,
    file_name: Option<String>,
    content_type: Option<String>,
}

impl Multipart {
    pub fn new() -> Multipart {
        Multipart {
            boundary: multipart_boundary(),
            fields: Vec::new(),
        }
    }

    pub fn text(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.part(name, Part::text(value))
    }

    pub fn file(
        self,
        name: impl Into<String>,
        file_name: impl Into<String>,
        data: impl Into<Bytes>,
    ) -> Self {
        self.part(name, Part::bytes(data).file_name(file_name))
    }

    pub fn part(mut self, name: impl Into<String>, part: Part) -> Self {
        self.fields.push((name.into(), part));
        self
    }

    pub(crate) fn encode(self) -> Result<(HeaderValue, Bytes)> {
        let content_type =
            HeaderValue::from_str(&format!("multipart/form-data; boundary={}", self.boundary))
                .map_err(|_| Error::InvalidForm(format!("invalid boundary {:?}", self.boundary)))?;

        let size = self
            .fields
            .iter()
            .map(|(_, part)| part.body.len() + PART_HEAD_ROOM)
            .sum::<usize>()
            + PART_HEAD_ROOM;
        let mut body = BytesMut::with_capacity(size);

        for (name, part) in &self.fields {
            body.put_slice(b"--");
            body.put_slice(self.boundary.as_bytes());
            body.put_slice(b"\r\nContent-Disposition: form-data; name=\"");
            body.put_slice(escape_quoted(name).as_bytes());
            body.put_slice(b"\"");
            if let Some(file_name) = &part.file_name {
                body.put_slice(b"; filename=\"");
                body.put_slice(escape_quoted(file_name).as_bytes());
                body.put_slice(b"\"");
            }
            body.put_slice(b"\r\n");

            let content_type = part
                .content_type
                .as_deref()
                .or(part.file_name.as_ref().map(|_| OCTET_STREAM));
            if let Some(content_type) = content_type {
                if HeaderValue::from_str(content_type).is_err() {
                    return Err(Error::InvalidForm(format!(
                        "invalid content type {content_type:?}"
                    )));
                }
                body.put_slice(b"Content-Type: ");
                body.put_slice(content_type.as_bytes());
                body.put_slice(b"\r\n");
            }

            body.put_slice(b"\r\n");
            body.put_slice(&part.body);
            body.put_slice(b"\r\n");
        }

        body.put_slice(b"--");
        body.put_slice(self.boundary.as_bytes());
        body.put_slice(b"--\r\n");

        Ok((content_type, body.freeze()))
    }
}

impl Default for Multipart {
    fn default() -> Multipart {
        Multipart::new()
    }
}

impl fmt::Debug for Multipart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Multipart")
            .field("boundary", &self.boundary)
            .field("fields", &self.fields)
            .finish()
    }
}

impl Part {
    pub fn text(value: impl Into<String>) -> Part {
        Part::bytes(Bytes::from(value.into()))
    }

    pub fn bytes(data: impl Into<Bytes>) -> Part {
        Part {
            body: data.into(),
            file_name: None,
            content_type: None,
        }
    }

    pub fn file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

impl fmt::Debug for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Part")
            .field("bytes", &self.body.len())
            .field("file_name", &self.file_name)
            .field("content_type", &self.content_type)
            .finish()
    }
}

#[must_use]
pub fn multipart_boundary() -> String {
    let entropy = rand::random::<[u8; BOUNDARY_ENTROPY]>();
    format!("{BOUNDARY_PREFIX}{}", rikka_types::hex::encode(entropy))
}

fn escape_quoted(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("%22"),
            '\r' => quoted.push_str("%0D"),
            '\n' => quoted.push_str("%0A"),
            other => quoted.push(other),
        }
    }
    quoted
}
